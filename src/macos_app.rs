use crate::config::ShaderConfig;
use crate::errors::Result;
use crate::installer::StartupRegistrationStatus;
use crate::macos_support::{detect_install_layout, update_instruction, InstallLayout};
use crate::renderer::macos::MacRendererApp;
use crate::renderer::precompiled::ShaderAssets;
use crate::renderer::RendererEvent;
use crate::tray::{format_running_duration, SessionStats, TrayEvent};
use anyhow::{anyhow, Context};
use objc2::runtime::AnyObject;
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSAlert, NSAlertStyle, NSApplication, NSScreen, NSWorkspace,
    NSWorkspaceDesktopImageAllowClippingKey, NSWorkspaceDesktopImageOptionKey,
    NSWorkspaceDesktopImageScalingKey,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};
use objc2_service_management::{SMAppService, SMAppServiceStatus};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::window::WindowId;

static PROXY: OnceLock<EventLoopProxy<UserEvent>> = OnceLock::new();

type CommandResponse = std::sync::mpsc::Sender<std::result::Result<(), String>>;
type LoginResponse =
    std::sync::mpsc::Sender<std::result::Result<StartupRegistrationStatus, String>>;
type ShaderResponse = tokio::sync::oneshot::Sender<std::result::Result<(), String>>;

pub enum UserEvent {
    ApplyWallpaper {
        path: PathBuf,
        response: CommandResponse,
    },
    CreateTray {
        config_path: PathBuf,
        event_tx: UnboundedSender<TrayEvent>,
        stats: Arc<SessionStats>,
        response: CommandResponse,
    },
    DestroyTray,
    OpenSettings {
        path: PathBuf,
        response: CommandResponse,
    },
    EnsureLoginItem {
        response: LoginResponse,
    },
    ShowUpdateInstructions,
    ShowFatal(String),
    StartShader {
        config: ShaderConfig,
        assets: &'static ShaderAssets,
        event_tx: UnboundedSender<RendererEvent>,
        response: CommandResponse,
    },
    ApplyShaderConfig {
        config: ShaderConfig,
        assets: &'static ShaderAssets,
        response: ShaderResponse,
    },
    StopShader {
        response: Option<ShaderResponse>,
    },
    MenuEvent(MenuEvent),
    CoreFinished(std::result::Result<(), String>),
}

pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let debug_requested = crate::debug_capture::is_debug_requested(&args);
    let _debug_capture = if debug_requested {
        match crate::debug_capture::DebugCapture::init() {
            Ok(capture) => {
                let _ = writeln!(
                    std::io::stderr(),
                    "debug logging enabled: {}",
                    capture.path().display()
                );
                Some(capture)
            }
            Err(error) => {
                let _ = writeln!(
                    std::io::stderr(),
                    "failed to initialize debug logging: {error:#}"
                );
                None
            }
        }
    } else {
        None
    };

    crate::crash_ui::install_panic_hook(debug_requested);
    if let Err(error) = crate::crash_capture::install() {
        let _ = writeln!(
            std::io::stderr(),
            "failed to initialize native crash capture: {error:#}"
        );
    }

    let mut builder = EventLoop::<UserEvent>::with_user_event();
    builder
        .with_activation_policy(ActivationPolicy::Accessory)
        .with_default_menu(false);
    let event_loop = match builder.build() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            let _ = writeln!(
                std::io::stderr(),
                "failed to create macOS event loop: {error}"
            );
            return;
        }
    };
    let proxy = event_loop.create_proxy();
    let _ = PROXY.set(proxy.clone());

    let worker_proxy = proxy.clone();
    let worker = std::thread::Builder::new()
        .name("aura-core".to_string())
        .spawn(move || {
            let outcome = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(anyhow::Error::from)
                .and_then(|runtime| runtime.block_on(crate::run(args, debug_requested)));
            let _ = worker_proxy.send_event(UserEvent::CoreFinished(
                outcome.map_err(|error| format!("{error:#}")),
            ));
        });

    let worker = match worker {
        Ok(worker) => Some(worker),
        Err(error) => {
            show_alert(
                "Aura - Fatal Error",
                &format!("Failed to start Aura: {error}"),
            );
            return;
        }
    };

    let mut app = MacApplication::new(worker);
    if let Err(error) = event_loop.run_app(&mut app) {
        let _ = writeln!(std::io::stderr(), "macOS event loop failed: {error}");
        app.exit_code = 1;
    }
    app.join_worker();
    if app.exit_code != 0 {
        std::process::exit(app.exit_code);
    }
}

