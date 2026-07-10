use super::precompiled::{self, ShaderAssets};
use super::wgpu_runtime::{SharedWgpuContext, WgpuRuntime};
use super::{DesktopRect, RendererEvent};
use crate::config::{ShaderConfig, ShaderDesktopScope};
use crate::errors::Result;
use anyhow::{anyhow, bail, Context};
use objc2_app_kit::{NSEvent, NSView, NSWindowCollectionBehavior};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::monitor::MonitorHandle;
use winit::window::{Window, WindowId};

pub struct ShaderRenderer {
    event_rx: Option<mpsc::UnboundedReceiver<RendererEvent>>,
    stopped: bool,
}

impl ShaderRenderer {
    pub fn start(config: ShaderConfig) -> Result<Self> {
        let assets = resolve_shader_assets(&config)?;
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        crate::macos_app::start_shader(config, assets, event_tx)?;
        Ok(Self {
            event_rx: Some(event_rx),
            stopped: false,
        })
    }

    pub fn take_event_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<RendererEvent>> {
        self.event_rx.take()
    }

    pub async fn apply_config(&self, config: ShaderConfig) -> Result<()> {
        let assets = resolve_shader_assets(&config)?;
        let (tx, rx) = oneshot::channel();
        crate::macos_app::apply_shader_config(config, assets, tx)?;
        rx.await
            .context("macOS renderer dropped its config response")?
            .map_err(anyhow::Error::msg)
    }

    pub async fn stop_async(&mut self) -> Result<()> {
        if self.stopped {
            return Ok(());
        }
        self.stopped = true;
        let (tx, rx) = oneshot::channel();
        crate::macos_app::stop_shader(tx)?;
        rx.await
            .context("macOS renderer dropped its shutdown response")?
            .map_err(anyhow::Error::msg)
    }
}

impl Drop for ShaderRenderer {
    fn drop(&mut self) {
        if !self.stopped {
            crate::macos_app::stop_shader_without_waiting();
        }
    }
}

fn resolve_shader_assets(config: &ShaderConfig) -> Result<&'static ShaderAssets> {
    precompiled::shader_assets(&config.name).ok_or_else(|| {
        anyhow!(
            "unknown shader {:?}; available shaders: {}",
            config.name,
            precompiled::shader_names().join(", ")
        )
    })
}

pub(crate) struct MacRendererApp {
    config: ShaderConfig,
    assets: &'static ShaderAssets,
    event_tx: mpsc::UnboundedSender<RendererEvent>,
    displays: Vec<DisplaySurface>,
    scene_rect: DesktopRect,
    next_frame_at: Instant,
    next_reconcile_at: Instant,
    monitor_signature: Vec<MonitorSignature>,
}

struct DisplaySurface {
    window: Arc<Window>,
    runtime: WgpuRuntime,
}

#[derive(Debug, Clone, PartialEq)]
struct MonitorSignature {
    name: Option<String>,
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    scale_factor: f64,
}

impl MacRendererApp {
    pub(crate) fn start(
        event_loop: &ActiveEventLoop,
        config: ShaderConfig,
        assets: &'static ShaderAssets,
        event_tx: mpsc::UnboundedSender<RendererEvent>,
    ) -> Result<Self> {
        let monitors = selected_monitors(event_loop, config.desktop_scope);
        if monitors.is_empty() {
            bail!("macOS reported no displays for shader rendering");
        }
        let scene_rect = scene_rect(&monitors);
        let signature = monitor_signature(&monitors);
        let displays = create_display_surfaces(event_loop, &monitors, scene_rect, &config, assets)?;
        let _ = event_tx.send(RendererEvent::Ready);
        let _ = event_tx.send(RendererEvent::Running);
        Ok(Self {
            config,
            assets,
            event_tx,
            displays,
            scene_rect,
            next_frame_at: Instant::now(),
            next_reconcile_at: Instant::now() + Duration::from_secs(2),
            monitor_signature: signature,
        })
    }

