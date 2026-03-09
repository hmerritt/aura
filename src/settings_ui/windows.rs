use super::SettingsUiEvent;
use crate::errors::Result;
use crate::tray::TrayAnchor;
use anyhow::{anyhow, bail, Context};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::borrow::Cow;
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
use wry::http::{header::CONTENT_TYPE, Request, Response, StatusCode};
use wry::{WebView, WebViewBuilder, WebViewBuilderExtWindows};

const SETTINGS_WINDOW_TITLE: &str = "Aura Settings";
const SETTINGS_UI_DEV_URL_ENV: &str = "AURA_SETTINGS_UI_DEV_URL";
const SETTINGS_UI_PROTOCOL: &str = "aura-settings";
const SETTINGS_UI_INDEX_URL: &str = "https://aura-settings.localhost/index.html";
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
type EmbeddedAsset = (&'static str, &'static str, &'static [u8]);
const SETTINGS_UI_ASSETS: &[EmbeddedAsset] =
    include!(concat!(env!("OUT_DIR"), "/settings_ui_assets.rs"));

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
            .name("aura-ui".to_string())
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
            })
            .with_custom_protocol(SETTINGS_UI_PROTOCOL.into(), |_webview_id, request| {
                serve_settings_ui_request(request)
            })
            .with_https_scheme(true);
        builder = if let Some(url) = self.dev_url.as_deref() {
            builder.with_url(url)
        } else {
            builder.with_url(SETTINGS_UI_INDEX_URL)
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

fn serve_settings_ui_request(request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let path = normalize_settings_ui_asset_path(request.uri().path());
    if let Some((mime_type, bytes)) = lookup_settings_ui_asset(&path).or_else(|| {
        should_fallback_to_index(&path)
            .then(|| lookup_settings_ui_asset("index.html"))
            .flatten()
    }) {
        return Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, mime_type)
            .body(Cow::Borrowed(bytes))
            .unwrap();
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Cow::Owned(
            format!("missing settings UI asset: {path}").into_bytes(),
        ))
        .unwrap()
}

fn normalize_settings_ui_asset_path(path: &str) -> String {
    let trimmed = path.trim().trim_start_matches('/');
    if trimmed.is_empty() {
        "index.html".to_string()
    } else {
        trimmed.to_string()
    }
}

fn lookup_settings_ui_asset(path: &str) -> Option<(&'static str, &'static [u8])> {
    SETTINGS_UI_ASSETS
        .iter()
        .find_map(|(asset_path, mime_type, bytes)| {
            (*asset_path == path).then_some((*mime_type, *bytes))
        })
}

fn should_fallback_to_index(path: &str) -> bool {
    !path.is_empty() && !path.contains('.') && !path.ends_with('/')
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

    #[test]
    fn root_asset_path_maps_to_index() {
        assert_eq!(normalize_settings_ui_asset_path("/"), "index.html");
        assert_eq!(normalize_settings_ui_asset_path(""), "index.html");
    }

    #[test]
    fn route_like_paths_fallback_to_index() {
        assert!(should_fallback_to_index("settings"));
        assert!(should_fallback_to_index("settings/profile"));
        assert!(!should_fallback_to_index("assets/app.js"));
        assert!(!should_fallback_to_index("assets/style.css"));
    }
}
