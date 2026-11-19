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
- 🛡 **自定义域名路由规则 (Rules, `5`)**：
  - 在 Rules 页按 `a` 录入域名，弹窗内用 `Tab` 选择去向：**DIRECT（直连）** 或 **PROXY（代理组）**，自动持久化至 `rules.json` 并注入当前生效配置的 `rules` 列表顶部（优先级最高）。
  - 支持三种匹配语法：`example.com`（DOMAIN-SUFFIX 含子域名）/ `full:example.com`（DOMAIN 精确匹配）/ `keyword:google`（DOMAIN-KEYWORD 关键字）。
  - 输入可加 `direct:` / `proxy:` 前缀直接指定去向（如 `direct:example.com`、`proxy:keyword:google`），优先于弹窗中的选择。
  - PROXY 目标代理组从订阅配置 `proxy-groups` 自动解析（优先含 `proxy` 的通用组）；列表内按 `t` 可随时切换选中规则的 DIRECT/PROXY，新增/删除/切换后自动通过 REST API 热重载内核即时生效。
  - DIRECT 规则会自动在配置 `dns.nameserver-policy` 中补 `system` 解析策略（`DOMAIN` 精确 / `DOMAIN-SUFFIX` 用 `+.域名`），内网域名不再走订阅里的公共 DNS；只补缺失项、不覆盖订阅已有策略。重复 apply 会自动清掉同域名的旧规则，不再堆积。
- 🧭 **DNS 覆盖编辑器 (Settings → `d`)**：
  - 内置弹窗可覆盖订阅 DNS 的 `nameserver` / `fallback` / `proxy-server-nameserver` / `default-nameserver`，并支持自定义 `域名=服务器` 策略（`;` 分隔多条，写入 `dns.nameserver-policy`，优先于订阅同名策略）。
  - `DIRECT DNS POLICY` 开关（默认开启）控制是否为 DIRECT 规则自动补 `system` 策略，内网域名不再被公共 DNS 解析失败拖累。
  - 保存到 `data/dns.json`，立即合并进 `data/config.yaml` 并 REST 热重载；留空字段沿用订阅值。
- ⬇️ **内核一键下载与更新 (Settings → `K`)**：
  - 自动从官方 `MetaCubeX/mihomo` GitHub Release 探测最新版本，按当前平台挑选资产（Windows `.zip` / Linux、macOS `.gz`），下载解压后替换 `bin/mihomo`。
  - 更新前自动停止内核并备份旧文件，写入成功后按需重启；GitHub 直连失败时自动回退到本地混合端口代理。
  - 内核缺失时同一按键直接完成下载安装，无需再手动翻找内核文件。
- 🔄 **版本显示与自更新 (Settings → `U`)**：
  - 顶栏与设置页显示当前 `clash-tui` 版本（CI 用 Release tag 编译注入，本地构建回退 `git describe`）。
  - 启动 5 秒后静默检查 GitHub Release；发现新版本只提示，不自动下载。
  - 按 `U` 手动下载并原位替换当前可执行文件（Windows 先改名旧文件再写入），退出时自动用新版本重启，不影响内核。
- ⌨️ **全量快捷键帮助 (`?`)**：
  - 任意界面按 `?` 弹出分栏帮助浮层，按作用域列出全部按键（全局/各标签页/弹窗）；`Esc`、`?` 或 `q` 关闭。
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
| `t` | 测速当前高亮节点的延迟（ms）（代理页） / 切换选中规则的 DIRECT/PROXY 去向（规则页） |
| `T` | 一键并发测速当前分组内的所有节点 |
| `o` | 轮转切换节点排序（`Default` 原始顺序 → `Name` 按名称 → `Ping` 按延迟升序）（代理页） |
| `a` | 弹出窗口新增订阅源（订阅页） / 新增域名路由规则（规则页） |
| `u` / `U` | 异步更新选中的订阅源 / 一键并发更新所有订阅源（订阅页）；`U` 在设置页 = **检查并安装 clash-tui 自身更新** |
| `x` | 删除选中的订阅配置（订阅页） / 删除选中的域名路由规则（规则页） |
| `d` | 终止选中的网络连接（连接页） / 打开 **DNS 覆盖编辑器**（设置页） |
| `D` | 终止所有活动网络连接（连接页） |
| `A` | **以管理员身份重启 Mihomo 内核 (UAC 提权)**（设置页） |
| `K` (Shift+K) | **检查并下载/更新 Mihomo 内核**（设置页；自动停内核 → 替换 → 重启，缺失时直接安装） |
| `?` | 打开 / 关闭**全量快捷键帮助浮层**（`Esc`、`?` 或 `q` 关闭） |
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

## 常见问题排查

### 提示「找不到 mihomo」/ 设置页显示 `[✘ MISSING]`

程序按「当前工作目录优先、可执行文件所在目录兜底」的顺序定位内核，均接受以下三种放置方式
（Windows 对应 `mihomo.exe`）：

```
<目录>/mihomo          # 直接放在根目录
<目录>/bin/mihomo      # 推荐
<目录>/bin/mihomo/mihomo
```

判定「某目录是数据目录」只认内核二进制，`data/config.yaml`、`rules.json` 等程序自己生成的产物
不作为依据。因此：

- 内核与 `clash-tui` 放在一起时，在**任意目录**执行 `clash-tui` 都能正常启动，数据始终跟随安装目录；
- 旧版本曾把 `data/config.yaml` 等生成物当作判断依据，因此在 `bin/` 这类子目录里误启动过一次后，
  该目录会被永久当作数据目录，内核路径变成 `<目录>/bin/bin/mihomo`，从而一直报「找不到 mihomo」。
  当前版本已修复，这类残留文件也可以直接删除。

排查命令：

```bash
ls -l ./bin/mihomo            # 或 ./mihomo、./bin/mihomo/mihomo
chmod +x ./bin/mihomo         # macOS / Linux 确认可执行
```

设置页 `Binary Image` 一行会显示程序**实际使用**的绝对路径，缺失时附 `[✘ MISSING]` 标记，可直接对照。
