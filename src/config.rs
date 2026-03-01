use crate::errors::Result;
use anyhow::{bail, Context};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEFAULT_TIMER_SECS: u64 = 300;
const DEFAULT_REMOTE_UPDATE_TIMER_SECS: u64 = 3600;
const MIN_TIMER_SECS: u64 = 5;
const MIN_REMOTE_UPDATE_SECS: u64 = 30;
const DEFAULT_JPEG_QUALITY: u8 = 90;
const DEFAULT_MAX_CACHE_MB: u64 = 1024;
const DEFAULT_MAX_CACHE_AGE_DAYS: u64 = 30;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Jpg,
    Png,
}

impl OutputFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpg => "jpg",
            Self::Png => "png",
        }
    }
}

fn default_recursive() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum RawSourceConfig {
    File {
        path: PathBuf,
    },
    Directory {
        path: PathBuf,
        #[serde(default = "default_recursive")]
        recursive: bool,
        extensions: Option<Vec<String>>,
    },
    Rss {
        url: String,
        max_items: Option<usize>,
        download_dir: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Deserialize)]
struct RawConfig {
    timer: Option<u64>,
    #[serde(rename = "remoteUpdateTimer")]
    remote_update_timer: Option<u64>,
    sources: Vec<RawSourceConfig>,
    cache_dir: Option<PathBuf>,
    state_file: Option<PathBuf>,
    log_level: Option<String>,
    image_format: Option<OutputFormat>,
    jpeg_quality: Option<u8>,
    max_cache_mb: Option<u64>,
    max_cache_age_days: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum SourceConfig {
    File {
        path: PathBuf,
    },
    Directory {
        path: PathBuf,
        recursive: bool,
        extensions: Option<Vec<String>>,
    },
    Rss {
        url: String,
        max_items: usize,
        download_dir: Option<PathBuf>,
    },
}

#[derive(Debug, Clone)]
pub struct BgmConfig {
    pub timer: Duration,
    pub remote_update_timer: Duration,
    pub sources: Vec<SourceConfig>,
    pub cache_dir: PathBuf,
    pub state_file: PathBuf,
    pub log_level: String,
    pub image_format: OutputFormat,
    pub jpeg_quality: u8,
    pub max_cache_bytes: u64,
    pub max_cache_age: Duration,
}

pub fn load_from_path(path: &Path) -> Result<BgmConfig> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read config from {}", path.display()))?;
    parse_from_str(&content, path)
}

pub fn parse_from_str(content: &str, path: &Path) -> Result<BgmConfig> {
    let raw: RawConfig =
        hcl::from_str(content).with_context(|| format!("invalid HCL in {}", path.display()))?;
    BgmConfig::from_raw(raw, path)
}

pub fn default_hcl(pictures_dir: &Path) -> String {
    let pictures = hcl_path(pictures_dir);
    format!(
        r#"timer = 300
remoteUpdateTimer = 3600
image_format = "jpg"
jpeg_quality = 90
log_level = "info"

sources = [
  {{ type = "directory", path = "{}", recursive = true, extensions = ["jpg", "jpeg", "png", "webp", "bmp", "gif"] }}
]
"#,
        pictures
    )
}

impl BgmConfig {
    fn from_raw(raw: RawConfig, config_path: &Path) -> Result<Self> {
        let config_parent = config_path.parent().unwrap_or_else(|| Path::new("."));
        let app_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("bgm");

        let timer_secs = raw.timer.unwrap_or(DEFAULT_TIMER_SECS);
        if timer_secs < MIN_TIMER_SECS {
            bail!("timer must be at least {MIN_TIMER_SECS} seconds");
        }

        let remote_secs = raw
            .remote_update_timer
            .unwrap_or(DEFAULT_REMOTE_UPDATE_TIMER_SECS);
        if remote_secs < MIN_REMOTE_UPDATE_SECS {
            bail!("remoteUpdateTimer must be at least {MIN_REMOTE_UPDATE_SECS} seconds");
        }

        let jpeg_quality = raw.jpeg_quality.unwrap_or(DEFAULT_JPEG_QUALITY);
        if jpeg_quality == 0 || jpeg_quality > 100 {
            bail!("jpeg_quality must be between 1 and 100");
        }

        let cache_dir = resolve_path(
            raw.cache_dir.unwrap_or_else(|| app_dir.join("cache")),
            config_parent,
        );
        let state_file = resolve_path(
            raw.state_file.unwrap_or_else(|| app_dir.join("state.json")),
            config_parent,
        );

        let sources = raw
            .sources
            .into_iter()
            .map(|source| validate_source(source, config_parent))
            .collect::<Result<Vec<_>>>()?;

        if sources.is_empty() {
            bail!("at least one source is required");
        }

        let max_cache_bytes = raw.max_cache_mb.unwrap_or(DEFAULT_MAX_CACHE_MB) * 1024 * 1024;
        let max_cache_age = Duration::from_secs(
            raw.max_cache_age_days.unwrap_or(DEFAULT_MAX_CACHE_AGE_DAYS) * 24 * 60 * 60,
        );

        Ok(Self {
            timer: Duration::from_secs(timer_secs),
            remote_update_timer: Duration::from_secs(remote_secs),
            sources,
            cache_dir,
            state_file,
            log_level: raw.log_level.unwrap_or_else(|| "info".to_string()),
            image_format: raw.image_format.unwrap_or(OutputFormat::Jpg),
            jpeg_quality,
            max_cache_bytes,
            max_cache_age,
        })
    }
}

