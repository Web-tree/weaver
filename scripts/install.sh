#!/bin/sh
# Install the `wvr` CLI from GitHub Releases.
#
#   curl -fsSL https://raw.githubusercontent.com/Web-tree/weaver/main/scripts/install.sh | sh
#
# Environment:
#   WVR_VERSION      release tag to install (default: latest)
#   WVR_INSTALL_DIR  install directory (default: ~/.local/bin, or /usr/local/bin when writable and running as root)
#   WVR_REPO         owner/repo to install from (default: Web-tree/weaver)
#   WVR_API_BASE     GitHub API base URL (default: https://api.github.com)
#   WVR_DOWNLOAD_BASE  release download base URL (default: https://github.com/<repo>/releases/download)
#   GITHUB_TOKEN     used for API requests when set (avoids rate limits)

set -eu

REPO="${WVR_REPO:-Web-tree/weaver}"
BIN_NAME="wvr"
API="${WVR_API_BASE:-https://api.github.com}"
DOWNLOAD_BASE="${WVR_DOWNLOAD_BASE:-https://github.com/$REPO/releases/download}"

log() { printf '%s\n' "$*" >&2; }
die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

need() {
    command -v "$1" >/dev/null 2>&1 || die "$1 is required"
}

detect_target() {
    os=$(uname -s)
    arch=$(uname -m)

    case "$os" in
        Darwin) os_part="apple-darwin" ;;
        Linux) os_part="unknown-linux-gnu" ;;
        *) die "unsupported operating system: $os (build from source: cargo install --git https://github.com/$REPO)" ;;
    esac

    case "$arch" in
        arm64 | aarch64) arch_part="aarch64" ;;
        x86_64 | amd64) arch_part="x86_64" ;;
        *) die "unsupported architecture: $arch" ;;
    esac

    printf '%s-%s' "$arch_part" "$os_part"
}

# Latest release tag, without pulling in jq.
latest_tag() {
    fetch "$API/repos/$REPO/releases/latest" |
        tr ',' '\n' |
        sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' |
        head -n 1
}

fetch() {
    if [ -n "${GITHUB_TOKEN:-}" ]; then
        curl -fsSL -H "Authorization: Bearer $GITHUB_TOKEN" "$1"
    else
        curl -fsSL "$1"
    fi
}

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | cut -d' ' -f1
    else
        die "need sha256sum or shasum to verify the download"
    fi
}

default_install_dir() {
    if [ "$(id -u)" = "0" ] && [ -w /usr/local/bin ]; then
        printf '/usr/local/bin'
    else
        printf '%s/.local/bin' "$HOME"
    fi
}

need curl
need tar

target=$(detect_target)
tag="${WVR_VERSION:-}"
if [ -z "$tag" ]; then
    tag=$(latest_tag)
    [ -n "$tag" ] || die "could not determine the latest release of $REPO"
fi
case "$tag" in
    v*) ;;
    *) tag="v$tag" ;;
esac

install_dir="${WVR_INSTALL_DIR:-$(default_install_dir)}"
archive="$BIN_NAME-$tag-$target.tar.gz"
base="$DOWNLOAD_BASE/$tag"

log "Installing $BIN_NAME $tag ($target) to $install_dir"

tmp=$(mktemp -d)
cleanup() { rm -rf "$tmp"; }
trap cleanup EXIT INT TERM

curl -fsL -o "$tmp/$archive" "$base/$archive" ||
    die "no release asset $archive in $tag (see https://github.com/$REPO/releases/tag/$tag)"

# Verify against the release's checksum manifest before unpacking anything.
if curl -fsL -o "$tmp/SHA256SUMS" "$base/SHA256SUMS"; then
    expected=$(sed -n "s/^\([0-9a-f]\{64\}\)[[:space:]][[:space:]]*\(\*\)\{0,1\}$archive$/\1/p" "$tmp/SHA256SUMS" | head -n 1)
elif curl -fsL -o "$tmp/$archive.sha256" "$base/$archive.sha256"; then
    expected=$(cut -d' ' -f1 "$tmp/$archive.sha256")
else
    die "release $tag publishes no checksum for $archive"
fi

[ -n "${expected:-}" ] || die "no checksum entry for $archive"

actual=$(sha256_of "$tmp/$archive")
if [ "$actual" != "$expected" ]; then
    die "checksum mismatch for $archive (expected $expected, got $actual)"
fi
log "Checksum OK: $actual"

tar -xzf "$tmp/$archive" -C "$tmp"
[ -f "$tmp/$BIN_NAME" ] || die "$archive does not contain a $BIN_NAME executable"
chmod +x "$tmp/$BIN_NAME"

# Refuse to install something that cannot run on this machine.
"$tmp/$BIN_NAME" --version >/dev/null 2>&1 || die "the downloaded $BIN_NAME does not run on this system"

mkdir -p "$install_dir" || die "cannot create $install_dir"
if ! mv "$tmp/$BIN_NAME" "$install_dir/$BIN_NAME" 2>/dev/null; then
    die "cannot write to $install_dir; set WVR_INSTALL_DIR or re-run with sudo"
fi

installed_version=$("$install_dir/$BIN_NAME" --version)
log "Installed $installed_version -> $install_dir/$BIN_NAME"

case ":$PATH:" in
    *":$install_dir:"*) ;;
    *)
        log ""
        log "$install_dir is not on your PATH. Add it, for example:"
        log "  echo 'export PATH=\"$install_dir:\$PATH\"' >> ~/.zshrc"
        ;;
esac

log ""
log "Next: $BIN_NAME --help  |  keep it current with '$BIN_NAME self-update'"
