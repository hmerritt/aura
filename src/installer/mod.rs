use crate::errors::Result;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupRegistrationStatus {
    SkippedNotInstalled,
    AlreadyRegistered,
    RegisteredNow,
    ApprovalRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SquirrelEvent {
    Install,
    Updated,
    Uninstall,
    Obsolete,
    Firstrun,
}

impl SquirrelEvent {
    pub fn from_flag(flag: &str) -> Option<Self> {
        match flag {
            "--squirrel-install" => Some(Self::Install),
            "--squirrel-updated" => Some(Self::Updated),
            "--squirrel-uninstall" => Some(Self::Uninstall),
            "--squirrel-obsolete" => Some(Self::Obsolete),
            "--squirrel-firstrun" => Some(Self::Firstrun),
            _ => None,
        }
    }
}

#[cfg(windows)]
mod windows_squirrel;

#[cfg(windows)]
pub fn handle_squirrel_event(event: Option<SquirrelEvent>) -> Result<bool> {
    let Some(event) = event else {
        return Ok(false);
    };
    windows_squirrel::handle(event)
}

#[cfg(not(windows))]
pub fn handle_squirrel_event(event: Option<SquirrelEvent>) -> Result<bool> {
    let _ = event;
    Ok(false)
}

#[cfg(windows)]
pub fn locate_update_exe() -> Result<PathBuf> {
    windows_squirrel::locate_update_exe()
}

#[cfg(windows)]
pub fn ensure_startup_registered() -> Result<StartupRegistrationStatus> {
    windows_squirrel::ensure_startup_registered()
}

#[cfg(not(windows))]
pub fn locate_update_exe() -> Result<PathBuf> {
    anyhow::bail!("squirrel updates are only supported on Windows")
}

#[cfg(all(not(windows), not(target_os = "macos")))]
pub fn ensure_startup_registered() -> Result<StartupRegistrationStatus> {
    Ok(StartupRegistrationStatus::SkippedNotInstalled)
}

#[cfg(target_os = "macos")]
pub fn ensure_startup_registered() -> Result<StartupRegistrationStatus> {
    crate::macos_app::ensure_login_item()
}
