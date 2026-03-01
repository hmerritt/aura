pub mod directory;
pub mod rss;
pub mod single;

use crate::cache::CacheManager;
use crate::config::{BgmConfig, SourceConfig};
use crate::errors::Result;
use anyhow::Context;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy)]
pub enum Origin {
    File,
    Directory,
    Rss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    File,
    Directory,
    Rss,
}

#[derive(Debug, Clone)]
pub struct ImageCandidate {
    pub id: String,
    pub origin: Origin,
    pub local_path: PathBuf,
    pub mtime: Option<SystemTime>,
}

#[async_trait]
pub trait ImageSource: Send {
    fn name(&self) -> &str;
    fn kind(&self) -> SourceKind;
    async fn refresh(&mut self) -> Result<Vec<ImageCandidate>>;
}

pub fn build_sources(
    config: &BgmConfig,
    cache: Arc<CacheManager>,
) -> Result<Vec<Box<dyn ImageSource>>> {
    let mut sources: Vec<Box<dyn ImageSource>> = Vec::new();

    for source in &config.sources {
        match source {
            SourceConfig::File { path } => {
                sources.push(Box::new(single::SingleSource::new(path.clone())));
            }
            SourceConfig::Directory {
                path,
                recursive,
                extensions,
            } => {
                sources.push(Box::new(directory::DirectorySource::new(
                    path.clone(),
                    *recursive,
                    extensions.clone().unwrap_or_default(),
                )));
            }
            SourceConfig::Rss {
                url,
                max_items,
                download_dir,
            } => {
                let dir = if let Some(download_dir) = download_dir {
                    download_dir.clone()
                } else {
                    cache
                        .ensure_remote_source_dir(url)
                        .with_context(|| format!("failed to initialize RSS cache for {url}"))?
                };
                sources.push(Box::new(rss::RssSource::new(url.clone(), *max_items, dir)?));
            }
        }
    }

    Ok(sources)
}

pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "bmp" | "gif" | "webp"
            )
        })
        .unwrap_or(false)
}

pub fn image_id(prefix: &str, path: &Path) -> String {
    let raw = format!("{prefix}:{}", path.to_string_lossy());
    blake3::hash(raw.as_bytes()).to_hex().to_string()
}
