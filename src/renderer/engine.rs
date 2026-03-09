use super::desktop_windows::{
    attach_window_to_desktop, cursor_position_for_scope, desktop_rect_for_scope,
    place_window_over_desktop, show_desktop_window, DesktopRect,
};
use super::precompiled;
use super::wgpu_runtime::WgpuRuntime;
use super::RendererEvent;
use crate::config::ShaderConfig;
use crate::errors::Result;
use anyhow::{anyhow, bail, Context};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use windows_sys::Win32::Foundation::HWND;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::platform::windows::EventLoopBuilderExtWindows;
use winit::window::Window;

pub struct ShaderRenderer {
    proxy: EventLoopProxy<UserEvent>,
    event_rx: Option<UnboundedReceiver<RendererEvent>>,
    join_handle: Option<JoinHandle<()>>,
}

struct PendingShutdown {
    result_rx: Option<oneshot::Receiver<Result<()>>>,
    join_handle: JoinHandle<()>,
}

impl ShaderRenderer {
    pub fn start(config: ShaderConfig) -> Result<Self> {
        let shader_bytes = resolve_shader_bytes(&config)?;

        let (event_tx, event_rx) = mpsc::unbounded_channel::<RendererEvent>();
        let (init_tx, init_rx) = std::sync::mpsc::channel::<Result<EventLoopProxy<UserEvent>>>();
        let thread_config = config.clone();
        let join_handle = thread::Builder::new()
            .name("aura-shader-renderer".to_string())
            .spawn(move || {
                run_renderer_thread(thread_config, shader_bytes, event_tx, init_tx);
            })
            .context("failed to spawn shader renderer thread")?;

        let proxy = init_rx
            .recv()
            .context("shader renderer failed to initialize")??;

        Ok(Self {
            proxy,
            event_rx: Some(event_rx),
            join_handle: Some(join_handle),
        })
    }

    pub fn take_event_receiver(&mut self) -> Option<UnboundedReceiver<RendererEvent>> {
        self.event_rx.take()
    }

    pub async fn apply_config(&self, config: ShaderConfig) -> Result<()> {
        let shader_bytes = resolve_shader_bytes(&config)?;
        let (result_tx, result_rx) = oneshot::channel();
        self.proxy
            .send_event(UserEvent::ApplyConfig {
                config,
                shader_bytes,
                result_tx,
            })
            .map_err(|error| anyhow!("failed to send renderer config update: {error}"))?;
        result_rx
            .await
            .context("shader renderer dropped config update response")??;
        Ok(())
    }

    pub async fn stop_async(&mut self) -> Result<()> {
        let Some(pending) = self.begin_shutdown() else {
            return Ok(());
        };
        let PendingShutdown {
            result_rx,
            join_handle,
        } = pending;

        if let Some(result_rx) = result_rx {
            result_rx
                .await
                .context("shader renderer shutdown response channel closed before completion")??;
        }

        tokio::task::spawn_blocking(move || join_handle.join())
            .await
            .context("failed to join shader renderer shutdown task")?
            .map_err(|panic| anyhow!("shader renderer thread panicked during shutdown: {panic:?}"))?;

        Ok(())
    }

    fn stop_blocking(&mut self) {
        let Some(pending) = self.begin_shutdown() else {
            return;
        };
        let PendingShutdown {
            result_rx,
            join_handle,
        } = pending;

        if let Some(result_rx) = result_rx {
            match result_rx.blocking_recv() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::warn!(error = %error, "shader renderer shutdown reported an error");
                }
                Err(_) => {
                    tracing::warn!(
                        "shader renderer shutdown response channel closed before completion"
                    );
                }
            }
        }

        if let Err(panic) = join_handle.join() {
            tracing::warn!(panic = ?panic, "shader renderer thread panicked during shutdown");
        }
    }

    fn begin_shutdown(&mut self) -> Option<PendingShutdown> {
        let join_handle = self.join_handle.take()?;
        let (result_tx, result_rx) = oneshot::channel();
        let result_rx = match self.proxy.send_event(UserEvent::Shutdown { result_tx }) {
            Ok(()) => Some(result_rx),
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "failed to send shader renderer shutdown event; joining renderer thread directly"
                );
                None
            }
        };

        Some(PendingShutdown {
            result_rx,
            join_handle,
        })
    }
}

impl Drop for ShaderRenderer {
    fn drop(&mut self) {
        self.stop_blocking();
    }
}

