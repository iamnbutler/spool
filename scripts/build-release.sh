#!/bin/bash
# Build fabric release binaries for distribution

set -e

VERSION="${1:-$(cargo metadata --format-version=1 --no-deps | grep -o '"version":"[^"]*' | head -1 | cut -d'"' -f4)}"
TARGET_DIR="target/release"
DIST_DIR="dist"

echo "Building fabric v${VERSION}"

# Clean dist directory
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

# Build for current platform
echo "Building release binary..."
cargo build --release

# Get current platform info
ARCH=$(uname -m)
OS=$(uname -s | tr '[:upper:]' '[:lower:]')

case "$ARCH" in
    x86_64) ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

case "$OS" in
    darwin) OS="apple-darwin" ;;
    linux) OS="unknown-linux-gnu" ;;
    *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

TARGET="${ARCH}-${OS}"
BINARY_NAME="fabric"
ARCHIVE_NAME="fabric-${VERSION}-${TARGET}"

# Copy binary to dist
echo "Packaging ${ARCHIVE_NAME}..."
cp "${TARGET_DIR}/${BINARY_NAME}" "${DIST_DIR}/${BINARY_NAME}"

# Create tarball
cd "$DIST_DIR"
tar -czvf "${ARCHIVE_NAME}.tar.gz" "$BINARY_NAME"
rm "$BINARY_NAME"

# Create checksum
shasum -a 256 "${ARCHIVE_NAME}.tar.gz" > "${ARCHIVE_NAME}.tar.gz.sha256"

echo ""
echo "Release artifacts created in ${DIST_DIR}/"
ls -la
