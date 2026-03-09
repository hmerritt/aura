use super::SettingsUiEvent;
use crate::errors::Result;
use crate::tray::TrayAnchor;
use anyhow::{anyhow, bail, Context};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::ffi::c_void;
use std::mem::size_of;
use std::thread;
use std::thread::JoinHandle;
use tokio::sync::mpsc::UnboundedSender;
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize, Position};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::platform::windows::EventLoopBuilderExtWindows;
use winit::window::{Window, WindowId};
use wry::{WebView, WebViewBuilder};

const SETTINGS_WINDOW_TITLE: &str = "Aura Settings";
const SETTINGS_UI_DEV_URL_ENV: &str = "AURA_SETTINGS_UI_DEV_URL";
const POPUP_WIDTH: f64 = 650.0;
const POPUP_HEIGHT: f64 = 750.0;
const POPUP_GAP_PX: i32 = 8;
const HOST_INIT_SCRIPT: &str = r#"
window.__AURA_SETTINGS_HOST = {
  listeners: [],
  pending: [],
  onMessage(callback) {
    this.listeners.push(callback);
    while (this.pending.length > 0) {
      callback(this.pending.shift());
    }
  },
  handleRustMessage(message) {
    if (this.listeners.length === 0) {
      this.pending.push(message);
      return;
    }
    for (const callback of this.listeners) {
      callback(message);
    }
  },
  post(message) {
    const payload = typeof message === "string" ? message : JSON.stringify(message);
    window.ipc.postMessage(payload);
  }
};
"#;
const PLACEHOLDER_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Aura Settings</title>
  <style>
    :root {
      color-scheme: light;
      --ink: #182226;
      --paper: #f6f1e7;
      --panel: #fffaf2;
      --line: #d6c9b2;
      --accent: #2c7a64;
      --muted: #6d6b67;
    }
    body {
      margin: 0;
      font-family: "Segoe UI", "Trebuchet MS", sans-serif;
      color: var(--ink);
      background:
        radial-gradient(circle at top left, rgba(44, 122, 100, 0.18), transparent 28%),
        linear-gradient(180deg, #fbf7ef 0%, var(--paper) 100%);
      min-height: 100vh;
    }
    main {
      max-width: 880px;
      margin: 0 auto;
      padding: 40px 24px 56px;
    }
    h1 {
      margin: 0 0 8px;
      font-size: 32px;
      font-weight: 700;
      letter-spacing: -0.03em;
    }
    p {
      margin: 0 0 16px;
      color: var(--muted);
      line-height: 1.5;
    }
    .card {
      background: rgba(255, 250, 242, 0.92);
      border: 1px solid var(--line);
      border-radius: 18px;
      padding: 20px;
      box-shadow: 0 10px 30px rgba(24, 34, 38, 0.07);
    }
    button {
      border: 1px solid transparent;
      border-radius: 999px;
      background: var(--accent);
      color: white;
      padding: 10px 16px;
      font: inherit;
      cursor: pointer;
    }
    pre {
      margin: 16px 0 0;
      padding: 16px;
      border-radius: 12px;
      background: rgba(24, 34, 38, 0.05);
      border: 1px solid rgba(24, 34, 38, 0.08);
      overflow: auto;
      white-space: pre-wrap;
      word-break: break-word;
      font-family: "Cascadia Mono", "Consolas", monospace;
      font-size: 13px;
    }
  </style>
</head>
<body>
  <main>
    <h1>Settings shell ready</h1>
    <p>This placeholder confirms the WebView host and IPC bridge are working. Replace it with a real frontend later.</p>
    <div class="card">
      <button id="reload" type="button">Reload config snapshot</button>
      <pre id="log">Waiting for host...</pre>
    </div>
  </main>
  <script>
    const log = document.getElementById("log");
    const render = (value) => {
      log.textContent = typeof value === "string" ? value : JSON.stringify(value, null, 2);
    };

    window.__AURA_SETTINGS_HOST.onMessage((raw) => {
      try {
        render(JSON.parse(raw));
      } catch (error) {
        render({ raw, error: String(error) });
      }
    });

    const send = (command, payload = {}) => {
      window.__AURA_SETTINGS_HOST.post({
        id: `${command}-${Date.now()}`,
        command,
        payload
      });
    };

    document.getElementById("reload").addEventListener("click", () => {
      send("load_settings");
    });

    send("bootstrap");
    send("load_settings");
  </script>
</body>
</html>
"#;

pub struct SettingsUiController {
    proxy: EventLoopProxy<UserEvent>,
    join_handle: Option<JoinHandle<()>>,
}

enum UserEvent {
    OpenWindow(TrayAnchor),
    CloseWindow,
    DispatchJson(String),
    Shutdown,
}

struct SettingsUiApp {
    event_tx: UnboundedSender<SettingsUiEvent>,
    window: Option<Window>,
    webview: Option<WebView>,
    pending_messages: Vec<String>,
    dev_url: Option<String>,
    close_on_focus_loss_armed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PopupPlacement {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RectBounds {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskbarEdge {
    Top,
    Bottom,
    Left,
    Right,
}

impl SettingsUiController {
    pub fn spawn(event_tx: UnboundedSender<SettingsUiEvent>) -> Result<Self> {
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<EventLoopProxy<UserEvent>>>();
        let join_handle = thread::Builder::new()
            .name("aura-settings-ui".to_string())
            .spawn(move || run_settings_ui_thread(event_tx, ready_tx))
            .context("failed to spawn settings UI thread")?;
        let proxy = ready_rx
            .recv()
            .context("settings UI thread terminated during startup")??;
        Ok(Self {
            proxy,
            join_handle: Some(join_handle),
        })
    }

    pub fn open_window(&self, anchor: TrayAnchor) -> Result<()> {
        self.proxy
            .send_event(UserEvent::OpenWindow(anchor))
            .map_err(|error| anyhow!("failed to request settings window open: {error}"))?;
        Ok(())
    }

    pub fn close_window(&self) -> Result<()> {
        self.proxy
            .send_event(UserEvent::CloseWindow)
            .map_err(|error| anyhow!("failed to request settings window close: {error}"))?;
        Ok(())
    }

    pub fn dispatch_json(&self, json: String) -> Result<()> {
        self.proxy
            .send_event(UserEvent::DispatchJson(json))
            .map_err(|error| anyhow!("failed to send settings UI response: {error}"))?;
        Ok(())
    }
}

impl Drop for SettingsUiController {
    fn drop(&mut self) {
        let _ = self.proxy.send_event(UserEvent::Shutdown);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

impl SettingsUiApp {
    fn new(event_tx: UnboundedSender<SettingsUiEvent>) -> Self {
        Self {
            event_tx,
            window: None,
            webview: None,
            pending_messages: Vec::new(),
            dev_url: std::env::var(SETTINGS_UI_DEV_URL_ENV)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            close_on_focus_loss_armed: false,
        }
    }

    fn ensure_window(&mut self, event_loop: &ActiveEventLoop, anchor: TrayAnchor) -> Result<()> {
        let placement = popup_placement_for_anchor(anchor, desired_popup_size(event_loop, anchor))?;

        if let Some(window) = self.window.as_ref() {
            place_existing_window(window, placement);
            self.show_window();
            return Ok(());
        }

        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title(SETTINGS_WINDOW_TITLE)
                    .with_visible(false)
                    .with_decorations(false)
                    .with_resizable(false)
                    .with_position(Position::Physical(placement.position))
                    .with_inner_size(placement.size),
            )
            .context("failed to create settings window")?;
        apply_rounded_corners(&window);

        let ipc_tx = self.event_tx.clone();
        let mut builder = WebViewBuilder::new()
            .with_initialization_script(HOST_INIT_SCRIPT)
            .with_ipc_handler(move |request| {
                let _ = ipc_tx.send(SettingsUiEvent::IpcMessage(request.body().to_string()));
            });
        builder = if let Some(url) = self.dev_url.as_deref() {
            builder.with_url(url)
        } else {
            builder.with_html(PLACEHOLDER_HTML)
        };
        let webview = builder
            .build(&window)
            .context("failed to build settings webview")?;

        self.window = Some(window);
        self.webview = Some(webview);
        self.show_window();

        let pending = std::mem::take(&mut self.pending_messages);
        for json in pending {
            self.dispatch_json(json)?;
        }

        Ok(())
    }

    fn show_window(&mut self) {
        self.close_on_focus_loss_armed = false;
        if let Some(window) = self.window.as_ref() {
            window.set_visible(true);
            window.focus_window();
        }
    }

    fn dispatch_json(&mut self, json: String) -> Result<()> {
        let Some(webview) = self.webview.as_ref() else {
            self.pending_messages.push(json);
            return Ok(());
        };
        dispatch_json_to_webview(webview, &json)
    }

    fn close_window(&mut self) {
        self.close_on_focus_loss_armed = false;
        self.webview = None;
        self.window = None;
    }
}

impl ApplicationHandler<UserEvent> for SettingsUiApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::OpenWindow(anchor) => {
                if let Err(error) = self.ensure_window(event_loop, anchor) {
                    let _ = self.event_tx.send(SettingsUiEvent::OpenFailed {
                        message: error.to_string(),
                    });
                    self.close_window();
                }
            }
            UserEvent::CloseWindow => {
                self.close_window();
            }
            UserEvent::DispatchJson(json) => {
                if let Err(error) = self.dispatch_json(json) {
                    let _ = self.event_tx.send(SettingsUiEvent::OpenFailed {
                        message: error.to_string(),
                    });
                    self.close_window();
                }
            }
            UserEvent::Shutdown => {
                self.close_window();
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
        if self.window.as_ref().map(Window::id) != Some(window_id) {
            return;
        }

        match event {
            WindowEvent::CloseRequested => self.close_window(),
            WindowEvent::Focused(true) => {
                self.close_on_focus_loss_armed = true;
            }
            WindowEvent::Focused(false) if self.close_on_focus_loss_armed => {
                self.close_window();
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed
                    && matches!(event.logical_key, Key::Named(NamedKey::Escape)) =>
            {
                self.close_window();
            }
            _ => {}
        }
    }
}

fn run_settings_ui_thread(
    event_tx: UnboundedSender<SettingsUiEvent>,
    ready_tx: std::sync::mpsc::Sender<Result<EventLoopProxy<UserEvent>>>,
) {
    let mut builder = EventLoop::<UserEvent>::with_user_event();
    builder.with_any_thread(true);
    let event_loop = match builder.build() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            let _ = ready_tx.send(Err(anyhow!(
                "failed to create settings event loop: {error}"
            )));
            return;
        }
    };
    let proxy = event_loop.create_proxy();
    let _ = ready_tx.send(Ok(proxy));

    let mut app = SettingsUiApp::new(event_tx.clone());
    if let Err(error) = event_loop.run_app(&mut app) {
        let _ = event_tx.send(SettingsUiEvent::OpenFailed {
            message: format!("settings UI event loop failed: {error}"),
        });
    }
}

fn dispatch_json_to_webview(webview: &WebView, json: &str) -> Result<()> {
    let payload =
        serde_json::to_string(json).context("failed to serialize settings UI response payload")?;
    webview
        .evaluate_script(&format!(
            "window.__AURA_SETTINGS_HOST.handleRustMessage({payload});"
        ))
        .context("failed to evaluate settings UI response script")?;
    Ok(())
}

fn apply_rounded_corners(window: &Window) {
    let hwnd = match window_hwnd(window) {
        Ok(hwnd) => hwnd,
        Err(error) => {
            tracing::warn!(error = %error, "failed to resolve settings window handle for rounded corners");
            return;
        }
    };

    let preference = rounded_corner_preference();
    let status = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &preference as *const _ as *const c_void,
            size_of::<i32>() as u32,
        )
    };
    if status < 0 {
        tracing::debug!(
            status,
            "settings popup rounded corner preference was not applied"
        );
    }
}

fn rounded_corner_preference() -> i32 {
    DWMWCP_ROUND
}

fn window_hwnd(window: &Window) -> Result<HWND> {
    let handle = window
        .window_handle()
        .context("settings window handle is unavailable")?;
    match handle.as_raw() {
        RawWindowHandle::Win32(raw) => {
            let hwnd = raw.hwnd.get() as HWND;
            if hwnd.is_null() {
                bail!("settings window handle is null");
            }
            Ok(hwnd)
        }
        _ => bail!("unsupported raw window handle type for Windows settings UI"),
    }
}

fn desired_popup_size(event_loop: &ActiveEventLoop, anchor: TrayAnchor) -> PhysicalSize<u32> {
    let logical_size = LogicalSize::new(POPUP_WIDTH, POPUP_HEIGHT);
    let scale_factor = event_loop
        .available_monitors()
        .find(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            let rect = RectBounds {
                left: position.x,
                top: position.y,
                right: position.x.saturating_add(size.width as i32),
                bottom: position.y.saturating_add(size.height as i32),
            };
            rect.contains(anchor.x, anchor.y)
        })
        .map(|monitor| monitor.scale_factor())
        .unwrap_or(1.0);
    logical_size.to_physical(scale_factor)
}