enum UserEvent {
    Shutdown {
        result_tx: oneshot::Sender<Result<()>>,
    },
    ApplyConfig {
        config: ShaderConfig,
        shader_bytes: &'static [u8],
        result_tx: oneshot::Sender<Result<()>>,
    },
}

fn run_renderer_thread(
    config: ShaderConfig,
    shader_bytes: &'static [u8],
    event_tx: UnboundedSender<RendererEvent>,
    init_tx: std::sync::mpsc::Sender<Result<EventLoopProxy<UserEvent>>>,
) {
    let mut builder = EventLoop::<UserEvent>::with_user_event();
    // The shader renderer runs on a dedicated thread, so we must opt in to any-thread mode.
    builder.with_any_thread(true);
    let event_loop = match builder.build() {
        Ok(loop_handle) => loop_handle,
        Err(error) => {
            let _ = init_tx.send(Err(anyhow!("failed to create event loop: {error}")));
            return;
        }
    };
    let proxy = event_loop.create_proxy();
    let _ = init_tx.send(Ok(proxy.clone()));

    let mut app = RendererApp::new(config, shader_bytes, event_tx);
    if let Err(error) = event_loop.run_app(&mut app) {
        app.emit_fatal(format!("renderer loop failed: {error}"));
    }
    let _ = app.event_tx.send(RendererEvent::Stopped);
}

struct RendererApp {
    config: ShaderConfig,
    event_tx: UnboundedSender<RendererEvent>,
    window: Option<Arc<Window>>,
    runtime: Option<WgpuRuntime>,
    paused: bool,
    enabled: bool,
    next_frame_at: Instant,
    next_geometry_poll_at: Instant,
    geometry_poll_interval: Duration,
    last_desktop_rect: Option<DesktopRect>,
    frame_interval: Duration,
    shader_bytes: &'static [u8],
}

impl RendererApp {
    fn new(
        config: ShaderConfig,
        shader_bytes: &'static [u8],
        event_tx: UnboundedSender<RendererEvent>,
    ) -> Self {
        let geometry_poll_interval = Duration::from_millis(1000);
        Self {
            frame_interval: frame_interval_for_target_fps(config.target_fps),
            next_geometry_poll_at: Instant::now() + geometry_poll_interval,
            geometry_poll_interval,
            last_desktop_rect: None,
            config,
            event_tx,
            window: None,
            runtime: None,
            paused: false,
            enabled: true,
            next_frame_at: Instant::now(),
            shader_bytes,
        }
    }

    fn emit_fatal(&self, message: String) {
        let _ = self.event_tx.send(RendererEvent::Fatal { message });
    }

    fn shutdown(&mut self) -> Result<()> {
        let started_at = Instant::now();
        tracing::info!("shader renderer shutdown started");

        if let Some(window) = self.window.as_ref() {
            if let Ok(hwnd) = window_hwnd(window.as_ref()) {
                show_desktop_window(hwnd, false);
            }
        }

        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown();
        }
        self.window = None;
        self.last_desktop_rect = None;
        self.enabled = false;
        self.paused = true;

