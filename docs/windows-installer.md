# Windows Installer (Squirrel)

`aura` uses `Squirrel.Windows` for per-user installation and update packaging.

## Install Scope

- Installs into user space under `%LOCALAPPDATA%`.
- No administrator rights are required.

## Startup Behavior

- Installer registers app startup using Squirrel shortcut management.
- Startup is always enabled for the current user.
- Startup and Start Menu shortcuts are created on install/update and removed on uninstall.

## Runtime Lifecycle Flags

Squirrel may launch the app with one of these internal flags:

- `--squirrel-install`
- `--squirrel-updated`
- `--squirrel-uninstall`
- `--squirrel-obsolete`
- `--squirrel-firstrun`

`aura` handles these flags at startup before normal runtime initialization.

## Build Artifacts

The release pipeline publishes both existing assets and Squirrel assets:

- `aura-<version>-windows-x86_64.exe`
- `aura-<version>-windows-x86_64.zip`
- `aura-<version>-setup.exe`
- `RELEASES`
- `*.nupkg` (full package and delta package when generated)

These Squirrel artifacts are retained intentionally for future in-app self-update support.