fn popup_placement_for_anchor(
    anchor: TrayAnchor,
    desired_size: PhysicalSize<u32>,
) -> Result<PopupPlacement> {
    let (work_area, monitor_area) = monitor_bounds_for_anchor(anchor)?;
    Ok(compute_popup_placement(
        anchor,
        work_area,
        monitor_area,
        desired_size,
    ))
}

fn place_existing_window(window: &Window, placement: PopupPlacement) {
    let _ = window.request_inner_size(placement.size);
    window.set_outer_position(Position::Physical(placement.position));
}

fn monitor_bounds_for_anchor(anchor: TrayAnchor) -> Result<(RectBounds, RectBounds)> {
    let monitor = unsafe {
        MonitorFromPoint(
            POINT {
                x: anchor.x,
                y: anchor.y,
            },
            MONITOR_DEFAULTTONEAREST,
        )
    };
    if monitor.is_null() {
        bail!("failed to resolve monitor for tray anchor");
    }

    let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
    info.cbSize = size_of::<MONITORINFO>() as u32;
    let ok = unsafe { GetMonitorInfoW(monitor, &mut info as *mut MONITORINFO) };
    if ok == 0 {
        bail!("GetMonitorInfoW failed for tray anchor");
    }

    Ok((
        RectBounds::from_rect(info.rcWork),
        RectBounds::from_rect(info.rcMonitor),
    ))
}

