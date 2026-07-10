use super::{UpdateTrigger, UpdaterEvent, UpdaterStatus};
use crate::config::UpdaterConfig;
use crate::errors::Result;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RestartContext;

pub struct UpdaterRuntime;

impl UpdaterRuntime {
    pub fn status(&self) -> UpdaterStatus {
        UpdaterStatus::ExternallyManaged
    }

    pub fn check_interval(&self) -> Option<Duration> {
        None
    }

    pub fn request_check(&self, trigger: UpdateTrigger) -> bool {
        if trigger == UpdateTrigger::Manual {
            crate::macos_app::show_update_instructions();
        }
        false
    }

    pub fn take_event_receiver(
        &mut self,
    ) -> Option<tokio::sync::mpsc::UnboundedReceiver<UpdaterEvent>> {
        None
    }

    pub fn restart_context(&self) -> Option<RestartContext> {
        None
    }
}

pub fn initialize(_config: &UpdaterConfig, _relaunch_args: Vec<String>) -> UpdaterRuntime {
    UpdaterRuntime
}

pub fn restart_installed_app(_ctx: &RestartContext) -> Result<()> {
    anyhow::bail!("Aura updates on macOS are managed by Homebrew")
}
