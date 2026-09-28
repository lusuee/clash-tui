# Clash TUI - 高性能跨平台（Windows & macOS）现代终端客户端

基于 **Rust + Ratatui + Tokio + Crossterm** 打造的高颜值、极轻量 Clash/Mihomo 终端交互客户端。专为解决主流 GUI 客户端内存臃肿、启动缓慢、依赖繁复的问题而生。

---

## 核心特性

- ⚡ **极致轻量与瞬间秒开**：
  - 单一原生编译二进制（仅 ~9MB），**内存占用低至 8MB~15MB**，冷启动 `<10ms`。
  - 无需安装 Python 运行环境或 Node.js/Chromium 开销。
- 🎨 **赛博科技感 HUD 终端美学**：
  - 采用深空深渊底色配合高对比度 Neon 赛博（Electric Cyan、Matrix Emerald、Electric Violet 等）调色盘。
  - 顶部双舱位一体化 **系统遥测中枢与动态流量波形（Sparkline）**，实时呈现上下行流量脉冲趋势与系统代理/TUN/端口状态胶囊。
  - 全面采用现代化圆角边框（Rounded Borders）与模块编号导航卡片（`01 PROXIES`、`02 CONNECTIONS`...）。
  - 节点列表配备多级延迟状态指示仪表（`⚡` 超低延时 `<100ms` / `●` 良好 `<200ms` / `▲` 中等 `<350ms` / `◆` 较高 `<600ms` / `■` 延迟）与协议彩色标签。
  - 内置 Emoji 字体排版保护（自动规避现代终端中天气、国旗等宽字符与英文字母的渲染重叠）。
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
- 🛡 **自定义代理域名规则 (Rules, `5`)**：
  - 在 Rules 页按 `a` 录入域名，自动持久化至 `rules.json` 并注入当前生效配置的 `rules` 列表顶部（优先级最高）。
  - 支持三种匹配语法：`example.com`（DOMAIN-SUFFIX 含子域名）/ `full:example.com`（DOMAIN 精确匹配）/ `keyword:google`（DOMAIN-KEYWORD 关键字）。
  - 目标代理组从订阅配置 `proxy-groups` 自动解析（优先含 `proxy` 的通用组），新增/删除后自动通过 REST API 热重载内核即时生效。
- 📊 **活跃连接追踪与控制**：
  - 实时监视活跃 TCP/UDP 连接、目标 Host、传输速率与命中分流规则。
  - 支持 `d` 终止单个连接，`D` 终止所有活动连接。

---

## 快捷键速查表

| 按键 | 说明 |
| :--- | :--- |
| `Tab` / `BackTab` | 轮转切换标签页（或按数字键 `1`:代理 / `2`:连接 / `3`:订阅 / `4`:设置 / `5`:规则） |
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
| `o` | 轮转切换节点排序（`Default` 原始顺序 → `Name` 按名称 → `Ping` 按延迟升序）（代理页） |
| `a` | 弹出窗口新增订阅源（订阅页） / 新增代理域名规则（规则页） |
| `u` / `U` | 异步更新选中的订阅源 / 一键并发更新所有订阅源（订阅页） |
| `x` | 删除选中的订阅配置（订阅页） / 删除选中的代理域名规则（规则页） |
| `d` / `D` | 终止选中的网络连接 / 终止所有活动网络连接（连接页） |
| `A` | **以管理员身份重启 Mihomo 内核 (UAC 提权)**（设置页） |
| `s` / `S` | 手动终止 / 普通重启后台 Mihomo 内核进程（设置页） |
| `r` | 手动强制刷新数据与重载本地订阅 |
| `q` / `Ctrl+C` | 退出客户端（内核继续常驻后台，代理不中断） |
| `Q` (Shift+Q) | 退出客户端**并停止后台 Mihomo 内核**（一键彻底退出） |

---

## 运行与使用（纯单二进制设计）

本项目已全面消除所有外部脚本依赖。所有功能（图形界面、全局环境变量配置、开机自启、服务管理）均由单一原生可执行文件 `clash-tui` 原生提供。

### 1. 直接运行

- **Windows**：直接在文件管理器中**双击 `clash-tui.exe`** 即可在终端窗口中秒级打开；或在终端中运行：
  ```powershell
  .\clash-tui.exe
  # 或指定远程 controller 与 secret
  .\clash-tui.exe -u http://127.0.0.1:9090 --secret your_secret
  ```
- **macOS / Linux**：
  ```bash
  ./clash-tui
  ```

---

### 2. 全局安装（自动配置环境变量 PATH）

无需手动编辑系统 PATH。在任意解压位置运行 `install` 命令：

```bash
# 自动将当前目录写入系统/用户 PATH，即刻支持全局命令
clash-tui install

# 或一步到位：配置全局 PATH + 启用开机后台静默自启
clash-tui install --autostart
```

安装完成后，打开一个**新终端**，即可在任意工作目录下敲 `clash-tui` 使用。

- **卸载**：
  ```bash
  clash-tui uninstall
  ```
  自动从 PATH 中移除，并清理相关的开机自启计划任务或后台服务。

---

### 3. 开机自启服务管理（静默接管，无黑框）

Mihomo 内核可在开机或登录后由系统自动拉起并在后台静默代理，开机即用，零黑框弹窗。日常需要调节点或测速时，随时敲 `clash-tui` 打开控制面板，退出终端（`q`）也不会中断后台网络：

```bash
# 查看当前自启状态
clash-tui autostart status

# 开启开机静默自启（Windows 自动 UAC 提权并以无黑框静默运行）
clash-tui autostart on

# 关闭开机自启
clash-tui autostart off
```

---

### 4. 本地重新编译

```bash
cargo build --release
```

- **GitHub Actions 云端自动构建**：已配置 `.github/workflows/release.yml`。推送 `v*` 格式标签（如 `git tag v0.1.0 && git push origin v0.1.0`）时，GitHub 将自动为 macOS、Windows、Linux 构建原生可执行文件。

---

## 目录结构

```
clash-tui/
├── clash-tui.exe / clash-tui # 原生编译可执行程序（单二进制）
├── bin/                      # Mihomo 内核放置目录 (mihomo.exe / mihomo)
├── data/                     # 内核配置 (config.yaml) 与运行日志 (mihomo.log)
├── profiles/                 # 本地订阅配置存档 (.yaml)
├── subscriptions.json        # 订阅元数据存档
├── rules.json                # 自定义代理域名规则存档
├── src/                      # 全部 Rust 源码
│   ├── main.rs               # 异步主事件循环、键盘交互、CLI 命令分发
│   ├── app.rs                # 核心应用状态机、焦点管理、流量历史队列
│   ├── ui.rs                 # Ratatui 现代界面渲染器、Sparkline 流量波形
│   ├── theme.rs              # Cyber Neon HUD 调色板与组件样式定义
│   ├── installer.rs          # 纯原生跨平台环境变量 PATH 配置与卸载
│   ├── envproxy.rs           # 跨平台环境变量代理 (Windows 注册表 + Shell 脚本生成)
│   ├── sysproxy.rs           # 跨平台系统代理切换 (WinINet / networksetup)
│   ├── core.rs               # 内核进程管理、静默自启引导与 UAC 提权
│   ├── subscriptions.rs      # 订阅增删改查、YAML 解析、配置热更新
│   ├── rules.rs              # 自定义代理域名规则持久化与注入
│   └── api.rs                # Mihomo RESTful 控制器交互客户端
└── Cargo.toml                # Rust 项目清单
```
