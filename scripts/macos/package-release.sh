#!/usr/bin/env bash

set -euo pipefail

usage() {
  printf '%s\n' 'Usage: scripts/macos/package-release.sh --version VERSION --binary PATH [--output DIR]'
}

version=""
binary=""
output="release-assets"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) version="${2:-}"; shift 2 ;;
    --binary) binary="${2:-}"; shift 2 ;;
    --output) output="${2:-}"; shift 2 ;;
    --help|-h) usage; exit 0 ;;
    *) printf 'Unknown argument: %s\n' "$1" >&2; usage >&2; exit 1 ;;
  esac
done

[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { printf 'Invalid version: %s\n' "$version" >&2; exit 1; }
[[ -f "$binary" && -x "$binary" ]] || { printf 'Binary is not executable: %s\n' "$binary" >&2; exit 1; }
[[ "$(uname -m)" == "arm64" ]] || { printf '%s\n' 'macOS packages must be built natively on Apple Silicon.' >&2; exit 1; }
[[ "$(lipo -archs "$binary")" == "arm64" ]] || { printf '%s\n' 'Aura executable is not arm64-only.' >&2; exit 1; }
minos="$(otool -l "$binary" | awk '/LC_BUILD_VERSION/{found=1; next} found && /minos/{print $2; exit}')"
[[ "$minos" == "14.0" ]] || { printf 'Unexpected deployment target: %s (expected 14.0)\n' "$minos" >&2; exit 1; }

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
output="$(mkdir -p -- "$output" && cd -- "$output" && pwd)"
stage="$(mktemp -d "${TMPDIR:-/tmp}/aura-macos.XXXXXX")"
mount_point=""
cleanup() {
  if [[ -n "$mount_point" && -d "$mount_point" ]]; then
    hdiutil detach "$mount_point" -quiet || true
  fi
  rm -rf -- "$stage"
}
trap cleanup EXIT

app="$stage/Aura.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
install -m 0755 "$binary" "$app/Contents/MacOS/aura"
sed "s/@VERSION@/$version/g" "$repo_root/packaging/macos/Info.plist.in" > "$app/Contents/Info.plist"
"$app/Contents/MacOS/aura" --version | grep -F "Version $version" >/dev/null

iconset="$stage/Aura.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$repo_root/assets/tray.png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$repo_root/assets/tray.png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/Aura.icns"

plutil -lint "$app/Contents/Info.plist"
[[ "$(plutil -extract CFBundleIdentifier raw "$app/Contents/Info.plist")" == "io.github.hmerritt.Aura" ]]
[[ "$(plutil -extract LSMinimumSystemVersion raw "$app/Contents/Info.plist")" == "14.0" ]]
[[ "$(plutil -extract LSUIElement raw "$app/Contents/Info.plist")" == "true" ]]

codesign --force --deep --sign - --identifier io.github.hmerritt.Aura "$app"
codesign --verify --deep --strict --verbose=2 "$app"

tarball="$output/aura-macos-arm64.tar.gz"
COPYFILE_DISABLE=1 tar -C "$stage" -czf "$tarball" Aura.app
tar -tzf "$tarball" | grep -Fx 'Aura.app/Contents/MacOS/aura' >/dev/null

dmg="$output/aura-macos-arm64.dmg"
dmg_root="$stage/dmg-root"
mkdir -p "$dmg_root"
cp -a "$app" "$dmg_root/Aura.app"
hdiutil create -quiet -fs HFS+ -format UDZO -volname Aura -srcfolder "$dmg_root" "$dmg"
hdiutil verify "$dmg" >/dev/null
mount_point="$stage/mount"
mkdir -p "$mount_point"
hdiutil attach "$dmg" -quiet -nobrowse -readonly -mountpoint "$mount_point"
[[ -x "$mount_point/Aura.app/Contents/MacOS/aura" ]]
codesign --verify --deep --strict "$mount_point/Aura.app"
hdiutil detach "$mount_point" -quiet
mount_point=""

printf '%s\n%s\n' "$dmg" "$tarball"
