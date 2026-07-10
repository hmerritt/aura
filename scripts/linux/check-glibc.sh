#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 1 ]]; then
    printf 'Usage: scripts/linux/check-glibc.sh BINARY\n' >&2
    exit 1
fi

binary=$1
[[ -f "$binary" ]] || { printf 'Binary not found: %s\n' "$binary" >&2; exit 1; }

highest=$(
    readelf --version-info "$binary" \
        | grep -oE 'GLIBC_[0-9]+\.[0-9]+' \
        | sed 's/GLIBC_//' \
        | sort -V \
        | tail -n 1
)

[[ -n "$highest" ]] || { printf 'No GLIBC symbol versions found in %s\n' "$binary" >&2; exit 1; }
if [[ "$(printf '%s\n%s\n' '2.35' "$highest" | sort -V | tail -n 1)" != '2.35' ]]; then
    printf 'Binary requires GLIBC_%s, newer than supported GLIBC_2.35\n' "$highest" >&2
    exit 1
fi

printf 'Maximum required GLIBC version: %s\n' "$highest"
