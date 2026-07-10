#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateTrigger {
    Startup,
    Periodic,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdaterStatus {
    Disabled,
    Unsupported,
    Idle,
    Checking,
    UpdateAvailable,
    Installing,
    UpToDate,
    InstalledPendingRestart,
    Error,
}

impl UpdaterStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::Unsupported => "Unsupported",
            Self::Idle => "Idle",
            Self::Checking => "Checking",
            Self::UpdateAvailable => "Update Available",
            Self::Installing => "Installing",
            Self::UpToDate => "Up To Date",
            Self::InstalledPendingRestart => "Installed (Restart Pending)",
            Self::Error => "Error",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdaterEvent {
    Status(UpdaterStatus),
    InstallReady,
}

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{initialize, restart_installed_app, RestartContext, UpdaterRuntime};

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub use linux::{initialize, restart_installed_app, RestartContext, UpdaterRuntime};

#[cfg(not(any(windows, target_os = "linux")))]
mod non_windows {
    use super::*;
    use crate::config::UpdaterConfig;
    use crate::errors::Result;
    use std::time::Duration;

    #[derive(Debug, Clone)]
    pub struct RestartContext;

    pub struct UpdaterRuntime {
        status: UpdaterStatus,
        check_interval: Option<Duration>,
    }

    impl UpdaterRuntime {
        pub fn status(&self) -> UpdaterStatus {
            self.status
        }

        pub fn check_interval(&self) -> Option<Duration> {
            self.check_interval
        }

        pub fn request_check(&self, _trigger: UpdateTrigger) -> bool {
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

    pub fn initialize(config: &UpdaterConfig, _relaunch_args: Vec<String>) -> UpdaterRuntime {
        let status = if config.enabled {
            UpdaterStatus::Unsupported
        } else {
            UpdaterStatus::Disabled
        };

        UpdaterRuntime {
            status,
            check_interval: None,
        }
    }

    pub fn restart_installed_app(_ctx: &RestartContext) -> Result<()> {
        anyhow::bail!("self-update restart is only supported on Windows")
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
pub use non_windows::{initialize, restart_installed_app, RestartContext, UpdaterRuntime};
