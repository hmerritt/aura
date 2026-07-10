use crate::errors::Result;
use crate::installer::SquirrelEvent;
use crate::installer::StartupRegistrationStatus;
use crate::version::{APP_NAME, BINARY_FILENAME};
use anyhow::Context;
use std::ffi::OsStr;
use std::fs;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE,
    REG_OPTION_NON_VOLATILE, REG_SZ,
};

const UNINSTALL_REGISTRY_BASE: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
const DISPLAY_VERSION: &str = env!("CARGO_PKG_VERSION");
const APP_ICON_FILENAME: &str = "app.ico";
const PUBLISHER: &str = match option_env!("AURA_PUBLISHER") {
    Some(value) => value,
    None => "",
};

pub fn handle(event: SquirrelEvent) -> Result<bool> {
    match event {
        SquirrelEvent::Install | SquirrelEvent::Updated => {
            let shortcut_result = create_startup_and_start_menu_shortcuts();
            let metadata_result = upsert_uninstall_registry_metadata();

            match (shortcut_result, metadata_result) {
                (Ok(()), Ok(())) => {}
                (Err(shortcut_error), Ok(())) => return Err(shortcut_error),
                (Ok(()), Err(metadata_error)) => return Err(metadata_error),
                (Err(shortcut_error), Err(metadata_error)) => {
                    anyhow::bail!(
                        "failed to create startup/start menu shortcuts: {shortcut_error:#}; \
                         additionally failed to upsert uninstall registry metadata: {metadata_error:#}"
                    );
                }
            }

            Ok(true)
        }
        SquirrelEvent::Uninstall => {
            remove_startup_and_start_menu_shortcuts()?;
            Ok(true)
        }
        SquirrelEvent::Obsolete => Ok(true),
        SquirrelEvent::Firstrun => Ok(false),
    }
}

pub fn ensure_startup_registered() -> Result<StartupRegistrationStatus> {
    let current_exe = std::env::current_exe().context("failed to resolve current executable")?;
    let update_exe = locate_update_exe_if_installed_from_current_exe(&current_exe)?;
    if update_exe.is_some() {
        if let Err(error) = sync_app_icon_to_squirrel_root(&current_exe) {
            tracing::warn!(
                error = %error,
                "failed to sync root app.ico during startup registration check"
            );
        }
        if let Err(error) = upsert_uninstall_registry_metadata_for_current_exe(&current_exe) {
            tracing::warn!(
                error = %error,
                "failed to upsert uninstall registry metadata during startup registration check"
            );
        }
    }
    let startup_shortcut_path = startup_shortcut_path()?;
    let startup_shortcut_exists = startup_shortcut_path.exists();
    ensure_startup_registered_inner(
        update_exe.as_deref(),
        startup_shortcut_exists,
        create_startup_and_start_menu_shortcuts_with_update_exe,
    )
}

fn create_startup_and_start_menu_shortcuts() -> Result<()> {
    let update_exe = locate_update_exe()?;
    create_startup_and_start_menu_shortcuts_with_update_exe(&update_exe)
}

fn create_startup_and_start_menu_shortcuts_with_update_exe(update_exe: &Path) -> Result<()> {
    let exe_name = binary_filename_with_extension();

    let variants = [
        vec![
            "--createShortcut".to_string(),
            exe_name.clone(),
            "--shortcut-locations=Startup,StartMenu".to_string(),
        ],
        vec![
            format!("--createShortcut={exe_name}"),
            "--shortcut-locations=Startup,StartMenu".to_string(),
        ],
        vec![
            "--createShortcut".to_string(),
            exe_name.clone(),
            "-l=Startup,StartMenu".to_string(),
        ],
        vec![
            format!("--createShortcut={exe_name}"),
            "-l=Startup,StartMenu".to_string(),
        ],
    ];

    run_update_variants(update_exe, &variants, "create startup/start menu shortcuts")
}

fn remove_startup_and_start_menu_shortcuts() -> Result<()> {
    let update_exe = locate_update_exe()?;
    remove_startup_and_start_menu_shortcuts_with_update_exe(&update_exe)
}

