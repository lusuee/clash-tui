#!/bin/bash
# Clash TUI All-in-One Installer & Service Manager for macOS / Linux
# Usage:
#   bash install.sh                  # 交互式安装（安装 + 自动询问是否开启开机自启）
#   bash install.sh --autostart      # 自动安装并开启开机自启（静默无交互）
#   bash install.sh --no-autostart   # 仅安装，不配置开机自启
#   bash install.sh --uninstall      # 卸载 clash-tui 并清除开机自启
#   bash install.sh --autostart-on   # 单独开启开机自启
#   bash install.sh --autostart-off  # 单独关闭开机自启
#   bash install.sh --status         # 查看当前安装及服务状态

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

OS="$(uname -s)"
INSTALL_DIR="$HOME/.clash-tui"
CORE_EXE="$INSTALL_DIR/bin/mihomo"
DATA_DIR="$INSTALL_DIR/data"
LOG_FILE="$DATA_DIR/mihomo.log"

PLIST_DIR="$HOME/Library/LaunchAgents"
PLIST_FILE="$PLIST_DIR/com.clash-tui.mihomo.plist"
SERVICE_DIR="$HOME/.config/systemd/user"
SERVICE_FILE="$SERVICE_DIR/clash-tui-mihomo.service"

show_help() {
  echo "Clash TUI 统一安装与服务管理脚本"
  echo ""
  echo "用法: bash install.sh [选项]"
  echo ""
  echo "选项:"
  echo "  (无参数)              交互式安装（安装 + 自动提示是否开启开机自启）"
  echo "  -a, --autostart       安装并配置开机自启（静默模式）"
  echo "  --no-autostart        仅安装，不配置开机自启"
  echo "  -u, --uninstall       卸载 clash-tui 并清理开机自启"
  echo "  --autostart-on        单独配置并启用开机后台自启"
  echo "  --autostart-off       单独关闭并移除开机自启"
  echo "  --status              查看安装与服务运行状态"
  echo "  -h, --help            显示帮助信息"
  echo ""
}

enable_autostart() {
  echo "[配置自启] 正在配置后台开机自启服务..."
  mkdir -p "$DATA_DIR"

  if [ ! -f "$CORE_EXE" ]; then
    echo "[警告] 未找到内核文件: $CORE_EXE"
    echo "       自启服务需要内核二进制，请先完成安装或放置内核至 $CORE_EXE"
  fi

  case "$OS" in
    Darwin)
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
      launchctl unload -w "$PLIST_FILE" 2>/dev/null || true
      launchctl load -w "$PLIST_FILE"
      echo "✔ [成功] 已配置并启动 macOS 开机后台自启 (LaunchAgents: com.clash-tui.mihomo)"
      ;;

    Linux)
      mkdir -p "$SERVICE_DIR"
      cat <<EOF > "$SERVICE_FILE"
[Unit]
Description=Clash TUI Mihomo Core Daemon
After=network.target

[Service]
Type=simple
ExecStart=$CORE_EXE -d $DATA_DIR
Restart=always
RestartSec=3
StandardOutput=append:$LOG_FILE
StandardError=append:$LOG_FILE

[Install]
WantedBy=default.target
EOF
      systemctl --user daemon-reload
      systemctl --user enable --now clash-tui-mihomo
      echo "✔ [成功] 已配置并启动 Linux 开机后台自启 (systemd: clash-tui-mihomo)"
      ;;

    *)
      echo "[错误] 不支持的操作系统: $OS"
      exit 1
      ;;
  esac
}

disable_autostart() {
  echo "[取消自启] 正在关闭并移除自启服务..."
  case "$OS" in
    Darwin)
      if [ -f "$PLIST_FILE" ]; then
        launchctl unload -w "$PLIST_FILE" 2>/dev/null || true
        rm -f "$PLIST_FILE"
        echo "✔ [成功] 已取消 macOS 开机自启 (LaunchAgents 配置文件已清理)。"
      else
        echo "ℹ [提示] 未找到自启配置文件: $PLIST_FILE"
      fi
      ;;

    Linux)
      if [ -f "$SERVICE_FILE" ]; then
        systemctl --user disable --now clash-tui-mihomo 2>/dev/null || true
        rm -f "$SERVICE_FILE"
        systemctl --user daemon-reload 2>/dev/null || true
        echo "✔ [成功] 已取消 Linux 开机自启 (systemd 服务已清理)。"
      else
        echo "ℹ [提示] 未找到自启服务文件: $SERVICE_FILE"
      fi
      ;;
  esac
}