fn proxy() -> Result<&'static EventLoopProxy<UserEvent>> {
    PROXY
        .get()
        .context("macOS main-thread bridge is not initialized")
}

fn request(command: impl FnOnce(CommandResponse) -> UserEvent) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    proxy()?
        .send_event(command(tx))
        .map_err(|error| anyhow!("macOS event loop is unavailable: {error}"))?;
    rx.recv()
        .context("macOS main-thread command response was dropped")?
        .map_err(anyhow::Error::msg)
}

pub fn set_wallpaper(path: &Path) -> Result<()> {
    request(|response| UserEvent::ApplyWallpaper {
        path: path.to_path_buf(),
        response,
    })
}

pub fn create_tray(
    config_path: PathBuf,
    event_tx: UnboundedSender<TrayEvent>,
    stats: Arc<SessionStats>,
) -> Result<()> {
    request(|response| UserEvent::CreateTray {
        config_path,
        event_tx,
        stats,
        response,
    })
}

pub fn destroy_tray() {
    if let Some(proxy) = PROXY.get() {
        let _ = proxy.send_event(UserEvent::DestroyTray);
    }
}

pub fn open_settings(path: &Path) -> Result<()> {
    request(|response| UserEvent::OpenSettings {
        path: path.to_path_buf(),
        response,
    })
}

pub fn ensure_login_item() -> Result<StartupRegistrationStatus> {
    if detect_install_layout(&std::env::current_exe()?) != InstallLayout::Cask {
        return Ok(StartupRegistrationStatus::SkippedNotInstalled);
    }
    let (tx, rx) = std::sync::mpsc::channel();
    proxy()?
        .send_event(UserEvent::EnsureLoginItem { response: tx })
        .map_err(|error| anyhow!("macOS event loop is unavailable: {error}"))?;
    rx.recv()
        .context("login item response was dropped")?
        .map_err(anyhow::Error::msg)
}

pub fn show_update_instructions() {
    if let Some(proxy) = PROXY.get() {
        let _ = proxy.send_event(UserEvent::ShowUpdateInstructions);
    }
}

pub fn show_fatal_error(message: &str) {
    if let Some(proxy) = PROXY.get() {
        let _ = proxy.send_event(UserEvent::ShowFatal(message.to_string()));
    } else {
        let _ = writeln!(std::io::stderr(), "Aura fatal error: {message}");
    }
}

pub fn start_shader(
    config: ShaderConfig,
    assets: &'static ShaderAssets,
    event_tx: UnboundedSender<RendererEvent>,
) -> Result<()> {
    request(|response| UserEvent::StartShader {
        config,
        assets,
        event_tx,
        response,
    })
}

pub fn apply_shader_config(
    config: ShaderConfig,
    assets: &'static ShaderAssets,
    response: ShaderResponse,
) -> Result<()> {
    proxy()?
        .send_event(UserEvent::ApplyShaderConfig {
            config,
            assets,
            response,
        })
        .map_err(|error| anyhow!("macOS event loop is unavailable: {error}"))
}

pub fn stop_shader(response: ShaderResponse) -> Result<()> {
    proxy()?
        .send_event(UserEvent::StopShader {
            response: Some(response),
        })
        .map_err(|error| anyhow!("macOS event loop is unavailable: {error}"))
}

pub fn stop_shader_without_waiting() {
    if let Some(proxy) = PROXY.get() {
        let _ = proxy.send_event(UserEvent::StopShader { response: None });
    }
}

pub fn activate_existing_instance() {
    let _ = std::process::Command::new("open")
        .args(["-a", "Aura"])
        .status();
}

struct MacApplication {
    worker: Option<JoinHandle<()>>,
    tray: Option<MacTray>,
    renderer: Option<MacRendererApp>,
    last_wallpaper: Option<PathBuf>,
    next_wallpaper_reapply: Instant,
    exit_code: i32,
}

impl MacApplication {
    fn new(worker: Option<JoinHandle<()>>) -> Self {
        Self {
            worker,
            tray: None,
            renderer: None,
            last_wallpaper: None,
            next_wallpaper_reapply: Instant::now() + Duration::from_secs(5),
            exit_code: 0,
        }
    }

    fn join_worker(&mut self) {
        if let Some(worker) = self.worker.take() {
            if worker.join().is_err() {
                self.exit_code = 1;
            }
        }
    }
}

