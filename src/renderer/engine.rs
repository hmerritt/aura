use super::desktop_windows::{
    attach_window_to_desktop, place_window_over_virtual_desktop, show_desktop_window,
    virtual_cursor_position,
};
use super::precompiled;
use super::wgpu_runtime::WgpuRuntime;
use super::{RendererCommand, RendererEvent};
use crate::config::ShaderConfig;
use crate::errors::Result;
use anyhow::{anyhow, bail, Context};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use windows_sys::Win32::Foundation::HWND;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::platform::windows::EventLoopBuilderExtWindows;
use winit::window::Window;

pub struct ShaderRenderer {
    proxy: EventLoopProxy<UserEvent>,
    event_rx: Option<UnboundedReceiver<RendererEvent>>,
    join_handle: Option<JoinHandle<()>>,
}

impl ShaderRenderer {
    pub fn start(config: ShaderConfig) -> Result<Self> {
        let shader_name = config.name.clone();
        let shader_bytes = precompiled::shader_bytes(&shader_name).ok_or_else(|| {
            let available = precompiled::shader_names();
            anyhow!(
                "configured shader \"{}\" is not available; precompiled shaders: {}",
                shader_name,
                available.join(", ")
            )
        })?;

        let (event_tx, event_rx) = mpsc::unbounded_channel::<RendererEvent>();
        let (init_tx, init_rx) = std::sync::mpsc::channel::<Result<EventLoopProxy<UserEvent>>>();
        let thread_config = config.clone();
        let join_handle = thread::Builder::new()
            .name("bgm-shader-renderer".to_string())
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

    pub fn send_command(&self, command: RendererCommand) -> Result<()> {
        let user_event = match command {
            RendererCommand::DisableOutput => UserEvent::DisableOutput,
            RendererCommand::Stop => UserEvent::Stop,
        };
        self.proxy
            .send_event(user_event)
            .map_err(|error| anyhow!("failed to send renderer command: {error}"))?;
        Ok(())
    }

    pub fn stop(&mut self) {
        let _ = self.send_command(RendererCommand::Stop);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ShaderRenderer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug, Clone)]
enum UserEvent {
    DisableOutput,
    Stop,
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
    frame_interval: Duration,
    shader_bytes: &'static [u8],
}

impl RendererApp {
    fn new(
        config: ShaderConfig,
        shader_bytes: &'static [u8],
        event_tx: UnboundedSender<RendererEvent>,
    ) -> Self {
        Self {
            frame_interval: Duration::from_secs_f64(1.0 / f64::from(config.target_fps)),
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
}

impl ApplicationHandler<UserEvent> for RendererApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("bgm-shader")
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
        if let Err(error) = place_window_over_virtual_desktop(hwnd) {
            self.emit_fatal(format!("failed to size render window: {error}"));
            event_loop.exit();
            return;
        }
        show_desktop_window(hwnd, true);

        let runtime =
            match WgpuRuntime::new(window.clone(), self.shader_bytes, self.config.mouse_enabled) {
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
            WindowEvent::Resized(size) => {
                if let Some(runtime) = self.runtime.as_mut() {
                    runtime.resize(size);
                }
            }
            WindowEvent::RedrawRequested => {
                if self.paused || !self.enabled {
                    return;
                }
                let mouse = if self.config.mouse_enabled {
                    virtual_cursor_position().unwrap_or((0.0, 0.0))
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
            UserEvent::DisableOutput => {
                self.enabled = false;
                self.paused = true;
                if let Some(window) = &self.window {
                    if let Ok(hwnd) = window_hwnd(window.as_ref()) {
                        show_desktop_window(hwnd, false);
                    }
                }
                let _ = self.event_tx.send(RendererEvent::Paused);
            }
            UserEvent::Stop => {
                event_loop.exit();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.paused || !self.enabled {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }

        if let Some(window) = self.window.as_ref() {
            let now = Instant::now();
            if now >= self.next_frame_at {
                window.request_redraw();
                self.next_frame_at = now + self.frame_interval;
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame_at));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
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
