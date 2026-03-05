use anyhow::{Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

const APP_DIR_NAME: &str = "aura";
const DEBUG_LOG_FILENAME: &str = "aura-debug.log";

pub struct DebugCapture {
    _stdout_redirect: gag::Redirect<File>,
    _stderr_redirect: gag::Redirect<File>,
    path: PathBuf,
}

impl DebugCapture {
    pub fn init() -> Result<Self> {
        let path = debug_log_path()?;
        let log_file = open_debug_log(&path)?;
        let stdout_redirect = gag::Redirect::stdout(
            log_file
                .try_clone()
                .with_context(|| format!("failed to clone debug log handle {}", path.display()))?,
        )
        .context("failed to redirect stdout to debug log file")?;
        let stderr_redirect = gag::Redirect::stderr(log_file)
            .context("failed to redirect stderr to debug log file")?;

        Ok(Self {
            _stdout_redirect: stdout_redirect,
            _stderr_redirect: stderr_redirect,
            path,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn is_debug_requested(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--debug")
}

pub fn debug_log_path() -> Result<PathBuf> {
    let Some(local_data_dir) = dirs::data_local_dir() else {
        anyhow::bail!("failed to resolve local data directory");
    };
    Ok(build_debug_log_path(&local_data_dir))
}

fn build_debug_log_path(local_data_dir: &Path) -> PathBuf {
    local_data_dir.join(APP_DIR_NAME).join(DEBUG_LOG_FILENAME)
}

fn open_debug_log(path: &Path) -> Result<File> {
    let existed = path.exists();
    let Some(parent) = path.parent() else {
        anyhow::bail!("debug log path has no parent directory");
    };
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create debug log directory {}",
            parent.display()
        )
    })?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open debug log file {}", path.display()))?;

    if existed {
        writeln!(file, "\n\n--- Run [{}] ---", run_timestamp())
            .with_context(|| format!("failed to write run banner to {}", path.display()))?;
        file.flush()
            .with_context(|| format!("failed to flush run banner to {}", path.display()))?;
    }

    Ok(file)
}

fn run_timestamp() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown-timestamp".to_string())
}

pub fn install_debug_panic_hook() {
    std::panic::set_hook(Box::new(|panic_info| {
        let _ = writeln!(std::io::stderr(), "panic: {panic_info}");
        let _ = writeln!(
            std::io::stderr(),
            "backtrace:\n{}",
            std::backtrace::Backtrace::force_capture()
        );
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn detects_debug_flag_in_args() {
        assert!(is_debug_requested(&[
            "--no-tray".to_string(),
            "--debug".to_string()
        ]));
        assert!(!is_debug_requested(&["--version".to_string()]));
    }

    #[test]
    fn builds_log_path_under_aura_dir() {
        let root = PathBuf::from("C:\\Users\\user\\AppData\\Local");
        let path = build_debug_log_path(&root);
        assert_eq!(path, root.join("aura").join("aura-debug.log"));
    }

    #[test]
    fn log_file_is_opened_in_append_mode() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("aura").join("aura-debug.log");

        {
            let mut file = open_debug_log(&path).unwrap();
            writeln!(file, "first").unwrap();
        }
        {
            let mut file = open_debug_log(&path).unwrap();
            writeln!(file, "second").unwrap();
        }

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("first"));
        assert!(content.contains("second"));
    }

    #[test]
    fn existing_log_gets_run_separator_banner() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("aura").join("aura-debug.log");

        {
            let mut file = open_debug_log(&path).unwrap();
            writeln!(file, "first").unwrap();
        }
        {
            let _file = open_debug_log(&path).unwrap();
        }

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("\n\n--- Run ["));
    }
}