    pub(crate) fn apply_config(
        &mut self,
        event_loop: &ActiveEventLoop,
        config: ShaderConfig,
        assets: &'static ShaderAssets,
    ) -> Result<()> {
        self.config = config;
        self.assets = assets;
        self.rebuild(event_loop)
    }

    pub(crate) fn render_if_due(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if now >= self.next_reconcile_at {
            let monitors = selected_monitors(event_loop, self.config.desktop_scope);
            let signature = monitor_signature(&monitors);
            if signature != self.monitor_signature {
                if let Err(error) = self.rebuild(event_loop) {
                    self.fail(format!("failed to reconcile macOS displays: {error:#}"));
                    return;
                }
            }
            self.next_reconcile_at = now + Duration::from_secs(2);
        }
        if now < self.next_frame_at {
            return;
        }

        let mouse = if self.config.mouse_enabled {
            scene_mouse_position(self.scene_rect)
        } else {
            [0.0, 0.0]
        };
        for display in &mut self.displays {
            if let Err(error) = display.runtime.render(mouse) {
                self.fail(format!("Metal renderer failed: {error:#}"));
                return;
            }
        }
        let interval = Duration::from_secs_f64(1.0 / f64::from(self.config.target_fps.max(1)));
        self.next_frame_at = now + interval;
    }

    pub(crate) fn next_wake(&self) -> Instant {
        self.next_frame_at.min(self.next_reconcile_at)
    }

    pub(crate) fn owns_window(&self, id: WindowId) -> bool {
        self.displays
            .iter()
            .any(|display| display.window.id() == id)
    }

    pub(crate) fn mark_reconcile_needed(&mut self) {
        self.next_reconcile_at = Instant::now();
    }

    pub(crate) fn stop(mut self) {
        for display in self.displays.drain(..) {
            display.runtime.shutdown();
        }
        let _ = self.event_tx.send(RendererEvent::Stopped);
    }

    fn rebuild(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let monitors = selected_monitors(event_loop, self.config.desktop_scope);
        if monitors.is_empty() {
            bail!("macOS reported no displays for shader rendering");
        }
        let new_scene = scene_rect(&monitors);
        let new_displays =
            create_display_surfaces(event_loop, &monitors, new_scene, &self.config, self.assets)?;
        for display in self.displays.drain(..) {
            display.runtime.shutdown();
        }
        self.scene_rect = new_scene;
        self.monitor_signature = monitor_signature(&monitors);
        self.displays = new_displays;
        self.next_frame_at = Instant::now();
        Ok(())
    }

    fn fail(&mut self, message: String) {
        for display in &self.displays {
            display.window.set_visible(false);
        }
        self.next_frame_at = Instant::now() + Duration::from_secs(86_400);
        let _ = self.event_tx.send(RendererEvent::Fatal { message });
    }
}

fn selected_monitors(
    event_loop: &ActiveEventLoop,
    scope: ShaderDesktopScope,
) -> Vec<MonitorHandle> {
    match scope {
        ShaderDesktopScope::Primary => event_loop.primary_monitor().into_iter().collect(),
        ShaderDesktopScope::Virtual => event_loop.available_monitors().collect(),
    }
}

fn monitor_signature(monitors: &[MonitorHandle]) -> Vec<MonitorSignature> {
    monitors
        .iter()
        .map(|monitor| MonitorSignature {
            name: monitor.name(),
            position: monitor.position(),
            size: monitor.size(),
            scale_factor: monitor.scale_factor(),
        })
        .collect()
}

