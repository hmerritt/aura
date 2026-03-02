#[derive(Debug, Clone, Copy)]
pub enum TrayEvent {
    NextWallpaper,
    Exit,
}

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{spawn, try_acquire_single_instance};

#[cfg(not(windows))]
pub struct TrayController;

#[cfg(not(windows))]
impl TrayController {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(windows))]
pub struct SingleInstanceGuard;

#[cfg(not(windows))]
use crate::errors::Result;
#[cfg(not(windows))]
use std::path::PathBuf;
#[cfg(not(windows))]
use tokio::sync::mpsc::UnboundedSender;

#[cfg(not(windows))]
pub fn try_acquire_single_instance() -> Result<Option<SingleInstanceGuard>> {
    Ok(Some(SingleInstanceGuard))
}

#[cfg(not(windows))]
pub fn spawn(
    _config_path: PathBuf,
    _event_tx: UnboundedSender<TrayEvent>,
) -> Result<TrayController> {
    Ok(TrayController::new())
}
