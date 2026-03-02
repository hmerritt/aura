# Background Image Manager: `bgm`

A simple, lightweight, background image manager written in Rust.

## ⚡ Features

- Small in size, low memory footprint
- Caches remote images locally for faster switching
- Automatically re-encodes images for wider format support: `jpeg` | `png` | `bmp` | `gif` | `webp`
- Tray icon to trigger a new image quickly
- Multiple image `sources` can be added
    - Single image path
    - Directory path
    - RSS feed

## Example `bgm.hcl`

Default location is `~/.config/bgm.hcl`

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

```

## Development

`bgm` can be developed and tested on Windows, Linux, and macOS. Full wallpaper application and tray behavior are implemented for Windows.

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

# Run with default config path (~/.config/bgm.hcl)
cargo run --release

# Run without tray mode
cargo run --release -- --no-tray

# Run with an explicit config path
cargo run --release -- /path/to/bgm.hcl

# Print version information
cargo run --release -- --version
```

### Platform Notes

- Windows: tray and wallpaper update flow are supported.
- Linux/macOS: check/test/build are supported for development; wallpaper apply is currently unsupported at runtime.

### Default Config Location

- If no config path is provided, `bgm` uses `~/.config/bgm.hcl`.
- On first run, if the file is missing, `bgm` creates it with recommended defaults.
- The default source is your Pictures directory.

### Current Implementation

- Windows-first wallpaper backend (`SystemParametersInfoW`)
- Forces Windows wallpaper style to `Fill` on apply
- Windows tray icon (enabled by default)
    - Double-click tray icon: switch to next wallpaper immediately
    - Right-click tray icon: shows stats (`Timer`, `Remote Update`, `Images`, `Shown`, `Skipped`, `Running`) and menu items `Next Background`, `Settings`, and `Exit`
    - `Images` counts unique merged candidates across all sources, and `Shown` counts images applied in the current session
    - `Next Background` switches immediately, `Settings` opens the active `bgm.hcl`, and a separator appears above `Exit`
    - `Running` is minute-precision (`<1m` when under a minute) and shows days once runtime exceeds 72 hours (example: `3d 21h 49m`)
    - Uses embedded tray/menu icons generated from `assets/tray.png`, `assets/menu-next-background.png`, `assets/menu-settings.png`, and `assets/menu-exit.png` (menu icons fall back to embedded icon resources if bitmap loading fails)
- No-repeat shuffle rotation cycle
- Local and remote image cache
- Zero-open passthrough for matching `image_format` (`jpg`/`jpeg` alias supported)
- Conversion-only image pipeline for format mismatches
- Persisted runtime state across restarts
