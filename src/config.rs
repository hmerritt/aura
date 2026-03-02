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
const DEFAULT_SHADER_TARGET_FPS: u16 = 60;
const DEFAULT_SHADER_NAME: &str = "gradient_glossy";
const LEGACY_SHADER_NAME: &str = "gradient_shader";
const DEFAULT_SHADER_QUALITY: ShaderQualityPreset = ShaderQualityPreset::Medium;
const DEFAULT_MAX_CACHE_MB: u64 = 1024;
const DEFAULT_MAX_CACHE_AGE_DAYS: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RendererMode {
    Image,
    Shader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShaderPowerPreference {
    LowPower,
    HighPerformance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShaderDesktopScope {
    Virtual,
    Primary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShaderQualityPreset {
    Vlow,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShaderQualitySettings {
    pub memory_target_mb: u64,
    pub power_preference: ShaderPowerPreference,
    pub max_frame_latency: u8,
}

impl ShaderQualityPreset {
    pub fn settings(self) -> ShaderQualitySettings {
        match self {
            Self::Vlow => ShaderQualitySettings {
                memory_target_mb: 48,
                power_preference: ShaderPowerPreference::LowPower,
                max_frame_latency: 1,
            },
            Self::Low => ShaderQualitySettings {
                memory_target_mb: 80,
                power_preference: ShaderPowerPreference::LowPower,
                max_frame_latency: 1,
            },
            Self::Medium => ShaderQualitySettings {
                memory_target_mb: 100,
                power_preference: ShaderPowerPreference::LowPower,
                max_frame_latency: 2,
            },
            Self::High => ShaderQualitySettings {
                memory_target_mb: 150,
                power_preference: ShaderPowerPreference::HighPerformance,
                max_frame_latency: 3,
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RawShaderConfig {
    name: Option<String>,
    target_fps: Option<u16>,
    mouse_enabled: Option<bool>,
    quality: Option<ShaderQualityPreset>,
    desktop_scope: Option<ShaderDesktopScope>,
    #[serde(rename = "memory_target_mb")]
    _memory_target_mb: Option<hcl::Value>,
    #[serde(rename = "power_preference")]
    _power_preference: Option<hcl::Value>,
    #[serde(rename = "max_frame_latency")]
    _max_frame_latency: Option<hcl::Value>,
    #[serde(rename = "crate_path")]
    _crate_path: Option<PathBuf>,
    #[serde(rename = "hot_reload")]
    _hot_reload: Option<bool>,
    #[serde(rename = "reload_debounce_ms")]
    _reload_debounce_ms: Option<u64>,
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
#[serde(untagged)]
enum DurationInput {
    Seconds(u64),
    Text(String),
}

#[derive(Debug, Clone, Deserialize)]
struct RawConfig {
    timer: Option<DurationInput>,
    #[serde(rename = "remoteUpdateTimer")]
    remote_update_timer: Option<DurationInput>,
    sources: Vec<RawSourceConfig>,
    cache_dir: Option<PathBuf>,
    state_file: Option<PathBuf>,
    log_level: Option<String>,
    image_format: Option<OutputFormat>,
    jpeg_quality: Option<u8>,
    max_cache_mb: Option<u64>,
    max_cache_age_days: Option<u64>,
    renderer: Option<RendererMode>,
    shader: Option<RawShaderConfig>,
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
pub struct AuraConfig {
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
    pub renderer: RendererMode,
    pub shader: Option<ShaderConfig>,
}

#[derive(Debug, Clone)]
pub struct ShaderConfig {
    pub name: String,
    pub target_fps: u16,
    pub mouse_enabled: bool,
    pub quality: ShaderQualityPreset,
    pub desktop_scope: ShaderDesktopScope,
}

pub fn load_from_path(path: &Path) -> Result<AuraConfig> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read config from {}", path.display()))?;
    parse_from_str(&content, path)
}

pub fn parse_from_str(content: &str, path: &Path) -> Result<AuraConfig> {
    let raw: RawConfig =
        hcl::from_str(content).with_context(|| format!("invalid HCL in {}", path.display()))?;
    AuraConfig::from_raw(raw, path)
}

pub fn default_hcl(pictures_dir: &Path) -> String {
    let pictures = hcl_path(pictures_dir);
    format!(
        r#"# aura (Background Manager) configuration file

# Image sources array. Multiple sources will be combined together to pick the next wallpaper from.
# Supported source types: "file" | "directory" | "rss"
sources = [
  {{ type = "directory", path = "{}", recursive = true, extensions = ["jpg", "jpeg", "png", "webp", "bmp", "gif"] }}
]

# Duration for switching to a new wallpaper: "40s" | "12m" | "3h"
timer = "3h"

# Target image format for wallpapers. All source images will be converted to this format before being set as wallpaper: "jpg" | "png"
image_format = "jpg"
# Quality for JPEG output (ignored for other formats): 1-100
jpeg_quality = 90

# Duration for checking remote sources for new images: "40s" | "12m" | "3h"
remoteUpdateTimer = "2h"

# Log level: "error" | "warn" | "info" | "debug" | "trace"
log_level = "warn"

# Runtime renderer mode: "image" | "shader"
renderer = "image"

# Shader mode options (used when renderer = "shader")
shader = {{
	name = "gradient_glossy" # "gradient_glossy" | "limestone_cave" | "dither_asci_1" | "dither_asci_2"
	target_fps = 50
	mouse_enabled = false
	quality = "low" # "vlow" | "low" | "medium" | "high"
	desktop_scope = "virtual" # "virtual" | "primary"
}}
"#,
        pictures
    )
}

impl AuraConfig {
    fn from_raw(raw: RawConfig, config_path: &Path) -> Result<Self> {
        let config_parent = config_path.parent().unwrap_or_else(|| Path::new("."));
        let app_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("aura");

        let timer_secs = parse_duration_field("timer", raw.timer, DEFAULT_TIMER_SECS)?;
        if timer_secs < MIN_TIMER_SECS {
            bail!("timer must be at least {MIN_TIMER_SECS} seconds");
        }

        let remote_secs = parse_duration_field(
            "remoteUpdateTimer",
            raw.remote_update_timer,
            DEFAULT_REMOTE_UPDATE_TIMER_SECS,
        )?;
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
        let renderer = raw.renderer.unwrap_or(RendererMode::Image);
        let shader = parse_shader_config(raw.shader, renderer)?;

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
            renderer,
            shader,
        })
    }
}

fn parse_shader_config(
    raw: Option<RawShaderConfig>,
    renderer: RendererMode,
) -> Result<Option<ShaderConfig>> {
    let Some(raw) = raw else {
        if renderer == RendererMode::Shader {
            return Ok(Some(ShaderConfig {
                name: DEFAULT_SHADER_NAME.to_string(),
                target_fps: DEFAULT_SHADER_TARGET_FPS,
                mouse_enabled: false,
                quality: DEFAULT_SHADER_QUALITY,
                desktop_scope: ShaderDesktopScope::Virtual,
            }));
        }
        return Ok(None);
    };

    let target_fps = raw.target_fps.unwrap_or(DEFAULT_SHADER_TARGET_FPS);
    if target_fps == 0 || target_fps > 240 {
        bail!("shader.target_fps must be between 1 and 240");
    }
    let name = raw
        .name
        .as_deref()
        .unwrap_or(DEFAULT_SHADER_NAME)
        .trim()
        .to_string();
    if name.is_empty() {
        bail!("shader.name must not be empty");
    }
    let name = if name == LEGACY_SHADER_NAME {
        DEFAULT_SHADER_NAME.to_string()
    } else {
        name
    };

    Ok(Some(ShaderConfig {
        name,
        target_fps,
        mouse_enabled: raw.mouse_enabled.unwrap_or(false),
        quality: raw.quality.unwrap_or(DEFAULT_SHADER_QUALITY),
        desktop_scope: raw.desktop_scope.unwrap_or(ShaderDesktopScope::Virtual),
    }))
}

fn parse_duration_field(
    field_name: &str,
    value: Option<DurationInput>,
    default_secs: u64,
) -> Result<u64> {
    match value {
        Some(DurationInput::Seconds(secs)) => Ok(secs),
        Some(DurationInput::Text(raw)) => parse_duration_string(field_name, &raw),
        None => Ok(default_secs),
    }
}

fn parse_duration_string(field_name: &str, raw: &str) -> Result<u64> {
    let trimmed = raw.trim();
    let Some((unit_pos, unit_char)) = trimmed
        .char_indices()
        .rev()
        .find(|(_, ch)| !ch.is_whitespace())
    else {
        bail!(
            "{field_name} must be a positive integer or duration string like \"40s\", \"12m\", \"3h\""
        );
    };

    let unit = unit_char.to_ascii_lowercase();
    let multiplier = match unit {
        's' => 1_u64,
        'm' => 60_u64,
        'h' => 3600_u64,
        _ => {
            bail!(
                "{field_name} has invalid duration unit \"{unit_char}\"; expected one of: s, m, h"
            )
        }
    };

    let number_str = trimmed[..unit_pos].trim();
    if number_str.is_empty() || !number_str.chars().all(|ch| ch.is_ascii_digit()) {
        bail!(
            "{field_name} must be a positive integer followed by s/m/h, for example \"40s\", \"12m\", \"3h\""
        );
    }

    let value = number_str.parse::<u64>().with_context(|| {
        format!("{field_name} must contain a valid integer before its unit; got \"{number_str}\"")
    })?;

    value.checked_mul(multiplier).with_context(|| {
        format!("{field_name} duration is too large to represent in seconds: \"{trimmed}\"")
    })
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

        let cfg = parse_from_str(&raw, &tmp.path().join("aura.hcl")).unwrap();
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

        assert!(parse_from_str(&raw, &tmp.path().join("aura.hcl")).is_err());
    }

    #[test]
    fn generated_default_hcl_parses() {
        let tmp = tempdir().unwrap();
        let pictures = tmp.path().join("Pictures");
        fs::create_dir_all(&pictures).unwrap();

        let raw = default_hcl(&pictures);
        assert!(raw.contains("name = \"gradient_glossy\""));
        assert!(raw.contains("quality = \"medium\""));
        let cfg = parse_from_str(&raw, &tmp.path().join("aura.hcl")).unwrap();
        // `default_hcl` uses explicit template durations (3h / 2h), not parser fallback defaults.
        assert_eq!(cfg.timer.as_secs(), 10_800);
        assert_eq!(cfg.remote_update_timer.as_secs(), 7_200);
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

    #[test]
    fn deprecated_shader_fields_are_ignored() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("imgs");
        fs::create_dir_all(&dir).unwrap();

        let raw = format!(
            r#"
renderer = "shader"
sources = [ {{ type = "directory", path = "{}" }} ]
        shader = {{
  name = "gradient_glossy"
  memory_target_mb = 9999
  power_preference = "not_a_real_mode"
  max_frame_latency = 99
  crate_path = "shaders/legacy"
  hot_reload = true
  reload_debounce_ms = 500
  target_fps = 75
  mouse_enabled = true
}}
"#,
            hcl_path(&dir)
        );

        let cfg = parse_from_str(&raw, &tmp.path().join("aura.hcl")).unwrap();
        let shader = cfg.shader.expect("shader config should exist");
        assert_eq!(shader.name, "gradient_glossy");
        assert_eq!(shader.target_fps, 75);
        assert!(shader.mouse_enabled);
        assert_eq!(shader.quality, DEFAULT_SHADER_QUALITY);
        assert_eq!(shader.desktop_scope, ShaderDesktopScope::Virtual);
    }

    #[test]
    fn legacy_shader_name_alias_maps_to_gradient_glossy() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("imgs");
        fs::create_dir_all(&dir).unwrap();

        let raw = format!(
            r#"
renderer = "shader"
sources = [ {{ type = "directory", path = "{}" }} ]
shader = {{
  name = "gradient_shader"
}}
"#,
            hcl_path(&dir)
        );

        let cfg = parse_from_str(&raw, &tmp.path().join("aura.hcl")).unwrap();
        let shader = cfg.shader.expect("shader config should exist");
        assert_eq!(shader.name, "gradient_glossy");
        assert_eq!(shader.quality, DEFAULT_SHADER_QUALITY);
        assert_eq!(shader.desktop_scope, ShaderDesktopScope::Virtual);
    }

    #[test]
    fn parses_shader_quality_and_desktop_scope() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("imgs");
        fs::create_dir_all(&dir).unwrap();

        let raw = format!(
            r#"
renderer = "shader"
sources = [ {{ type = "directory", path = "{}" }} ]
shader = {{
  name = "gradient_glossy"
  quality = "high"
  desktop_scope = "primary"
}}
"#,
            hcl_path(&dir)
        );

        let cfg = parse_from_str(&raw, &tmp.path().join("aura.hcl")).unwrap();
        let shader = cfg.shader.expect("shader config should exist");
        assert_eq!(shader.quality, ShaderQualityPreset::High);
        assert_eq!(shader.desktop_scope, ShaderDesktopScope::Primary);
    }

    #[test]
    fn rejects_invalid_shader_quality_options() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("imgs");
        fs::create_dir_all(&dir).unwrap();

        let raw = format!(
            r#"
renderer = "shader"
sources = [ {{ type = "directory", path = "{}" }} ]
shader = {{
  quality = "ultra"
}}
"#,
            hcl_path(&dir)
        );
        assert!(parse_from_str(&raw, &tmp.path().join("aura.hcl")).is_err());
    }

    #[test]
    fn shader_quality_settings_match_expected_values() {
        let vlow = ShaderQualityPreset::Vlow.settings();
        assert_eq!(vlow.memory_target_mb, 48);
        assert_eq!(vlow.power_preference, ShaderPowerPreference::LowPower);
        assert_eq!(vlow.max_frame_latency, 1);

        let low = ShaderQualityPreset::Low.settings();
        assert_eq!(low.memory_target_mb, 80);
        assert_eq!(low.power_preference, ShaderPowerPreference::LowPower);
        assert_eq!(low.max_frame_latency, 1);

        let medium = ShaderQualityPreset::Medium.settings();
        assert_eq!(medium.memory_target_mb, 100);
        assert_eq!(medium.power_preference, ShaderPowerPreference::LowPower);
        assert_eq!(medium.max_frame_latency, 2);

        let high = ShaderQualityPreset::High.settings();
        assert_eq!(high.memory_target_mb, 150);
        assert_eq!(
            high.power_preference,
            ShaderPowerPreference::HighPerformance
        );
        assert_eq!(high.max_frame_latency, 3);
    }

    #[test]
    fn parses_duration_strings_for_timer_fields() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("imgs");
        fs::create_dir_all(&dir).unwrap();

        let raw = format!(
            r#"
timer = "40s"
remoteUpdateTimer = "12m"
sources = [ {{ type = "directory", path = "{}" }} ]
"#,
            hcl_path(&dir)
        );

        let cfg = parse_from_str(&raw, &tmp.path().join("aura.hcl")).unwrap();
        assert_eq!(cfg.timer.as_secs(), 40);
        assert_eq!(cfg.remote_update_timer.as_secs(), 720);
    }

    #[test]
    fn parses_case_insensitive_duration_strings_with_spaces() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("imgs");
        fs::create_dir_all(&dir).unwrap();

        let raw = format!(
            r#"
timer = "3 H"
remoteUpdateTimer = "40 S"
sources = [ {{ type = "directory", path = "{}" }} ]
"#,
            hcl_path(&dir)
        );

        let cfg = parse_from_str(&raw, &tmp.path().join("aura.hcl")).unwrap();
        assert_eq!(cfg.timer.as_secs(), 10_800);
        assert_eq!(cfg.remote_update_timer.as_secs(), 40);
    }

    #[test]
    fn rejects_invalid_duration_string_formats() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("imgs");
        fs::create_dir_all(&dir).unwrap();

        for timer in [
            "\"40\"",
            "\"1d\"",
            "\"abc\"",
            "\"-5m\"",
            "\"1.5h\"",
            "\"1h30m\"",
        ] {
            let raw = format!(
                r#"
timer = {}
sources = [ {{ type = "directory", path = "{}" }} ]
"#,
                timer,
                hcl_path(&dir)
            );
            assert!(
                parse_from_str(&raw, &tmp.path().join("aura.hcl")).is_err(),
                "expected timer={} to fail",
                timer
            );
        }
    }

    #[test]
    fn applies_minimums_after_duration_parsing() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("imgs");
        fs::create_dir_all(&dir).unwrap();

        let tiny_timer = format!(
            r#"
timer = "2s"
sources = [ {{ type = "directory", path = "{}" }} ]
"#,
            hcl_path(&dir)
        );
        assert!(parse_from_str(&tiny_timer, &tmp.path().join("aura.hcl")).is_err());

        let tiny_remote = format!(
            r#"
timer = "10s"
remoteUpdateTimer = "20s"
sources = [ {{ type = "directory", path = "{}" }} ]
"#,
            hcl_path(&dir)
        );
        assert!(parse_from_str(&tiny_remote, &tmp.path().join("aura.hcl")).is_err());
    }

    #[test]
    fn rejects_overflowing_duration_values() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("imgs");
        fs::create_dir_all(&dir).unwrap();

        let huge = format!("\"{}h\"", u64::MAX);
        let raw = format!(
            r#"
timer = {}
sources = [ {{ type = "directory", path = "{}" }} ]
"#,
            huge,
            hcl_path(&dir)
        );

        assert!(parse_from_str(&raw, &tmp.path().join("aura.hcl")).is_err());
    }
}
