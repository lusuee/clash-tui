#!/bin/bash
# Clash TUI autostart uninstaller for macOS (LaunchAgents) / Linux (systemd)
# Usage: bash unautostart.sh

OS="$(uname -s)"

case "$OS" in
    Darwin)
        PLIST_FILE="$HOME/Library/LaunchAgents/com.clash-tui.mihomo.plist"
        if [ -f "$PLIST_FILE" ]; then
            echo "正在卸载 launchd 服务..."
            launchctl unload -w "$PLIST_FILE" 2>/dev/null || true
            rm -f "$PLIST_FILE"
            echo "[成功] 已取消 macOS 开机自启 (LaunchAgents 已移除)。"
        else
            echo "[提示] 未找到自启配置文件: $PLIST_FILE (可能尚未配置或已删除)。"
        fi
        ;;

    Linux)
        SERVICE_FILE="$HOME/.config/systemd/user/clash-tui-mihomo.service"
        if [ -f "$SERVICE_FILE" ]; then
            echo "正在停用并移除 systemd 服务..."
            systemctl --user disable --now clash-tui-mihomo 2>/dev/null || true
            rm -f "$SERVICE_FILE"
            systemctl --user daemon-reload 2>/dev/null || true
            echo "[成功] 已取消 Linux 开机自启 (systemd 服务已移除)。"
        else
            echo "[提示] 未找到自启服务文件: $SERVICE_FILE (可能尚未配置或已删除)。"
        fi
        ;;

    *)
        echo "[ERROR] Unsupported OS: $OS"
        exit 1
        ;;
esac