fn compute_popup_placement(
    anchor: TrayAnchor,
    work_area: RectBounds,
    monitor_area: RectBounds,
    desired_size: PhysicalSize<u32>,
) -> PopupPlacement {
    let edge = infer_taskbar_edge(work_area, monitor_area);
    let width = desired_size.width.min(work_area.width().max(1) as u32);
    let height = desired_size.height.min(work_area.height().max(1) as u32);
    let width_i32 = width as i32;
    let height_i32 = height as i32;

    let (raw_x, raw_y) = match edge {
        TaskbarEdge::Bottom => (
            anchor.x.saturating_sub(width_i32),
            anchor.y.saturating_sub(height_i32 + POPUP_GAP_PX),
        ),
        TaskbarEdge::Top => (anchor.x.saturating_sub(width_i32), anchor.y + POPUP_GAP_PX),
        TaskbarEdge::Left => (anchor.x + POPUP_GAP_PX, anchor.y.saturating_sub(height_i32)),
        TaskbarEdge::Right => (
            anchor.x.saturating_sub(width_i32 + POPUP_GAP_PX),
            anchor.y.saturating_sub(height_i32),
        ),
    };

    let max_x = work_area.right.saturating_sub(width_i32);
    let max_y = work_area.bottom.saturating_sub(height_i32);
    let x = clamp_i32(raw_x, work_area.left, max_x);
    let y = clamp_i32(raw_y, work_area.top, max_y);

    PopupPlacement {
        position: PhysicalPosition::new(x, y),
        size: PhysicalSize::new(width, height),
    }
}

