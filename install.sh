#!/bin/sh

set -eu

PROGRAM_NAME="Aura installer"
REPOSITORY="hmerritt/aura"
DEFAULT_FEED_URL="https://github.com/${REPOSITORY}/releases/latest/download"
MANIFEST_NAME="aura-linux-manifest"
GNOME_UUID="aura@hmerritt.github.io"
PLASMA_PLUGIN_ID="io.github.hmerritt.Aura"
DESKTOP_ID="io.github.hmerritt.Aura"

temp_dir=""

cleanup() {
    if [ -n "$temp_dir" ] && [ -d "$temp_dir" ]; then
        rm -rf -- "$temp_dir"
    fi
}

trap cleanup EXIT HUP INT TERM

fail() {
    printf 'Error: %s\n' "$*" >&2
    exit 1
}

usage() {
    cat <<'EOF'
Install Aura for the current Linux user.

Usage:
  install.sh [--version VERSION] [--feed-url URL]
  install.sh --uninstall
  install.sh --help

Environment:
  AURA_VERSION       Install an exact numeric SemVer release.
  AURA_FEED_URL      Override the release feed base URL.
EOF
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || fail "required command '$1' was not found"
}

download() {
    url=$1
    destination=$2
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --retry 3 --connect-timeout 15 "$url" -o "$destination"
    elif command -v wget >/dev/null 2>&1; then
        wget -q -O "$destination" "$url"
    else
        fail "either curl or wget is required"
    fi
}

is_semver() {
    printf '%s\n' "$1" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'
}

is_sha256() {
    printf '%s\n' "$1" | grep -Eq '^[0-9a-fA-F]{64}$'
}

is_http_url() {
    printf '%s\n' "$1" | grep -Eq '^https?://[^[:space:]]+$'
}

resolve_platform() {
    platform=${AURA_OS:-$(uname -s)}
    [ "$platform" = "Linux" ] || fail "unsupported operating system '$platform'; Aura packages support Linux only"

    machine=${AURA_ARCH:-$(uname -m)}
    case "$machine" in
        x86_64|amd64) arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *) fail "unsupported architecture '$machine'; expected x86_64 or aarch64" ;;
    esac
}

resolve_paths() {
    [ -n "${HOME:-}" ] || fail "HOME is not set"
    data_home=${XDG_DATA_HOME:-"$HOME/.local/share"}
    config_home=${XDG_CONFIG_HOME:-"$HOME/.config"}
    state_home=${XDG_STATE_HOME:-"$HOME/.local/state"}
    runtime_home=${XDG_RUNTIME_DIR:-}
    app_data_root="$data_home/aura"
    app_root="$app_data_root/app"
    versions_root="$app_root/versions"
    current_link="$app_root/current"
    bin_dir="$HOME/.local/bin"
    bin_link="$bin_dir/aura"
    launcher="$data_home/applications/$DESKTOP_ID.desktop"
    autostart="$config_home/autostart/$DESKTOP_ID.desktop"
    icon="$data_home/icons/hicolor/512x512/apps/$DESKTOP_ID.png"
    gnome_target="$data_home/gnome-shell/extensions/$GNOME_UUID"
    plasma_target="$data_home/plasma/wallpapers/$PLASMA_PLUGIN_ID"
}

parse_manifest() {
    manifest=$1
    manifest_schema=""
    manifest_version=""
    x86_64_url=""
    x86_64_sha256=""
    aarch64_url=""
    aarch64_sha256=""

    while IFS= read -r line || [ -n "$line" ]; do
        [ -n "$line" ] || fail "manifest contains an empty line"
        key=${line%%=*}
        value=${line#*=}
        [ "$key" != "$line" ] || fail "manifest line is missing '=': $line"
        case "$key" in
            schema) [ -z "$manifest_schema" ] || fail "manifest contains duplicate key '$key'"; manifest_schema=$value ;;
            version) [ -z "$manifest_version" ] || fail "manifest contains duplicate key '$key'"; manifest_version=$value ;;
            x86_64_url) [ -z "$x86_64_url" ] || fail "manifest contains duplicate key '$key'"; x86_64_url=$value ;;
            x86_64_sha256) [ -z "$x86_64_sha256" ] || fail "manifest contains duplicate key '$key'"; x86_64_sha256=$value ;;
            aarch64_url) [ -z "$aarch64_url" ] || fail "manifest contains duplicate key '$key'"; aarch64_url=$value ;;
            aarch64_sha256) [ -z "$aarch64_sha256" ] || fail "manifest contains duplicate key '$key'"; aarch64_sha256=$value ;;
            *) fail "manifest contains unknown key '$key'" ;;
        esac
    done < "$manifest"

    [ "$manifest_schema" = "1" ] || fail "unsupported manifest schema '$manifest_schema'"
    is_semver "$manifest_version" || fail "manifest version is not numeric SemVer: '$manifest_version'"
    is_http_url "$x86_64_url" || fail "manifest x86_64 URL is invalid"
    is_http_url "$aarch64_url" || fail "manifest aarch64 URL is invalid"
    is_sha256 "$x86_64_sha256" || fail "manifest x86_64 SHA-256 is invalid"
    is_sha256 "$aarch64_sha256" || fail "manifest aarch64 SHA-256 is invalid"
}

