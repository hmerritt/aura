#[derive(Debug, Clone)]
pub enum SettingsUiEvent {
    IpcMessage(String),
    OpenFailed { message: String },
}

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::SettingsUiController;

#[cfg(not(windows))]
pub struct SettingsUiController;

#[cfg(not(windows))]
impl SettingsUiController {
    pub fn spawn(
        _event_tx: tokio::sync::mpsc::UnboundedSender<SettingsUiEvent>,
    ) -> crate::errors::Result<Self> {
        anyhow::bail!("settings UI is only supported on Windows")
    }

    pub fn open_window(&self) -> crate::errors::Result<()> {
        anyhow::bail!("settings UI is only supported on Windows")
    }

    pub fn close_window(&self) -> crate::errors::Result<()> {
        anyhow::bail!("settings UI is only supported on Windows")
    }

    pub fn dispatch_json(&self, _json: String) -> crate::errors::Result<()> {
        anyhow::bail!("settings UI is only supported on Windows")
    }
}
