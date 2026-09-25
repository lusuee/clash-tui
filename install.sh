#!/bin/bash
# Clash TUI installer for macOS / Linux
# Usage: bash install.sh
set -e
cd "$(dirname "$0")"

INSTALL_DIR="$HOME/.clash-tui"
EXE=""
if [ -f "./clash-tui" ]; then
  EXE="./clash-tui"
  echo "[1/3] Using pre-compiled release binary ($EXE)..."
elif [ -f "target/release/clash-tui" ]; then
  EXE="target/release/clash-tui"
  echo "[1/3] Using existing build binary ($EXE)..."
else
  # Try to source cargo env if installed via rustup but not yet in PATH
  if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
    . "$HOME/.cargo/env"
  fi

  if ! command -v cargo >/dev/null 2>&1; then
    echo "[ERROR] cargo command not found and no pre-compiled binary found."
    echo "        To build from source, please install Rust:"
    echo "          1. Official rustup (recommended):"
    echo "             curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo "             source \"\$HOME/.cargo/env\""
    echo "          2. Or via Homebrew:"
    echo "             brew install rust"
    exit 1
  fi

  echo "[1/3] Building release binary from source..."
  cargo build --release
  EXE="target/release/clash-tui"
fi

if [ ! -f "$EXE" ]; then
  echo "[ERROR] $EXE not found."
  exit 1
fi

echo "[2/3] Installing to $INSTALL_DIR ..."
mkdir -p "$INSTALL_DIR/bin"
cp "$EXE" "$INSTALL_DIR/clash-tui"
chmod +x "$INSTALL_DIR/clash-tui"

# Seed kernel and data on first install
if [ ! -f "$INSTALL_DIR/bin/mihomo" ]; then
  if [ -f bin/mihomo ]; then
    cp bin/mihomo "$INSTALL_DIR/bin/mihomo"
    chmod +x "$INSTALL_DIR/bin/mihomo"
  elif [ -f bin/mihomo/mihomo ]; then
    cp bin/mihomo/mihomo "$INSTALL_DIR/bin/mihomo"
    chmod +x "$INSTALL_DIR/bin/mihomo"
  else
    echo "[WARNING] Mihomo kernel not found at bin/mihomo."
    echo "          Download it from https://github.com/MetaCubeX/mihomo/releases"
    echo "          and place it at $INSTALL_DIR/bin/mihomo (chmod +x)."
  fi
fi
if [ ! -f "$INSTALL_DIR/subscriptions.json" ] && [ -f subscriptions.json ]; then
  cp subscriptions.json "$INSTALL_DIR/"
fi
if [ ! -f "$INSTALL_DIR/rules.json" ] && [ -f rules.json ]; then
  cp rules.json "$INSTALL_DIR/"
fi
if [ ! -d "$INSTALL_DIR/profiles" ] && [ -d profiles ]; then
  cp -R profiles "$INSTALL_DIR/profiles"
fi
if [ ! -d "$INSTALL_DIR/data" ] && [ -d data ]; then
  cp -R data "$INSTALL_DIR/data"
fi

echo "[3/3] Creating 'clash-tui' command..."
LINK_DIR=""
if [ -w /usr/local/bin ]; then
  LINK_DIR="/usr/local/bin"
else
  LINK_DIR="$HOME/.local/bin"
  mkdir -p "$LINK_DIR"
  case ":$PATH:" in
    *":$LINK_DIR:"*) ;;
    *)
      # macOS default shell is zsh
      printf '\nexport PATH="$HOME/.local/bin:$PATH"\n' >> "$HOME/.zshrc"
      echo "[INFO] Added ~/.local/bin to PATH in ~/.zshrc (reopen terminal to apply)"
      ;;
  esac
fi
ln -sf "$INSTALL_DIR/clash-tui" "$LINK_DIR/clash-tui"

echo ""
echo "[DONE] Installed. Run anywhere:"
echo "    clash-tui [-u http://127.0.0.1:9090] [--secret SECRET]"
echo ""
echo "Data / kernel location: $INSTALL_DIR"
echo "Uninstall: bash uninstall.sh"
