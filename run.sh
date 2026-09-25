#!/bin/bash
# Convenient runner for Clash TUI on macOS / Linux
set -e
cd "$(dirname "$0")"

if [ -f "./clash-tui" ]; then
  ./clash-tui "$@"
elif [ -f "./target/release/clash-tui" ]; then
  ./target/release/clash-tui "$@"
elif [ -f "$HOME/.clash-tui/clash-tui" ]; then
  "$HOME/.clash-tui/clash-tui" "$@"
else
  echo "[ERROR] clash-tui executable not found."
  echo "        Run './install.sh' or 'cargo build --release' first."
  exit 1
fi