        tracing::info!(
            elapsed_ms = started_at.elapsed().as_millis(),
            "shader renderer shutdown finished"
        );
        Ok(())
    }

    fn apply_config(&mut self, config: ShaderConfig, shader_bytes: &'static [u8]) -> Result<()> {
        let frame_interval = frame_interval_for_target_fps(config.target_fps);
        let Some(window) = self.window.as_ref().cloned() else {
            self.config = config;
            self.shader_bytes = shader_bytes;
            self.frame_interval = frame_interval;
            return Ok(());
        };

        let target_rect = desktop_rect_for_scope(config.desktop_scope);
        if target_rect.width <= 0 || target_rect.height <= 0 {
            bail!(
                "failed to size render window for config update: empty desktop bounds for {:?}",
                config.desktop_scope
            );
        }

        if let Some(runtime) = self.runtime.as_mut() {
            runtime.apply_config(shader_bytes, config.clone(), target_rect)?;
        } else {
            self.runtime = Some(WgpuRuntime::new(
                window.clone(),
                shader_bytes,
                config.clone(),
                target_rect,
            )?);
        }

        self.config = config;
        self.shader_bytes = shader_bytes;
        self.frame_interval = frame_interval;
        self.enabled = true;
        self.paused = false;
        self.next_frame_at = Instant::now();
        self.next_geometry_poll_at = Instant::now() + self.geometry_poll_interval;
        self.last_desktop_rect = None;

        if let Ok(hwnd) = window_hwnd(window.as_ref()) {
            show_desktop_window(hwnd, true);
        }
        self.sync_desktop_geometry(true);

        Ok(())
    }

    fn sync_desktop_geometry(&mut self, force: bool) {
        let Some(window) = self.window.as_ref().cloned() else {
            return;
        };

        let current_rect = desktop_rect_for_scope(self.config.desktop_scope);
        if current_rect.width <= 0 || current_rect.height <= 0 {
            tracing::warn!(
                x = current_rect.x,
                y = current_rect.y,
                width = current_rect.width,
                height = current_rect.height,
                scope = ?self.config.desktop_scope,
                "ignoring invalid desktop bounds while syncing renderer geometry"
            );
            return;
        }

        if !should_apply_geometry_update(self.last_desktop_rect, current_rect, force) {
            return;
        }

        let hwnd = match window_hwnd(window.as_ref()) {
            Ok(hwnd) => hwnd,
            Err(error) => {
                tracing::warn!(error = %error, "failed to fetch render window handle for geometry sync");
                return;
            }
        };

        let applied_rect = match place_window_over_desktop(hwnd, self.config.desktop_scope) {
            Ok(rect) => rect,
            Err(error) => {
                tracing::warn!(error = %error, "failed to sync desktop geometry; will retry");
                return;
            }
        };

        if let Some(runtime) = self.runtime.as_mut() {
            runtime.resize(PhysicalSize::new(
                applied_rect.width as u32,
                applied_rect.height as u32,
            ));
        }

        let previous = self.last_desktop_rect;
        self.last_desktop_rect = Some(applied_rect);
        tracing::info!(
            old_x = previous.map(|rect| rect.x),
            old_y = previous.map(|rect| rect.y),
            old_width = previous.map(|rect| rect.width),
            old_height = previous.map(|rect| rect.height),
            new_x = applied_rect.x,
            new_y = applied_rect.y,
            new_width = applied_rect.width,
            new_height = applied_rect.height,
            scope = ?self.config.desktop_scope,
            "shader renderer desktop geometry synced"
        );
        window.request_redraw();
    }
}

