#!/usr/bin/env bash

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/linux/package-release.sh --version VERSION --arch ARCH --binary PATH [--output DIR]
EOF
}

version=""
arch=""
binary=""
output="dist"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) version="${2:-}"; shift 2 ;;
        --arch) arch="${2:-}"; shift 2 ;;
        --binary) binary="${2:-}"; shift 2 ;;
        --output) output="${2:-}"; shift 2 ;;
        --help|-h) usage; exit 0 ;;
        *) printf 'Unknown argument: %s\n' "$1" >&2; usage >&2; exit 1 ;;
    esac
done

[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { printf 'Invalid version: %s\n' "$version" >&2; exit 1; }
case "$arch" in
    x86_64|aarch64) ;;
    *) printf 'Unsupported architecture: %s\n' "$arch" >&2; exit 1 ;;
esac
[[ -f "$binary" && -x "$binary" ]] || { printf 'Binary is not executable: %s\n' "$binary" >&2; exit 1; }

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
output="$(mkdir -p -- "$output" && cd -- "$output" && pwd)"
stage="$(mktemp -d "${TMPDIR:-/tmp}/aura-package.XXXXXX")"
trap 'rm -rf -- "$stage"' EXIT

root="$stage/aura"
desktop_id="io.github.hmerritt.Aura"
gnome_uuid="aura@hmerritt.github.io"

mkdir -p \
    "$root/bin" \
    "$root/share/applications" \
    "$root/share/autostart" \
    "$root/share/icons/hicolor/512x512/apps" \
    "$root/share/gnome-shell/extensions" \
    "$root/share/plasma/wallpapers"

install -m 0755 "$binary" "$root/bin/aura"
install -m 0755 "$repo_root/install.sh" "$root/install.sh"
install -m 0644 "$repo_root/LICENSE" "$root/LICENSE"
install -m 0644 \
    "$repo_root/packaging/linux/$desktop_id.desktop.in" \
    "$root/share/applications/$desktop_id.desktop.in"
install -m 0644 \
    "$repo_root/packaging/linux/$desktop_id-autostart.desktop.in" \
    "$root/share/autostart/$desktop_id.desktop.in"
install -m 0644 \
    "$repo_root/assets/tray.png" \
    "$root/share/icons/hicolor/512x512/apps/$desktop_id.png"
cp -a \
    "$repo_root/integrations/linux/gnome" \
    "$root/share/gnome-shell/extensions/$gnome_uuid"
cp -a \
    "$repo_root/integrations/linux/plasma" \
    "$root/share/plasma/wallpapers/$desktop_id"

archive="$output/aura-linux-$arch.tar.gz"
epoch="${SOURCE_DATE_EPOCH:-$(date +%s)}"
tar \
    --sort=name \
    --mtime="@$epoch" \
    --owner=0 \
    --group=0 \
    --numeric-owner \
    -C "$stage" \
    -cf - aura | gzip -n -9 > "$archive"

printf '%s\n' "$archive"