impl ApplicationHandler<UserEvent> for MacApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_secs(1),
        ));
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::ApplyWallpaper { path, response } => {
                let result = apply_wallpaper(&path);
                if result.is_ok() {
                    self.last_wallpaper = Some(path);
                    self.next_wallpaper_reapply = Instant::now() + Duration::from_secs(5);
                }
                let _ = response.send(result.map_err(|error| format!("{error:#}")));
            }
            UserEvent::CreateTray {
                config_path,
                event_tx,
                stats,
                response,
            } => {
                let result = MacTray::new(config_path, event_tx, stats);
                match result {
                    Ok(tray) => {
                        self.tray = Some(tray);
                        let _ = response.send(Ok(()));
                    }
                    Err(error) => {
                        let _ = response.send(Err(format!("{error:#}")));
                    }
                }
            }
            UserEvent::DestroyTray => self.tray = None,
            UserEvent::OpenSettings { path, response } => {
                let result = open_path(&path);
                let _ = response.send(result.map_err(|error| format!("{error:#}")));
            }
            UserEvent::EnsureLoginItem { response } => {
                let _ = response.send(register_login_item().map_err(|error| format!("{error:#}")));
            }
            UserEvent::ShowUpdateInstructions => show_homebrew_update_instructions(),
            UserEvent::ShowFatal(message) => show_alert("Aura - Fatal Error", &message),
            UserEvent::StartShader {
                config,
                assets,
                event_tx,
                response,
            } => {
                if let Some(renderer) = self.renderer.take() {
                    renderer.stop();
                }
                match MacRendererApp::start(event_loop, config, assets, event_tx) {
                    Ok(renderer) => {
                        self.renderer = Some(renderer);
                        let _ = response.send(Ok(()));
                    }
                    Err(error) => {
                        let _ = response.send(Err(format!("{error:#}")));
                    }
                }
            }
            UserEvent::ApplyShaderConfig {
                config,
                assets,
                response,
            } => {
                let result = self
                    .renderer
                    .as_mut()
                    .context("macOS shader renderer is not running")
                    .and_then(|renderer| renderer.apply_config(event_loop, config, assets));
                let _ = response.send(result.map_err(|error| format!("{error:#}")));
            }
            UserEvent::StopShader { response } => {
                if let Some(renderer) = self.renderer.take() {
                    renderer.stop();
                }
                if let Some(response) = response {
                    let _ = response.send(Ok(()));
                }
            }
            UserEvent::MenuEvent(event) => {
                if let Some(tray) = self.tray.as_mut() {
                    tray.handle_menu_event(&event);
                }
            }
            UserEvent::CoreFinished(result) => {
                if let Some(renderer) = self.renderer.take() {
                    renderer.stop();
                }
                if let Err(error) = result {
                    self.exit_code = 1;
                    let _ = writeln!(std::io::stderr(), "fatal error: {error}");
                    show_alert("Aura - Fatal Error", &error);
                }
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(renderer) = self.renderer.as_mut() {
            if renderer.owns_window(window_id)
                && matches!(
                    event,
                    WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
                )
            {
                renderer.mark_reconcile_needed();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(tray) = self.tray.as_mut() {
            tray.refresh_labels();
        }
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.render_if_due(event_loop);
        }
        if Instant::now() >= self.next_wallpaper_reapply {
            if let Some(path) = self.last_wallpaper.as_ref() {
                if let Err(error) = apply_wallpaper(path) {
                    tracing::error!(error = %error, "failed to reapply wallpaper after a display/Space change");
                }
            }
            self.next_wallpaper_reapply = Instant::now() + Duration::from_secs(5);
        }
        let mut wake = Instant::now() + Duration::from_secs(1);
        if let Some(renderer) = self.renderer.as_ref() {
            wake = wake.min(renderer.next_wake());
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(wake));
    }
}

struct MacTray {
    _icon: TrayIcon,
    event_tx: UnboundedSender<TrayEvent>,
    stats: Arc<SessionStats>,
    next: MenuItem,
    reload: MenuItem,
    settings: MenuItem,
    updates: MenuItem,
    login_items: MenuItem,
    exit: MenuItem,
    running: MenuItem,
    images: MenuItem,
    renderer: MenuItem,
    update_status: MenuItem,
}

