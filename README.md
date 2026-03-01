# Background Image Manager: `bgm`

A simple, lightweight, background image manager written in Rust.

## ⚡ Features

- Small in size, low memory footprint
- Scales images to perfectly fit the screen (will crop if necessary)
- Tray icon to trigger a new image quickly
- Multiple image `sources` can be added
    - Single image path
    - Directory path
    - RSS feed

## Example `bgm.hcl`

```hcl
timer = 300
remoteUpdateTimer = 3600
image_format = "jpg"
jpeg_quality = 90
log_level = "info"

sources = [
  { type = "file", path = "C:/wallpapers/favorite.jpg" },
  { type = "directory", path = "C:/wallpapers/library", recursive = true, extensions = ["jpg", "png", "webp"] },
  { type = "rss", url = "https://example.com/feed.xml", max_items = 100 }
]
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
    - Right-click tray icon: open active `bgm.hcl`
    - Uses embedded icon generated from `assets/new-tray.png` (falls back to default if unavailable)
- No-repeat shuffle rotation cycle
- Local and remote image cache
- Cover resize + center crop image processing
- Persisted runtime state across restarts
