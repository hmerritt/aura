#[derive(Debug, Clone)]
pub enum RendererEvent {
    Ready,
    Running,
    Fatal { message: String },
    Stopped,
}

#[cfg(windows)]
mod desktop_windows;
#[cfg(windows)]
mod engine;
#[cfg(any(windows, target_os = "linux"))]
pub(crate) mod precompiled;
#[cfg(windows)]
mod wgpu_runtime;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(windows)]
pub use engine::ShaderRenderer;

#[cfg(target_os = "linux")]
pub use linux::ShaderRenderer;

#[cfg(not(any(windows, target_os = "linux")))]
use crate::config::ShaderConfig;
#[cfg(not(any(windows, target_os = "linux")))]
use crate::errors::Result;
#[cfg(not(any(windows, target_os = "linux")))]
pub struct ShaderRenderer;

#[cfg(not(any(windows, target_os = "linux")))]
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