fn remove_startup_and_start_menu_shortcuts_with_update_exe(update_exe: &Path) -> Result<()> {
    let exe_name = binary_filename_with_extension();

    let variants = [
        vec![
            "--removeShortcut".to_string(),
            exe_name.clone(),
            "--shortcut-locations=Startup,StartMenu".to_string(),
        ],
        vec![
            format!("--removeShortcut={exe_name}"),
            "--shortcut-locations=Startup,StartMenu".to_string(),
        ],
        vec![
            "--removeShortcut".to_string(),
            exe_name.clone(),
            "-l=Startup,StartMenu".to_string(),
        ],
        vec![
            format!("--removeShortcut={exe_name}"),
            "-l=Startup,StartMenu".to_string(),
        ],
    ];

    run_update_variants(update_exe, &variants, "remove startup/start menu shortcuts")
}

pub(crate) fn locate_update_exe() -> Result<PathBuf> {
    let current_exe = std::env::current_exe().context("failed to resolve current executable")?;
    let update_exe = update_exe_path_from_current_exe(&current_exe)?;
    if !update_exe.exists() {
        anyhow::bail!(
            "squirrel Update.exe was not found at {}; lifecycle event cannot continue",
            update_exe.display()
        );
    }
    Ok(update_exe)
}

fn locate_update_exe_if_installed_from_current_exe(current_exe: &Path) -> Result<Option<PathBuf>> {
    let update_exe = update_exe_path_from_current_exe(current_exe)?;
    if update_exe.exists() {
        return Ok(Some(update_exe));
    }
    Ok(None)
}

fn update_exe_path_from_current_exe(current_exe: &Path) -> Result<PathBuf> {
    let root_dir = squirrel_root_dir_from_current_exe(current_exe)?;
    Ok(root_dir.join("Update.exe"))
}

fn startup_shortcut_path() -> Result<PathBuf> {
    let appdata = std::env::var_os("APPDATA")
        .context("failed to resolve APPDATA for startup shortcut path")?;
    Ok(startup_shortcut_path_from_appdata(Path::new(&appdata)))
}

fn startup_shortcut_path_from_appdata(appdata: &Path) -> PathBuf {
    appdata
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Startup")
        .join(format!("{}.lnk", BINARY_FILENAME))
}

fn ensure_startup_registered_inner<F>(
    update_exe: Option<&Path>,
    startup_shortcut_exists: bool,
    create_shortcuts: F,
) -> Result<StartupRegistrationStatus>
where
    F: FnOnce(&Path) -> Result<()>,
{
    let Some(update_exe) = update_exe else {
        return Ok(StartupRegistrationStatus::SkippedNotInstalled);
    };

    if startup_shortcut_exists {
        return Ok(StartupRegistrationStatus::AlreadyRegistered);
    }

    create_shortcuts(update_exe)?;
    Ok(StartupRegistrationStatus::RegisteredNow)
}

