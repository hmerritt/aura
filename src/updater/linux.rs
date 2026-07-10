use super::{UpdateTrigger, UpdaterEvent, UpdaterStatus};
use crate::config::UpdaterConfig;
use crate::errors::Result;
use crate::version;
use anyhow::{bail, Context};
use futures_util::StreamExt;
use semver::Version;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

const MANIFEST_NAME: &str = "aura-linux-manifest";
const MAX_MANIFEST_BYTES: usize = 64 * 1024;
const RESTART_HELPER_SCRIPT: &str =
    "pid=$1; shift; while kill -0 \"$pid\" 2>/dev/null; do sleep 0.1; done; exec \"$@\"";

#[derive(Debug, Clone)]
pub struct RestartContext {
    stable_binary: PathBuf,
    relaunch_args: Vec<String>,
}

pub struct UpdaterRuntime {
    status: UpdaterStatus,
    check_interval: Option<Duration>,
    trigger_tx: Option<UnboundedSender<UpdateTrigger>>,
    event_rx: Option<UnboundedReceiver<UpdaterEvent>>,
    restart_context: Option<RestartContext>,
}

impl UpdaterRuntime {
    pub fn status(&self) -> UpdaterStatus {
        self.status
    }

    pub fn check_interval(&self) -> Option<Duration> {
        self.check_interval
    }

    pub fn request_check(&self, trigger: UpdateTrigger) -> bool {
        self.trigger_tx
            .as_ref()
            .map(|sender| sender.send(trigger).is_ok())
            .unwrap_or(false)
    }

    pub fn take_event_receiver(&mut self) -> Option<UnboundedReceiver<UpdaterEvent>> {
        self.event_rx.take()
    }

    pub fn restart_context(&self) -> Option<RestartContext> {
        self.restart_context.clone()
    }
}

#[derive(Debug, Clone)]
struct ManagedInstall {
    installer: PathBuf,
    stable_binary: PathBuf,
}

