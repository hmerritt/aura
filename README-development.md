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

## Development

`aura` can be developed and tested on Windows, Linux, and macOS. Full wallpaper and tray behavior are implemented for Windows, GNOME 45+, and KDE Plasma 6+.

### Prerequisites

- Rust stable toolchain (`rustup`, `cargo`)
- Rust nightly `nightly-2026-05-22-x86_64-pc-windows-msvc`
- Windows development: MSVC toolchain/Visual Studio Build Tools (C++ build tools)
- Linux: standard native build tools, D-Bus, Node.js, Qt 6 declarative tools, and Qt Shader Baker (`qsb`)
- macOS: standard native build tools (`clang` and linker)

```sh
rustup toolchain install nightly-2026-05-22-x86_64-pc-windows-msvc
```

```sh
rustup component add rustc-dev --toolchain nightly-2026-05-22-x86_64-pc-windows-msvc
```

```sh
rustup component add rust-src --toolchain nightly-2026-05-22-x86_64-pc-windows-msvc
```

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

# Run with debug logging enabled (`--debug`)
cargo run --release -- --debug

# Print version information
cargo run --release -- --version

# Build Squirrel installer/update artifacts
pwsh -File scripts/windows/package-squirrel.ps1 -Version 1.2.3

# Build with an explicit pinned Squirrel.Windows tool version
pwsh -File scripts/windows/package-squirrel.ps1 -Version 1.2.3 -SquirrelWindowsVersion 2.0.1

# Validate the Linux installer and desktop companions
shellcheck install.sh scripts/linux/*.sh
dash -n install.sh
python3 scripts/linux/validate-companions.py
bash scripts/linux/test-installer.sh

# Build a fixed-name Linux release package
bash scripts/linux/check-glibc.sh target/release/aura
bash scripts/linux/package-release.sh --version 1.2.3 --arch x86_64 --binary target/release/aura --output dist

# Generate the release manifest after both architecture archives exist
bash scripts/linux/generate-manifest.sh 1.2.3 dist dist/aura-linux-manifest
```

### Platform Notes

- Windows: tray and wallpaper update flow are supported.
- Windows launch behavior:
    - Default launch uses the GUI subsystem and does not open a terminal window.
    - `--debug` writes and appends all runtime output to `%LOCALAPPDATA%\aura\aura-debug.log` (file-first diagnostics, no console required).
    - On native crashes in `--debug`, Aura overwrites `%LOCALAPPDATA%\aura\aura-crash.dmp` and `%LOCALAPPDATA%\aura\aura-crash.txt` with the latest crash details.
- Windows installer packaging uses `Squirrel.Windows` in per-user scope (`%LOCALAPPDATA%`) and supports startup registration.
- Windows Squirrel installs automatically check/download app updates in the background and expose `Check for Updates` in tray.
- Installer details: `docs/windows-installer.md`
- Windows shader mode: shaders are compiled at build time from `shaders/*` (excluding `shader_builder`) using rust-gpu.
- Linux managed installs automatically check/download verified tarball updates and expose `Check for Updates` in both desktop menus.
- Linux release and installer details: `docs/linux-installer.md`
- Linux runtime details: `docs/linux-runtime.md`
- macOS: check/test/build are supported for development; wallpaper apply is currently unsupported at runtime.

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
- Zero-open passthrough for matching `image.format` (`jpg`/`jpeg` alias supported)
- Conversion-only image pipeline for format mismatches
- Persisted runtime state across restarts

---

<small>
    <a href="https://www.flaticon.com/free-icons/gallery" title="gallery icons">Gallery icons created by Freepik - Flaticon</a>
</small>
