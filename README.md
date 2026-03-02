# `aura` — Wallpaper Manager

A simple, lightweight, wallpaper manager written in Rust.

## ⚡ Features

- Small in size, low memory footprint
- Support for live **_Shaders_** as wallpapers
- Caches remote images locally for faster switching
- Automatically re-encodes images for wider format support: `jpeg` | `png` | `bmp` | `gif` | `webp`
- Tray icon to trigger a new image quickly
- Optional live shader renderer mode (Windows)
- Multiple image `sources` can be added
    - Single image path
    - Directory path
    - RSS feed

## Example `aura.hcl`

Default location is `~/.config/aura.hcl`

```hcl
# Image sources array. Multiple sources will be combined together to pick the next wallpaper from.
# Supported source types: "file" | "directory" | "rss"
sources = [
  { type = "file", path = "C:/wallpapers/favorite.jpg" },
  { type = "directory", path = "C:/wallpapers/library", recursive = true, extensions = ["jpg", "png", "webp"] },
  { type = "rss", url = "https://example.com/feed.xml", max_items = 100 }
]

# Duration for switching to a new wallpaper: "40s" | "12m" | "3h"
timer = "45m"

# Target image format for wallpapers. All source images will be converted to this format before being set as wallpaper: "jpg" | "png"
image_format = "jpg"
# Quality for JPEG output (ignored for other formats): 1-100
jpeg_quality = 90

# Duration for checking remote sources for new images: "40s" | "12m" | "3h"
remoteUpdateTimer = "1h"

# Log level: "error" | "warn" | "info" | "debug" | "trace"
log_level = "info"

# Runtime renderer mode: "image" | "shader"
renderer = "image"

# Shader mode options (used when renderer = "shader")
shader = {{
	name = "gradient_glossy" # "gradient_glossy" | "limestone_cave" | "dither_asci_1" | "dither_asci_2"
	target_fps = 50
	mouse_enabled = false
	quality = "medium" # "vlow" | "low" | "medium" | "high"
	desktop_scope = "virtual" # "virtual" | "primary"
}}
```

## Development

`aura` can be developed and tested on Windows, Linux, and macOS. Full wallpaper application and tray behavior are implemented for Windows.

### Prerequisites

- Rust stable toolchain (`rustup`, `cargo`)
- Windows development: MSVC toolchain/Visual Studio Build Tools (C++ build tools)
- Linux/macOS: standard native build tools (`clang`/`gcc` and linker)

### Commands

Run commands from the repository root.

```bash
# Fast local validation
cargo check --all-targets

# Run tests
cargo test --locked --all-targets

# Build release binary
cargo build --release --locked

# Run with default config path (~/.config/aura.hcl)
cargo run --release

# Run without tray mode
cargo run --release -- --no-tray

# Run with an explicit config path
cargo run --release -- /path/to/aura.hcl

# Print version information
cargo run --release -- --version
```

### Platform Notes

- Windows: tray and wallpaper update flow are supported.
- Windows shader mode: shaders are compiled at build time from `shaders/*` (excluding `shader_builder`) using rust-gpu.
- Linux/macOS: check/test/build are supported for development; wallpaper apply is currently unsupported at runtime.

### Default Config Location

- If no config path is provided, `aura` uses `~/.config/aura.hcl`.
- On first run, if the file is missing, `aura` creates it with recommended defaults.
- The default source is your Pictures directory.

### Current Implementation

- Windows-first wallpaper backend (`SystemParametersInfoW`)
- Forces Windows wallpaper style to `Fill` on apply
- Windows tray icon (enabled by default)
    - Double-click tray icon: switch to next wallpaper immediately in image mode (no-op in shader mode)
    - Right-click tray icon: shows stats and control menu items
    - In image mode, stats are `Timer`, `Remote Update`, `Images`, `Shown`, `Skipped`, and `Running`
    - In shader mode, only `Running` is shown in stats
    - `Images` counts unique merged candidates across all sources, and `Shown` counts images applied in the current session
    - In image mode: `Next Background`, `Reload Settings`, `Settings`, `Exit`
    - In shader mode: `Reload Settings`, `Settings`, `Exit`
    - `Next Background` switches immediately, `Reload Settings` reloads `aura.hcl` into the running process, `Settings` opens the active `aura.hcl`, and a separator appears above `Exit`
    - `Running` is minute-precision (`<1m` when under a minute) and shows days once runtime exceeds 72 hours (example: `3d 21h 49m`)
    - Uses embedded tray/menu icons generated from `assets/tray.png`, `assets/menu-next-background.png`, `assets/menu-refresh.png`, `assets/menu-settings.png`, and `assets/menu-exit.png` (menu icons fall back to embedded icon resources if bitmap loading fails)
- No-repeat shuffle rotation cycle
- Local and remote image cache
- Zero-open passthrough for matching `image_format` (`jpg`/`jpeg` alias supported)
- Conversion-only image pipeline for format mismatches
- Persisted runtime state across restarts
