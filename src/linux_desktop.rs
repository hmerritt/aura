use crate::config::{ShaderColorSpace, ShaderConfig, ShaderDesktopScope};
use crate::errors::Result;
use crate::renderer::precompiled;
use crate::renderer::RendererEvent;
use crate::tray::{format_running_duration, SessionStats, TrayEvent};
use anyhow::{anyhow, bail, Context};
use serde::Serialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::UnboundedSender;
use url::Url;
use zbus::object_server::SignalEmitter;

pub const BUS_NAME: &str = "io.github.hmerritt.Aura";
pub const OBJECT_PATH: &str = "/io/github/hmerritt/Aura";
pub const INTERFACE_NAME: &str = "io.github.hmerritt.Aura1";
const GNOME_SHELL_BUS_NAME: &str = "org.gnome.Shell";
const GNOME_COMPANION_BUS_NAME: &str = "io.github.hmerritt.Aura.Gnome";
const PLASMA_SHELL_BUS_NAME: &str = "org.kde.plasmashell";
const PLASMA_PLUGIN_ID: &str = "io.github.hmerritt.Aura";
const PLASMA_LEASE: Duration = Duration::from_secs(15);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

static RUNTIME: OnceLock<Arc<LinuxDesktopRuntime>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DesktopKind {
    Gnome,
    Plasma,
}

impl DesktopKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Gnome => "gnome",
            Self::Plasma => "plasma",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionType {
    X11,
    Wayland,
}

