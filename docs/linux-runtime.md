# Linux desktop runtime

Aura supports GNOME 45+ and KDE Plasma 6+ on X11 and Wayland. The Linux build keeps the existing HCL configuration and CLI. Companion deployment and desktop package metadata are intentionally outside the runtime scope described here.

## Runtime architecture

The Rust process detects the desktop from `XDG_CURRENT_DESKTOP`, records the display protocol from `XDG_SESSION_TYPE`, and verifies the active shell on the session D-Bus. It then acquires `io.github.hmerritt.Aura`, including when `--no-tray` is used, and exports `io.github.hmerritt.Aura1` at `/io/github/hmerritt/Aura`.

The version 1 JSON snapshot is the sole source of truth for the shell companions. It contains the current image or shader generation, canonical image URI, shader controls, desktop/session diagnostics, lease deadline, and live session statistics. D-Bus and tray actions are translated into the application's Tokio event channel.

GNOME renders an image actor or `Shell.GLSLEffect` in the Shell background group and provides native panel controls. Plasma uses the `io.github.hmerritt.Aura` QML wallpaper and a StatusNotifierItem. Virtual shader scope shares one coordinate space across all monitors; primary scope leaves the last image visible on other monitors.

Plasma shader snapshots carry a renewable 15-second lease. If Aura disappears, the wallpaper falls back to its last image after expiry. GNOME removes Aura's actors when the D-Bus owner disappears. A normal renderer shutdown returns both companions to the last image, or to the wallpaper that was active before Aura when no image exists.

## Shader build

Each shader lives in `shaders/cores` and implements:

```glsl
vec4 aura_main(vec2 fragCoord, AuraUniforms uniforms);
```

The stable Rust build validates every core with Naga and emits Vulkan SPIR-V for the Windows wgpu renderer. Linux builds also emit GNOME snippets and Qt 6 vertex/fragment wrappers, then run the target system's `qsb` to produce the embedded Plasma assets. Set `QSB` only when `qsb` is not on a standard Qt 6 path.

The contract carries time, frame count, mouse state, internal resolution, and mouse coordinates. Shell wrappers enforce configured resolution scaling and explicit linear-to-sRGB conversion.

## Diagnostics and failure behavior

Missing or unsupported desktops, session buses, shells, and companions are startup errors with actionable messages. Renderer acknowledgements are generation checked. A shader error switches to image mode; a Linux image-application error is fatal rather than silently leaving the desktop unchanged.

Fatal Rust errors and panics are written with backtraces and shown through freedesktop notifications. Fatal Linux signals append a minimal record to the Aura state directory before restoring the default handler and re-raising, preserving normal core-dump handling.

## Development validation

Linux CI installs Qt Shader Tools only as a build dependency, validates both companions, builds all shader forms, runs the Rust suite, and exercises D-Bus ownership, methods, signals, reconnection, and single-instance rejection on a private session bus.

The compositor acceptance matrix is GNOME and Plasma, X11 and Wayland, each with single- and dual-monitor layouts. It covers local/file/RSS rotation, state/cache/timers, reload and manual-next controls, image/shader transitions, monitor changes, shell restarts, renderer failures, normal shutdown, and crash fallback.