validate_archive_paths() {
    archive=$1
    listing=$2
    tar -tzf "$archive" > "$listing" || fail "release archive could not be listed"
    [ -s "$listing" ] || fail "release archive is empty"
    while IFS= read -r entry || [ -n "$entry" ]; do
        case "$entry" in
            aura|aura/*) ;;
            *) fail "release archive contains a path outside the aura root: '$entry'" ;;
        esac
        case "/$entry/" in
            */../*|*/./*) fail "release archive contains an unsafe path: '$entry'" ;;
        esac
    done < "$listing"

    tar -tvzf "$archive" > "$listing.verbose" || fail "release archive metadata could not be read"
    while IFS= read -r entry_metadata || [ -n "$entry_metadata" ]; do
        case "$entry_metadata" in
            d*|-*) ;;
            *) fail "release archive contains a link or unsupported entry type" ;;
        esac
    done < "$listing.verbose"
}

sed_replacement() {
    printf '%s' "$1" | sed 's/[\\&|]/\\&/g'
}

render_desktop_file() {
    source_file=$1
    destination=$2
    escaped_bin=$(sed_replacement "$bin_link")
    mkdir -p -- "$(dirname -- "$destination")"
    rendered="$destination.aura-new.$$"
    sed "s|@AURA_BIN@|$escaped_bin|g" "$source_file" > "$rendered"
    chmod 0644 "$rendered"
    mv -f -- "$rendered" "$destination"
}

install_managed_link() {
    target=$1
    destination=$2
    mkdir -p -- "$(dirname -- "$destination")"
    temporary="$destination.aura-new.$$"
    rm -rf -- "$temporary"
    ln -s -- "$target" "$temporary"
    rm -rf -- "$destination"
    mv -fT -- "$temporary" "$destination"
}

preflight_bin_link() {
    if [ -L "$bin_link" ]; then
        existing_target=$(readlink "$bin_link")
        case "$existing_target" in
            "$app_root"/current/bin/aura|"$app_root"/versions/*/bin/aura) return 0 ;;
            *) fail "$bin_link is an unrelated symbolic link; move it before installing Aura" ;;
        esac
    fi
    [ ! -e "$bin_link" ] || fail "$bin_link already exists and is not managed by Aura"
}

