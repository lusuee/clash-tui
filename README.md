# Clash TUI - 高性能跨平台（Windows & macOS）现代终端客户端

基于 **Rust + Ratatui + Tokio + Crossterm** 打造的高颜值、极轻量 Clash/Mihomo 终端交互客户端。专为解决主流 GUI 客户端内存臃肿、启动缓慢、依赖繁复的问题而生。

---

## 核心特性

- ⚡ **极致轻量与瞬间秒开**：
  - 单一原生编译二进制（仅 ~9MB），**内存占用低至 8MB~15MB**，冷启动 `<10ms`。
  - 无需安装 Python 运行环境或 Node.js/Chromium 开销。
- 🎨 **Catppuccin Mocha 现代暗色美学**：
  - 采用优雅柔和的 Catppuccin Mocha 终端调色盘。
  - 顶部嵌入 **Ratatui Sparkline 动态折线波形图**，毫秒级实时呈现上下行流量脉冲趋势。
  - 节点列表采用 Nerd Fonts 与彩色协议药丸胶囊（`[SS]`, `[Trojan]`, `[VMess]`, `[Hy2]`），延迟阶梯指示灯（绿 `<150ms` / 黄 `<300ms` / 红 `>300ms`）。
- 🌐 **多维度网络代理控制**：
  - **系统代理 (SysProxy, `p`)**：
    - Windows：直接调用 WinINet 底层 API 刷新系统网络栈，浏览器即时生效。
    - macOS：内置 `networksetup` 原生网卡探测与自动 HTTP/HTTPS/SOCKS 代理切换。
  - **一键环境变量代理 (EnvProxy, `e`)**：
    - Windows 注册表持久化（`HKCU\Environment`）并广播系统刷新，新建终端与应用自动走代理。
    - 客户端目录自动生成 `env.nu` / `env.bat` / `env.ps1`，当前 Shell（如 Nushell）运行 `source env.nu` 秒级生效。
  - **TUN 虚拟网卡模式 (TUN Mode, `n`)**：
    - 通过 Mihomo REST API 实时开启/关闭底层虚拟网卡接管。
    - 贴心管理员权限诊断，设置页支持按 `A` 键调用 UAC 提权拉起内核。
  - **动态修改代理端口 (Mixed Port, `P`)**：
    - 全局按 `P` 键弹出输入弹窗，支持 1~65535 任意合法端口，修改后 API 热生效并同步联动系统代理与环境变量代理。
- 📦 **多订阅中心管理**：
  - 本地化集中管理多条订阅源（自动持久化至 `subscriptions.json`）。
  - 支持按 `a` 键弹出浮层弹窗录入名称与链接。
  - 支持快捷键单选更新（`u`）、并发批量更新（`U`）、删除（`x`）。
  - 支持按 `Enter` 激活订阅并自动热重载 Mihomo 内核生效。
- 📊 **活跃连接追踪与控制**：
  - 实时监视活跃 TCP/UDP 连接、目标 Host、传输速率与命中分流规则。
  - 支持 `d` 终止单个连接，`D` 终止所有活动连接。

---

## 快捷键速查表

| 按键 | 说明 |
| :--- | :--- |
| `Tab` / `BackTab` | 轮转切换标签页（或按数字键 `1`:代理 / `2`:连接 / `3`:订阅 / `4`:设置） |
| `↑` / `↓` 或 `k` / `j` | 列表上下移动光标 |
| `←` / `→` 或 `h` / `l` | 在代理页切换「分组列表」与「节点列表」焦点 |
| `Enter` | 切换选中节点为活动节点（代理页） / 激活并应用选中订阅（订阅页） |
| `m` | 轮转切换模式（`Rule` 规则 -> `Global` 全局 -> `Direct` 直连） |
| `p` | 实时开启 / 关闭 Windows/macOS 系统全局代理 |
| `e` | 实时开启 / 关闭 **终端环境变量代理**（注册表持久化 + 自动更新脚本） |
| `n` | 实时开启 / 关闭 **TUN 虚拟网卡模式** |
| `P` (Shift+P) | 弹出修改 **混合代理端口 (Mixed Port)** 交互表单 |
| `t` | 测速当前高亮节点的延迟（ms） |
| `T` | 一键并发测速当前分组内的所有节点 |
| `a` | 弹出窗口新增订阅源（订阅页） |
| `u` / `U` | 异步更新选中的订阅源 / 一键并发更新所有订阅源（订阅页） |
| `x` | 删除选中的订阅配置（订阅页） |
| `d` / `D` | 终止选中的网络连接 / 终止所有活动网络连接（连接页） |
| `A` | **以管理员身份重启 Mihomo 内核 (UAC 提权)**（设置页） |
| `s` / `S` | 手动终止 / 普通重启后台 Mihomo 内核进程（设置页） |
| `r` | 手动强制刷新数据与重载本地订阅 |
| `q` / `Ctrl+C` | 退出客户端（内核继续常驻后台，代理不中断） |

---

## 运行与构建

### 直接运行

项目已编译完成 release 二进制，双击根目录 `run.bat` 或在终端运行：

```powershell
# 切换到目录
cd C:\Users\fm\.gemini\antigravity\scratch\clash-tui

# 运行 Rust 原生客户端（默认连接 127.0.0.1:9090，代理端口 7897）
.\target\release\clash-tui.exe

# 或指定远程 controller 与 secret
.\target\release\clash-tui.exe -u http://127.0.0.1:9090 --secret your_secret
```

### 重新编译

```powershell
cargo build --release
```

---

## 目录结构

```
clash-tui/
├── src/
│   ├── main.rs          # 异步主事件循环、键盘交互、后台任务监听
│   ├── app.rs           # 核心应用状态机、焦点管理、流量历史队列
│   ├── ui.rs            # Ratatui 现代界面渲染器、Sparkline 流量波形、弹窗
│   ├── theme.rs         # Catppuccin Mocha 调色板与组件样式定义
│   ├── envproxy.rs      # 跨平台环境变量代理 (Windows 注册表 + Shell 脚本生成)
│   ├── sysproxy.rs      # 跨平台 (Windows WinINet / macOS networksetup) 系统代理切换
│   ├── core.rs          # 跨平台 (Windows/macOS) 内核守护进程管理与 UAC 提权
│   ├── subscriptions.rs # 订阅增删改查、YAML/Base64 解析、配置热更新
│   └── api.rs           # Mihomo RESTful 控制器交互客户端 (TUN/Port/Proxy)
├── env.nu / unenv.nu    # Nushell 代理环境变量即时加载 / 清除脚本
├── env.bat / unenv.bat  # CMD 代理环境变量即时加载 / 清除脚本
├── env.ps1 / unenv.ps1  # PowerShell 代理环境变量即时加载 / 清除脚本
├── bin/                 # Mihomo 内核放置目录 (mihomo.exe / mihomo)
├── data/                # 内核配置 (config.yaml) 与运行日志 (mihomo.log)
├── profiles/            # 本地订阅配置存档 (.yaml)
├── subscriptions.json   # 订阅元数据存档
├── run.bat              # 便捷启动脚本
└── Cargo.toml           # Rust 项目清单
```