fn infer_taskbar_edge(work_area: RectBounds, monitor_area: RectBounds) -> TaskbarEdge {
    if work_area.top > monitor_area.top {
        TaskbarEdge::Top
    } else if work_area.left > monitor_area.left {
        TaskbarEdge::Left
    } else if work_area.right < monitor_area.right {
        TaskbarEdge::Right
    } else {
        TaskbarEdge::Bottom
    }
}

fn clamp_i32(value: i32, min: i32, max: i32) -> i32 {
    if max < min {
        return min;
    }
    value.clamp(min, max)
}

impl RectBounds {
    fn from_rect(rect: RECT) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        }
    }

    fn width(self) -> i32 {
        self.right.saturating_sub(self.left)
    }

    fn height(self) -> i32 {
        self.bottom.saturating_sub(self.top)
    }

    fn contains(self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> RectBounds {
        RectBounds {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn bottom_taskbar_anchor_opens_upward_and_clamps_right() {
        let placement = compute_popup_placement(
            TrayAnchor::new(1915, 1075),
            rect(0, 0, 1920, 1040),
            rect(0, 0, 1920, 1080),
            PhysicalSize::new(980, 760),
        );

        assert_eq!(
            infer_taskbar_edge(rect(0, 0, 1920, 1040), rect(0, 0, 1920, 1080)),
            TaskbarEdge::Bottom
        );
        assert_eq!(placement.position.x, 935);
        assert_eq!(placement.position.y, 280);
    }

    #[test]
    fn top_taskbar_anchor_opens_downward() {
        let placement = compute_popup_placement(
            TrayAnchor::new(1915, 5),
            rect(0, 40, 1920, 1080),
            rect(0, 0, 1920, 1080),
            PhysicalSize::new(980, 760),
        );

        assert_eq!(placement.position.y, 40);
    }

    #[test]
    fn left_taskbar_anchor_opens_rightward() {
        let placement = compute_popup_placement(
            TrayAnchor::new(5, 1075),
            rect(48, 0, 1920, 1080),
            rect(0, 0, 1920, 1080),
            PhysicalSize::new(980, 760),
        );

        assert_eq!(placement.position.x, 48);
        assert_eq!(placement.position.y, 315);
    }

    #[test]
    fn right_taskbar_anchor_opens_leftward() {
        let placement = compute_popup_placement(
            TrayAnchor::new(1915, 1075),
            rect(0, 0, 1872, 1080),
            rect(0, 0, 1920, 1080),
            PhysicalSize::new(980, 760),
        );

        assert_eq!(placement.position.x, 892);
        assert_eq!(placement.position.y, 315);
    }

    #[test]
    fn popup_size_clamps_to_work_area() {
        let placement = compute_popup_placement(
            TrayAnchor::new(100, 100),
            rect(0, 0, 400, 300),
            rect(0, 0, 400, 340),
            PhysicalSize::new(980, 760),
        );

        assert_eq!(placement.size, PhysicalSize::new(400, 300));
        assert_eq!(placement.position, PhysicalPosition::new(0, 0));
    }

    #[test]
    fn rounded_corner_preference_uses_native_rounding() {
        assert_eq!(rounded_corner_preference(), DWMWCP_ROUND);
    }
}