impl ApplicationHandler<UserEvent> for RendererApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("aura-shader")
            .with_decorations(false)
            .with_visible(true)
            .with_resizable(true);

        let window = match event_loop.create_window(window_attrs) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.emit_fatal(format!("failed to create render window: {error}"));
                event_loop.exit();
                return;
            }
        };
        if let Err(error) = window.set_cursor_hittest(false) {
            self.emit_fatal(format!(
                "failed to enable desktop input passthrough: {error}"
            ));
            event_loop.exit();
            return;
        }
        tracing::info!("shader render window mouse passthrough enabled");

        let hwnd = match window_hwnd(window.as_ref()) {
            Ok(hwnd) => hwnd,
            Err(error) => {
                self.emit_fatal(format!("failed to extract window handle: {error}"));
                event_loop.exit();
                return;
            }
        };
        if let Err(error) = attach_window_to_desktop(hwnd) {
            self.emit_fatal(format!(
                "failed to attach render window to desktop: {error}"
            ));
            event_loop.exit();
            return;
        }
        let desktop_rect = match place_window_over_desktop(hwnd, self.config.desktop_scope) {
            Ok(rect) => rect,
            Err(error) => {
                self.emit_fatal(format!("failed to size render window: {error}"));
                event_loop.exit();
                return;
            }
        };
        if desktop_rect.width <= 0 || desktop_rect.height <= 0 {
            self.emit_fatal("failed to size render window: empty desktop bounds".to_string());
            event_loop.exit();
            return;
        }
        show_desktop_window(hwnd, true);

        let runtime = match WgpuRuntime::new(
            window.clone(),
            self.shader_bytes,
            self.config.clone(),
            desktop_rect,
        ) {
            Ok(runtime) => runtime,
            Err(error) => {
                self.emit_fatal(format!("failed to initialize GPU runtime: {error}"));
                event_loop.exit();
                return;
            }
        };

        self.window = Some(window);
        self.runtime = Some(runtime);
        self.next_frame_at = Instant::now();
        self.next_geometry_poll_at = Instant::now() + self.geometry_poll_interval;
        self.sync_desktop_geometry(true);
        let _ = self.event_tx.send(RendererEvent::Ready);
        let _ = self.event_tx.send(RendererEvent::Running);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(_) => {
                self.sync_desktop_geometry(true);
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.sync_desktop_geometry(true);
            }
            WindowEvent::RedrawRequested => {
                if self.paused || !self.enabled {
                    return;
                }
                let mouse = if self.config.mouse_enabled {
                    cursor_position_for_scope(self.config.desktop_scope).unwrap_or((0.0, 0.0))
                } else {
                    (0.0, 0.0)
                };
                if let Some(runtime) = self.runtime.as_mut() {
                    if let Err(error) = runtime.render([mouse.0, mouse.1]) {
                        self.enabled = false;
                        self.paused = true;
                        if let Some(window) = &self.window {
                            if let Ok(hwnd) = window_hwnd(window.as_ref()) {
                                show_desktop_window(hwnd, false);
                            }
                        }
                        self.emit_fatal(format!("renderer runtime failure: {error}"));
                    }
                }
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Shutdown { result_tx } => {
                let result = self.shutdown();
                let _ = result_tx.send(result);
                event_loop.exit();
            }
            UserEvent::ApplyConfig {
                config,
                shader_bytes,
                result_tx,
            } => {
                let result = self.apply_config(config, shader_bytes);
                let _ = result_tx.send(result);
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.paused || !self.enabled {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }

        if let Some(window) = self.window.as_ref().cloned() {
            let now = Instant::now();
            if now >= self.next_geometry_poll_at {
                self.sync_desktop_geometry(false);
                self.next_geometry_poll_at = now + self.geometry_poll_interval;
            }
            if now >= self.next_frame_at {
                window.request_redraw();
                self.next_frame_at = now + self.frame_interval;
            }
            let next_wake = self.next_frame_at.min(self.next_geometry_poll_at);
            event_loop.set_control_flow(ControlFlow::WaitUntil(next_wake));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

fn should_apply_geometry_update(
    previous_rect: Option<DesktopRect>,
    current_rect: DesktopRect,
    force: bool,
) -> bool {
    force || previous_rect != Some(current_rect)
}

fn frame_interval_for_target_fps(target_fps: u16) -> Duration {
    Duration::from_secs_f64(1.0 / f64::from(target_fps))
}

fn resolve_shader_bytes(config: &ShaderConfig) -> Result<&'static [u8]> {
    let shader_name = config.name.clone();
    precompiled::shader_bytes(&shader_name).ok_or_else(|| {
        let available = precompiled::shader_names();
        anyhow!(
            "configured shader \"{}\" is not available; precompiled shaders: {}",
            shader_name,
            available.join(", ")
        )
    })
}

fn window_hwnd(window: &Window) -> Result<HWND> {
    let handle = window
        .window_handle()
        .context("window handle is unavailable")?;
    match handle.as_raw() {
        RawWindowHandle::Win32(raw) => {
            let hwnd = raw.hwnd.get() as HWND;
            if hwnd.is_null() {
                bail!("window handle is null");
            }
            Ok(hwnd)
        }
        _ => bail!("unsupported raw window handle type for Windows renderer"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, width: i32, height: i32) -> DesktopRect {
        DesktopRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn geometry_update_is_skipped_when_rect_is_unchanged() {
        let current = rect(0, 0, 1920, 1080);
        assert!(!should_apply_geometry_update(Some(current), current, false));
    }

    #[test]
    fn geometry_update_runs_when_size_changes() {
        let previous = rect(0, 0, 1920, 1080);
        let current = rect(0, 0, 2560, 1440);
        assert!(should_apply_geometry_update(Some(previous), current, false));
    }

    #[test]
    fn geometry_update_runs_when_origin_changes() {
        let previous = rect(0, 0, 3840, 2160);
        let current = rect(-1920, 0, 3840, 2160);
        assert!(should_apply_geometry_update(Some(previous), current, false));
    }

    #[test]
    fn computes_frame_interval_from_fps() {
        assert_eq!(frame_interval_for_target_fps(50), Duration::from_millis(20));
    }

    #[test]
    fn shutdown_clears_runtime_state() {
        let (event_tx, _event_rx) = mpsc::unbounded_channel();
        let mut app = RendererApp::new(
            ShaderConfig {
                name: "gradient_glossy".to_string(),
                target_fps: 60,
                resolution: 100,
                mouse_enabled: false,
                desktop_scope: crate::config::ShaderDesktopScope::Virtual,
                color_space: crate::config::ShaderColorSpace::Unorm,
            },
            &[],
            event_tx,
        );
        app.enabled = true;
        app.paused = false;
        app.last_desktop_rect = Some(rect(0, 0, 1920, 1080));

        app.shutdown().unwrap();

        assert!(!app.enabled);
        assert!(app.paused);
        assert!(app.window.is_none());
        assert!(app.runtime.is_none());
        assert!(app.last_desktop_rect.is_none());
    }
}