stop_running_aura() {
    if command -v gdbus >/dev/null 2>&1; then
        if ! gdbus call --session \
            --dest io.github.hmerritt.Aura \
            --object-path /io/github/hmerritt/Aura \
            --method io.github.hmerritt.Aura1.PrepareForUninstall >/dev/null 2>&1; then
            gdbus call --session \
                --dest io.github.hmerritt.Aura \
                --object-path /io/github/hmerritt/Aura \
                --method io.github.hmerritt.Aura1.Exit >/dev/null 2>&1 || true
        fi
    elif command -v busctl >/dev/null 2>&1; then
        if ! busctl --user call io.github.hmerritt.Aura \
            /io/github/hmerritt/Aura io.github.hmerritt.Aura1 PrepareForUninstall >/dev/null 2>&1; then
            busctl --user call io.github.hmerritt.Aura \
                /io/github/hmerritt/Aura io.github.hmerritt.Aura1 Exit >/dev/null 2>&1 || true
        fi
    fi

    sleep 1
    for executable in /proc/[0-9]*/exe; do
        [ -L "$executable" ] || continue
        resolved=$(readlink "$executable" 2>/dev/null || true)
        case "$resolved" in
            "$app_root"/versions/*/bin/aura)
                pid=${executable#/proc/}
                pid=${pid%/exe}
                kill "$pid" 2>/dev/null || true
                ;;
        esac
    done
}

uninstall_aura() {
    resolve_paths
    printf 'Uninstalling Aura for the current user...\n'
    stop_running_aura

    if command -v gnome-extensions >/dev/null 2>&1; then
        gnome-extensions disable "$GNOME_UUID" >/dev/null 2>&1 || true
    fi

    if [ -L "$bin_link" ]; then
        existing_target=$(readlink "$bin_link")
        case "$existing_target" in
            "$app_root"/*) rm -f -- "$bin_link" ;;
            *) printf 'Leaving unrelated link %s untouched.\n' "$bin_link" >&2 ;;
        esac
    elif [ -e "$bin_link" ]; then
        printf 'Leaving unrelated file %s untouched.\n' "$bin_link" >&2
    fi

    rm -rf -- \
        "$gnome_target" \
        "$plasma_target" \
        "$launcher" \
        "$autostart" \
        "$icon" \
        "$app_data_root" \
        "$HOME/.config/aura.hcl" \
        "$state_home/aura"
    if [ -n "$runtime_home" ]; then
        rm -rf -- "$runtime_home/aura"
    fi

    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$data_home/applications" >/dev/null 2>&1 || true
    fi
    printf 'Aura and its default user data have been removed.\n'
}

install_aura() {
    require_command uname
    resolve_platform
    resolve_paths
    require_command cut
    require_command dirname
    require_command grep
    require_command ln
    require_command mktemp
    require_command mkdir
    require_command mv
    require_command readlink
    require_command rm
    require_command sed
    require_command sha256sum
    require_command sleep
    require_command tar
    require_command tr

    preflight_bin_link

    requested_version=${requested_version:-${AURA_VERSION:-}}
    feed_url=${feed_url:-${AURA_FEED_URL:-}}
    if [ -z "$feed_url" ]; then
        if [ -n "$requested_version" ]; then
            is_semver "$requested_version" || fail "requested version is not numeric SemVer: '$requested_version'"
            feed_url="https://github.com/${REPOSITORY}/releases/download/$requested_version"
        else
            feed_url=$DEFAULT_FEED_URL
        fi
    fi
    feed_url=${feed_url%/}
    is_http_url "$feed_url" || fail "feed URL must use http:// or https://"

    if [ -n "$requested_version" ]; then
        is_semver "$requested_version" || fail "requested version is not numeric SemVer: '$requested_version'"
    fi
    if [ -n "${expected_version:-}" ]; then
        is_semver "$expected_version" || fail "expected version is not numeric SemVer: '$expected_version'"
    fi

    temp_base=${TMPDIR:-/tmp}
    [ -d "$temp_base" ] || fail "temporary directory does not exist: $temp_base"
    temp_dir=$(mktemp -d "$temp_base/aura-install.XXXXXX")
    manifest_file="$temp_dir/$MANIFEST_NAME"
    archive="$temp_dir/aura-linux-$arch.tar.gz"

    printf 'Downloading Aura release manifest...\n'
    download "$feed_url/$MANIFEST_NAME" "$manifest_file"
    parse_manifest "$manifest_file"

    if [ -n "$requested_version" ] && [ "$manifest_version" != "$requested_version" ]; then
        fail "feed returned Aura $manifest_version, expected requested version $requested_version"
    fi
    if [ -n "${expected_version:-}" ] && [ "$manifest_version" != "$expected_version" ]; then
        fail "feed changed during update: returned $manifest_version, expected $expected_version"
    fi

    case "$arch" in
        x86_64) archive_url=$x86_64_url; archive_sha256=$x86_64_sha256 ;;
        aarch64) archive_url=$aarch64_url; archive_sha256=$aarch64_sha256 ;;
    esac
    archive_sha256=$(printf '%s' "$archive_sha256" | tr 'A-F' 'a-f')

    printf 'Downloading Aura %s for %s...\n' "$manifest_version" "$arch"
    download "$archive_url" "$archive"
    printf '%s  %s\n' "$archive_sha256" "$archive" | sha256sum -c - >/dev/null \
        || fail "release archive SHA-256 verification failed"
    validate_archive_paths "$archive" "$temp_dir/archive.list"
    tar -xzf "$archive" -C "$temp_dir" || fail "release archive extraction failed"

    payload="$temp_dir/aura"
    for required in \
        bin/aura \
        install.sh \
        LICENSE \
        share/applications/$DESKTOP_ID.desktop.in \
        share/autostart/$DESKTOP_ID.desktop.in \
        share/icons/hicolor/512x512/apps/$DESKTOP_ID.png \
        share/gnome-shell/extensions/$GNOME_UUID/extension.js \
        share/gnome-shell/extensions/$GNOME_UUID/metadata.json \
        share/plasma/wallpapers/$PLASMA_PLUGIN_ID/contents/config/main.xml \
        share/plasma/wallpapers/$PLASMA_PLUGIN_ID/contents/ui/main.qml \
        share/plasma/wallpapers/$PLASMA_PLUGIN_ID/metadata.json; do
        [ -f "$payload/$required" ] || fail "release payload is missing $required"
    done
    [ -x "$payload/bin/aura" ] || fail "release payload binary is not executable"
    [ -x "$payload/install.sh" ] || fail "release payload installer is not executable"
    version_output=$("$payload/bin/aura" --version 2>&1) || fail "packaged Aura binary did not run"
    case "$version_output" in
        *"Version $manifest_version"*) ;;
        *) fail "packaged binary version does not match manifest: $version_output" ;;
    esac

    release_id="$manifest_version-$(printf '%s' "$archive_sha256" | cut -c 1-12)"
    release_dir="$versions_root/$release_id"
    mkdir -p -- "$versions_root"
    if [ ! -d "$release_dir" ]; then
        staged_release="$versions_root/.aura-$release_id.$$"
        rm -rf -- "$staged_release"
        mv -- "$payload" "$staged_release"
        mv -- "$staged_release" "$release_dir"
    fi

    install_managed_link \
        "$app_root/current/share/gnome-shell/extensions/$GNOME_UUID" \
        "$gnome_target"
    install_managed_link \
        "$app_root/current/share/plasma/wallpapers/$PLASMA_PLUGIN_ID" \
        "$plasma_target"
    install_managed_link \
        "$app_root/current/share/icons/hicolor/512x512/apps/$DESKTOP_ID.png" \
        "$icon"
    render_desktop_file \
        "$release_dir/share/applications/$DESKTOP_ID.desktop.in" \
        "$launcher"
    render_desktop_file \
        "$release_dir/share/autostart/$DESKTOP_ID.desktop.in" \
        "$autostart"

    previous_release=""
    if [ -L "$current_link" ]; then
        previous_release=$(readlink "$current_link" || true)
    fi
    new_current="$app_root/.current.$$"
    rm -f -- "$new_current"
    ln -s -- "versions/$release_id" "$new_current"
    mv -fT -- "$new_current" "$current_link"

    mkdir -p -- "$bin_dir"
    rm -f -- "$bin_link"
    ln -s -- "$app_root/current/bin/aura" "$bin_link"

    for old_release in "$versions_root"/*; do
        [ -d "$old_release" ] || continue
        old_name=${old_release##*/}
        [ "$old_name" = "$release_id" ] && continue
        [ "versions/$old_name" = "$previous_release" ] && continue
        rm -rf -- "$old_release"
    done

    if command -v gnome-extensions >/dev/null 2>&1; then
        if ! gnome-extensions enable "$GNOME_UUID" >/dev/null 2>&1; then
            printf "GNOME could not enable %s in this session. After logging in, run: gnome-extensions enable %s\n" \
                "$GNOME_UUID" "$GNOME_UUID" >&2
        fi
    else
        printf "For GNOME, enable the companion after logging in with the Extensions app or run: gnome-extensions enable %s\n" \
            "$GNOME_UUID"
    fi
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$data_home/applications" >/dev/null 2>&1 || true
    fi

    printf 'Aura %s has been installed.\n' "$manifest_version"
    printf 'Aura will start automatically the next time you log in.\n'
    if command -v aura >/dev/null 2>&1; then
        printf "Run 'aura' to start it manually.\n"
    else
        printf 'Add %s to PATH to run Aura from a terminal, or run %s directly.\n' "$bin_dir" "$bin_link"
    fi
}

requested_version=""
expected_version=""
feed_url=""
uninstall=false

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)
            [ "$#" -ge 2 ] || fail "--version requires a value"
            requested_version=$2
            shift 2
            ;;
        --expected-version)
            [ "$#" -ge 2 ] || fail "--expected-version requires a value"
            expected_version=$2
            shift 2
            ;;
        --feed-url)
            [ "$#" -ge 2 ] || fail "--feed-url requires a value"
            feed_url=$2
            shift 2
            ;;
        --uninstall)
            uninstall=true
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *) fail "unknown argument: $1" ;;
    esac
done

command -v id >/dev/null 2>&1 || fail "required command 'id' was not found"
if [ "$(id -u)" -eq 0 ] && [ "${AURA_ALLOW_ROOT:-0}" != "1" ]; then
    fail "$PROGRAM_NAME must not be run as root or through sudo"
fi

if [ "$uninstall" = true ]; then
    [ -z "$requested_version" ] || fail "--uninstall cannot be combined with --version"
    [ -z "$expected_version" ] || fail "--uninstall cannot be combined with --expected-version"
    [ -z "$feed_url" ] || fail "--uninstall cannot be combined with --feed-url"
    uninstall_aura
else
    install_aura
fi
