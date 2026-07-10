use super::{SessionStats, TrayEvent};
use crate::errors::Result;
use anyhow::Context;
use std::fs::{self, File, OpenOptions};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

pub struct SingleInstanceGuard {
    _file: File,
}

pub struct TrayController;

impl Drop for TrayController {
    fn drop(&mut self) {
        crate::macos_app::destroy_tray();
    }
}

pub fn try_acquire_single_instance() -> Result<Option<SingleInstanceGuard>> {
    let lock_dir = dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("aura");
    fs::create_dir_all(&lock_dir)
        .with_context(|| format!("failed to create {}", lock_dir.display()))?;
    let lock_path = lock_dir.join("aura.lock");
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open {}", lock_path.display()))?;
    try_lock_file(file)
}

fn try_lock_file(file: File) -> Result<Option<SingleInstanceGuard>> {
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result == 0 {
        Ok(Some(SingleInstanceGuard { _file: file }))
    } else if std::io::Error::last_os_error().kind() == std::io::ErrorKind::WouldBlock {
        Ok(None)
    } else {
        Err(std::io::Error::last_os_error()).context("failed to lock the Aura process lock")
    }
}

pub async fn spawn(
    config_path: PathBuf,
    event_tx: UnboundedSender<TrayEvent>,
    session_stats: Arc<SessionStats>,
) -> Result<TrayController> {
    crate::macos_app::create_tray(config_path, event_tx, session_stats)?;
    Ok(TrayController)
}

pub fn open_settings(path: &Path) -> Result<()> {
    crate::macos_app::open_settings(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_lock_rejects_a_second_file_description() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("aura.lock");
        let open = || {
            OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .open(&path)
                .unwrap()
        };
        let first = try_lock_file(open()).unwrap().expect("first lock");
        assert!(try_lock_file(open()).unwrap().is_none());
        drop(first);
        assert!(try_lock_file(open()).unwrap().is_some());
    }
}
