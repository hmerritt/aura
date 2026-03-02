mod cache;
mod config;
mod errors;
mod image_pipeline;
mod logging;
mod renderer;
mod rotation;
mod scheduler;
mod sources;
mod state;
mod tray;
mod version;
mod wallpaper;

use crate::cache::CacheManager;
use crate::config::{load_from_path, RendererMode};
use crate::errors::Result;
use crate::renderer::{RendererCommand, RendererEvent, ShaderRenderer};
use crate::rotation::RotationManager;
use crate::scheduler::{Scheduler, SchedulerEvent};
use crate::sources::{build_sources, ImageCandidate, ImageSource, Origin, SourceKind};
use crate::state::{PersistedState, StateStore};
use crate::tray::{format_config_duration, SessionStats, TrayEvent};
use anyhow::Context;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveMode {
    Image,
    Shader,
}

#[derive(Debug)]
struct CliOptions {
    config_path: PathBuf,
    tray_enabled: bool,
    print_version: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let options = parse_cli_options(&args)?;

    print_version_banner();
    if options.print_version {
        return Ok(());
    }

    let config_path = options.config_path.clone();
    let created = ensure_config_exists(&config_path)?;

    let mut config = load_from_path(&config_path)?;
    logging::init(&config.log_level);
    if created {
        info!(path = %config_path.display(), "created default config");
    }
    info!(path = %config_path.display(), "loaded config");

    let mut cache = Arc::new(CacheManager::new(&config)?);
    if let Err(error) = cache.cleanup() {
        warn!(error = %error, "cache cleanup failed");
    }

    let mut sources = build_sources(&config, cache.clone())?;
    let backend = wallpaper::default_backend();
    let mut state_store = StateStore::new(config.state_file.clone());

    let persisted_state = match state_store.load() {
        Ok(state) => state,
        Err(error) => {
            warn!(error = %error, "failed to load state, starting fresh");
            PersistedState::default()
        }
    };

    let initial_candidates = refresh_all_sources(&mut sources).await?;
    let (mut local_images_count, mut remote_images_count) =
        count_images_by_origin(&initial_candidates);
    let mut rotation = RotationManager::new();
    rotation.rebuild_pool(initial_candidates);
    rotation.restore_state(&persisted_state);
    let session_stats = Arc::new(SessionStats::new(
        format_config_duration(config.timer),
        format_config_duration(config.remote_update_timer),
    ));
    session_stats.set_total_images(local_images_count + remote_images_count);

    let (tray_event_tx, mut tray_event_rx) = tokio::sync::mpsc::unbounded_channel::<TrayEvent>();
    let mut _single_instance_guard = None;
    let mut _tray_controller = None;
    if options.tray_enabled && cfg!(windows) {
        _single_instance_guard = match tray::try_acquire_single_instance()? {
            Some(guard) => Some(guard),
            None => {
                info!("another tray-enabled bgm instance is already running, exiting");
                return Ok(());
            }
        };

        _tray_controller = Some(tray::spawn(
            config_path.clone(),
            tray_event_tx.clone(),
            session_stats.clone(),
        )?);
        info!("tray mode enabled");
    }

    let mut renderer: Option<ShaderRenderer> = None;
    let mut renderer_event_rx: Option<tokio::sync::mpsc::UnboundedReceiver<RendererEvent>> = None;
    let mut active_mode = ActiveMode::Image;
    if config.renderer == RendererMode::Shader {
        if let Some(shader_config) = config.shader.clone() {
            match ShaderRenderer::start(shader_config) {
                Ok(mut shader_renderer) => {
                    renderer_event_rx = shader_renderer.take_event_receiver();
                    renderer = Some(shader_renderer);
                    active_mode = ActiveMode::Shader;
                    info!("shader renderer started");
                }
                Err(error) => {
                    warn!(
                        error = %error,
                        "shader renderer startup failed, falling back to image mode"
                    );
                }
            }
        } else {
            warn!("renderer is set to shader but shader config is missing, using image mode");
        }
    }
    session_stats.set_shader_active(active_mode == ActiveMode::Shader);

    let mut last_image_id = persisted_state.last_image_id.clone();
    if active_mode == ActiveMode::Image {
        if let Some(next_id) =
            try_switch_once(&mut rotation, cache.as_ref(), &*backend, &config).await?
        {
            session_stats.inc_images_shown();
            last_image_id = Some(next_id);
            persist_state(&state_store, &rotation, last_image_id.clone())?;
        }
    }

