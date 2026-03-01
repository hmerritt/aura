# bgm

Background manager.

A rust program, continually running, to manage the current OS background image.

## ⚡ Features

- Small in size, low memory footprint
- Always scales images to fit the screen (will crop if necessary)
- `bgm.hcl` — config file
  - `sources` — mixed image sources:
    - Single image path
    - Directory path
    - RSS feed
  - `timer` — image display duration before switching
  - `remoteUpdateTimer` — RSS refresh interval

## Current Implementation

- Windows-first wallpaper backend (`SystemParametersInfoW`)
- Forces Windows wallpaper style to `Fill` on apply
- Windows tray icon (enabled by default)
  - Double-click tray icon: switch to next wallpaper immediately
  - Right-click tray icon: open active `bgm.hcl`
- No-repeat shuffle rotation cycle
- Local and remote image cache
- Cover resize + center crop image processing
- Persisted runtime state across restarts

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

## Run

```powershell
cargo run --release
```

Opt out of tray mode:

```powershell
cargo run --release -- --no-tray
```

## Default Config Location

- If no config path is provided, `bgm` uses `~/.config/bgm.hcl`.
- On first run, if the file is missing, `bgm` creates it with recommended defaults.
- The default source is your Pictures directory.
