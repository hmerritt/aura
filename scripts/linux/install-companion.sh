#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/../.." && pwd)"

if [[ -z "${HOME:-}" ]]; then
    printf 'Error: HOME is not set.\n' >&2
    exit 1
fi

printf 'Choose the desktop companion to install:\n'
PS3='Selection: '
desktop=''

select choice in gnome plasma; do
    case "$choice" in
        gnome|plasma)
            desktop="$choice"
            break
            ;;
        *)
            printf 'Invalid selection. Choose 1 for gnome or 2 for plasma.\n' >&2
            ;;
    esac
done

if [[ -z "$desktop" ]]; then
    printf 'Error: no desktop companion selected.\n' >&2
    exit 1
fi

case "$desktop" in
    gnome)
        source_dir="$repo_root/integrations/linux/gnome"
        target_dir="$HOME/.local/share/gnome-shell/extensions/aura@hmerritt.github.io"
        ;;
    plasma)
        source_dir="$repo_root/integrations/linux/plasma"
        target_dir="$HOME/.local/share/plasma/wallpapers/io.github.hmerritt.Aura"
        ;;
esac

if [[ ! -d "$source_dir" ]]; then
    printf 'Error: companion source directory does not exist: %s\n' "$source_dir" >&2
    exit 1
fi

mkdir -p -- "$(dirname -- "$target_dir")"
rm -rf -- "$target_dir"
cp -a -- "$source_dir" "$target_dir"

printf 'Installed %s companion to %s\n' "$desktop" "$target_dir"