fn scene_rect(monitors: &[MonitorHandle]) -> DesktopRect {
    let min_x = monitors
        .iter()
        .map(|monitor| monitor.position().x)
        .min()
        .unwrap_or(0);
    let min_y = monitors
        .iter()
        .map(|monitor| monitor.position().y)
        .min()
        .unwrap_or(0);
    let max_x = monitors
        .iter()
        .map(|monitor| {
            monitor
                .position()
                .x
                .saturating_add(monitor.size().width as i32)
        })
        .max()
        .unwrap_or(1);
    let max_y = monitors
        .iter()
        .map(|monitor| {
            monitor
                .position()
                .y
                .saturating_add(monitor.size().height as i32)
        })
        .max()
        .unwrap_or(1);
    DesktopRect {
        x: min_x,
        y: min_y,
        width: (max_x - min_x).max(1),
        height: (max_y - min_y).max(1),
    }
}

fn create_display_surfaces(
    event_loop: &ActiveEventLoop,
    monitors: &[MonitorHandle],
    scene_rect: DesktopRect,
    config: &ShaderConfig,
    assets: &'static ShaderAssets,
) -> Result<Vec<DisplaySurface>> {
    let mut displays = Vec::with_capacity(monitors.len());
    let mut shared_context: Option<SharedWgpuContext> = None;
    for monitor in monitors {
        let position = monitor.position();
        let size = monitor.size();
        let rect = DesktopRect {
            x: position.x,
            y: position.y,
            width: size.width as i32,
            height: size.height as i32,
        };
        let attributes = Window::default_attributes()
            .with_title("aura-shader")
            .with_decorations(false)
            .with_resizable(false)
            .with_visible(false)
            .with_position(position)
            .with_inner_size(size);
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .context("failed to create a macOS shader window")?,
        );
        window
            .set_cursor_hittest(false)
            .context("failed to make the shader window input-transparent")?;
        configure_desktop_window(&window)?;
        let (runtime, context) = WgpuRuntime::new_shared(
            window.clone(),
            assets,
            config.clone(),
            rect,
            scene_rect,
            shared_context.clone(),
        )
        .with_context(|| {
            format!(
                "failed to initialize Metal surface for {:?}",
                monitor.name()
            )
        })?;
        shared_context = Some(context);
        window.set_visible(true);
        displays.push(DisplaySurface { window, runtime });
    }
    Ok(displays)
}

fn configure_desktop_window(window: &Window) -> Result<()> {
    let handle = window
        .window_handle()
        .context("shader window has no native handle")?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        bail!("shader window did not expose an AppKit handle");
    };
    let view = unsafe { &*(handle.ns_view.as_ptr().cast::<NSView>()) };
    let native = view
        .window()
        .context("AppKit shader view has no NSWindow")?;
    native.setIgnoresMouseEvents(true);
    native.setCanHide(false);
    native.setHasShadow(false);
    native.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    let desktop_level = unsafe { CGWindowLevelForKey(CG_DESKTOP_WINDOW_LEVEL_KEY) };
    native.setLevel((desktop_level + 1) as isize);
    native.orderFrontRegardless();
    Ok(())
}

fn scene_mouse_position(scene: DesktopRect) -> [f32; 2] {
    let point = NSEvent::mouseLocation();
    [
        (point.x as f32 - scene.x as f32).max(0.0),
        (point.y as f32 - scene.y as f32).max(0.0),
    ]
}

const CG_DESKTOP_WINDOW_LEVEL_KEY: i32 = 2;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGWindowLevelForKey(key: i32) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_union_handles_negative_origins() {
        let rects = [
            DesktopRect {
                x: -1920,
                y: 0,
                width: 1920,
                height: 1080,
            },
            DesktopRect {
                x: 0,
                y: -200,
                width: 2560,
                height: 1440,
            },
        ];
        let min_x = rects.iter().map(|rect| rect.x).min().unwrap();
        let min_y = rects.iter().map(|rect| rect.y).min().unwrap();
        let max_x = rects.iter().map(|rect| rect.x + rect.width).max().unwrap();
        let max_y = rects.iter().map(|rect| rect.y + rect.height).max().unwrap();
        assert_eq!(
            (min_x, min_y, max_x - min_x, max_y - min_y),
            (-1920, -200, 4480, 1440)
        );
    }
}
