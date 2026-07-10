#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 3 ]]; then
    printf 'Usage: scripts/linux/generate-manifest.sh VERSION ASSET_DIR OUTPUT\n' >&2
    exit 1
fi

version=$1
asset_dir=$2
output=$3
repo="${GITHUB_REPOSITORY:-hmerritt/aura}"

[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { printf 'Invalid version: %s\n' "$version" >&2; exit 1; }

x86_name="aura-linux-x86_64.tar.gz"
arm_name="aura-linux-aarch64.tar.gz"
[[ -f "$asset_dir/$x86_name" ]] || { printf 'Missing %s\n' "$asset_dir/$x86_name" >&2; exit 1; }
[[ -f "$asset_dir/$arm_name" ]] || { printf 'Missing %s\n' "$asset_dir/$arm_name" >&2; exit 1; }

x86_sha=$(sha256sum "$asset_dir/$x86_name" | awk '{print $1}')
arm_sha=$(sha256sum "$asset_dir/$arm_name" | awk '{print $1}')
base="https://github.com/$repo/releases/download/$version"

{
    printf 'schema=1\n'
    printf 'version=%s\n' "$version"
    printf 'x86_64_url=%s/%s\n' "$base" "$x86_name"
    printf 'x86_64_sha256=%s\n' "$x86_sha"
    printf 'aarch64_url=%s/%s\n' "$base" "$arm_name"
    printf 'aarch64_sha256=%s\n' "$arm_sha"
} > "$output"
