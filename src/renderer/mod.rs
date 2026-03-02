#[derive(Debug, Clone)]
pub enum RendererCommand {
    DisableOutput,
    Stop,
}

#[derive(Debug, Clone)]
pub enum RendererEvent {
    Ready,
    Running,
    Paused,
    Fatal { message: String },
    Stopped,
}

#[cfg(windows)]
mod desktop_windows;
#[cfg(windows)]
mod engine;
#[cfg(windows)]
mod precompiled;
#[cfg(windows)]
mod wgpu_runtime;

#[cfg(windows)]
pub use engine::ShaderRenderer;

#[cfg(not(windows))]
use crate::config::ShaderConfig;
#[cfg(not(windows))]
use crate::errors::Result;
#[cfg(not(windows))]
pub struct ShaderRenderer;

#[cfg(not(windows))]
impl ShaderRenderer {
    pub fn start(_config: ShaderConfig) -> Result<Self> {
        anyhow::bail!("shader renderer is only supported on Windows")
    }

    pub fn take_event_receiver(
        &mut self,
    ) -> Option<tokio::sync::mpsc::UnboundedReceiver<RendererEvent>> {
        None
    }

    pub fn send_command(&self, _command: RendererCommand) -> Result<()> {
        Ok(())
    }

    pub fn stop(&mut self) {}
}