show_status() {
  echo "=== Clash TUI 状态检查 ==="
  echo "操作系统: $OS"
  echo "安装路径: $INSTALL_DIR"

  if [ -f "$INSTALL_DIR/clash-tui" ]; then
    echo "主程序:   ✔ 已安装 ($INSTALL_DIR/clash-tui)"
  else
    echo "主程序:   ✘ 未安装"
  fi

  if [ -f "$CORE_EXE" ]; then
    echo "内核程序: ✔ 已安装 ($CORE_EXE)"
  else
    echo "内核程序: ✘ 未找到"
  fi

  # 检查进程
  if pgrep -x "mihomo" >/dev/null 2>&1 || pgrep -f "mihomo -d" >/dev/null 2>&1; then
    echo "内核运行: ● 正在运行 (active)"
  else
    echo "内核运行: ○ 未运行 (inactive)"
  fi

  # 检查自启
  case "$OS" in
    Darwin)
      if [ -f "$PLIST_FILE" ]; then
        echo "开机自启: ✔ 已开启 ($PLIST_FILE)"
      else
        echo "开机自启: ○ 未开启"
      fi
      ;;
    Linux)
      if [ -f "$SERVICE_FILE" ]; then
        echo "开机自启: ✔ 已开启 ($SERVICE_FILE)"
      else
        echo "开机自启: ○ 未开启"
      fi
      ;;
  esac
  echo "========================="
}

do_uninstall() {
  echo "[1/2] 清理开机自启服务..."
  disable_autostart

  echo "[2/2] 移除全局命令..."
  for d in /usr/local/bin "$HOME/.local/bin"; do
    if [ -L "$d/clash-tui" ] || [ -f "$d/clash-tui" ]; then
      rm -f "$d/clash-tui"
      echo "✔ 已移除命令: $d/clash-tui"
    fi
  done

  if [ -d "$INSTALL_DIR" ]; then
    printf "是否同时删除用户配置与订阅数据 (%s)？[y/N]: " "$INSTALL_DIR"
    read -r ans
    case "$ans" in
      y|Y)
        rm -rf "$INSTALL_DIR"
        echo "✔ 已彻底删除安装目录与所有数据。"
        ;;
      *)
        echo "ℹ 保留数据目录: $INSTALL_DIR"
        ;;
    esac
  fi
  echo ""
  echo "✔ [完成] Clash TUI 卸载完成。"
}

