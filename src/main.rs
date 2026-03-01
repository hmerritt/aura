mod cache;
mod config;
mod errors;
mod image_pipeline;
mod logging;
mod rotation;
mod scheduler;
mod sources;
mod state;
mod wallpaper;

use crate::cache::CacheManager;
use crate::config::load_from_path;
use crate::errors::Result;
use crate::rotation::RotationManager;
use crate::scheduler::{Scheduler, SchedulerEvent};
use crate::sources::{build_sources, ImageCandidate, ImageSource, Origin};
use crate::state::{PersistedState, StateStore};
use anyhow::Context;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let config_path = resolve_config_path_from_args(&args)?;
    let created = ensure_config_exists(&config_path)?;

    let config = load_from_path(&config_path)?;
    logging::init(&config.log_level);
    if created {
        info!(path = %config_path.display(), "created default config");
    }
    info!(path = %config_path.display(), "loaded config");

    let cache = Arc::new(CacheManager::new(&config)?);
    if let Err(error) = cache.cleanup() {
        warn!(error = %error, "cache cleanup failed");
    }

    let mut sources = build_sources(&config, cache.clone())?;
    let backend = wallpaper::default_backend();
    let state_store = StateStore::new(config.state_file.clone());

    let persisted_state = match state_store.load() {
        Ok(state) => state,
        Err(error) => {
            warn!(error = %error, "failed to load state, starting fresh");
            PersistedState::default()
        }
    };

    let mut candidates = refresh_all_sources(&mut sources).await?;
    let mut rotation = RotationManager::new();
    rotation.rebuild_pool(candidates.clone());
    rotation.restore_state(&persisted_state);

    let mut last_image_id = persisted_state.last_image_id.clone();
    if let Some(next_id) =
        try_switch_once(&mut rotation, cache.as_ref(), &*backend, &config).await?
    {
        last_image_id = Some(next_id);
        persist_state(&state_store, &rotation, last_image_id.clone())?;
    }

    let mut scheduler = Scheduler::new(config.timer, config.remote_update_timer);
    info!("bgm is running");

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("ctrl-c received, stopping bgm");
                persist_state(&state_store, &rotation, last_image_id.clone())?;
                break;
            }
            event = scheduler.next_event() => {
                match event {
                    SchedulerEvent::SwitchImage => {
                        match try_switch_once(&mut rotation, cache.as_ref(), &*backend, &config).await {
                            Ok(Some(next_id)) => {
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
                                candidates = updated;
                                rotation.rebuild_pool(candidates.clone());
                                info!(pool_size = rotation.pool_size(), "refresh complete");
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

async fn refresh_all_sources(sources: &mut [Box<dyn ImageSource>]) -> Result<Vec<ImageCandidate>> {
    let mut candidates = Vec::new();
    let mut unique = HashSet::new();

    for source in sources.iter_mut() {
        match source.refresh().await {
            Ok(items) => {
                info!(
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

    info!(count = candidates.len(), "total merged candidates");
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

    let screen = backend
        .screen_spec()
        .context("failed to resolve screen size")?;
    info!(
        width = screen.width,
        height = screen.height,
        "resolved screen dimensions"
    );
    let processed = image_pipeline::prepare_for_screen(
        &candidate.local_path,
        screen,
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

fn resolve_config_path_from_args(args: &[String]) -> Result<PathBuf> {
    if let Some(path) = args.first() {
        return expand_tilde(path);
    }
    default_user_config_path()
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
}
