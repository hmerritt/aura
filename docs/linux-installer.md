# Linux installer and releases

Aura's Linux release is a no-`sudo`, per-user installation for x86_64 and aarch64 systems with glibc 2.35 or newer.

## Install

Install the latest published release:

```sh
curl -fsSL https://raw.githubusercontent.com/hmerritt/aura/master/install.sh | sh
```

Pin an exact numeric SemVer release:

```sh
curl -fsSL https://raw.githubusercontent.com/hmerritt/aura/master/install.sh | sh -s -- --version 1.2.3
```

The installer downloads `aura-linux-manifest`, validates its strict schema, verifies the selected archive's SHA-256, rejects unsafe archive entries, checks the complete payload and binary version, and only then switches the active release. It installs both GNOME and Plasma companions and enables login startup without launching Aura in the current session.

If GNOME's command-line extension tool cannot enable the companion in the current session, log out and back in, then enable `aura@hmerritt.github.io` in the Extensions application or run:

```sh
gnome-extensions enable aura@hmerritt.github.io
```

## Managed layout

Releases live below `${XDG_DATA_HOME:-$HOME/.local/share}/aura/app/versions`. The `app/current` symlink is switched atomically, and `$HOME/.local/bin/aura` is a stable link to `app/current/bin/aura`. Desktop integrations point through the same active bundle, so the executable, icon, templates, and both companions change together.

The generated launcher is installed in `${XDG_DATA_HOME:-$HOME/.local/share}/applications`; the absolute-path autostart entry is installed in `${XDG_CONFIG_HOME:-$HOME/.config}/autostart`. Aura refuses to replace an unrelated `$HOME/.local/bin/aura` file or symlink.

## Updates

Only a binary whose resolved executable and stable command link match the managed layout enables Linux self-updates. Source builds and manually copied binaries show updates as unsupported.

Managed builds validate the release manifest, compare numeric SemVer versions, and invoke the installed copy of `install.sh` with the same feed and expected version. If the feed changes during the update or any download, checksum, archive, payload, or version check fails, the existing `app/current` remains active. Published releases and their fixed-name assets are therefore treated as immutable.

Shader mode restarts through `$HOME/.local/bin/aura` as soon as installation finishes. Image mode keeps the current process running and restarts after the next wallpaper switch. A detached helper waits for the old process to exit before launching the new version.

## Uninstall

```sh
curl -fsSL https://raw.githubusercontent.com/hmerritt/aura/master/install.sh | sh -s -- --uninstall
```

Uninstall requests a clean Aura exit first so the prior wallpaper is restored. It then removes the managed binary link, release bundles, companions, launcher, autostart entry, default configuration, cache, state, crash logs, and runtime files. It never deletes wallpaper source files or arbitrary custom paths configured by the user, and it leaves an unrelated `$HOME/.local/bin/aura` untouched.

## Release artifacts

Each draft GitHub release contains fixed-name `aura-linux-x86_64.tar.gz`, `aura-linux-aarch64.tar.gz`, and `aura-linux-manifest` assets, plus `SHA256SUMS` and the Windows artifacts. The manifest is generated only after both native Ubuntu 22.04 builds are collected. Publishing the draft changes what `/releases/latest/download` resolves to.

For local packaging and validation commands, see [README-development.md](../README-development.md).
