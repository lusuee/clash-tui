#!/bin/bash
# Local packaging script for Clash TUI
set -e
cd "$(dirname "$0")"

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "$ARCH" in
  x86_64) ARCH="x64" ;;
  arm64|aarch64) ARCH="arm64" ;;
esac

echo "[1/3] Building release binary..."
cargo build --release

EXE="target/release/clash-tui"
if [ ! -f "$EXE" ]; then
  echo "[ERROR] $EXE not found after build."
  exit 1
fi

PACKAGE_NAME="clash-tui-${OS}-${ARCH}"
DIST_DIR="dist/${PACKAGE_NAME}"
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

echo "[2/3] Collecting files into $DIST_DIR..."
cp "$EXE" "$DIST_DIR/clash-tui"
chmod +x "$DIST_DIR/clash-tui"

# Copy helper scripts
for f in install.sh uninstall.sh run.sh autostart.sh unautostart.sh env.sh unenv.sh README.md; do
  if [ -f "$f" ]; then
    cp "$f" "$DIST_DIR/"
  fi
done

# Copy bin/mihomo if present
if [ -f "bin/mihomo" ]; then
  mkdir -p "$DIST_DIR/bin"
  cp "bin/mihomo" "$DIST_DIR/bin/"
  chmod +x "$DIST_DIR/bin/mihomo"
fi

echo "[3/3] Creating archive: dist/${PACKAGE_NAME}.tar.gz..."
mkdir -p dist
tar -czvf "dist/${PACKAGE_NAME}.tar.gz" -C dist "${PACKAGE_NAME}"
shasum -a 256 "dist/${PACKAGE_NAME}.tar.gz" > "dist/${PACKAGE_NAME}.tar.gz.sha256"

echo ""
echo "[DONE] Package created at dist/${PACKAGE_NAME}.tar.gz"