impl MacTray {
    fn new(
        _config_path: PathBuf,
        event_tx: UnboundedSender<TrayEvent>,
        stats: Arc<SessionStats>,
    ) -> Result<Self> {
        let menu = Menu::new();
        let running = MenuItem::new("Running: <1m", false, None);
        let images = MenuItem::new("Images shown: 0", false, None);
        let renderer = MenuItem::new("Renderer: image", false, None);
        let update_status = MenuItem::new("Updates: Managed by Homebrew", false, None);
        let next = MenuItem::new("Next Background", true, None);
        let reload = MenuItem::new("Reload Settings", true, None);
        let settings = MenuItem::new("Settings…", true, None);
        let updates = MenuItem::new("Homebrew Update Instructions…", true, None);
        let login_items = MenuItem::new(
            "Login Items Settings…",
            login_item_requires_approval(),
            None,
        );
        let exit = MenuItem::new("Exit Aura", true, None);
        let sep1 = PredefinedMenuItem::separator();
        let sep2 = PredefinedMenuItem::separator();
        let sep3 = PredefinedMenuItem::separator();
        menu.append_items(&[
            &running,
            &images,
            &renderer,
            &update_status,
            &sep1,
            &next,
            &reload,
            &settings,
            &sep2,
            &updates,
            &login_items,
            &sep3,
            &exit,
        ])
        .context("failed to build macOS status menu")?;

        let rgba = image::load_from_memory(include_bytes!("../assets/tray.png"))
            .context("failed to decode menu bar icon")?
            .into_rgba8();
        let (width, height) = rgba.dimensions();
        let icon = Icon::from_rgba(rgba.into_raw(), width, height)
            .context("failed to create menu bar icon")?;

        let proxy = proxy()?.clone();
        MenuEvent::set_event_handler(Some(move |event| {
            let _ = proxy.send_event(UserEvent::MenuEvent(event));
        }));
        let icon = TrayIconBuilder::new()
            .with_tooltip("Aura")
            .with_icon(icon)
            .with_icon_as_template(false)
            .with_menu(Box::new(menu))
            .build()
            .context("failed to create macOS status item")?;

        Ok(Self {
            _icon: icon,
            event_tx,
            stats,
            next,
            reload,
            settings,
            updates,
            login_items,
            exit,
            running,
            images,
            renderer,
            update_status,
        })
    }

    fn handle_menu_event(&mut self, event: &MenuEvent) {
        let tray_event = if event.id == *self.next.id() {
            Some(TrayEvent::NextWallpaper)
        } else if event.id == *self.reload.id() {
            Some(TrayEvent::ReloadSettings)
        } else if event.id == *self.settings.id() {
            Some(TrayEvent::OpenSettings)
        } else if event.id == *self.updates.id() {
            show_homebrew_update_instructions();
            None
        } else if event.id == *self.login_items.id() {
            unsafe { SMAppService::openSystemSettingsLoginItems() };
            None
        } else if event.id == *self.exit.id() {
            Some(TrayEvent::Exit)
        } else {
            None
        };
        if let Some(event) = tray_event {
            let _ = self.event_tx.send(event);
        }
    }

    fn refresh_labels(&mut self) {
        self.running.set_text(format!(
            "Running: {}",
            format_running_duration(self.stats.running_duration())
        ));
        self.images.set_text(format!(
            "Images shown: {} ({} manual)",
            self.stats.images_shown(),
            self.stats.manual_skips()
        ));
        let renderer = if self.stats.is_shader_active() {
            format!("Renderer: shader ({})", self.stats.shader_name())
        } else {
            "Renderer: image".to_string()
        };
        self.renderer.set_text(renderer);
        self.update_status
            .set_text(format!("Updates: {}", self.stats.app_update_status()));
        self.login_items.set_enabled(login_item_requires_approval());
        self.login_items
            .set_text(if login_item_requires_approval() {
                "Login Items Settings… (Approval Required)"
            } else {
                "Login Items Settings…"
            });
    }
}

fn apply_wallpaper(path: &Path) -> Result<()> {
    let mtm =
        MainThreadMarker::new().context("wallpaper update was not called on the main thread")?;
    let url = NSURL::from_file_path(path).ok_or_else(|| {
        anyhow!(
            "cannot convert wallpaper path to a file URL: {}",
            path.display()
        )
    })?;
    let scaling = NSNumber::new_usize(3);
    let clipping = NSNumber::new_bool(true);
    let typed_options = NSDictionary::<NSString, NSNumber>::from_slices(
        unsafe {
            &[
                NSWorkspaceDesktopImageScalingKey,
                NSWorkspaceDesktopImageAllowClippingKey,
            ]
        },
        &[&scaling, &clipping],
    );
    let options: &NSDictionary<NSWorkspaceDesktopImageOptionKey, AnyObject> =
        unsafe { typed_options.cast_unchecked() };
    let workspace = NSWorkspace::sharedWorkspace();
    let screens = NSScreen::screens(mtm);
    let mut errors = Vec::new();
    for screen in screens.iter() {
        let screen_name = screen.localizedName().to_string();
        let result =
            unsafe { workspace.setDesktopImageURL_forScreen_options_error(&url, &screen, options) };
        if let Err(error) = result {
            errors.push(format!("{screen_name}: {}", error.localizedDescription()));
        }
    }
    wallpaper_fanout_result(errors)
}