fn run_update_variants(update_exe: &Path, variants: &[Vec<String>], action: &str) -> Result<()> {
    let mut failures = Vec::new();

    for args in variants {
        let output = Command::new(update_exe)
            .args(args)
            .output()
            .with_context(|| format!("failed to execute {}", update_exe.display()))?;

        if output.status.success() {
            return Ok(());
        }

        failures.push(format!(
            "args=[{}] status={} stderr={}",
            args.join(" "),
            output
                .status
                .code()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "<signal>".to_string()),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    anyhow::bail!(
        "failed to {action} via {}: {}",
        update_exe.display(),
        failures.join(" | ")
    );
}

fn binary_filename_with_extension() -> String {
    format!("{BINARY_FILENAME}.exe")
}

fn upsert_uninstall_registry_metadata() -> Result<()> {
    let current_exe = std::env::current_exe()
        .context("failed to resolve current executable for uninstall registry metadata")?;
    upsert_uninstall_registry_metadata_for_current_exe(&current_exe)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UninstallRegistryMetadata {
    display_icon: String,
    display_version: String,
    publisher: Option<String>,
}

fn upsert_uninstall_registry_metadata_for_current_exe(current_exe: &Path) -> Result<()> {
    let metadata = build_uninstall_registry_metadata(current_exe, PUBLISHER);

    with_uninstall_registry_key(|key| {
        set_registry_string_value(key, "DisplayIcon", &metadata.display_icon)?;
        set_registry_string_value(key, "DisplayVersion", &metadata.display_version)?;
        if let Some(publisher) = metadata.publisher.as_deref() {
            set_registry_string_value(key, "Publisher", publisher)?;
        }
        Ok(())
    })
}

fn uninstall_registry_subkey() -> String {
    format!(r"{UNINSTALL_REGISTRY_BASE}\{APP_NAME}")
}

fn resolve_display_icon_value(current_exe: &Path) -> String {
    if let Ok(icon_path) = sync_app_icon_to_squirrel_root(current_exe) {
        return display_icon_value_for_icon_file(&icon_path);
    }

    if let Ok(root_icon_path) = app_icon_destination_path_from_current_exe(current_exe) {
        if root_icon_path.exists() {
            return display_icon_value_for_icon_file(&root_icon_path);
        }
    }

    if let Ok(version_icon_path) = app_icon_source_path_from_current_exe(current_exe) {
        if version_icon_path.exists() {
            return display_icon_value_for_icon_file(&version_icon_path);
        }
    }

    display_icon_value_for_executable(current_exe)
}

fn squirrel_root_dir_from_current_exe(current_exe: &Path) -> Result<PathBuf> {
    let app_dir = current_exe.parent().with_context(|| {
        format!(
            "failed to resolve app directory for {}",
            current_exe.display()
        )
    })?;

    // Prefer the nearest ancestor containing Update.exe.
    if let Some(root_dir) = app_dir
        .ancestors()
        .find(|candidate| candidate.join("Update.exe").exists())
    {
        return Ok(root_dir.to_path_buf());
    }

    // Fallback to standard Squirrel app-version layout.
    let root_dir = app_dir.parent().with_context(|| {
        format!(
            "failed to resolve squirrel root directory from {}",
            app_dir.display()
        )
    })?;
    Ok(root_dir.to_path_buf())
}

fn app_icon_source_path_from_current_exe(current_exe: &Path) -> Result<PathBuf> {
    let app_dir = current_exe.parent().with_context(|| {
        format!(
            "failed to resolve app directory for {}",
            current_exe.display()
        )
    })?;
    Ok(app_dir.join(APP_ICON_FILENAME))
}

fn app_icon_destination_path_from_current_exe(current_exe: &Path) -> Result<PathBuf> {
    let root_dir = squirrel_root_dir_from_current_exe(current_exe)?;
    Ok(root_dir.join(APP_ICON_FILENAME))
}

fn sync_app_icon_to_squirrel_root(current_exe: &Path) -> Result<PathBuf> {
    let source = app_icon_source_path_from_current_exe(current_exe)?;
    let destination = app_icon_destination_path_from_current_exe(current_exe)?;

    if !source.exists() {
        anyhow::bail!(
            "packaged app icon does not exist at {}; cannot refresh root app icon",
            source.display()
        );
    }

    fs::copy(&source, &destination).with_context(|| {
        format!(
            "failed to copy app icon from {} to {}",
            source.display(),
            destination.display()
        )
    })?;

    Ok(destination)
}

fn display_icon_value_for_executable(current_exe: &Path) -> String {
    format!("{},0", current_exe.display())
}

fn display_icon_value_for_icon_file(icon_path: &Path) -> String {
    icon_path.display().to_string()
}

fn build_uninstall_registry_metadata(
    current_exe: &Path,
    publisher_raw: &str,
) -> UninstallRegistryMetadata {
    UninstallRegistryMetadata {
        display_icon: resolve_display_icon_value(current_exe),
        display_version: DISPLAY_VERSION.to_string(),
        publisher: normalize_publisher_value(publisher_raw),
    }
}

fn normalize_publisher_value(raw_value: &str) -> Option<String> {
    let value = raw_value.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_string())
}

fn with_uninstall_registry_key<F>(write_values: F) -> Result<()>
where
    F: FnOnce(HKEY) -> Result<()>,
{
    let subkey = uninstall_registry_subkey();
    let subkey_wide = to_wide_z(&subkey);
    let mut key: HKEY = ptr::null_mut();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey_wide.as_ptr(),
            0,
            ptr::null_mut(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            ptr::null(),
            &mut key,
            ptr::null_mut(),
        )
    };
    if status != 0 {
        anyhow::bail!(
            "failed to create/open uninstall registry key '{}' (status={status})",
            subkey
        );
    }

    let write_result = write_values(key);
    let close_status = unsafe { RegCloseKey(key) };
    write_result?;
    if close_status != 0 {
        anyhow::bail!(
            "failed to close uninstall registry key '{}' (status={close_status})",
            subkey
        );
    }

    Ok(())
}

fn set_registry_string_value(key: HKEY, name: &str, value: &str) -> Result<()> {
    let name_wide = to_wide_z(name);
    let value_wide = to_wide_z(value);
    let value_bytes = (value_wide.len() * size_of::<u16>()) as u32;
    let status = unsafe {
        RegSetValueExW(
            key,
            name_wide.as_ptr(),
            0,
            REG_SZ,
            value_wide.as_ptr() as *const u8,
            value_bytes,
        )
    };
    if status != 0 {
        anyhow::bail!("failed to set registry value '{}' (status={status})", name);
    }
    Ok(())
}

fn to_wide_z(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn startup_shortcut_path_uses_expected_windows_location() {
        let appdata = Path::new(r"C:\Users\alice\AppData\Roaming");
        let shortcut = startup_shortcut_path_from_appdata(appdata);
        assert!(shortcut.ends_with(Path::new(r"Startup\aura.lnk")));
    }

    #[test]
    fn update_exe_path_resolves_from_current_exe_layout() {
        let current_exe = Path::new(r"C:\Users\alice\AppData\Local\aura\app-1.2.3\aura.exe");
        let update_exe = update_exe_path_from_current_exe(current_exe).unwrap();
        assert_eq!(
            update_exe,
            PathBuf::from(r"C:\Users\alice\AppData\Local\aura\Update.exe")
        );
    }

    #[test]
    fn startup_registration_skips_when_not_installed() {
        let status = ensure_startup_registered_inner(None, false, |_| {
            anyhow::bail!("create should not be called")
        })
        .unwrap();
        assert_eq!(status, StartupRegistrationStatus::SkippedNotInstalled);
    }

    #[test]
    fn startup_registration_is_noop_when_shortcut_exists() {
        let status = ensure_startup_registered_inner(Some(Path::new("Update.exe")), true, |_| {
            anyhow::bail!("create should not be called")
        })
        .unwrap();
        assert_eq!(status, StartupRegistrationStatus::AlreadyRegistered);
    }

    #[test]
    fn startup_registration_restores_when_shortcut_is_missing() {
        let mut create_called = false;
        let status = ensure_startup_registered_inner(Some(Path::new("Update.exe")), false, |_| {
            create_called = true;
            Ok(())
        })
        .unwrap();
        assert_eq!(status, StartupRegistrationStatus::RegisteredNow);
        assert!(create_called);
    }

    #[test]
    fn update_exe_path_supports_temp_layout() {
        let tmp = tempdir().unwrap();
        let app_dir = tmp.path().join("app-2.0.0");
        std::fs::create_dir_all(&app_dir).unwrap();
        let current_exe = app_dir.join("aura.exe");
        std::fs::write(&current_exe, b"").unwrap();

        let update_exe = update_exe_path_from_current_exe(&current_exe).unwrap();
        assert_eq!(update_exe, tmp.path().join("Update.exe"));
    }

    #[test]
    fn app_icon_source_path_resolves_from_current_exe_layout() {
        let current_exe = Path::new(r"C:\Users\alice\AppData\Local\aura\app-1.2.3\aura.exe");
        let source = app_icon_source_path_from_current_exe(current_exe).unwrap();
        assert_eq!(
            source,
            PathBuf::from(r"C:\Users\alice\AppData\Local\aura\app-1.2.3\app.ico")
        );
    }

    #[test]
    fn app_icon_destination_path_resolves_to_squirrel_root() {
        let current_exe = Path::new(r"C:\Users\alice\AppData\Local\aura\app-1.2.3\aura.exe");
        let destination = app_icon_destination_path_from_current_exe(current_exe).unwrap();
        assert_eq!(
            destination,
            PathBuf::from(r"C:\Users\alice\AppData\Local\aura\app.ico")
        );
    }

    #[test]
    fn sync_app_icon_to_squirrel_root_copies_icon_file() {
        let tmp = tempdir().unwrap();
        let app_dir = tmp.path().join("app-1.2.3");
        std::fs::create_dir_all(&app_dir).unwrap();
        let current_exe = app_dir.join("aura.exe");
        std::fs::write(&current_exe, b"exe").unwrap();
        let source_icon = app_dir.join("app.ico");
        std::fs::write(&source_icon, b"icon-data").unwrap();

        let copied = sync_app_icon_to_squirrel_root(&current_exe).unwrap();
        assert_eq!(copied, tmp.path().join("app.ico"));
        assert_eq!(std::fs::read(copied).unwrap(), b"icon-data");
    }

    #[test]
    fn resolve_display_icon_value_falls_back_to_executable_when_app_icon_missing() {
        let tmp = tempdir().unwrap();
        let app_dir = tmp.path().join("app-1.2.3");
        std::fs::create_dir_all(&app_dir).unwrap();
        let current_exe = app_dir.join("aura.exe");
        std::fs::write(&current_exe, b"exe").unwrap();

        let value = resolve_display_icon_value(&current_exe);
        assert_eq!(value, format!("{},0", current_exe.display()));
    }

    #[test]
    fn resolve_display_icon_value_falls_back_to_existing_root_icon_when_sync_fails() {
        let tmp = tempdir().unwrap();
        let app_dir = tmp.path().join("app-1.2.3");
        std::fs::create_dir_all(&app_dir).unwrap();
        let current_exe = app_dir.join("aura.exe");
        std::fs::write(&current_exe, b"exe").unwrap();
        std::fs::write(tmp.path().join("app.ico"), b"root-icon").unwrap();

        let value = resolve_display_icon_value(&current_exe);
        assert_eq!(value, tmp.path().join("app.ico").display().to_string());
    }

    #[test]
    fn squirrel_root_dir_prefers_nearest_update_exe_ancestor() {
        let tmp = tempdir().unwrap();
        let root_dir = tmp.path().join("aura");
        let app_dir = root_dir.join("app-1.2.3");
        std::fs::create_dir_all(&app_dir).unwrap();
        std::fs::write(root_dir.join("Update.exe"), b"update").unwrap();
        let current_exe = app_dir.join("aura.exe");
        std::fs::write(&current_exe, b"exe").unwrap();

        let root = squirrel_root_dir_from_current_exe(&current_exe).unwrap();
        assert_eq!(root, root_dir);
    }

    #[test]
    fn squirrel_root_dir_supports_root_executable_layout_with_update_exe() {
        let tmp = tempdir().unwrap();
        let root_dir = tmp.path().join("aura");
        std::fs::create_dir_all(&root_dir).unwrap();
        std::fs::write(root_dir.join("Update.exe"), b"update").unwrap();
        let current_exe = root_dir.join("aura.exe");
        std::fs::write(&current_exe, b"exe").unwrap();

        let root = squirrel_root_dir_from_current_exe(&current_exe).unwrap();
        assert_eq!(root, root_dir);
    }

    #[test]
    fn uninstall_registry_subkey_is_app_specific() {
        assert_eq!(
            uninstall_registry_subkey(),
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall\aura"
        );
    }

    #[test]
    fn display_icon_value_for_executable_appends_icon_index() {
        let exe_path = Path::new(r"C:\Users\alice\AppData\Local\aura\app-1.2.3\aura.exe");
        assert_eq!(
            display_icon_value_for_executable(exe_path),
            r"C:\Users\alice\AppData\Local\aura\app-1.2.3\aura.exe,0"
        );
    }

    #[test]
    fn display_icon_value_for_icon_file_uses_plain_path() {
        let icon_path = Path::new(r"C:\Users\alice\AppData\Local\aura\app.ico");
        assert_eq!(
            display_icon_value_for_icon_file(icon_path),
            r"C:\Users\alice\AppData\Local\aura\app.ico"
        );
    }

    #[test]
    fn normalize_publisher_value_returns_none_for_empty_input() {
        assert_eq!(normalize_publisher_value(""), None);
        assert_eq!(normalize_publisher_value("   "), None);
    }

    #[test]
    fn normalize_publisher_value_trims_non_empty_input() {
        assert_eq!(
            normalize_publisher_value("  Aura Publisher  "),
            Some("Aura Publisher".to_string())
        );
    }

    #[test]
    fn build_uninstall_registry_metadata_omits_empty_publisher_and_keeps_required_fields() {
        let tmp = tempdir().unwrap();
        let app_dir = tmp.path().join("app-1.2.3");
        std::fs::create_dir_all(&app_dir).unwrap();
        let current_exe = app_dir.join("aura.exe");
        std::fs::write(&current_exe, b"exe").unwrap();

        let metadata = build_uninstall_registry_metadata(&current_exe, "   ");

        assert_eq!(
            metadata.display_icon,
            format!("{},0", current_exe.display())
        );
        assert_eq!(metadata.display_version, DISPLAY_VERSION);
        assert_eq!(metadata.publisher, None);
    }
}
