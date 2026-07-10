#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
temp="$(mktemp -d "${TMPDIR:-/tmp}/aura-installer-test.XXXXXX")"
server_pid=""
fake_pid=""

cleanup() {
    if [[ -n "$fake_pid" ]]; then
        kill "$fake_pid" 2>/dev/null || true
    fi
    if [[ -n "$server_pid" ]]; then
        kill "$server_pid" 2>/dev/null || true
    fi
    rm -rf -- "$temp"
}
trap cleanup EXIT HUP INT TERM

fail() {
    printf 'Installer test failed: %s\n' "$*" >&2
    exit 1
}

assert_exists() {
    [[ -e "$1" || -L "$1" ]] || fail "expected path to exist: $1"
}

assert_missing() {
    [[ ! -e "$1" && ! -L "$1" ]] || fail "expected path to be absent: $1"
}

make_binary() {
    local version=$1
    local destination=$2
    mkdir -p -- "$(dirname -- "$destination")"
    cat > "$destination.c" <<EOF
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

static void stop(int signal_number) {
    (void)signal_number;
    _exit(0);
}

int main(int argc, char **argv) {
    if (argc == 2 && strcmp(argv[1], "--version") == 0) {
        puts("aura [Version $version (test)]");
        return 0;
    }
    signal(SIGTERM, stop);
    signal(SIGINT, stop);
    while (1) pause();
}
EOF
    cc -O2 -o "$destination" "$destination.c"
}

write_manifest() {
    local directory=$1
    local version=$2
    local url=$3
    local sha=$4
    cat > "$directory/aura-linux-manifest" <<EOF
schema=1
version=$version
x86_64_url=$url
x86_64_sha256=$sha
aarch64_url=$url
aarch64_sha256=$sha
EOF
}

run_installer() {
    local test_home=$1
    local feed=$2
    shift 2
    HOME="$test_home" \
    XDG_DATA_HOME="$test_home/data" \
    XDG_CONFIG_HOME="$test_home/config" \
    XDG_STATE_HOME="$test_home/state" \
    XDG_RUNTIME_DIR="$test_home/runtime" \
    AURA_ALLOW_ROOT=1 \
    AURA_ARCH=x86_64 \
        dash "$repo_root/install.sh" --feed-url "$feed" "$@"
}

server_root="$temp/server"
mkdir -p \
    "$server_root/v1" \
    "$server_root/v2" \
    "$server_root/bad-sha" \
    "$server_root/malformed" \
    "$server_root/unsafe"
make_binary 1.1.0 "$temp/aura-1.1.0"
make_binary 1.1.1 "$temp/aura-1.1.1"

SOURCE_DATE_EPOCH=1700000000 bash "$repo_root/scripts/linux/package-release.sh" \
    --version 1.1.0 --arch x86_64 --binary "$temp/aura-1.1.0" --output "$server_root/v1" >/dev/null
SOURCE_DATE_EPOCH=1700000001 bash "$repo_root/scripts/linux/package-release.sh" \
    --version 1.1.1 --arch x86_64 --binary "$temp/aura-1.1.1" --output "$server_root/v2" >/dev/null

port_file="$temp/port"
python3 - "$server_root" "$port_file" <<'PY' &
import functools
import http.server
import os
import pathlib
import sys

root = sys.argv[1]
port_file = pathlib.Path(sys.argv[2])
class QuietHandler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, format, *args):
        pass

handler = functools.partial(QuietHandler, directory=root)
server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
port_file.write_text(str(server.server_address[1]), encoding="ascii")
server.serve_forever()
PY
server_pid=$!
for _ in {1..100}; do
    [[ -s "$port_file" ]] && break
    sleep 0.05
done
[[ -s "$port_file" ]] || fail "local release server did not start"
port=$(<"$port_file")
base="http://127.0.0.1:$port"

v1_sha=$(sha256sum "$server_root/v1/aura-linux-x86_64.tar.gz" | awk '{print $1}')
v2_sha=$(sha256sum "$server_root/v2/aura-linux-x86_64.tar.gz" | awk '{print $1}')
write_manifest "$server_root/v1" 1.1.0 "$base/v1/aura-linux-x86_64.tar.gz" "$v1_sha"
write_manifest "$server_root/v2" 1.1.1 "$base/v2/aura-linux-x86_64.tar.gz" "$v2_sha"
cp "$server_root/v1/aura-linux-x86_64.tar.gz" "$server_root/bad-sha/"
write_manifest "$server_root/bad-sha" 1.1.0 "$base/bad-sha/aura-linux-x86_64.tar.gz" \
    aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
printf 'schema=1\nversion=1.1.0\nunknown=value\n' > "$server_root/malformed/aura-linux-manifest"
python3 - "$server_root/unsafe/aura-linux-x86_64.tar.gz" <<'PY'
import io
import tarfile
import sys

with tarfile.open(sys.argv[1], "w:gz") as archive:
    payload = b"unsafe\n"
    entry = tarfile.TarInfo("../outside-aura")
    entry.size = len(payload)
    archive.addfile(entry, io.BytesIO(payload))
