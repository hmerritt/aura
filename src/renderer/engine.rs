use super::compiler::{compile_shader, CompileOptions};
use super::desktop_windows::{
    attach_window_to_desktop, place_window_over_virtual_desktop, show_desktop_window,
    virtual_cursor_position,
};
use super::watcher::start_shader_watcher;
use super::wgpu_runtime::WgpuRuntime;
use super::{RendererCommand, RendererEvent};
use crate::config::ShaderConfig;
use crate::errors::Result;
use anyhow::{anyhow, bail, Context};
use notify::RecommendedWatcher;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use windows_sys::Win32::Foundation::HWND;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::Window;

pub struct ShaderRenderer {
    proxy: EventLoopProxy<UserEvent>,
    event_rx: Option<UnboundedReceiver<RendererEvent>>,
    join_handle: Option<JoinHandle<()>>,
}

impl ShaderRenderer {
    pub fn start(config: ShaderConfig) -> Result<Self> {
        let compile_options = CompileOptions {
            shader_crate: config.crate_path.clone(),
            output_spv: shader_output_path()?,
        };
        compile_shader(&compile_options).context("initial shader compile failed")?;

        let (event_tx, event_rx) = mpsc::unbounded_channel::<RendererEvent>();
        let (init_tx, init_rx) = std::sync::mpsc::channel::<Result<EventLoopProxy<UserEvent>>>();
        let thread_config = config.clone();
        let compile_options_for_thread = compile_options.clone();
        let join_handle = thread::Builder::new()
            .name("bgm-shader-renderer".to_string())
            .spawn(move || {
                run_renderer_thread(thread_config, compile_options_for_thread, event_tx, init_tx);
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
            RendererCommand::ReloadShader => UserEvent::ReloadRequested,
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
    ReloadRequested,
    CompileFinished(std::result::Result<(), String>),
    DisableOutput,
    Stop,
}

fn run_renderer_thread(
    config: ShaderConfig,
    compile_options: CompileOptions,
    event_tx: UnboundedSender<RendererEvent>,
    init_tx: std::sync::mpsc::Sender<Result<EventLoopProxy<UserEvent>>>,
) {
    let event_loop = match EventLoop::<UserEvent>::with_user_event().build() {
        Ok(loop_handle) => loop_handle,
        Err(error) => {
            let _ = init_tx.send(Err(anyhow!("failed to create event loop: {error}")));
            return;
        }
    };
    let proxy = event_loop.create_proxy();
    let _ = init_tx.send(Ok(proxy.clone()));

    let mut app = RendererApp::new(config, compile_options, event_tx, proxy);
    if let Err(error) = event_loop.run_app(&mut app) {
        app.emit_fatal(format!("renderer loop failed: {error}"));
    }
    let _ = app.event_tx.send(RendererEvent::Stopped);
}

struct RendererApp {
    config: ShaderConfig,
    compile_options: CompileOptions,
    proxy: EventLoopProxy<UserEvent>,
    event_tx: UnboundedSender<RendererEvent>,
    window: Option<Arc<Window>>,
    runtime: Option<WgpuRuntime>,
    paused: bool,
    enabled: bool,
    compiling: bool,
    pending_reload: bool,
    next_frame_at: Instant,
    frame_interval: Duration,
    reload_deadline: Option<Instant>,
    watcher: Option<RecommendedWatcher>,
    watcher_rx: Option<Receiver<()>>,
}

impl RendererApp {
    fn new(
        config: ShaderConfig,
        compile_options: CompileOptions,
        event_tx: UnboundedSender<RendererEvent>,
        proxy: EventLoopProxy<UserEvent>,
    ) -> Self {
        Self {
            frame_interval: Duration::from_secs_f64(1.0 / f64::from(config.target_fps)),
            config,
            compile_options,
            proxy,
            event_tx,
            window: None,
            runtime: None,
            paused: false,
            enabled: true,
            compiling: false,
            pending_reload: false,
            next_frame_at: Instant::now(),
            reload_deadline: None,
            watcher: None,
            watcher_rx: None,
        }
    }

    fn emit_fatal(&self, message: String) {
        let _ = self.event_tx.send(RendererEvent::Fatal { message });
    }

    fn request_compile(&mut self) {
        if self.compiling {
            self.pending_reload = true;
            return;
        }
        self.compiling = true;
        let options = self.compile_options.clone();
        let proxy = self.proxy.clone();
        thread::spawn(move || {
            let result = compile_shader(&options).map_err(|error| error.to_string());
            let _ = proxy.send_event(UserEvent::CompileFinished(result.map(|_| ())));
        });
    }

    fn handle_compile_finished(&mut self, result: std::result::Result<(), String>) {
        self.compiling = false;
        match result {
            Ok(()) => {
                if let Some(runtime) = self.runtime.as_mut() {
                    if let Err(error) = runtime.reload_shader(&self.compile_options.output_spv) {
                        self.enabled = false;
                        self.paused = true;
                        if let Some(window) = &self.window {
                            if let Ok(hwnd) = window_hwnd(window.as_ref()) {
                                show_desktop_window(hwnd, false);
                            }
                        }
                        self.emit_fatal(format!("shader reload failed: {error}"));
                        return;
                    }
                }
                let _ = self.event_tx.send(RendererEvent::Reloaded);
            }
            Err(error) => {
                self.enabled = false;
                self.paused = true;
                if let Some(window) = &self.window {
                    if let Ok(hwnd) = window_hwnd(window.as_ref()) {
                        show_desktop_window(hwnd, false);
                    }
                }
                self.emit_fatal(error);
            }
        }

        if self.pending_reload {
            self.pending_reload = false;
            self.request_compile();
        }
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

        let runtime = match WgpuRuntime::new(
            window.clone(),
            &self.compile_options.output_spv,
            self.config.mouse_enabled,
        ) {
            Ok(runtime) => runtime,
            Err(error) => {
                self.emit_fatal(format!("failed to initialize GPU runtime: {error}"));
                event_loop.exit();
                return;
            }
        };

        if self.config.hot_reload {
            match start_shader_watcher(&self.config.crate_path) {
                Ok((watcher, rx)) => {
                    self.watcher = Some(watcher);
                    self.watcher_rx = Some(rx);
                }
                Err(error) => {
                    self.emit_fatal(format!("failed to start shader watcher: {error}"));
                    event_loop.exit();
                    return;
                }
            }
        }

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
            UserEvent::ReloadRequested => self.request_compile(),
            UserEvent::CompileFinished(result) => self.handle_compile_finished(result),
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
        if let Some(rx) = self.watcher_rx.as_ref() {
            let mut touched = false;
            while rx.try_recv().is_ok() {
                touched = true;
            }
            if touched {
                self.reload_deadline = Some(Instant::now() + self.config.reload_debounce);
            }
        }

        if let Some(deadline) = self.reload_deadline {
            if Instant::now() >= deadline {
                self.reload_deadline = None;
                self.request_compile();
            }
        }

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

fn shader_output_path() -> Result<PathBuf> {
    let base = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("bgm")
        .join("shaders");
    std::fs::create_dir_all(&base)
        .with_context(|| format!("failed to create shader cache dir {}", base.display()))?;
    Ok(base.join("live.spv"))
}