    let mut scheduler = Scheduler::new(config.timer, config.remote_update_timer);
    info!("bgm is running");

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("ctrl-c received, stopping bgm");
                if let Some(renderer) = renderer.as_mut() {
                    renderer.stop();
                }
                persist_state(&state_store, &rotation, last_image_id.clone())?;
                break;
            }
            renderer_event = async {
                if let Some(receiver) = renderer_event_rx.as_mut() {
                    receiver.recv().await
                } else {
                    std::future::pending::<Option<RendererEvent>>().await
                }
            } => {
                if let Some(renderer_event) = renderer_event {
                    match renderer_event {
                        RendererEvent::Ready => info!("shader renderer ready"),
                        RendererEvent::Running => info!("shader renderer running"),
                        RendererEvent::Paused => info!("shader renderer paused"),
                        RendererEvent::Stopped => info!("shader renderer stopped"),
                        RendererEvent::Fatal { message } => {
                            warn!(error = %message, "shader renderer failed, switching to image mode");
                            if let Some(renderer) = renderer.as_mut() {
                                renderer.stop();
                            }
                            renderer = None;
                            renderer_event_rx = None;
                            active_mode = ActiveMode::Image;
                            session_stats.set_shader_active(false);
                            match try_switch_once(&mut rotation, cache.as_ref(), &*backend, &config).await {
                                Ok(Some(next_id)) => {
                                    session_stats.inc_images_shown();
                                    last_image_id = Some(next_id);
                                    if let Err(error) = persist_state(&state_store, &rotation, last_image_id.clone()) {
                                        warn!(error = %error, "failed to persist state after shader fallback");
                                    }
                                }
                                Ok(None) => warn!("shader fallback requested image mode but no image was available"),
                                Err(error) => warn!(error = %error, "failed to apply image mode fallback"),
                            }
                        }
                    }
                }
            }
            tray_event = tray_event_rx.recv() => {
                match tray_event {
                    Some(TrayEvent::NextWallpaper) => {
                        if active_mode != ActiveMode::Image {
                            warn!("Next Background ignored while shader mode is active");
                            continue;
                        }
                        match refresh_local_sources(&mut sources).await {
                            Ok(updated) => {
                                let (next_local_count, _) = count_images_by_origin(&updated);
                                local_images_count = next_local_count;
                                session_stats
                                    .set_total_images(local_images_count + remote_images_count);
                                rotation.rebuild_pool(updated);
                                info!(pool_size = rotation.pool_size(), "local refresh complete before tray switch");
                            }
                            Err(error) => warn!(error = %error, "local refresh failed before tray switch"),
                        }
                        match try_switch_once(&mut rotation, cache.as_ref(), &*backend, &config).await {
                            Ok(Some(next_id)) => {
                                session_stats.inc_images_shown();
                                session_stats.inc_manual_skips();
                                last_image_id = Some(next_id);
                                if let Err(error) = persist_state(&state_store, &rotation, last_image_id.clone()) {
                                    warn!(error = %error, "failed to persist state after tray wallpaper switch");
                                }
                            }
                            Ok(None) => warn!("tray requested switch but no image available"),
                            Err(error) => warn!(error = %error, "tray-requested wallpaper switch failed"),
                        }
                    }
                    Some(TrayEvent::ReloadSettings) => {
                        info!("tray requested settings reload");

                        let new_config = match load_from_path(&config_path) {
                            Ok(new_config) => new_config,
                            Err(error) => {
                                warn!(error = %error, "failed to reload config; keeping current runtime settings");
                                continue;
                            }
                        };

                        let new_cache = match CacheManager::new(&new_config) {
                            Ok(new_cache) => Arc::new(new_cache),
                            Err(error) => {
                                warn!(error = %error, "failed to initialize cache from reloaded config; keeping current runtime settings");
                                continue;
                            }
                        };
                        if let Err(error) = new_cache.cleanup() {
                            warn!(error = %error, "cache cleanup failed after settings reload");
                        }

                        let mut new_sources = match build_sources(&new_config, new_cache.clone()) {
                            Ok(new_sources) => new_sources,
                            Err(error) => {
                                warn!(error = %error, "failed to build sources from reloaded config; keeping current runtime settings");
                                continue;
                            }
                        };

                        let refreshed_candidates = match refresh_all_sources(&mut new_sources).await {
                            Ok(candidates) => candidates,
                            Err(error) => {
                                warn!(error = %error, "failed to refresh sources after settings reload; keeping current runtime settings");
                                continue;
                            }
                        };

                        let (next_local_count, next_remote_count) =
                            count_images_by_origin(&refreshed_candidates);
                        let mut preserved_state = rotation.export_state();
                        preserved_state.last_image_id = last_image_id.clone();
                        rotation.rebuild_pool(refreshed_candidates);
                        rotation.restore_state(&preserved_state);

                        config = new_config;
                        cache = new_cache;
                        sources = new_sources;
                        state_store = StateStore::new(config.state_file.clone());
                        scheduler = Scheduler::new(config.timer, config.remote_update_timer);
                        local_images_count = next_local_count;
                        remote_images_count = next_remote_count;
                        session_stats.set_total_images(local_images_count + remote_images_count);
                        session_stats.set_timer_display(format_config_duration(config.timer));
                        session_stats.set_remote_update_timer_display(format_config_duration(
                            config.remote_update_timer,
                        ));
                        logging::set_level(&config.log_level);

                        if let Some(renderer) = renderer.as_mut() {
                            renderer.stop();
                        }
                        renderer = None;
                        renderer_event_rx = None;
                        active_mode = ActiveMode::Image;
                        session_stats.set_shader_active(false);
                        if config.renderer == RendererMode::Shader {
                            if let Some(shader_config) = config.shader.clone() {
                                match ShaderRenderer::start(shader_config) {
                                    Ok(mut new_renderer) => {
                                        renderer_event_rx = new_renderer.take_event_receiver();
                                        renderer = Some(new_renderer);
                                        active_mode = ActiveMode::Shader;
                                        session_stats.set_shader_active(true);
                                        info!("settings reload switched runtime to shader mode");
                                    }
                                    Err(error) => {
                                        warn!(error = %error, "failed to restart shader mode from reloaded settings");
                                    }
                                }
                            }
                        }

                        if active_mode == ActiveMode::Image {
                            match try_switch_once(&mut rotation, cache.as_ref(), &*backend, &config).await {
                                Ok(Some(next_id)) => {
                                    session_stats.inc_images_shown();
                                    last_image_id = Some(next_id);
                                }
                                Ok(None) => warn!("settings reload kept image mode but no image was available"),
                                Err(error) => warn!(error = %error, "failed to apply wallpaper after settings reload"),
                            }
                        }

                        if let Err(error) =
                            persist_state(&state_store, &rotation, last_image_id.clone())
                        {
                            warn!(error = %error, "failed to persist state after settings reload");
                        }
                        info!("settings reload complete");
                    }
                    Some(TrayEvent::FallbackToImage) => {
                        if let Some(renderer) = renderer.as_ref() {
                            let _ = renderer.send_command(RendererCommand::DisableOutput);
                        }
                        if let Some(renderer) = renderer.as_mut() {
                            renderer.stop();
                        }
                        renderer = None;
                        renderer_event_rx = None;
                        active_mode = ActiveMode::Image;
                        session_stats.set_shader_active(false);

                        match try_switch_once(&mut rotation, cache.as_ref(), &*backend, &config).await {
                            Ok(Some(next_id)) => {
                                session_stats.inc_images_shown();
                                last_image_id = Some(next_id);
                                if let Err(error) = persist_state(&state_store, &rotation, last_image_id.clone()) {
                                    warn!(error = %error, "failed to persist state after tray fallback to image");
                                }
                            }
                            Ok(None) => warn!("tray requested image fallback but no image was available"),
                            Err(error) => warn!(error = %error, "tray fallback to image failed"),
                        }
                    }
                    Some(TrayEvent::Exit) => {
                        info!("tray requested exit, stopping bgm");
                        if let Some(renderer) = renderer.as_mut() {
                            renderer.stop();
                        }
                        persist_state(&state_store, &rotation, last_image_id.clone())?;
                        break;
                    }
                    None => {
                        info!("tray event channel closed, stopping bgm");
                        if let Some(renderer) = renderer.as_mut() {
                            renderer.stop();
                        }
                        persist_state(&state_store, &rotation, last_image_id.clone())?;
                        break;
                    }
                }
            }
            event = scheduler.next_event() => {
                match event {
                    SchedulerEvent::SwitchImage => {
                        if active_mode != ActiveMode::Image {
                            continue;
                        }
                        match refresh_local_sources(&mut sources).await {
                            Ok(updated) => {
                                let (next_local_count, _) = count_images_by_origin(&updated);
                                local_images_count = next_local_count;
                                session_stats
                                    .set_total_images(local_images_count + remote_images_count);
                                rotation.rebuild_pool(updated);
                                info!(pool_size = rotation.pool_size(), "local refresh complete before timer switch");
                            }
                            Err(error) => warn!(error = %error, "local refresh failed before timer switch"),
                        }
                        match try_switch_once(&mut rotation, cache.as_ref(), &*backend, &config).await {
                            Ok(Some(next_id)) => {
                                session_stats.inc_images_shown();
                                last_image_id = Some(next_id);
                                if let Err(error) = persist_state(&state_store, &rotation, last_image_id.clone()) {
                                    warn!(error = %error, "failed to persist state after wallpaper switch");
                                }
                            }
                            Ok(None) => {
                                warn!("no image available for switch");
                            }
                            Err(error) => {
                                warn!(error = %error, "failed to switch wallpaper");
                            }
                        }
                    }
                    SchedulerEvent::RefreshRemote => {
                        match refresh_all_sources(&mut sources).await {
                            Ok(updated) => {
                                let (next_local_count, next_remote_count) =
                                    count_images_by_origin(&updated);
                                local_images_count = next_local_count;
                                remote_images_count = next_remote_count;
                                session_stats
                                    .set_total_images(local_images_count + remote_images_count);
                                rotation.rebuild_pool(updated);
                                info!(pool_size = rotation.pool_size(), "full refresh complete");
                            }
                            Err(error) => warn!(error = %error, "source refresh failed"),
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn print_version_banner() {
    let info = version::get_version();
    println!("{}", info.full_version_number(true));
}

async fn refresh_all_sources(sources: &mut [Box<dyn ImageSource>]) -> Result<Vec<ImageCandidate>> {
    refresh_sources_filtered(sources, |_| true, "full").await
}

async fn refresh_local_sources(
    sources: &mut [Box<dyn ImageSource>],
) -> Result<Vec<ImageCandidate>> {
    refresh_sources_filtered(
        sources,
        |kind| matches!(kind, SourceKind::File | SourceKind::Directory),
        "local",
    )
    .await
}

async fn refresh_sources_filtered<F>(
    sources: &mut [Box<dyn ImageSource>],
    mut should_refresh: F,
    scope: &str,
) -> Result<Vec<ImageCandidate>>
where
    F: FnMut(SourceKind) -> bool,
{
    let mut candidates = Vec::new();
    let mut unique = HashSet::new();

    for source in sources.iter_mut() {
        if !should_refresh(source.kind()) {
            continue;
        }

        match source.refresh().await {
            Ok(items) => {
                info!(
                    scope = scope,
                    source = source.name(),
                    count = items.len(),
                    "source refresh"
                );
                for item in items {
                    if unique.insert(item.id.clone()) {
                        candidates.push(item);
                    }
                }
            }
            Err(error) => {
                warn!(source = source.name(), error = %error, "source refresh failed");
            }
        }
    }

    candidates.sort_by(|a, b| {
        let a_key = a
            .mtime
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let b_key = b
            .mtime
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        b_key
            .cmp(&a_key)
            .then_with(|| a.local_path.cmp(&b.local_path))
    });

    info!(scope = scope, count = candidates.len(), "merged candidates");
    Ok(candidates)
}

async fn try_switch_once(
    rotation: &mut RotationManager,
    cache: &CacheManager,
    backend: &dyn wallpaper::WallpaperBackend,
    config: &config::BgmConfig,
) -> Result<Option<String>> {
    if rotation.pool_size() == 0 {
        return Ok(None);
    }

    let candidate = match rotation.next() {
        Some(candidate) => candidate,
        None => return Ok(None),
    };

    let processed = image_pipeline::prepare_for_output(
        &candidate.local_path,
        cache,
        config.image_format,
        config.jpeg_quality,
    )
    .with_context(|| format!("failed to process {}", candidate.local_path.display()))?;

    backend
        .set_wallpaper(&processed)
        .with_context(|| format!("failed to set wallpaper {}", processed.display()))?;

    info!(
        id = %candidate.id,
        source = %origin_name(candidate.origin),
        input = %candidate.local_path.display(),
        output = %processed.display(),
        "wallpaper updated"
    );
    Ok(Some(candidate.id))
}

fn origin_name(origin: Origin) -> &'static str {
    match origin {
        Origin::File => "file",
        Origin::Directory => "directory",
        Origin::Rss => "rss",
    }
}

fn count_images_by_origin(candidates: &[ImageCandidate]) -> (u64, u64) {
    let mut local_images_count = 0_u64;
    let mut remote_images_count = 0_u64;

    for candidate in candidates {
        match candidate.origin {
            Origin::File | Origin::Directory => local_images_count += 1,
            Origin::Rss => remote_images_count += 1,
        }
    }

    (local_images_count, remote_images_count)
}

fn persist_state(
    state_store: &StateStore,
    rotation: &RotationManager,
    last_image_id: Option<String>,
) -> Result<()> {
    let mut persisted = rotation.export_state();
    persisted.last_image_id = last_image_id;
    state_store.save(&persisted)?;
    Ok(())
}

fn parse_cli_options(args: &[String]) -> Result<CliOptions> {
    let mut tray_enabled = true;
    let mut print_version = false;
    let mut config_arg: Option<String> = None;

    for arg in args {
        if arg == "--no-tray" {
            tray_enabled = false;
            continue;
        }
        if arg == "--version" {
            print_version = true;
            continue;
        }

        if arg.starts_with('-') {
            anyhow::bail!("unknown flag: {arg}");
        }

        if config_arg.is_some() {
            anyhow::bail!("only one config path positional argument is supported");
        }
        config_arg = Some(arg.clone());
    }

    let config_path = if print_version {
        match config_arg {
            Some(config_arg) => expand_tilde(&config_arg)?,
            None => PathBuf::new(),
        }
    } else if let Some(config_arg) = config_arg {
        expand_tilde(&config_arg)?
    } else {
        default_user_config_path()?
    };

    Ok(CliOptions {
        config_path,
        tray_enabled,
        print_version,
    })
}

fn expand_tilde(path: &str) -> Result<PathBuf> {
    if path == "~" || path.starts_with("~/") || path.starts_with("~\\") {
        let home = dirs::home_dir().context("failed to resolve home directory")?;
        if path == "~" {
            return Ok(home);
        }
        let suffix = &path[2..];
        return Ok(home.join(suffix));
    }
    Ok(PathBuf::from(path))
}

fn default_user_config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to resolve home directory")?;
    Ok(home.join(".config").join("bgm.hcl"))
}

fn default_pictures_dir() -> Result<PathBuf> {
    if let Some(path) = dirs::picture_dir() {
        return Ok(path);
    }
    let home = dirs::home_dir().context("failed to resolve home directory for Pictures path")?;
    Ok(home.join("Pictures"))
}

fn ensure_config_exists(config_path: &Path) -> Result<bool> {
    let pictures = default_pictures_dir()?;
    ensure_config_exists_with_pictures(config_path, &pictures)
}

fn ensure_config_exists_with_pictures(config_path: &Path, pictures_dir: &Path) -> Result<bool> {
    if config_path.exists() {
        return Ok(false);
    }

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config directory {}", parent.display()))?;
    }
    fs::create_dir_all(pictures_dir).with_context(|| {
        format!(
            "failed to create pictures directory {}",
            pictures_dir.display()
        )
    })?;

    let payload = config::default_hcl(pictures_dir);
    let tmp_path = config_path.with_extension("tmp");
    fs::write(&tmp_path, payload)
        .with_context(|| format!("failed to write {}", tmp_path.display()))?;
    fs::rename(&tmp_path, config_path)
        .with_context(|| format!("failed to create config {}", config_path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn creates_missing_config_with_directory_source() {
        let tmp = tempdir().unwrap();
        let config_path = tmp.path().join(".config").join("bgm.hcl");
        let pictures = tmp.path().join("Pictures");

        let created = ensure_config_exists_with_pictures(&config_path, &pictures).unwrap();
        assert!(created);
        assert!(config_path.exists());
        assert!(pictures.exists());

        let text = fs::read_to_string(&config_path).unwrap();
        let parsed = config::parse_from_str(&text, &config_path).unwrap();
        assert_eq!(parsed.sources.len(), 1);
    }

    #[test]
    fn does_not_overwrite_existing_config() {
        let tmp = tempdir().unwrap();
        let config_path = tmp.path().join(".config").join("bgm.hcl");
        let pictures = tmp.path().join("Pictures");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(&config_path, "timer = 300\nsources = []\n").unwrap();

        let created = ensure_config_exists_with_pictures(&config_path, &pictures).unwrap();
        assert!(!created);
    }

    #[test]
    fn cli_defaults_to_tray_with_default_path() {
        let options = parse_cli_options(&[]).unwrap();
        assert!(options.tray_enabled);
        assert!(!options.print_version);
        assert_eq!(options.config_path.file_name().unwrap(), "bgm.hcl");
    }

    #[test]
    fn cli_supports_no_tray_flag() {
        let options = parse_cli_options(&["--no-tray".to_string()]).unwrap();
        assert!(!options.tray_enabled);
        assert!(!options.print_version);
    }

    #[test]
    fn cli_supports_version_flag() {
        let options = parse_cli_options(&["--version".to_string()]).unwrap();
        assert!(options.print_version);
        assert!(options.config_path.as_os_str().is_empty());
    }

    #[test]
    fn cli_supports_version_with_no_tray() {
        let options =
            parse_cli_options(&["--version".to_string(), "--no-tray".to_string()]).unwrap();
        assert!(options.print_version);
        assert!(!options.tray_enabled);
    }
}
