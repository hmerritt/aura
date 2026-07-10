use super::{format_running_duration, SessionStats, TrayEvent};
use crate::errors::Result;
use anyhow::{bail, Context};
use ksni::menu::{StandardItem, SubMenu};
use ksni::{MenuItem, TrayMethods};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

struct AuraTray {
    events: UnboundedSender<TrayEvent>,
    stats: Arc<SessionStats>,
}

impl ksni::Tray for AuraTray {
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "io.github.hmerritt.Aura".to_string()
    }

    fn title(&self) -> String {
        "Aura".to_string()
    }

    fn icon_name(&self) -> String {
        "preferences-desktop-wallpaper".to_string()
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let shader_active = self.stats.is_shader_active();
        let mode_label = if shader_active {
            format!("Shader: {}", self.stats.shader_name())
        } else {
            "Mode: Images".to_string()
        };
        vec![
            SubMenu {
                label: "Statistics".into(),
                submenu: vec![
                    info_item(mode_label),
                    info_item(format!("Image timer: {}", self.stats.timer_display())),
                    info_item(format!(
                        "Remote refresh: {}",
                        self.stats.remote_update_timer_display()
                    )),
                    info_item(format!("Images: {}", self.stats.total_images())),
                    info_item(format!("Shown: {}", self.stats.images_shown())),
                    info_item(format!("Skipped: {}", self.stats.manual_skips())),
                    info_item(format!(
                        "Running: {}",
                        format_running_duration(self.stats.running_duration())
                    )),
                ],
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Next Background".into(),
                icon_name: "go-next".into(),
                enabled: !shader_active,
                activate: Box::new(|tray: &mut AuraTray| {
                    let _ = tray.events.send(TrayEvent::NextWallpaper);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Reload Settings".into(),
                icon_name: "view-refresh".into(),
                activate: Box::new(|tray: &mut AuraTray| {
                    let _ = tray.events.send(TrayEvent::ReloadSettings);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Settings".into(),
                icon_name: "document-properties".into(),
                activate: Box::new(|tray: &mut AuraTray| {
                    let _ = tray.events.send(TrayEvent::OpenSettings);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Exit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|tray: &mut AuraTray| {
                    let _ = tray.events.send(TrayEvent::Exit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn info_item(label: String) -> MenuItem<AuraTray> {
    StandardItem {
        label,
        enabled: false,
        ..Default::default()
    }
    .into()
}

pub struct TrayController {
    handle: ksni::Handle<AuraTray>,
}

impl Drop for TrayController {
    fn drop(&mut self) {
        let handle = self.handle.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move { handle.shutdown().await });
        }
    }
}

pub async fn spawn(
    _config_path: PathBuf,
    events: UnboundedSender<TrayEvent>,
    stats: Arc<SessionStats>,
) -> Result<TrayController> {
    let tray = AuraTray { events, stats };
    let handle = tray
        .spawn()
        .await
        .context("failed to register the Plasma StatusNotifierItem tray")?;
    Ok(TrayController { handle })
}

pub fn open_settings(path: &Path) -> Result<()> {
    let absolute = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    for opener in ["gio", "xdg-open"] {
        let mut command = Command::new(opener);
        if opener == "gio" {
            command.arg("open");
        }
        match command
            .arg(&absolute)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(_) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to launch {opener} for {}", absolute.display())
                })
            }
        }
    }
    bail!("no default-application opener was found (tried gio and xdg-open)")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksni::Tray;

    #[test]
    fn tray_menu_hides_image_action_in_shader_mode() {
        let stats = Arc::new(SessionStats::new(
            "5m".into(),
            "1h".into(),
            "Unsupported".into(),
            "silk".into(),
        ));
        stats.set_shader_active(true);
        let (events, _) = tokio::sync::mpsc::unbounded_channel();
        let tray = AuraTray { events, stats };
        let menu = tray.menu();
        let MenuItem::Standard(next) = &menu[2] else {
            panic!("expected Next Background menu item");
        };
        assert!(!next.enabled);
    }
}
