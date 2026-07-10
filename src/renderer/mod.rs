#[derive(Debug, Clone)]
pub enum RendererEvent {
    Ready,
    Running,
    Fatal { message: String },
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DesktopRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[cfg(windows)]
mod desktop_windows;
#[cfg(windows)]
mod engine;
#[cfg(all(test, any(windows, target_os = "macos")))]
mod golden_tests;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
pub(crate) mod precompiled;
#[cfg(any(windows, target_os = "macos"))]
mod wgpu_runtime;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
pub(crate) mod macos;

#[cfg(windows)]
pub use engine::ShaderRenderer;

#[cfg(target_os = "linux")]
pub use linux::ShaderRenderer;

#[cfg(target_os = "macos")]
pub use macos::ShaderRenderer;

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
use crate::config::ShaderConfig;
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
use crate::errors::Result;
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub struct ShaderRenderer;

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
impl ShaderRenderer {
    pub fn start(_config: ShaderConfig) -> Result<Self> {
        anyhow::bail!("shader renderer is only supported on Windows")
    }

    pub fn take_event_receiver(
        &mut self,
    ) -> Option<tokio::sync::mpsc::UnboundedReceiver<RendererEvent>> {
        None
    }

    pub async fn apply_config(&self, _config: ShaderConfig) -> Result<()> {
        anyhow::bail!("shader renderer is only supported on Windows")
    }

    pub async fn stop_async(&mut self) -> Result<()> {
        Ok(())
    }
}