#[derive(Debug, Clone)]
struct WorkerContext {
    installer: PathBuf,
    feed_url: String,
    current_version: Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReleaseAsset {
    url: String,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReleaseManifest {
    version: Version,
    x86_64: ReleaseAsset,
    aarch64: ReleaseAsset,
}

pub fn initialize(config: &UpdaterConfig, relaunch_args: Vec<String>) -> UpdaterRuntime {
    if !config.enabled {
        return unavailable_runtime(UpdaterStatus::Disabled);
    }

    let managed = match locate_managed_install() {
        Ok(managed) => managed,
        Err(error) => {
            tracing::info!(error = %error, "Linux self-updater is unavailable for this executable");
            return unavailable_runtime(UpdaterStatus::Unsupported);
        }
    };
    let current_version = match Version::parse(&version::get_version().version) {
        Ok(version) => version,
        Err(error) => {
            tracing::warn!(error = %error, "Linux self-updater cannot parse the current app version");
            return unavailable_runtime(UpdaterStatus::Unsupported);
        }
    };

    let worker_context = WorkerContext {
        installer: managed.installer,
        feed_url: config.feed_url.clone(),
        current_version,
    };
    let restart_context = RestartContext {
        stable_binary: managed.stable_binary,
        relaunch_args,
    };
    let (trigger_tx, trigger_rx) = unbounded_channel();
    let (event_tx, event_rx) = unbounded_channel();
    tokio::spawn(run_worker(worker_context, trigger_rx, event_tx));

    UpdaterRuntime {
        status: UpdaterStatus::Idle,
        check_interval: Some(config.check_interval),
        trigger_tx: Some(trigger_tx),
        event_rx: Some(event_rx),
        restart_context: Some(restart_context),
    }
}

fn unavailable_runtime(status: UpdaterStatus) -> UpdaterRuntime {
    UpdaterRuntime {
        status,
        check_interval: None,
        trigger_tx: None,
        event_rx: None,
        restart_context: None,
    }
}

fn locate_managed_install() -> Result<ManagedInstall> {
    let data_home = dirs::data_local_dir().context("failed to resolve Linux data directory")?;
    let home = dirs::home_dir().context("failed to resolve home directory")?;
    locate_managed_install_from(
        &std::env::current_exe().context("failed to resolve current executable")?,
        &data_home,
        &home,
    )
}

fn locate_managed_install_from(
    current_exe: &Path,
    data_home: &Path,
    home: &Path,
) -> Result<ManagedInstall> {
    let app_root = data_home.join("aura").join("app");
    let managed_binary = app_root.join("current").join("bin").join("aura");
    let running = fs::canonicalize(current_exe)
        .with_context(|| format!("failed to canonicalize {}", current_exe.display()))?;
    let managed = fs::canonicalize(&managed_binary).with_context(|| {
        format!(
            "managed binary is unavailable at {}",
            managed_binary.display()
        )
    })?;
    if running != managed {
        bail!(
            "current executable {} is not the managed Aura binary {}",
            running.display(),
            managed.display()
        );
    }

    let installer = app_root.join("current").join("install.sh");
    if !installer.is_file() {
        bail!(
            "managed installer is unavailable at {}",
            installer.display()
        );
    }
    let stable_binary = home.join(".local").join("bin").join("aura");
    let stable_target = fs::canonicalize(&stable_binary).with_context(|| {
        format!(
            "stable Aura command is unavailable at {}",
            stable_binary.display()
        )
    })?;
    if stable_target != managed {
        bail!(
            "stable Aura command {} does not target the managed binary",
            stable_binary.display()
        );
    }

    Ok(ManagedInstall {
        installer,
        stable_binary,
    })
}

async fn run_worker(
    context: WorkerContext,
    mut trigger_rx: UnboundedReceiver<UpdateTrigger>,
    event_tx: UnboundedSender<UpdaterEvent>,
) {
    while let Some(trigger) = trigger_rx.recv().await {
        tracing::debug!(?trigger, "processing Linux app update trigger");
        if !send_status(&event_tx, UpdaterStatus::Checking) {
            break;
        }

        match fetch_manifest(&context.feed_url).await {
            Ok(manifest) if !is_newer_release(&manifest.version, &context.current_version) => {
                if !send_status(&event_tx, UpdaterStatus::UpToDate) {
                    break;
                }
            }
            Ok(manifest) => {
                if !send_status(&event_tx, UpdaterStatus::UpdateAvailable)
                    || !send_status(&event_tx, UpdaterStatus::Installing)
                {
                    break;
                }
                match install_update(context.clone(), manifest.version.clone()).await {
                    Ok(()) => {
                        if !send_status(&event_tx, UpdaterStatus::InstalledPendingRestart)
                            || event_tx.send(UpdaterEvent::InstallReady).is_err()
                        {
                            break;
                        }
                    }
                    Err(error) => {
                        tracing::warn!(error = %error, "failed to install Linux app update");
                        if !send_status(&event_tx, UpdaterStatus::Error) {
                            break;
                        }
                    }
                }
            }
            Err(error) => {
                tracing::warn!(error = %error, "failed to check for Linux app update");
                if !send_status(&event_tx, UpdaterStatus::Error) {
                    break;
                }
            }
        }
    }
}

fn send_status(sender: &UnboundedSender<UpdaterEvent>, status: UpdaterStatus) -> bool {
    sender.send(UpdaterEvent::Status(status)).is_ok()
}

fn is_newer_release(candidate: &Version, current: &Version) -> bool {
    candidate > current
}

async fn fetch_manifest(feed_url: &str) -> Result<ReleaseManifest> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let url = format!("{}/{}", feed_url.trim_end_matches('/'), MANIFEST_NAME);
    let response = reqwest::get(&url)
        .await
        .with_context(|| format!("failed to download {url}"))?
        .error_for_status()
        .with_context(|| format!("release feed returned an error for {url}"))?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_MANIFEST_BYTES as u64)
    {
        bail!("release manifest is larger than {MAX_MANIFEST_BYTES} bytes");
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("failed to read release manifest")?;
        if bytes.len().saturating_add(chunk.len()) > MAX_MANIFEST_BYTES {
            bail!("release manifest is larger than {MAX_MANIFEST_BYTES} bytes");
        }
        bytes.extend_from_slice(&chunk);
    }
    let text = std::str::from_utf8(&bytes).context("release manifest is not UTF-8")?;
    parse_manifest(text)
}

