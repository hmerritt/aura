use super::SettingsUiEvent;
use crate::errors::Result;
use anyhow::{anyhow, Context};
use std::thread;
use std::thread::JoinHandle;
use tokio::sync::mpsc::UnboundedSender;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::platform::windows::EventLoopBuilderExtWindows;
use winit::window::{Window, WindowId};
use wry::{WebView, WebViewBuilder};

const SETTINGS_WINDOW_TITLE: &str = "Aura Settings";
const SETTINGS_UI_DEV_URL_ENV: &str = "AURA_SETTINGS_UI_DEV_URL";
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
    OpenWindow,
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

    pub fn open_window(&self) -> Result<()> {
        self.proxy
            .send_event(UserEvent::OpenWindow)
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
        }
    }

    fn ensure_window(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        if self.window.is_some() {
            self.focus_existing_window();
            return Ok(());
        }

        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title(SETTINGS_WINDOW_TITLE)
                    .with_inner_size(LogicalSize::new(980.0, 760.0)),
            )
            .context("failed to create settings window")?;

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
        self.focus_existing_window();

        let pending = std::mem::take(&mut self.pending_messages);
        for json in pending {
            self.dispatch_json(json)?;
        }

        Ok(())
    }

    fn focus_existing_window(&self) {
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
        self.webview = None;
        self.window = None;
    }
}

impl ApplicationHandler<UserEvent> for SettingsUiApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::OpenWindow => {
                if let Err(error) = self.ensure_window(event_loop) {
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

        if let WindowEvent::CloseRequested = event {
            self.close_window();
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
