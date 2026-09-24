#!/bin/bash
# Clash TUI uninstaller for macOS / Linux
# Usage: bash uninstall.sh

INSTALL_DIR="$HOME/.clash-tui"

for d in /usr/local/bin "$HOME/.local/bin"; do
  if [ -L "$d/clash-tui" ]; then
    rm -f "$d/clash-tui"
  fi
done
echo "[OK] Removed 'clash-tui' command (reopen terminal for PATH refresh)."

if [ -d "$INSTALL_DIR" ]; then
  printf "Delete %s including data (subscriptions/rules/profiles)? [y/N] " "$INSTALL_DIR"
  read -r ans
  case "$ans" in
    y|Y)
      rm -rf "$INSTALL_DIR"
      echo "[DONE] clash-tui uninstalled."
      ;;
    *)
      echo "[DONE] Data kept at $INSTALL_DIR"
      ;;
  esac
else
  echo "[DONE] Nothing installed at $INSTALL_DIR."
fi
