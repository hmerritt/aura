use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewAsset {
    pub revision: String,
    pub path: PathBuf,
    pub mime_type: String,
}

#[derive(Debug, Clone)]
pub enum SettingsUiEvent {
    IpcMessage(String),
    OpenFailed { message: String },
}

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::SettingsUiController;

#[cfg(windows)]
pub fn set_preview_assets(current: Option<PreviewAsset>, next: Option<PreviewAsset>) {
    windows::set_preview_assets(current, next);
}

#[cfg(windows)]
pub fn set_next_preview_asset_if_revision(next: PreviewAsset) {
    windows::set_next_preview_asset_if_revision(next);
}

#[cfg(not(windows))]
pub struct SettingsUiController;

#[cfg(not(windows))]
pub fn set_preview_assets(_current: Option<PreviewAsset>, _next: Option<PreviewAsset>) {}

#[cfg(not(windows))]
pub fn set_next_preview_asset_if_revision(_next: PreviewAsset) {}

#[cfg(not(windows))]
impl SettingsUiController {
    pub fn spawn(
        _event_tx: tokio::sync::mpsc::UnboundedSender<SettingsUiEvent>,
    ) -> crate::errors::Result<Self> {
        anyhow::bail!("settings UI is only supported on Windows")
    }

    pub fn open_window(&self, _anchor: crate::tray::TrayAnchor) -> crate::errors::Result<()> {
        anyhow::bail!("settings UI is only supported on Windows")
    }

    pub fn close_window(&self) -> crate::errors::Result<()> {
        anyhow::bail!("settings UI is only supported on Windows")
    }

    pub fn dispatch_json(&self, _json: String) -> crate::errors::Result<()> {
        anyhow::bail!("settings UI is only supported on Windows")
    }
}
