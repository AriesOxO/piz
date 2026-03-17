#!/usr/bin/env bash
# piz installer for macOS and Linux
# Usage: curl -fsSL https://raw.githubusercontent.com/AriesOxO/piz/main/install.sh | bash

set -euo pipefail

REPO="AriesOxO/piz"
INSTALL_DIR="/usr/local/bin"
BINARY="piz"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() { echo -e "${GREEN}[info]${NC} $1"; }
warn() { echo -e "${YELLOW}[warn]${NC} $1"; }
error() { echo -e "${RED}[error]${NC} $1"; exit 1; }

# Detect OS and architecture
detect_platform() {
    local os arch target

    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)  os="linux-gnu" ;;
        Darwin) os="apple-darwin" ;;
        *)      error "Unsupported OS: $os" ;;
    esac

    case "$arch" in
        x86_64|amd64)   arch="x86_64" ;;
        aarch64|arm64)  arch="aarch64" ;;
        *)              error "Unsupported architecture: $arch" ;;
    esac

    echo "${arch}-${os}"
}

# Get latest release tag
get_latest_version() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | sed -E 's/.*"([^"]+)".*/\1/'
}

main() {
    echo ""
    echo "  piz installer"
    echo "  ─────────────"
    echo ""

    local target version url tmpdir

    target="$(detect_platform)"
    info "Detected platform: ${target}"

    version="$(get_latest_version)"
    if [ -z "$version" ]; then
        error "Could not determine latest version. Check https://github.com/${REPO}/releases"
    fi
    info "Latest version: ${version}"

    url="https://github.com/${REPO}/releases/download/${version}/piz-${target}.tar.gz"
    info "Downloading: ${url}"

    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' EXIT

    curl -fsSL "$url" -o "${tmpdir}/piz.tar.gz" || error "Download failed. Check if release exists for ${target}"
    tar xzf "${tmpdir}/piz.tar.gz" -C "$tmpdir"

    # Install
    if [ -w "$INSTALL_DIR" ]; then
        mv "${tmpdir}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    else
        info "Installing to ${INSTALL_DIR} (requires sudo)"
        sudo mv "${tmpdir}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    fi

    chmod +x "${INSTALL_DIR}/${BINARY}"

    echo ""
    info "piz ${version} installed to ${INSTALL_DIR}/${BINARY}"
    echo ""
    echo "  Get started:"
    echo "    piz --version"
    echo "    piz config --init"
    echo "    piz list files"
    echo ""
}

main "$@"