fn parse_manifest(text: &str) -> Result<ReleaseManifest> {
    let mut values = BTreeMap::new();
    for line in text.lines() {
        if line.is_empty() {
            bail!("release manifest contains an empty line");
        }
        let (key, value) = line
            .split_once('=')
            .with_context(|| format!("release manifest line is missing '=': {line}"))?;
        match key {
            "schema" | "version" | "x86_64_url" | "x86_64_sha256" | "aarch64_url"
            | "aarch64_sha256" => {}
            _ => bail!("release manifest contains unknown key {key:?}"),
        }
        if values.insert(key, value).is_some() {
            bail!("release manifest contains duplicate key {key:?}");
        }
    }

    if required(&values, "schema")? != "1" {
        bail!("release manifest has an unsupported schema");
    }
    let version_text = required(&values, "version")?;
    let version_parts = version_text.split('.').collect::<Vec<_>>();
    if version_parts.len() != 3
        || version_parts
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        bail!("release manifest version is not numeric SemVer");
    }
    let version =
        Version::parse(version_text).context("release manifest version is not valid SemVer")?;
    let x86_64 = parse_asset(&values, "x86_64")?;
    let aarch64 = parse_asset(&values, "aarch64")?;
    Ok(ReleaseManifest {
        version,
        x86_64,
        aarch64,
    })
}

fn parse_asset(values: &BTreeMap<&str, &str>, arch: &str) -> Result<ReleaseAsset> {
    let url_key = format!("{arch}_url");
    let sha_key = format!("{arch}_sha256");
    let url = required(values, &url_key)?;
    let parsed = url::Url::parse(url).with_context(|| format!("invalid {arch} asset URL"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        bail!("{arch} asset URL must use http:// or https://");
    }
    let sha256 = required(values, &sha_key)?;
    if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("invalid {arch} SHA-256");
    }
    Ok(ReleaseAsset {
        url: url.to_string(),
        sha256: sha256.to_ascii_lowercase(),
    })
}

fn required<'a>(values: &'a BTreeMap<&str, &str>, key: &str) -> Result<&'a str> {
    values
        .get(key)
        .copied()
        .with_context(|| format!("release manifest is missing {key:?}"))
}

async fn install_update(context: WorkerContext, version: Version) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let output = Command::new("sh")
            .arg(&context.installer)
            .arg("--feed-url")
            .arg(&context.feed_url)
            .arg("--expected-version")
            .arg(version.to_string())
            .output()
            .with_context(|| format!("failed to execute {}", context.installer.display()))?;
        if !output.status.success() {
            bail!(
                "Linux installer failed with status {}: {}",
                output
                    .status
                    .code()
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "<signal>".to_string()),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(())
    })
    .await
    .context("Linux updater worker panicked while installing update")?
}