do_install() {
  AUTOSTART_CHOICE="$1"

  echo "[1/3] 检索或构建可执行文件..."
  EXE=""
  if [ -f "./clash-tui" ]; then
    EXE="./clash-tui"
    echo "✔ 发现预编译 release 二进制: $EXE"
  elif [ -f "target/release/clash-tui" ]; then
    EXE="target/release/clash-tui"
    echo "✔ 发现本地构建二进制: $EXE"
  else
    # 尝试引入 cargo 环境
    if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
      . "$HOME/.cargo/env"
    fi

    if ! command -v cargo >/dev/null 2>&1; then
      echo "[ERROR] 未找到 clash-tui 预编译二进制，且系统中未安装 cargo。"
      echo "        如需从源码编译，请先安装 Rust:"
      echo "          curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
      echo "          或 brew install rust"
      exit 1
    fi

    echo "正在使用 cargo 编译 release 二进制..."
    cargo build --release
    EXE="target/release/clash-tui"
  fi

  if [ ! -f "$EXE" ]; then
    echo "[ERROR] 二进制文件 $EXE 不存在。"
    exit 1
  fi

  echo "[2/3] 安装到 $INSTALL_DIR ..."
  mkdir -p "$INSTALL_DIR/bin"
  cp "$EXE" "$INSTALL_DIR/clash-tui"
  chmod +x "$INSTALL_DIR/clash-tui"

  # 复制内核与初始配置
  if [ ! -f "$INSTALL_DIR/bin/mihomo" ]; then
    if [ -f bin/mihomo ]; then
      cp bin/mihomo "$INSTALL_DIR/bin/mihomo"
      chmod +x "$INSTALL_DIR/bin/mihomo"
    elif [ -f bin/mihomo/mihomo ]; then
      cp bin/mihomo/mihomo "$INSTALL_DIR/bin/mihomo"
      chmod +x "$INSTALL_DIR/bin/mihomo"
    else
      echo "[WARNING] 未在 bin/mihomo 找到内核文件。"
      echo "          可前往 https://github.com/MetaCubeX/mihomo/releases 下载 darwin/linux 版本"
      echo "          并放置于 $INSTALL_DIR/bin/mihomo (chmod +x)。"
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

  echo "[3/3] 创建全局 'clash-tui' 命令..."
  LINK_DIR=""
  if [ -w /usr/local/bin ]; then
    LINK_DIR="/usr/local/bin"
  else
    LINK_DIR="$HOME/.local/bin"
    mkdir -p "$LINK_DIR"
    case ":$PATH:" in
      *":$LINK_DIR:"*) ;;
      *)
        if [ -n "$ZSH_VERSION" ] || [ -f "$HOME/.zshrc" ]; then
          printf '\nexport PATH="$HOME/.local/bin:$PATH"\n' >> "$HOME/.zshrc"
          echo "[INFO] 已将 ~/.local/bin 追加到 ~/.zshrc 的 PATH 中"
        elif [ -f "$HOME/.bashrc" ]; then
          printf '\nexport PATH="$HOME/.local/bin:$PATH"\n' >> "$HOME/.bashrc"
          echo "[INFO] 已将 ~/.local/bin 追加到 ~/.bashrc 的 PATH 中"
        fi
        ;;
    esac
  fi
  ln -sf "$INSTALL_DIR/clash-tui" "$LINK_DIR/clash-tui"

  # 开机自启配置
  echo ""
  if [ "$AUTOSTART_CHOICE" = "yes" ]; then
    enable_autostart
  elif [ "$AUTOSTART_CHOICE" = "no" ]; then
    echo "ℹ 已跳过开机自启配置。"
  else
    # 交互询问
    echo "=========================================================="
    printf "是否配置开机后台静默自启（推荐：系统登录时自动运行代理）？[Y/n]: "
    read -r ans < /dev/tty || ans="y"
    case "$ans" in
      n|N)
        echo "ℹ 已跳过自启配置。后续如需开启可随时运行: bash install.sh --autostart-on"
        ;;
      *)
        enable_autostart
        ;;
    esac
  fi

  echo ""
  echo "=========================================================="
  echo "✔ [安装成功] Clash TUI 已就绪！"
  echo ""
  echo "在任意终端中直接输入以下命令运行:"
  echo "    clash-tui"
  echo ""
  echo "服务管理命令速查:"
  echo "  开启开机自启:  bash install.sh --autostart-on (或 clash-tui autostart on)"
  echo "  关闭开机自启:  bash install.sh --autostart-off (或 clash-tui autostart off)"
  echo "  查看当前状态:  bash install.sh --status (或 clash-tui autostart status)"
  echo "  完整卸载软件:  bash install.sh --uninstall"
  echo "=========================================================="
}

# 命令行参数分发
case "$1" in
  -h|--help)
    show_help
    exit 0
    ;;
  -u|--uninstall)
    do_uninstall
    exit 0
    ;;
  --autostart-on|--enable-autostart)
    enable_autostart
    exit 0
    ;;
  --autostart-off|--disable-autostart)
    disable_autostart
    exit 0
    ;;
  --status)
    show_status
    exit 0
    ;;
  -a|--autostart)
    do_install "yes"
    exit 0
    ;;
  --no-autostart)
    do_install "no"
    exit 0
    ;;
  *)
    do_install "ask"
    exit 0
    ;;
esac
