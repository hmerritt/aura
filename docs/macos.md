# macOS support

Aura supports Apple Silicon (`aarch64-apple-darwin`) on macOS 14 Sonoma and
later. Intel, Universal 2, and Rosetta artifacts are intentionally not built.

## Installation

The Homebrew cask is the preferred installation and installs `Aura.app`. The
formula installs the same arm64 executable as a raw command and provides an
interactive per-user `brew services` LaunchAgent. The formula does not use
Aura's login-item registration because Homebrew owns its startup lifecycle.

Aura's release bundle is ad-hoc signed with the identifier
`io.github.hmerritt.Aura`. It has no Developer ID signature and is not
notarized. On first launch, open **System Settings → Privacy & Security** and
approve Aura if Gatekeeper blocks it. The cask caveat repeats this requirement.

The bundled app registers itself with `SMAppService.mainAppService`. A user can
deny or revoke this request. When approval is needed, the Aura menu reports
**Approval Required** and links directly to **General → Login Items**. Aura
checks this state after every launch so upgrades do not assume that an earlier
ad-hoc build retained authorization.

## Updates

Aura never performs a release check or in-place update on macOS. The menu bar
shows the appropriate command for the detected installation and copies it to
the clipboard:

```sh
brew upgrade --cask aura
# or, for the formula
brew upgrade aura
```

## Runtime behavior

Image wallpapers are applied through AppKit to every attached display using
proportional fill and clipping. Aura retains and periodically reapplies the
last image so a display or active-Space change cannot leave a blank desktop.

Live shaders use Metal through wgpu. Aura creates one borderless,
input-transparent desktop window per selected display. `desktop_scope =
"primary"` targets the primary display, while `"virtual"` maintains one
continuous shader scene across every display, including mixed monitor origins
and scale factors. Display topology is reconciled without restarting Aura. A
static wallpaper remains underneath the shader windows and is immediately
visible if the renderer closes or fails.

Configuration remains at `~/.config/aura.hcl`; the HCL schema, CLI flags,
sources, rotation, cache, and state behavior are shared with Windows and Linux.

## Release artifacts

Tagged builds publish exactly these macOS assets:

- `aura-macos-arm64.dmg`
- `aura-macos-arm64.tar.gz`

The packaging script creates `Aura.app`, converts the existing PNG icon to
ICNS, applies and verifies the ad-hoc signature, validates bundle metadata and
architecture, mounts and checks the DMG, and verifies the tarball payload. The
templates under `brew/` contain version, URL, and SHA-256 placeholders; release
automation does not publish or modify a tap.