fn validate_source(source: RawSourceConfig, config_parent: &Path) -> Result<SourceConfig> {
    match source {
        RawSourceConfig::File { path } => {
            let path = resolve_path(path, config_parent);
            if !path.exists() || !path.is_file() {
                bail!(
                    "file source does not exist or is not a file: {}",
                    path.display()
                );
            }
            Ok(SourceConfig::File { path })
        }
        RawSourceConfig::Directory {
            path,
            recursive,
            extensions,
        } => {
            let path = resolve_path(path, config_parent);
            if !path.exists() || !path.is_dir() {
                bail!(
                    "directory source does not exist or is not a directory: {}",
                    path.display()
                );
            }
            let extensions = extensions.map(|values| {
                values
                    .into_iter()
                    .map(|x| x.trim().trim_start_matches('.').to_ascii_lowercase())
                    .filter(|x| !x.is_empty())
                    .collect::<Vec<_>>()
            });
            Ok(SourceConfig::Directory {
                path,
                recursive,
                extensions,
            })
        }
        RawSourceConfig::Rss {
            url,
            max_items,
            download_dir,
        } => {
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                bail!("rss url must start with http:// or https://: {url}");
            }
            let download_dir = download_dir.map(|p| resolve_path(p, config_parent));
            Ok(SourceConfig::Rss {
                url,
                max_items: max_items.unwrap_or(200),
                download_dir,
            })
        }
    }
}

fn resolve_path(path: PathBuf, config_parent: &Path) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        config_parent.join(path)
    }
}

fn hcl_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parses_valid_config() {
        let tmp = tempdir().unwrap();
        let img = tmp.path().join("a.jpg");
        let dir = tmp.path().join("imgs");
        fs::write(&img, b"not-an-image").unwrap();
        fs::create_dir_all(&dir).unwrap();

        let raw = format!(
            r#"
timer = 15
remoteUpdateTimer = 600
sources = [
  {{ type = "file", path = "{}" }},
  {{ type = "directory", path = "{}", recursive = false }},
  {{ type = "rss", url = "https://example.com/feed.xml", max_items = 20 }}
]
"#,
            hcl_path(&img),
            hcl_path(&dir)
        );

        let cfg = parse_from_str(&raw, &tmp.path().join("bgm.hcl")).unwrap();
        assert_eq!(cfg.timer.as_secs(), 15);
        assert_eq!(cfg.remote_update_timer.as_secs(), 600);
        assert_eq!(cfg.sources.len(), 3);
    }

    #[test]
    fn rejects_tiny_timer() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("imgs");
        fs::create_dir_all(&dir).unwrap();

        let raw = format!(
            r#"
timer = 2
sources = [ {{ type = "directory", path = "{}" }} ]
"#,
            hcl_path(&dir)
        );

        assert!(parse_from_str(&raw, &tmp.path().join("bgm.hcl")).is_err());
    }

    #[test]
    fn generated_default_hcl_parses() {
        let tmp = tempdir().unwrap();
        let pictures = tmp.path().join("Pictures");
        fs::create_dir_all(&pictures).unwrap();

        let raw = default_hcl(&pictures);
        let cfg = parse_from_str(&raw, &tmp.path().join("bgm.hcl")).unwrap();
        assert_eq!(cfg.timer.as_secs(), 300);
        assert_eq!(cfg.remote_update_timer.as_secs(), 3600);
        assert_eq!(cfg.sources.len(), 1);

        match &cfg.sources[0] {
            SourceConfig::Directory {
                path, recursive, ..
            } => {
                assert_eq!(path, &pictures);
                assert!(*recursive);
            }
            _ => panic!("expected directory source in generated config"),
        }
    }
}