fn wallpaper_fanout_result(errors: Vec<String>) -> Result<()> {
    if errors.is_empty() {
        return Ok(());
    }
    Err(anyhow!(
        "failed to apply wallpaper to {} screen(s): {}",
        errors.len(),
        errors.join("; ")
    ))
}

fn open_path(path: &Path) -> Result<()> {
    let url = NSURL::from_file_path(path)
        .ok_or_else(|| anyhow!("cannot convert settings path to a file URL"))?;
    if NSWorkspace::sharedWorkspace().openURL(&url) {
        Ok(())
    } else {
        Err(anyhow!("macOS could not open {}", path.display()))
    }
}

fn register_login_item() -> Result<StartupRegistrationStatus> {
    let service = unsafe { SMAppService::mainAppService() };
    let status = unsafe { service.status() };
    if let Some(mapped) = map_login_item_status(status) {
        return mapped;
    }
    match status {
        SMAppServiceStatus::NotRegistered => {
            unsafe { service.registerAndReturnError() }.map_err(|error| {
                anyhow!(
                    "login item registration failed: {}",
                    error.localizedDescription()
                )
            })?;
            if unsafe { service.status() } == SMAppServiceStatus::RequiresApproval {
                Ok(StartupRegistrationStatus::ApprovalRequired)
            } else {
                Ok(StartupRegistrationStatus::RegisteredNow)
            }
        }
        _ => Err(anyhow!(
            "the Aura login item was not found in this app bundle"
        )),
    }
}

fn map_login_item_status(status: SMAppServiceStatus) -> Option<Result<StartupRegistrationStatus>> {
    match status {
        SMAppServiceStatus::Enabled => Some(Ok(StartupRegistrationStatus::AlreadyRegistered)),
        SMAppServiceStatus::RequiresApproval => {
            Some(Ok(StartupRegistrationStatus::ApprovalRequired))
        }
        SMAppServiceStatus::NotFound => Some(Err(anyhow!(
            "the Aura login item was not found in this app bundle"
        ))),
        _ => None,
    }
}

fn login_item_requires_approval() -> bool {
    detect_install_layout(&std::env::current_exe().unwrap_or_default()) == InstallLayout::Cask
        && unsafe { SMAppService::mainAppService().status() }
            == SMAppServiceStatus::RequiresApproval
}

fn show_homebrew_update_instructions() {
    let layout = std::env::current_exe()
        .ok()
        .map(|path| detect_install_layout(&path))
        .unwrap_or(InstallLayout::Unmanaged);
    let instruction = update_instruction(layout);
    if let Ok(mut child) = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
    {
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(instruction.as_bytes());
        }
        let _ = child.wait();
    }
    show_alert(
        "Aura Updates Are Managed by Homebrew",
        &format!("Run this command in Terminal:\n\n{instruction}\n\nThe command has been copied to the clipboard."),
    );
}

fn show_alert(title: &str, message: &str) {
    let Some(mtm) = MainThreadMarker::new() else {
        let _ = writeln!(std::io::stderr(), "{title}: {message}");
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    app.activateIgnoringOtherApps(true);
    let alert = NSAlert::new(mtm);
    alert.setAlertStyle(NSAlertStyle::Critical);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(message));
    alert.runModal();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_item_status_maps_user_approval() {
        assert_eq!(
            map_login_item_status(SMAppServiceStatus::Enabled)
                .unwrap()
                .unwrap(),
            StartupRegistrationStatus::AlreadyRegistered
        );
        assert_eq!(
            map_login_item_status(SMAppServiceStatus::RequiresApproval)
                .unwrap()
                .unwrap(),
            StartupRegistrationStatus::ApprovalRequired
        );
        assert!(map_login_item_status(SMAppServiceStatus::NotRegistered).is_none());
        assert!(map_login_item_status(SMAppServiceStatus::NotFound)
            .unwrap()
            .is_err());
    }

    #[test]
    fn wallpaper_fanout_reports_every_screen_error() {
        assert!(wallpaper_fanout_result(Vec::new()).is_ok());
        let error = wallpaper_fanout_result(vec![
            "Studio Display: denied".to_string(),
            "Projector: unavailable".to_string(),
        ])
        .unwrap_err()
        .to_string();
        assert!(error.contains("2 screen(s)"));
        assert!(error.contains("Studio Display: denied"));
        assert!(error.contains("Projector: unavailable"));
    }
}
