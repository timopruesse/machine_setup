#!/bin/sh
# machine_setup installer
# Usage: curl -fsSL https://raw.githubusercontent.com/timopruesse/machine_setup/main/install/install.sh | sh
set -e

REPO="timopruesse/machine_setup"
BINARY="machine_setup"

main() {
    echo "Installing machine_setup..."
    echo ""

    # Try Homebrew first
    if command -v brew >/dev/null 2>&1; then
        echo "Homebrew detected. Installing via brew..."
        brew install timopruesse/repo/machine_setup
        echo ""
        echo "Done! Run 'machine_setup --help' to get started."
        return 0
    fi

    # Detect platform
    OS=$(uname -s)
    ARCH=$(uname -m)

    case "${OS}" in
        Darwin)
            case "${ARCH}" in
                arm64)  TARGET="aarch64-apple-darwin" ;;
                x86_64) TARGET="x86_64-apple-darwin" ;;
                *)      fail "Unsupported architecture: ${ARCH}" ;;
            esac
            ;;
        Linux)
            case "${ARCH}" in
                x86_64) TARGET="x86_64-unknown-linux-gnu" ;;
                *)      fail "Unsupported architecture: ${ARCH}. Use 'cargo install machine_setup' instead." ;;
            esac
            ;;
        *)
            fail "Unsupported OS: ${OS}. Use 'cargo install machine_setup' or the PowerShell script for Windows."
            ;;
    esac

    # Get latest version
    echo "Fetching latest release..."
    if command -v curl >/dev/null 2>&1; then
        VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
    elif command -v wget >/dev/null 2>&1; then
        VERSION=$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
    else
        fail "Neither curl nor wget found. Please install one of them."
    fi

    if [ -z "${VERSION}" ]; then
        fail "Could not determine latest version."
    fi

    echo "Latest version: ${VERSION}"

    # Download
    ARTIFACT="${BINARY}-${TARGET}.tar.gz"
    URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARTIFACT}"
    TMPDIR=$(mktemp -d)

    echo "Downloading ${ARTIFACT}..."
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "${URL}" -o "${TMPDIR}/${ARTIFACT}"
    else
        wget -q "${URL}" -O "${TMPDIR}/${ARTIFACT}"
    fi

    # Extract
    tar xzf "${TMPDIR}/${ARTIFACT}" -C "${TMPDIR}"

    # Install
    INSTALL_DIR=""
    if [ -d "/usr/local/bin" ] && [ -w "/usr/local/bin" ]; then
        INSTALL_DIR="/usr/local/bin"
    else
        INSTALL_DIR="${HOME}/.local/bin"
        mkdir -p "${INSTALL_DIR}"
    fi

    mv "${TMPDIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    chmod +x "${INSTALL_DIR}/${BINARY}"

    # Cleanup
    rm -rf "${TMPDIR}"

    # Verify
    if "${INSTALL_DIR}/${BINARY}" --version >/dev/null 2>&1; then
        echo ""
        echo "Installed machine_setup ${VERSION} to ${INSTALL_DIR}/${BINARY}"
    else
        echo ""
        echo "Installed to ${INSTALL_DIR}/${BINARY} but could not verify."
    fi

    # PATH hint
    case ":${PATH}:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            echo ""
            echo "NOTE: ${INSTALL_DIR} is not in your PATH."
            echo "Add it by running:"
            echo ""
            echo "  echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.bashrc"
            echo ""
            ;;
    esac
}

fail() {
    echo "Error: $1" >&2
    exit 1
}

main
