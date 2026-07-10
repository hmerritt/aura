<p align="center">
  <img src="./assets/tray.png" alt="Aura app icon" width="96" height="96">
</p>

<h1 align="center">aura</h1>

<p align="center">
  <strong>A simple, lightweight, wallpaper manager written in Rust.</strong>
  <br />
  <br />
Includes an optional <code>shader</code> mode, which engages a GPU-accelerated shader renderer, that renders live shaders as your desktop wallpaper (this mode consumes more RAM, ~80-120MB, but is a lot of fun!).
</p>

<p align="center">
  <a href="https://github.com/hmerritt/aura/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/hmerritt/aura"></a>
  <a href="https://github.com/hmerritt/aura/releases/latest"><img alt="Downloads" src="https://img.shields.io/github/downloads/hmerritt/aura/total"></a>
  <a href="./LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-blue"></a>
</p>

<p align="center">
  <a href="#-install">💾 Install</a>
  |
  <a href="#-features">⚡ Features</a>
  |
  <a href="#-config">📝 Config</a>
</p>

https://github.com/user-attachments/assets/500d83a2-5c5e-4722-8a4e-b544f1f27784

## 💾 Install

[**➡️ Manually download the latest release here**](https://github.com/hmerritt/aura/releases/latest), or use the installer for your platform.

#### ➡️ Linux

```sh
curl -fsSL https://raw.githubusercontent.com/hmerritt/aura/master/install.sh | sh
```

#### ➡️ Windows via Scoop

```sh
scoop bucket add hmerritt https://github.com/hmerritt/scoop-bucket
scoop install hmerritt/aura
```

## ⚡ Features

- Small in size, low memory footprint (~3MB when in `image` mode)
- Multiple image `sources` can be added
    - Single image path
    - Directory path
    - RSS feed
- Caches remote images locally for faster switching
- Automatically re-encodes images for wider format support: `jpeg` | `png` | `bmp` | `gif` | `webp`
- Tray icon to trigger a new image quickly
- Built-in self-updater for seemless updates
- **Shader** mode which engages a GPU-accelerated shader renderer, allowing for live animated wallpapers

## 📝 Config

### Example `aura.hcl` config file

Default location is `~/.config/aura.hcl`

```hcl
# Runtime renderer mode: "image" | "shader"
renderer = "image"

# Image mode options (used when renderer = "image")
image = {
  # Image sources. Multiple sources will be combined together to pick the next wallpaper from.
  # Supported source types: "file" | "directory" | "rss"
  sources = [
    { type = "file", path = "C:/wallpapers/favorite.jpg" },
    { type = "directory", path = "C:/wallpapers/library", recursive = true, extensions = ["jpg", "png", "webp"] },
    { type = "rss", url = "https://example.com/feed.xml", max_items = 100 }
  ]
  timer = "3h"                     # "40s" | "12m" | "3h"
  remoteUpdateTimer = "2h"         # "40s" | "12m" | "3h"
  format = "jpg"                   # "jpg" | "png"
  jpeg_quality = 90                # 1-100
}

# Shader mode options (used when renderer = "shader")
shader = {
  name = "gradient_glossy"         # "gradient_glossy" | "limestone_cave" | "dither_asci_1" | "dither_asci_2" | "dither_warp" | "silk"
  target_fps = 60                  # 1-999
  resolution = 100                 # 1-100 (% internal shader render resolution; output stays full-screen)
  mouse_enabled = false            # false | true
  desktop_scope = "virtual"        # "virtual" | "primary"
  color_space = "unorm"            # "unorm" | "srgb"
}

# App update settings
updater = {
  enabled = true
  checkInterval = "6h"
  feedUrl = "https://github.com/hmerritt/aura/releases/latest/download"
}
```

---

<small>
    <a href="https://www.flaticon.com/free-icons/gallery" title="gallery icons">Gallery icons created by Freepik - Flaticon</a>
</small>