impl SessionType {
    pub fn label(self) -> &'static str {
        match self {
            Self::X11 => "x11",
            Self::Wayland => "wayland",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesktopSession {
    pub desktop: DesktopKind,
    pub session_type: SessionType,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShaderSnapshot {
    name: String,
    target_fps: u16,
    resolution_percentage: u8,
    mouse_enabled: bool,
    scope: &'static str,
    color_space: &'static str,
    phase_start_unix_ms: u64,
    gnome_glsl: String,
    plasma_vertex_uri: String,
    plasma_fragment_uri: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatisticsSnapshot {
    image_timer: String,
    remote_update_timer: String,
    image_count: u64,
    shown: u64,
    skipped: u64,
    running_duration: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopSnapshot {
    desktop: &'static str,
    session_type: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeSnapshot {
    version: u8,
    revision: u64,
    renderer_generation: u64,
    mode: &'static str,
    tray_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shader: Option<ShaderSnapshot>,
    lease_expires_at_unix_ms: u64,
    desktop: DesktopSnapshot,
    statistics: StatisticsSnapshot,
}

#[derive(Debug, Clone)]
struct RuntimeState {
    revision: u64,
    renderer_generation: u64,
    mode: &'static str,
    image_uri: Option<String>,
    shader: Option<ShaderSnapshot>,
    lease_expires_at_unix_ms: u64,
}

struct RendererListener {
    generation: u64,
    sender: UnboundedSender<RendererEvent>,
    ready_reported: bool,
}

pub struct LinuxDesktopRuntime {
    session: DesktopSession,
    tray_enabled: bool,
    commands: UnboundedSender<TrayEvent>,
    statistics: Arc<SessionStats>,
    state: RwLock<RuntimeState>,
    connection: OnceLock<zbus::Connection>,
    renderer_listener: Mutex<Option<RendererListener>>,
}

pub async fn initialize(
    _config_path: PathBuf,
    tray_enabled: bool,
    commands: UnboundedSender<TrayEvent>,
    statistics: Arc<SessionStats>,
) -> Result<DesktopSession> {
    if RUNTIME.get().is_some() {
        bail!("Linux desktop integration is already initialized");
    }

    let session = detect_desktop_session_from_env()?;
    verify_desktop_version(
        session.desktop,
        env::var("KDE_SESSION_VERSION").ok().as_deref(),
    )?;
    let probe = zbus::Connection::session()
        .await
        .context("Linux desktop integration requires an active D-Bus session")?;
    verify_shell(&probe, session.desktop).await?;
    verify_companion(&probe, session.desktop).await?;

    let runtime = Arc::new(LinuxDesktopRuntime {
        session,
        tray_enabled,
        commands,
        statistics,
        state: RwLock::new(RuntimeState {
            revision: 0,
            renderer_generation: 0,
            mode: "inactive",
            image_uri: None,
            shader: None,
            lease_expires_at_unix_ms: 0,
        }),
        connection: OnceLock::new(),
        renderer_listener: Mutex::new(None),
    });

    let service = AuraService {
        runtime: runtime.clone(),
    };
    let connection = zbus::connection::Builder::session()
        .context("failed to connect to the Linux D-Bus session")?
        .serve_at(OBJECT_PATH, service)
        .context("failed to register the Aura D-Bus object")?
        .build()
        .await
        .context("failed to build the Aura D-Bus service connection")?;
    connection
        .request_name_with_flags(BUS_NAME, zbus::fdo::RequestNameFlags::DoNotQueue.into())
        .await
        .map_err(|error| {
            anyhow!(
                "failed to acquire {BUS_NAME}; another Aura instance may already be running: {error}"
            )
        })?;
    runtime
        .connection
        .set(connection)
        .map_err(|_| anyhow!("Linux D-Bus connection was initialized twice"))?;
    RUNTIME
        .set(runtime.clone())
        .map_err(|_| anyhow!("Linux desktop runtime was initialized twice"))?;

    spawn_heartbeat(runtime);
    tracing::info!(
        desktop = session.desktop.label(),
        session_type = session.session_type.label(),
        bus_name = BUS_NAME,
        "Linux desktop integration initialized"
    );
    Ok(session)
}

pub fn current() -> Result<&'static Arc<LinuxDesktopRuntime>> {
    RUNTIME
        .get()
        .ok_or_else(|| anyhow!("Linux desktop integration has not been initialized"))
}

pub fn desktop_kind() -> Result<DesktopKind> {
    Ok(current()?.session.desktop)
}

pub fn publish_image(path: &Path) -> Result<()> {
    current()?.publish_image(path)
}

pub fn publish_shader(config: ShaderConfig, events: UnboundedSender<RendererEvent>) -> Result<u64> {
    current()?.publish_shader(config, events)
}

pub fn stop_shader(generation: u64) {
    if let Ok(runtime) = current() {
        runtime.stop_shader(generation);
    }
}

impl LinuxDesktopRuntime {
    fn publish_image(&self, path: &Path) -> Result<()> {
        let absolute = path
            .canonicalize()
            .with_context(|| format!("failed to canonicalize wallpaper {}", path.display()))?;
        let image_uri = Url::from_file_path(&absolute)
            .map_err(|_| anyhow!("failed to convert {} to a file URI", absolute.display()))?
            .to_string();
        {
            let mut state = self
                .state
                .write()
                .expect("Linux runtime state lock poisoned");
            state.revision = state.revision.wrapping_add(1);
            state.renderer_generation = state.renderer_generation.wrapping_add(1);
            state.mode = "image";
            state.image_uri = Some(image_uri);
            state.shader = None;
            state.lease_expires_at_unix_ms = 0;
        }
        self.clear_renderer_listener();
        self.broadcast_snapshot();
        Ok(())
    }

    fn publish_shader(
        &self,
        config: ShaderConfig,
        events: UnboundedSender<RendererEvent>,
    ) -> Result<u64> {
        let assets = precompiled::shader_assets(&config.name).ok_or_else(|| {
            anyhow!(
                "configured shader \"{}\" is unavailable; generated shaders: {}",
                config.name,
                precompiled::shader_names().join(", ")
            )
        })?;
        let (vertex_uri, fragment_uri) = materialize_plasma_assets(assets)?;
        let phase_start_unix_ms = unix_time_ms();
        let shader = ShaderSnapshot {
            name: config.name,
            target_fps: config.target_fps,
            resolution_percentage: config.resolution,
            mouse_enabled: config.mouse_enabled,
            scope: scope_name(config.desktop_scope),
            color_space: color_space_name(config.color_space),
            phase_start_unix_ms,
            gnome_glsl: assets.gnome_glsl.to_string(),
            plasma_vertex_uri: vertex_uri,
            plasma_fragment_uri: fragment_uri,
        };
        let generation = {
            let mut state = self
                .state
                .write()
                .expect("Linux runtime state lock poisoned");
            state.revision = state.revision.wrapping_add(1);
            state.renderer_generation = state.renderer_generation.wrapping_add(1);
            state.mode = "shader";
            state.shader = Some(shader);
            state.lease_expires_at_unix_ms = unix_time_ms() + PLASMA_LEASE.as_millis() as u64;
            state.renderer_generation
        };
        *self
            .renderer_listener
            .lock()
            .expect("renderer listener lock poisoned") = Some(RendererListener {
            generation,
            sender: events,
            ready_reported: false,
        });
        self.broadcast_snapshot();
        Ok(generation)
    }

    fn stop_shader(&self, generation: u64) {
        let should_broadcast = {
            let mut state = self
                .state
                .write()
                .expect("Linux runtime state lock poisoned");
            if generation != state.renderer_generation || state.mode != "shader" {
                false
            } else {
                state.revision = state.revision.wrapping_add(1);
                state.renderer_generation = state.renderer_generation.wrapping_add(1);
                state.shader = None;
                state.lease_expires_at_unix_ms = 0;
                state.mode = if state.image_uri.is_some() {
                    "image"
                } else {
                    "inactive"
                };
                true
            }
        };
        self.clear_renderer_listener();
        if should_broadcast {
            self.broadcast_snapshot();
        }
    }

    fn renderer_status(&self, generation: u64, status: &str, detail: &str) {
        let current_generation = self
            .state
            .read()
            .expect("Linux runtime state lock poisoned")
            .renderer_generation;
        if generation != current_generation {
            tracing::debug!(
                generation,
                current_generation,
                status,
                "ignoring stale Linux renderer acknowledgement"
            );
            return;
        }

        let mut listener = self
            .renderer_listener
            .lock()
            .expect("renderer listener lock poisoned");
        let Some(listener) = listener.as_mut() else {
            return;
        };
        if listener.generation != generation {
            return;
        }
        match status.to_ascii_lowercase().as_str() {
            "ready" | "running" => {
                if !listener.ready_reported {
                    let _ = listener.sender.send(RendererEvent::Ready);
                    let _ = listener.sender.send(RendererEvent::Running);
                    listener.ready_reported = true;
                }
            }
            "error" | "failed" | "fatal" => {
                let message = if detail.trim().is_empty() {
                    "Linux shell companion reported a renderer failure".to_string()
                } else {
                    detail.to_string()
                };
                let _ = listener.sender.send(RendererEvent::Fatal { message });
            }
            _ => tracing::warn!(generation, status, detail, "unknown renderer status"),
        }
    }

    fn clear_renderer_listener(&self) {
        if let Some(listener) = self
            .renderer_listener
            .lock()
            .expect("renderer listener lock poisoned")
            .take()
        {
            let _ = listener.sender.send(RendererEvent::Stopped);
        }
    }

    fn snapshot_json(&self) -> Result<String> {
        let state = self
            .state
            .read()
            .expect("Linux runtime state lock poisoned")
            .clone();
        let snapshot = RuntimeSnapshot {
            version: 1,
            revision: state.revision,
            renderer_generation: state.renderer_generation,
            mode: state.mode,
            tray_enabled: self.tray_enabled,
            image_uri: state.image_uri,
            shader: state.shader,
            lease_expires_at_unix_ms: state.lease_expires_at_unix_ms,
            desktop: DesktopSnapshot {
                desktop: self.session.desktop.label(),
                session_type: self.session.session_type.label(),
            },
            statistics: StatisticsSnapshot {
                image_timer: self.statistics.timer_display(),
                remote_update_timer: self.statistics.remote_update_timer_display(),
                image_count: self.statistics.total_images(),
                shown: self.statistics.images_shown(),
                skipped: self.statistics.manual_skips(),
                running_duration: format_running_duration(self.statistics.running_duration()),
            },
        };
        serde_json::to_string(&snapshot).context("failed to serialize Linux desktop snapshot")
    }

    fn broadcast_snapshot(&self) {
        let Ok(json) = self.snapshot_json() else {
            tracing::error!("failed to serialize Linux desktop snapshot");
            return;
        };
        let Some(connection) = self.connection.get().cloned() else {
            return;
        };
        let desktop = self.session.desktop;
        tokio::spawn(async move {
            match SignalEmitter::new(&connection, OBJECT_PATH) {
                Ok(emitter) => {
                    if let Err(error) = AuraService::snapshot_changed(&emitter, &json).await {
                        tracing::warn!(error = %error, "failed to emit SnapshotChanged");
                    }
                }
                Err(error) => {
                    tracing::warn!(error = %error, "failed to create D-Bus signal emitter")
                }
            }
            if desktop == DesktopKind::Plasma {
                if let Err(error) = apply_plasma_snapshot(&connection, &json).await {
                    tracing::warn!(error = %error, "failed to apply Plasma wallpaper snapshot");
                }
            }
        });
    }

    fn renew_lease(&self) -> bool {
        let mut state = self
            .state
            .write()
            .expect("Linux runtime state lock poisoned");
        if state.mode != "shader" {
            return false;
        }
        state.lease_expires_at_unix_ms = unix_time_ms() + PLASMA_LEASE.as_millis() as u64;
        true
    }
}

struct AuraService {
    runtime: Arc<LinuxDesktopRuntime>,
}

#[zbus::interface(name = "io.github.hmerritt.Aura1")]
impl AuraService {
    async fn get_snapshot(&self) -> zbus::fdo::Result<String> {
        self.runtime
            .snapshot_json()
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))
    }

    async fn next_background(&self) {
        let _ = self.runtime.commands.send(TrayEvent::NextWallpaper);
    }

    async fn reload_settings(&self) {
        let _ = self.runtime.commands.send(TrayEvent::ReloadSettings);
    }

    async fn open_settings(&self) {
        let _ = self.runtime.commands.send(TrayEvent::OpenSettings);
    }

    async fn exit(&self) {
        let _ = self.runtime.commands.send(TrayEvent::Exit);
    }

    async fn report_renderer_status(&self, renderer_generation: u64, status: &str, detail: &str) {
        self.runtime
            .renderer_status(renderer_generation, status, detail);
    }

    #[zbus(signal)]
    async fn snapshot_changed(emitter: &SignalEmitter<'_>, json: &str) -> zbus::Result<()>;
}

fn spawn_heartbeat(runtime: Arc<LinuxDesktopRuntime>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(HEARTBEAT_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        interval.tick().await;
        loop {
            interval.tick().await;
            if runtime.session.desktop == DesktopKind::Plasma {
                runtime.renew_lease();
            }
            runtime.broadcast_snapshot();
            if runtime.session.desktop == DesktopKind::Plasma {
                poll_plasma_status(&runtime).await;
            }
        }
    });
}

async fn verify_shell(connection: &zbus::Connection, desktop: DesktopKind) -> Result<()> {
    let name = match desktop {
        DesktopKind::Gnome => GNOME_SHELL_BUS_NAME,
        DesktopKind::Plasma => PLASMA_SHELL_BUS_NAME,
    };
    if !name_has_owner(connection, name).await? {
        bail!(
            "detected {} but its shell service {name} is unavailable on the session bus",
            desktop.label()
        );
    }
    Ok(())
}

async fn verify_companion(connection: &zbus::Connection, desktop: DesktopKind) -> Result<()> {
    if env::var_os("AURA_SKIP_COMPANION_CHECK").is_some() {
        return Ok(());
    }
    match desktop {
        DesktopKind::Gnome => {
            if !name_has_owner(connection, GNOME_COMPANION_BUS_NAME).await? {
                bail!(
                    "the Aura GNOME extension is not enabled; enable aura@hmerritt.github.io before starting Aura"
                );
            }
        }
        DesktopKind::Plasma => {
            if !plasma_companion_is_installed() {
                bail!(
                    "the Aura Plasma wallpaper plugin is unavailable; make {PLASMA_PLUGIN_ID} available to Plasma before starting Aura"
                );
            }
        }
    }
    Ok(())
}

async fn name_has_owner(connection: &zbus::Connection, name: &str) -> Result<bool> {
    let proxy = zbus::Proxy::new(
        connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )
    .await
    .context("failed to create D-Bus daemon proxy")?;
    proxy
        .call("NameHasOwner", &(name))
        .await
        .with_context(|| format!("failed to query D-Bus owner for {name}"))
}

fn plasma_companion_is_installed() -> bool {
    if let Some(path) = env::var_os("AURA_LINUX_COMPANION_DIR") {
        return PathBuf::from(path).join("plasma").is_dir();
    }
    linux_data_dirs().into_iter().any(|root| {
        root.join("plasma")
            .join("wallpapers")
            .join(PLASMA_PLUGIN_ID)
            .join("metadata.json")
            .is_file()
    })
}

fn linux_data_dirs() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = env::var_os("XDG_DATA_HOME") {
        paths.push(PathBuf::from(path));
    } else if let Some(home) = env::var_os("HOME") {
        paths.push(PathBuf::from(home).join(".local/share"));
    }
    let shared =
        env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    paths.extend(env::split_paths(&shared));
    paths
}

fn materialize_plasma_assets(
    assets: &'static precompiled::ShaderAssets,
) -> Result<(String, String)> {
    let runtime_dir = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("XDG_RUNTIME_DIR is required for Plasma shader assets"))?
        .join("aura/shaders");
    fs::create_dir_all(&runtime_dir)
        .with_context(|| format!("failed to create {}", runtime_dir.display()))?;
    let vertex = runtime_dir.join(format!("{}.vert.qsb", assets.name));
    let fragment = runtime_dir.join(format!("{}.frag.qsb", assets.name));
    write_if_changed(&vertex, assets.plasma_vertex_qsb)?;
    write_if_changed(&fragment, assets.plasma_fragment_qsb)?;
    let vertex_uri = Url::from_file_path(&vertex)
        .map_err(|_| anyhow!("failed to create URI for {}", vertex.display()))?
        .to_string();
    let fragment_uri = Url::from_file_path(&fragment)
        .map_err(|_| anyhow!("failed to create URI for {}", fragment.display()))?
        .to_string();
    Ok((vertex_uri, fragment_uri))
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<()> {
    if fs::read(path).ok().as_deref() == Some(bytes) {
        return Ok(());
    }
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

async fn apply_plasma_snapshot(connection: &zbus::Connection, json: &str) -> Result<()> {
    let json_literal = serde_json::to_string(json)?;
    let script = format!(
        r#"var auraSnapshot = {json_literal};
var auraState = JSON.parse(auraSnapshot);
var auraDesktops = desktops();
for (var i = 0; i < auraDesktops.length; ++i) {{
    var desktop = auraDesktops[i];
    var currentPlugin = desktop.wallpaperPlugin;
    desktop.currentConfigGroup = ['Wallpaper', '{PLASMA_PLUGIN_ID}', 'General'];
    if (auraState.mode === 'inactive') {{
        var previousPlugin = String(desktop.readConfig('PreviousWallpaperPlugin', ''));
        if (previousPlugin.length > 0)
            desktop.wallpaperPlugin = previousPlugin;
        continue;
    }}
    if (currentPlugin !== '{PLASMA_PLUGIN_ID}')
        desktop.writeConfig('PreviousWallpaperPlugin', currentPlugin);
    desktop.wallpaperPlugin = '{PLASMA_PLUGIN_ID}';
    desktop.currentConfigGroup = ['Wallpaper', '{PLASMA_PLUGIN_ID}', 'General'];
    desktop.writeConfig('Snapshot', auraSnapshot);
}}
'ok';"#
    );
    let proxy = zbus::Proxy::new(
        connection,
        PLASMA_SHELL_BUS_NAME,
        "/PlasmaShell",
        "org.kde.PlasmaShell",
    )
    .await
    .context("failed to create Plasma shell proxy")?;
    let _: String = proxy
        .call("evaluateScript", &(script))
        .await
        .context("Plasma rejected the Aura wallpaper configuration script")?;
    Ok(())
}

async fn poll_plasma_status(runtime: &Arc<LinuxDesktopRuntime>) {
    let Some(connection) = runtime.connection.get() else {
        return;
    };
    let script = format!(
        r#"var auraDesktops = desktops();
if (auraDesktops.length === 0) {{ JSON.stringify({{generation: 0, status: 'error', detail: 'Plasma has no desktop containments'}}); }}
else {{
    var desktop = auraDesktops[0];
    desktop.currentConfigGroup = ['Wallpaper', '{PLASMA_PLUGIN_ID}', 'General'];
    JSON.stringify({{
        generation: Number(desktop.readConfig('AckGeneration', '0')),
        status: String(desktop.readConfig('RendererStatus', 'waiting')),
        detail: String(desktop.readConfig('RendererDetail', ''))
    }});
}}"#
    );
    let result = async {
        let proxy = zbus::Proxy::new(
            connection,
            PLASMA_SHELL_BUS_NAME,
            "/PlasmaShell",
            "org.kde.PlasmaShell",
        )
        .await?;
        let response: String = proxy.call("evaluateScript", &(script)).await?;
        Ok::<String, zbus::Error>(response)
    }
    .await;
    match result {
        Ok(response) => {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&response) {
                let generation = value["generation"].as_u64().unwrap_or(0);
                let status = value["status"].as_str().unwrap_or("waiting");
                let detail = value["detail"].as_str().unwrap_or("");
                runtime.renderer_status(generation, status, detail);
            }
        }
        Err(error) => tracing::warn!(error = %error, "failed to poll Plasma renderer status"),
    }
}

pub fn detect_desktop_session_from_env() -> Result<DesktopSession> {
    detect_desktop_session(
        env::var("XDG_CURRENT_DESKTOP").ok().as_deref(),
        env::var("XDG_SESSION_TYPE").ok().as_deref(),
        env::var_os("WAYLAND_DISPLAY").is_some(),
        env::var_os("DISPLAY").is_some(),
    )
}

fn detect_desktop_session(
    desktop: Option<&str>,
    session_type: Option<&str>,
    wayland_display: bool,
    x11_display: bool,
) -> Result<DesktopSession> {
    let desktop_value = desktop.unwrap_or_default();
    let desktop_tokens = desktop_value
        .split([':', ';'])
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let desktop = if desktop_tokens.iter().any(|value| value.contains("gnome")) {
        DesktopKind::Gnome
    } else if desktop_tokens
        .iter()
        .any(|value| value.contains("kde") || value.contains("plasma"))
    {
        DesktopKind::Plasma
    } else {
        bail!(
            "unsupported Linux desktop {:?}; Aura requires GNOME 45+ or KDE Plasma 6+",
            desktop_value
        );
    };

    let session_type = match session_type
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "wayland" => SessionType::Wayland,
        "x11" | "xorg" => SessionType::X11,
        _ if wayland_display => SessionType::Wayland,
        _ if x11_display => SessionType::X11,
        value => bail!(
            "unsupported Linux session type {:?}; set XDG_SESSION_TYPE to x11 or wayland",
            value
        ),
    };
    Ok(DesktopSession {
        desktop,
        session_type,
    })
}

fn verify_desktop_version(desktop: DesktopKind, kde_session_version: Option<&str>) -> Result<()> {
    if desktop == DesktopKind::Plasma {
        if let Some(version) = kde_session_version {
            if version.trim() != "6" {
                bail!(
                    "unsupported KDE session version {:?}; Aura requires KDE Plasma 6+",
                    version
                );
            }
        }
    }
    Ok(())
}

fn scope_name(scope: ShaderDesktopScope) -> &'static str {
    match scope {
        ShaderDesktopScope::Virtual => "virtual",
        ShaderDesktopScope::Primary => "primary",
    }
}

