#!/bin/bash
# Clash TUI autostart registrar for macOS (LaunchAgents) / Linux (systemd)
# Usage: bash autostart.sh

set -e

OS="$(uname -s)"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Resolve binary and data path (check local install first, then repo dir)
if [ -f "$HOME/.clash-tui/bin/mihomo" ] && [ -d "$HOME/.clash-tui/data" ]; then
    CORE_EXE="$HOME/.clash-tui/bin/mihomo"
    DATA_DIR="$HOME/.clash-tui/data"
elif [ -f "$SCRIPT_DIR/bin/mihomo" ] && [ -d "$SCRIPT_DIR/data" ]; then
    CORE_EXE="$SCRIPT_DIR/bin/mihomo"
    DATA_DIR="$SCRIPT_DIR/data"
else
    echo "[ERROR] mihomo kernel binary not found."
    echo "        Make sure bin/mihomo exists or run 'bash install.sh' first."
    exit 1
fi

LOG_FILE="$DATA_DIR/mihomo.log"

case "$OS" in
    Darwin)
        PLIST_DIR="$HOME/Library/LaunchAgents"
        PLIST_FILE="$PLIST_DIR/com.clash-tui.mihomo.plist"

        echo "[1/2] Creating LaunchAgent: $PLIST_FILE ..."
        mkdir -p "$PLIST_DIR"

        cat <<EOF > "$PLIST_FILE"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.clash-tui.mihomo</string>
    <key>ProgramArguments</key>
    <array>
        <string>$CORE_EXE</string>
        <string>-d</string>
        <string>$DATA_DIR</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$LOG_FILE</string>
    <key>StandardErrorPath</key>
    <string>$LOG_FILE</string>
    <key>ProcessType</key>
    <string>Background</string>
</dict>
</plist>
EOF

        echo "[2/2] Loading service into launchd..."
        launchctl unload -w "$PLIST_FILE" 2>/dev/null || true
        launchctl load -w "$PLIST_FILE"

        echo ""
        echo "========================================================="
        echo " [成功] macOS 开机自启配置完毕 (LaunchAgents)！"
        echo "========================================================="
        echo " - Mihomo 内核将在开机登录后在后台静默运行"
        echo " - 配置文件: $PLIST_FILE"
        echo " - 日志输出: $LOG_FILE"
        echo " - 随时输入 clash-tui 即可打开面板切换节点或查看流量"
        echo " - 如需取消自启: bash unautostart.sh"
        echo "========================================================="
        ;;

    Linux)
        SERVICE_DIR="$HOME/.config/systemd/user"
        SERVICE_FILE="$SERVICE_DIR/clash-tui-mihomo.service"

        echo "[1/2] Creating systemd user service: $SERVICE_FILE ..."
        mkdir -p "$SERVICE_DIR"

        cat <<EOF > "$SERVICE_FILE"
[Unit]
Description=Mihomo Core Daemon for Clash TUI
After=network.target

[Service]
Type=simple
ExecStart=$CORE_EXE -d $DATA_DIR
Restart=on-failure
RestartSec=3

[Install]
WantedBy=default.target
EOF

        echo "[2/2] Enabling and starting systemd service..."
        systemctl --user daemon-reload
        systemctl --user enable --now clash-tui-mihomo

        echo ""
        echo "========================================================="
        echo " [成功] Linux 开机自启配置完毕 (systemd --user)！"
        echo "========================================================="
        echo " - 取消自启: bash unautostart.sh"
        echo "========================================================="
        ;;

    *)
        echo "[ERROR] Unsupported OS: $OS"
        exit 1
        ;;
esac
