use super::RendererEvent;
use crate::config::ShaderConfig;
use crate::errors::Result;
use crate::linux_desktop;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;

pub struct ShaderRenderer {
    generation: AtomicU64,
    event_tx: UnboundedSender<RendererEvent>,
    event_rx: Option<UnboundedReceiver<RendererEvent>>,
}

impl ShaderRenderer {
    pub fn start(config: ShaderConfig) -> Result<Self> {
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let generation = linux_desktop::publish_shader(config, event_tx.clone())?;
        Ok(Self {
            generation: AtomicU64::new(generation),
            event_tx,
            event_rx: Some(event_rx),
        })
    }

    pub fn take_event_receiver(&mut self) -> Option<UnboundedReceiver<RendererEvent>> {
        self.event_rx.take()
    }

    pub async fn apply_config(&self, config: ShaderConfig) -> Result<()> {
        let generation = linux_desktop::publish_shader(config, self.event_tx.clone())?;
        self.generation.store(generation, Ordering::Release);
        Ok(())
    }

    pub async fn stop_async(&mut self) -> Result<()> {
        linux_desktop::stop_shader(self.generation.load(Ordering::Acquire));
        Ok(())
    }
}