PY
unsafe_sha=$(sha256sum "$server_root/unsafe/aura-linux-x86_64.tar.gz" | awk '{print $1}')
write_manifest "$server_root/unsafe" 1.1.0 "$base/unsafe/aura-linux-x86_64.tar.gz" "$unsafe_sha"

home="$temp/home"
mkdir -p "$home/runtime" "$home/wallpapers"
printf 'keep me\n' > "$home/wallpapers/source.jpg"
run_installer "$home" "$base/v1" --version 1.1.0

app_root="$home/data/aura/app"
assert_exists "$app_root/current/bin/aura"
assert_exists "$home/.local/bin/aura"
assert_exists "$home/data/gnome-shell/extensions/aura@hmerritt.github.io/metadata.json"
assert_exists "$home/data/plasma/wallpapers/io.github.hmerritt.Aura/metadata.json"
assert_exists "$home/data/applications/io.github.hmerritt.Aura.desktop"
assert_exists "$home/config/autostart/io.github.hmerritt.Aura.desktop"
assert_exists "$home/data/icons/hicolor/512x512/apps/io.github.hmerritt.Aura.png"
assert_exists "$app_root/current/LICENSE"
[[ "$(readlink "$home/.local/bin/aura")" = "$app_root/current/bin/aura" ]] \
    || fail "stable command link has an unexpected target"
grep -F "Exec=\"$home/.local/bin/aura\"" "$home/data/applications/io.github.hmerritt.Aura.desktop" >/dev/null
grep -F "Exec=\"$home/.local/bin/aura\"" "$home/config/autostart/io.github.hmerritt.Aura.desktop" >/dev/null
"$home/.local/bin/aura" --version | grep -F 'Version 1.1.0' >/dev/null
[[ "$(find "$app_root/versions" -mindepth 1 -maxdepth 1 -type d | wc -l)" -eq 1 ]] \
    || fail "clean install should contain one version"

# Idempotent reinstall.
run_installer "$home" "$base/v1" --version 1.1.0

# A failed verification must leave the active release unchanged.
before=$(readlink "$app_root/current")
if run_installer "$home" "$base/bad-sha"; then
    fail "installer accepted an invalid checksum"
fi
[[ "$(readlink "$app_root/current")" = "$before" ]] || fail "failed install changed current release"
if run_installer "$home" "$base/malformed"; then
    fail "installer accepted a malformed manifest"
fi
[[ "$(readlink "$app_root/current")" = "$before" ]] || fail "malformed manifest changed current release"
if run_installer "$home" "$base/unsafe"; then
    fail "installer accepted a path-traversing archive"
fi
[[ "$(readlink "$app_root/current")" = "$before" ]] || fail "unsafe archive changed current release"

# Upgrade and retain the immediately previous release.
run_installer "$home" "$base/v2" --expected-version 1.1.1
"$home/.local/bin/aura" --version | grep -F 'Version 1.1.1' >/dev/null
[[ "$(find "$app_root/versions" -mindepth 1 -maxdepth 1 -type d | wc -l)" -eq 2 ]] \
    || fail "upgrade should retain current and previous releases"

# Refuse to replace an unrelated command.
foreign_home="$temp/foreign-home"
mkdir -p "$foreign_home/.local/bin" "$foreign_home/runtime"
printf 'foreign\n' > "$foreign_home/.local/bin/aura"
if run_installer "$foreign_home" "$base/v1"; then
    fail "installer overwrote an unrelated aura command"
fi
grep -F foreign "$foreign_home/.local/bin/aura" >/dev/null

# Reject unsupported platforms and missing prerequisites before downloading.
if HOME="$temp/unsupported" AURA_ALLOW_ROOT=1 AURA_ARCH=riscv64 dash "$repo_root/install.sh"; then
    fail "installer accepted an unsupported architecture"
fi
if HOME="$temp/missing-tools" AURA_ALLOW_ROOT=1 AURA_OS=Linux AURA_ARCH=x86_64 PATH=/nonexistent \
    /bin/dash "$repo_root/install.sh"; then
    fail "installer accepted a PATH without required tools"
fi

# Uninstall a running managed process and remove default Aura data only.
mkdir -p "$home/state/aura" "$home/runtime/aura" "$home/.config"
printf 'config\n' > "$home/.config/aura.hcl"
"$home/.local/bin/aura" &
fake_pid=$!
sleep 0.2
HOME="$home" \
XDG_DATA_HOME="$home/data" \
XDG_CONFIG_HOME="$home/config" \
XDG_STATE_HOME="$home/state" \
XDG_RUNTIME_DIR="$home/runtime" \
AURA_ALLOW_ROOT=1 \
    dash "$repo_root/install.sh" --uninstall
if kill -0 "$fake_pid" 2>/dev/null; then
    fail "uninstall did not stop the managed Aura process"
fi
fake_pid=""
assert_missing "$home/data/aura"
assert_missing "$home/.local/bin/aura"
assert_missing "$home/.config/aura.hcl"
assert_missing "$home/state/aura"
assert_missing "$home/runtime/aura"
assert_exists "$home/wallpapers/source.jpg"

printf 'Linux installer integration tests passed.\n'