fn color_space_name(color_space: ShaderColorSpace) -> &'static str {
    match color_space {
        ShaderColorSpace::Unorm => "unorm",
        ShaderColorSpace::Srgb => "srgb",
    }
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;
    use tokio::sync::mpsc;

    fn test_runtime(
        desktop: DesktopKind,
    ) -> (Arc<LinuxDesktopRuntime>, mpsc::UnboundedReceiver<TrayEvent>) {
        let stats = Arc::new(SessionStats::new(
            "5m".into(),
            "1h".into(),
            "Unsupported".into(),
            "gradient_glossy".into(),
        ));
        let (commands, receiver) = mpsc::unbounded_channel();
        (
            Arc::new(LinuxDesktopRuntime {
                session: DesktopSession {
                    desktop,
                    session_type: SessionType::Wayland,
                },
                tray_enabled: true,
                commands,
                statistics: stats,
                state: RwLock::new(RuntimeState {
                    revision: 2,
                    renderer_generation: 4,
                    mode: "shader",
                    image_uri: Some("file:///tmp/aura-fallback.png".into()),
                    shader: None,
                    lease_expires_at_unix_ms: 0,
                }),
                connection: OnceLock::new(),
                renderer_listener: Mutex::new(None),
            }),
            receiver,
        )
    }

    #[test]
    fn detects_gnome_wayland() {
        let session =
            detect_desktop_session(Some("ubuntu:GNOME"), Some("wayland"), true, false).unwrap();
        assert_eq!(session.desktop, DesktopKind::Gnome);
        assert_eq!(session.session_type, SessionType::Wayland);
    }

    #[test]
    fn detects_plasma_x11() {
        let session = detect_desktop_session(Some("KDE"), Some("x11"), false, true).unwrap();
        assert_eq!(session.desktop, DesktopKind::Plasma);
        assert_eq!(session.session_type, SessionType::X11);
    }

    #[test]
    fn display_variables_are_used_as_a_session_fallback() {
        let session = detect_desktop_session(Some("Plasma"), None, true, true).unwrap();
        assert_eq!(session.session_type, SessionType::Wayland);
    }

    #[test]
    fn unsupported_desktop_error_is_actionable() {
        let error = detect_desktop_session(Some("XFCE"), Some("x11"), false, true).unwrap_err();
        assert!(error.to_string().contains("GNOME 45+ or KDE Plasma 6+"));
    }

    #[test]
    fn rejects_plasma_5_session() {
        let error = verify_desktop_version(DesktopKind::Plasma, Some("5")).unwrap_err();
        assert!(error.to_string().contains("KDE Plasma 6+"));
        assert!(verify_desktop_version(DesktopKind::Plasma, Some("6")).is_ok());
        assert!(verify_desktop_version(DesktopKind::Gnome, None).is_ok());
    }

    #[test]
    fn stale_renderer_acknowledgement_is_ignored() {
        let (runtime, _) = test_runtime(DesktopKind::Gnome);
        let (sender, mut receiver) = mpsc::unbounded_channel();
        *runtime.renderer_listener.lock().unwrap() = Some(RendererListener {
            generation: 4,
            sender,
            ready_reported: false,
        });
        runtime.renderer_status(3, "error", "stale");
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn snapshot_serialization_is_versioned_and_complete() {
        let (runtime, _) = test_runtime(DesktopKind::Gnome);
        let value: serde_json::Value =
            serde_json::from_str(&runtime.snapshot_json().unwrap()).unwrap();
        assert_eq!(value["version"], 1);
        assert_eq!(value["revision"], 2);
        assert_eq!(value["rendererGeneration"], 4);
        assert_eq!(value["mode"], "shader");
        assert_eq!(value["trayEnabled"], true);
        assert_eq!(value["imageUri"], "file:///tmp/aura-fallback.png");
        assert_eq!(value["desktop"]["desktop"], "gnome");
        assert_eq!(value["desktop"]["sessionType"], "wayland");
        assert_eq!(value["statistics"]["imageTimer"], "5m");
    }

    #[test]
    fn shader_stop_returns_to_last_image_and_rejects_stale_stops() {
        let (runtime, _) = test_runtime(DesktopKind::Gnome);
        runtime.stop_shader(3);
        assert_eq!(runtime.state.read().unwrap().mode, "shader");
        runtime.stop_shader(4);
        let state = runtime.state.read().unwrap();
        assert_eq!(state.mode, "image");
        assert_eq!(state.renderer_generation, 5);
        assert!(state.shader.is_none());
    }

    #[test]
    fn plasma_lease_is_renewed_before_expiry() {
        let (runtime, _) = test_runtime(DesktopKind::Plasma);
        let before = unix_time_ms();
        assert!(runtime.renew_lease());
        let expires = runtime.state.read().unwrap().lease_expires_at_unix_ms;
        assert!(expires >= before + PLASMA_LEASE.as_millis() as u64);
        assert!(expires > before);
    }

    #[derive(Clone, Copy)]
    struct MonitorRect {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    }

    fn virtual_bounds(monitors: &[MonitorRect]) -> (i32, i32, u32, u32) {
        let min_x = monitors.iter().map(|rect| rect.x).min().unwrap_or(0);
        let min_y = monitors.iter().map(|rect| rect.y).min().unwrap_or(0);
        let max_x = monitors
            .iter()
            .map(|rect| rect.x + rect.width as i32)
            .max()
            .unwrap_or(0);
        let max_y = monitors
            .iter()
            .map(|rect| rect.y + rect.height as i32)
            .max()
            .unwrap_or(0);
        (min_x, min_y, (max_x - min_x) as u32, (max_y - min_y) as u32)
    }

    fn map_monitor_point(
        monitor: MonitorRect,
        local_x: f32,
        local_y: f32,
        virtual_origin: (i32, i32),
        scale: f32,
    ) -> (f32, f32) {
        (
            (monitor.x - virtual_origin.0) as f32 * scale + local_x * scale,
            (monitor.y - virtual_origin.1) as f32 * scale + local_y * scale,
        )
    }

    #[test]
    fn monitor_mapping_uses_one_continuous_virtual_coordinate_space() {
        let left = MonitorRect {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let primary = MonitorRect {
            x: 0,
            y: 0,
            width: 2560,
            height: 1440,
        };
        assert_eq!(virtual_bounds(&[left, primary]), (-1920, 0, 4480, 1440));
        assert_eq!(
            map_monitor_point(primary, 0.0, 0.0, (-1920, 0), 0.5),
            (960.0, 0.0)
        );
    }

    #[tokio::test]
    async fn private_session_bus_roundtrip_and_single_instance_rejection() {
        if env::var_os("AURA_TEST_PRIVATE_BUS").is_none() {
            return;
        }

        let (runtime, mut commands) = test_runtime(DesktopKind::Gnome);
        let service = AuraService {
            runtime: runtime.clone(),
        };
        let connection = zbus::connection::Builder::session()
            .unwrap()
            .serve_at(OBJECT_PATH, service)
            .unwrap()
            .build()
            .await
            .unwrap();
        connection
            .request_name_with_flags(BUS_NAME, zbus::fdo::RequestNameFlags::DoNotQueue.into())
            .await
            .unwrap();
        runtime.connection.set(connection.clone()).unwrap();

        let client = zbus::Connection::session().await.unwrap();
        let proxy = zbus::Proxy::new(&client, BUS_NAME, OBJECT_PATH, INTERFACE_NAME)
            .await
            .unwrap();
        let snapshot: String = proxy.call("GetSnapshot", &()).await.unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&snapshot).unwrap()["version"],
            1
        );

        let mut signals = proxy.receive_signal("SnapshotChanged").await.unwrap();
        runtime.broadcast_snapshot();
        let message = tokio::time::timeout(Duration::from_secs(2), signals.next())
            .await
            .unwrap()
            .unwrap();
        let (signal_snapshot,): (String,) = message.body().deserialize().unwrap();
        assert!(signal_snapshot.contains("\"version\":1"));

        let _: () = proxy.call("NextBackground", &()).await.unwrap();
        assert!(matches!(
            commands.recv().await,
            Some(TrayEvent::NextWallpaper)
        ));
        let _: () = proxy.call("ReloadSettings", &()).await.unwrap();
        assert!(matches!(
            commands.recv().await,
            Some(TrayEvent::ReloadSettings)
        ));
        let _: () = proxy.call("OpenSettings", &()).await.unwrap();
        assert!(matches!(
            commands.recv().await,
            Some(TrayEvent::OpenSettings)
        ));
        let _: () = proxy.call("Exit", &()).await.unwrap();
        assert!(matches!(commands.recv().await, Some(TrayEvent::Exit)));

        drop(proxy);
        let reconnected = zbus::Proxy::new(&client, BUS_NAME, OBJECT_PATH, INTERFACE_NAME)
            .await
            .unwrap();
        let _: String = reconnected.call("GetSnapshot", &()).await.unwrap();

        let second = zbus::Connection::session().await.unwrap();
        let second_request = second
            .request_name_with_flags(BUS_NAME, zbus::fdo::RequestNameFlags::DoNotQueue.into())
            .await;
        assert!(
            second_request.is_err(),
            "a second Aura bus owner must be rejected"
        );
    }
}