pub fn restart_installed_app(context: &RestartContext) -> Result<()> {
    Command::new("sh")
        .arg("-c")
        .arg(RESTART_HELPER_SCRIPT)
        .arg("aura-update-restart")
        .arg(std::process::id().to_string())
        .arg(&context.stable_binary)
        .args(&context.relaunch_args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| {
            format!(
                "failed to schedule restart through {}",
                context.stable_binary.display()
            )
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn manifest() -> String {
        format!(
            "schema=1\nversion=1.2.3\nx86_64_url=https://example.com/x.tar.gz\nx86_64_sha256={SHA_A}\naarch64_url=https://example.com/a.tar.gz\naarch64_sha256={SHA_B}\n"
        )
    }

    #[test]
    fn parses_release_manifest() {
        let parsed = parse_manifest(&manifest()).unwrap();
        assert_eq!(parsed.version, Version::new(1, 2, 3));
        assert_eq!(parsed.x86_64.sha256, SHA_A);
        assert_eq!(parsed.aarch64.sha256, SHA_B);
    }

    #[test]
    fn rejects_unknown_duplicate_and_invalid_values() {
        assert!(parse_manifest(&(manifest() + "extra=value\n")).is_err());
        assert!(parse_manifest(&manifest().replace("schema=1", "schema=1\nschema=1")).is_err());
        assert!(parse_manifest(&manifest().replace(SHA_A, "short")).is_err());
        assert!(parse_manifest(&manifest().replace("1.2.3", "latest")).is_err());
        assert!(parse_manifest(&manifest().replace("1.2.3", "1.2.3-beta.1")).is_err());
        assert!(parse_manifest(&manifest().replace("1.2.3", "1.2.3+build.1")).is_err());
    }

    #[test]
    fn compares_release_versions_using_semver_ordering() {
        assert!(is_newer_release(
            &Version::new(2, 0, 0),
            &Version::new(1, 99, 99)
        ));
        assert!(!is_newer_release(
            &Version::new(1, 9, 0),
            &Version::new(1, 10, 0)
        ));
        assert!(!is_newer_release(
            &Version::new(1, 2, 3),
            &Version::new(1, 2, 3)
        ));
    }

    #[test]
    fn managed_install_requires_running_and_stable_binaries_to_match() {
        let temp = tempdir().unwrap();
        let data = temp.path().join("data");
        let home = temp.path().join("home");
        let release = data.join("aura/app/versions/1.2.3").join("bin");
        fs::create_dir_all(&release).unwrap();
        fs::create_dir_all(data.join("aura/app/versions/1.2.3")).unwrap();
        fs::create_dir_all(home.join(".local/bin")).unwrap();
        fs::write(release.join("aura"), b"binary").unwrap();
        fs::write(
            data.join("aura/app/versions/1.2.3/install.sh"),
            b"#!/bin/sh\n",
        )
        .unwrap();
        symlink("versions/1.2.3", data.join("aura/app/current")).unwrap();
        symlink(
            data.join("aura/app/current/bin/aura"),
            home.join(".local/bin/aura"),
        )
        .unwrap();

        let managed = locate_managed_install_from(&release.join("aura"), &data, &home).unwrap();
        assert!(managed.installer.ends_with("current/install.sh"));

        let other = temp.path().join("other");
        fs::write(&other, b"binary").unwrap();
        assert!(locate_managed_install_from(&other, &data, &home).is_err());
    }

    #[tokio::test]
    async fn worker_reports_manifest_fetch_failures() {
        let context = WorkerContext {
            installer: PathBuf::from("/missing/aura/install.sh"),
            feed_url: "://invalid-feed".to_string(),
            current_version: Version::new(1, 0, 0),
        };
        let (trigger_tx, trigger_rx) = unbounded_channel();
        let (event_tx, mut event_rx) = unbounded_channel();
        let worker = tokio::spawn(run_worker(context, trigger_rx, event_tx));

        trigger_tx.send(UpdateTrigger::Manual).unwrap();
        assert_eq!(
            event_rx.recv().await,
            Some(UpdaterEvent::Status(UpdaterStatus::Checking))
        );
        assert_eq!(
            event_rx.recv().await,
            Some(UpdaterEvent::Status(UpdaterStatus::Error))
        );
        drop(trigger_tx);
        worker.await.unwrap();
    }

    #[tokio::test]
    async fn installer_failure_does_not_report_success() {
        let temp = tempdir().unwrap();
        let installer = temp.path().join("install.sh");
        fs::write(&installer, "#!/bin/sh\nexit 23\n").unwrap();
        fs::set_permissions(&installer, fs::Permissions::from_mode(0o755)).unwrap();
        let context = WorkerContext {
            installer,
            feed_url: "https://example.invalid/releases/latest/download".to_string(),
            current_version: Version::new(1, 0, 0),
        };

        let error = install_update(context, Version::new(1, 1, 0))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("status 23"));
    }

    #[test]
    fn restart_helper_waits_for_old_process_before_launching() {
        let temp = tempdir().unwrap();
        let marker = temp.path().join("launched");
        let mut old_process = Command::new("sh")
            .args(["-c", "sleep 0.3"])
            .spawn()
            .unwrap();
        let mut helper = Command::new("sh")
            .arg("-c")
            .arg(RESTART_HELPER_SCRIPT)
            .arg("aura-update-restart-test")
            .arg(old_process.id().to_string())
            .arg("sh")
            .arg("-c")
            .arg("printf launched > \"$1\"")
            .arg("write-marker")
            .arg(&marker)
            .spawn()
            .unwrap();

        std::thread::sleep(Duration::from_millis(100));
        assert!(!marker.exists());
        assert!(old_process.wait().unwrap().success());
        assert!(helper.wait().unwrap().success());
        assert_eq!(fs::read_to_string(marker).unwrap(), "launched");
    }
}
