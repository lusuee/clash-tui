# Clash TUI 架构文档

面向后续开发的架构与功能说明，目标是读完本文即可上手开发，无需扫描整个项目。

---

## 1. 项目概述

基于 **Rust + Ratatui + Tokio + Crossterm** 的 Clash/Mihomo 终端客户端（TUI）。
本程序**不实现代理协议**，而是作为 Mihomo 内核（`bin/mihomo(.exe)`，独立守护进程）的控制面板：

- 通过 **本地 YAML 配置文件**（`data/config.yaml`）控制内核的路由规则、端口等；
- 通过 **Mihomo RESTful API**（默认 `http://127.0.0.1:9090`）实时切换节点/模式/TUN/热重载配置。

```
┌─────────────── 终端 (TUI) ───────────────┐
│  clash-tui.exe (Rust)                    │
│  ┌─────────┐  事件   ┌────────────────┐  │
│  │ 输入监听 │ ───────▶ │  主事件循环     │  │
│  └─────────┘  mpsc   │  App (状态机)   │  │
│  ┌─────────┐ 轮询事件 │       │        │  │
│  │ 后台轮询 │ ───────▶ │  ui::render()  │  │
│  └─────────┘          └────────────────┘  │
└──────┬──────────────────┬─────────────────┘
       │ 读写              │ REST API (HTTP)
       ▼                  ▼
  data/config.yaml    Mihomo 内核 (127.0.0.1:9090)
  rules.json          ├─ 代理端口 (默认 mixed-port 7897)
  subscriptions.json  └─ 控制器 127.0.0.1:9090
  profiles/*.yaml
```

技术栈与依赖（`Cargo.toml`）：`tokio`（异步运行时）、`ratatui`/`crossterm`（TUI）、`reqwest`（HTTP 客户端）、`serde`/`serde_json`/`serde_yaml`（序列化与 YAML 注入）、`chrono`、`winreg`（Windows 注册表）。

---

## 2. 目录结构

```
clash-tui/
├── src/                  # 全部 Rust 源码（单二进制）
├── bin/                  # mihomo 内核二进制（gitignored，运行时放置）
├── data/                 # 内核运行目录（gitignored）：config.yaml、mihomo.log、
│                         # cache.db、geoip/geosite 数据、Rules/RuleSet
├── profiles/             # 订阅原始下载存档 {sub_id}.yaml（gitignored）
├── subscriptions.json    # 订阅元数据（gitignored，含私密 URL）
├── rules.json            # 用户自定义代理域名规则（gitignored）
├── env.nu/.bat/.ps1/.sh  # 环境变量代理脚本（由 envproxy.rs 运行时生成，已跟踪）
├── run.bat               # 启动脚本：运行 target/release/clash-tui.exe
├── install.bat / uninstall.bat       # Windows 全局安装 / 卸载（%LOCALAPPDATA%\clash-tui + 用户 PATH）
├── install.sh / uninstall.sh         # macOS/Linux 全局安装 / 卸载（~/.clash-tui + symlink）
└── Cargo.toml
```

---

## 3. 模块地图

| 文件 | 职责 | 关键类型 / 函数 |
|---|---|---|
| `src/main.rs` | 程序入口、事件循环、**所有键盘事件分发**、后台任务 | `main`、`AppEvent`、`handle_key_event`、`get_clean_config_path` |
| `src/app.rs` | 核心状态机 `App`（唯一可变状态）、Tab/焦点/光标、弹窗模型、格式化工具 | `App`、`ActiveTab`、`ProxyFocus`、`AddSubModal`、`AddRuleModal`、`update_proxies` |
| `src/ui.rs` | 纯渲染层（Cyber HUD 一体化面板、全息弹窗、Emoji 宽度安全排版），无业务逻辑 | `render`、`render_*_tab`、`render_add_sub_modal`、`format_display_name` |
| `src/api.rs` | Mihomo REST 客户端（可 Clone，供 tokio task 持有） | `ClashClient` |
| `src/core.rs` | Mihomo 内核守护进程生命周期管理 | `CoreManager::{start_core, stop_core, start_core_as_admin, is_running, ensure_default_config}` |
| `src/subscriptions.rs` | 订阅 CRUD、下载解析、**生成本地生效配置** | `SubscriptionManager::{add, delete, set_active, update_subscription}`、`parse_node_count` |
| `src/rules.rs` | 自定义代理域名规则：持久化、解析、YAML 注入 | `RuleManager`、`parse_domain_input`、`resolve_proxy_target`、`inject_domain_rules`、`apply_rules_to_config` |
| `src/sysproxy.rs` | 系统代理（WinINet / macOS networksetup） | `SysProxy::{get_status, set_proxy}` |
| `src/envproxy.rs` | 环境变量代理 + shell 脚本生成 | `EnvProxy::{get_status, set_env_proxy, generate_shell_scripts}` |
| `src/theme.rs` | Cyber Neon HUD 调色板与组件样式 | `Theme::{GREEN, CYAN, LAVENDER, latency_badge, protocol_color, ...}` |

**分层规则**：`main.rs`（输入/事件）→ `app.rs`（状态）→ `ui.rs`（渲染）；网络与系统操作经 `api.rs` / `core.rs` / `subscriptions.rs` / `sysproxy.rs` / `envproxy.rs`，通过 `tokio::spawn` 异步执行，结果以 `AppEvent` 回到主循环更新状态。渲染函数不做 I/O。

---

## 4. 运行时架构（事件驱动）

### 4.1 事件通道

主循环是唯一的 mut 消费者，所有并发结果通过 `tokio::sync::mpsc::unbounded_channel<AppEvent>` 汇聚：

| AppEvent 变体 | 生产者 | 作用 |
|---|---|---|
| `Key(KeyEvent)` | 输入监听 task（`event::poll` 50ms） | 键盘输入 |
| `Tick` | 输入监听 task（空闲时） | 心跳 |
| `Traffic(up, down)` | 流量轮询 task（1s） | 实时速率 + Sparkline 历史 |
| `Connections(...)` | 流量轮询 task（1s） | 连接表 |
| `Version / Configs / Proxies` | 慢速轮询 task（2.5s） | 内核版本、模式/端口/TUN、代理组 |
| `DelayResult { node, result }` | 按 `t`/`T` 触发的测速 task | 延迟结果 |
| `SubUpdated / SubActivated` | 订阅更新/激活 task | 状态提示 + 触发重载 |
| `CoreActionDone` | 内核启停 task | 状态提示 |
| `TunToggled / PortChanged` | TUN/端口修改 task | 回读实际状态并联动 SysProxy/EnvProxy |

### 4.2 启动流程（`main()`）

1. 解析参数 `-u/--url`（默认 `http://127.0.0.1:9090`）、`--secret`；
2. `resolve_working_dir()`：**工作目录解析**（全局安装的关键）。当前目录已存在 clash-tui 数据（`subscriptions.json` / `rules.json` / `bin/mihomo.exe` / `data/config.yaml` 任一）则保持 cwd；否则 `set_current_dir` 到 exe 所在目录——通过 PATH 全局启动时数据/内核跟随安装目录（`%LOCALAPPDATA%\clash-tui`，由 `install.bat` 创建），不会散落在用户任意 cwd；
3. `CoreManager::is_running()` 探测 → 未运行且已安装则 `start_core()`（detached 后台进程）；
4. 设置 panic hook（恢复终端）→ 进入 AlternateScreen；
5. 启动输入监听、流量轮询（1s）、慢速轮询（2.5s）三个 tokio task；
6. `App::new()`（读取 SysProxy/EnvProxy 状态、加载 subscriptions.json 与 rules.json）；
7. 进入主循环 `draw → recv → dispatch`。

---

## 5. 关键机制

### 5.1 订阅同步流水线（核心路径）

**本程序所有"让配置生效"的路径都收敛到同一流水线**：

```
订阅 URL ──update_subscription──▶ profiles/{sub_id}.yaml（原始下载存档）
                                        │
                          set_active(idx, mixed_port)
                                        ▼
              读 profile → serde_yaml 解析为 Value
              注入 external-controller=127.0.0.1:9090、mixed-port
              写入 data/config.yaml          ← 内核实际加载的文件
                                        │
              crate::rules::apply_rules_to_config()   ← 注入自定义域名规则
                                        ▼
              client.reload_config(get_clean_config_path())
              （PUT /configs?force=true，path 为去掉 \\?\ 前缀的绝对路径）
```

- `set_active`：激活订阅（Enter）时调用；
- `update_subscription`：更新订阅（`u`/`U`）后，若该订阅是 active 则自动重写配置；
- reload 失败无自动重启（Python 旧版有 restart fallback，已移除），用户可按 `S` 重启内核；
- Mock 订阅（URL 含 `example.com`/`demo`）返回假数据用于演示。

### 5.2 自定义代理域名规则（Rules 页，按键 `5`）

**功能**：用户录入域名，使其流量强制走代理。

**持久化** `rules.json`（JSON 数组）：

```json
[
  { "rule_type": "DOMAIN-SUFFIX", "value": "example.com" },
  { "rule_type": "DOMAIN", "value": "api.example.com" },
  { "rule_type": "DOMAIN-KEYWORD", "value": "openai" }
]
```

**输入语法**（`parse_domain_input`）：`example.com` → DOMAIN-SUFFIX；`full:example.com` 或 `domain:...` → DOMAIN；`keyword:xxx` → DOMAIN-KEYWORD；`suffix:xxx` → DOMAIN-SUFFIX。含空格/逗号或空值视为非法。

**注入**（`inject_domain_rules`）：

1. `resolve_proxy_target` 从 `data/config.yaml` 选目标代理组，优先级：
   `proxy-groups` 中名字含 `proxy` 的组 → 第一个非 direct/reject 组 → 第一个组 → `proxies[0].name` → `DIRECT`；
2. 生成规则行 `TYPE,VALUE,TARGET`，**插入 `rules` 列表最顶部**（Clash 规则自上而下匹配，顶部即最高优先级，先于订阅自带规则和 `MATCH`）；
3. 通过 `serde_yaml::Value` 操作，保留订阅配置其余字段不变。

**生效时机**：`RuleManager::add/remove` 后、`subscriptions.rs` 两个写配置点之后，统一调用无状态函数 `apply_rules_to_config() -> (规则数, 目标组)`（内部重新读 rules.json + data/config.yaml），随后由调用方 `reload_config` 热重载。订阅切换/内核重启不丢规则。

### 5.3 代理控制三件套

| 功能 | 按键 | 实现 |
|---|---|---|
| 系统代理 | `p` | Windows: 写注册表 `HKCU\...\Internet Settings`（ProxyEnable/ProxyServer）+ `InternetSetOptionW` 通知刷新；macOS: `networksetup`；端口联动 `mixed_port` |
| 环境变量代理 | `e` | Windows: 写 `HKCU\Environment`（http_proxy 等 6 个键）+ `SendMessageTimeoutW(WM_SETTINGCHANGE)` 广播；macOS: `~/.clash_env.sh` + `launchctl`；同时生成 `env.nu/.bat/.ps1/.sh` 与 `unenv.*` 当前目录脚本 |
| TUN 模式 | `n` | `PATCH /configs {"tun":{"enable":bool}}`，300ms 后回读 `/configs` 确认，未生效提示缺管理员权限（设置页按 `A` UAC 提权重启内核） |
| 修改端口 | `P` | 弹窗输入 → `PATCH /configs {"mixed-port":N}` → 成功后联动 SysProxy/EnvProxy 更新 |

### 5.4 内核守护进程管理（`core.rs`）

- `start_core`：`CreateProcess` detached（`DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW`），stdout/stderr 追加至 `data/mihomo.log`，参数 `-d data`；
- `stop_core`：Windows `taskkill /F /IM mihomo.exe`；`S` 键重启 = stop + 500ms + start；
- `start_core_as_admin`（`A`）：PowerShell `Start-Process -Verb RunAs` UAC 提权；
- 启动前 `ensure_default_config`：仅在 `data/config.yaml` 缺失时写入最小配置（mixed-port 7897 / controller 9090）；
- **生命周期与 TUI 退出**：TUI 启动时仅探测未运行才 `start_core`（不是 restart）；`q`/`Ctrl+C` 退出 TUI 后内核常驻；`Q` 退出前调用 `stop_core` 一并停止内核。

### 5.5 Proxies 页逻辑

- 分组 = `/proxies` 返回中 `all` 非空的条目；排序：`GLOBAL` → `Proxy`/`PROXY` → 其余字母序；
- 左右键切换焦点（分组列表 / 节点列表），Enter 对选中分组调用 `PUT /proxies/{group}`；
- 延迟来自 `history[-1].delay` 缓存 + 手动测速（`t` 单个 / `T` 整组，`GET /proxies/{node}/delay`）；
- 节点排序（`o` 循环：`Default` 订阅顺序 → `Name` 名称 → `Ping` 延迟升序，未测速排最后）：在 `App::current_group_nodes()` 统一实现，所有消费方（渲染/Enter 选择/测速）使用同一排序视图。

---

## 6. 数据文件格式

| 文件 | 格式 | 写入者 | 说明 |
|---|---|---|---|
| `subscriptions.json` | `Vec<SubscriptionItem>`：`{id, name, url, active, node_count, last_updated}` | subscriptions.rs | id 格式 `sub_{unix_ts}`，同时是 profile 文件名 |
| `rules.json` | `Vec<RuleItem>`：`{rule_type, value}` | rules.rs | 见 5.2 |
| `profiles/{sub_id}.yaml` | 订阅原始内容（Clash YAML 或 base64 节点列表） | subscriptions.rs | 只存原始内容，不做加工 |
| `data/config.yaml` | Mihomo 完整配置 | subscriptions.rs + rules.rs | 唯一生效配置，内核以 `-d data` 加载 |
| `data/mihomo.log` | 追加日志 | mihomo 内核 | 排障入口 |

**注意**：`serde_yaml::Value` 注入写回会丢失 YAML 注释；`data/`、`profiles/`、`subscriptions.json`、`rules.json`、`bin/` 均已 gitignore，禁止提交（含私密信息）。

---

## 7. Mihomo REST API 交互清单（api.rs）

| 方法 | 端点 | 封装 |
|---|---|---|
| GET | `/version` | `get_version` |
| GET | `/configs` | `get_configs` |
| PATCH | `/configs` | `set_mode`（mode）/ `set_mixed_port`（mixed-port）/ `set_tun`（tun.enable） |
| PUT | `/configs?force=true` | `reload_config(path)` —— 热重载本地配置文件 |
| GET | `/proxies` | `get_proxies` |
| PUT | `/proxies/{group}` | `select_proxy(group, node)` |
| GET | `/proxies/{node}/delay?timeout=&url=` | `test_delay` |
| GET | `/connections` | `get_connections`（含 uploadTotal/downloadTotal） |
| DELETE | `/connections/{id}`、`/connections` | `close_connection` / `close_all_connections` |

认证：`secret` 非空时附 `Authorization: Bearer <secret>`。URL 编码使用内置 `urlencoding_simple`。

---

## 8. 构建与运行

```powershell
cargo check                  # 快速类型检查
cargo build --release        # 产出 target/release/clash-tui.exe
.\run.bat                    # 或直接运行二进制（可带 -u/--secret）
```

- 无测试框架；验证方式 = `cargo check` 零警告 + release 构建成功 + 连接真实内核手动验证；
- **Windows 工具链**：默认 `stable-x86_64-pc-windows-gnu`，其自带精简 mingw **没有 `libwininet.a`**，因此 `sysproxy.rs` 中 WinINet 通过 `LoadLibraryW`/`GetProcAddress` 运行时动态解析（勿改回 `#[link(name="wininet")]` 或 `raw-dylib`，前者链接失败，后者缺 dlltool）。`user32`/`kernel32` 可正常静态链接；
- MSVC 工具链已安装但缺 Build Tools 链接器，暂不可用。

---

## 9. 扩展指南（How-to）

### 新增一个 Tab（以 Rules 页为范本）

1. `app.rs`：`ActiveTab` 加变体（`as usize` 即 Tab 序号）；`App` 加 `xxx_table_state: TableState`、光标索引、业务管理器；`next_tab/prev_tab` 补分支；`on_up/on_down/on_page_up/on_page_down/on_home/on_end` 补光标分支；
2. `ui.rs`：`render()` 的 `match app.active_tab` 加分支；新增 `render_xxx_tab`（照抄 `render_rules_tab` 结构）；`render_tabs` 的 titles 数组按枚举顺序追加；`render_footer` 的 per-tab hints 补充；
3. `main.rs`：数字键 `set_tab(ActiveTab::Xxx)`；per-tab 按键（`a`/`x`/`Enter` 等）在对应 `KeyCode::Char(..)` 分支按 `app.active_tab` 分发；异步结果定义新的 `AppEvent` 变体。

### 新增快捷键

`main.rs::handle_key_event` 加 `KeyCode::Char('k')` 分支 → 同步逻辑放 `app.rs` 方法，异步操作 `tokio::spawn` + `AppEvent` 回传 → `ui.rs::render_footer` 更新提示 → README 快捷键表同步。

### 新增弹窗

参照 `AddRuleModal`（单输入框）：`app.rs` 定义 struct + `show_xxx_modal: Option<...>` + open/close/backspace/handle_char 方法 → `main.rs` 在 `handle_key_event` 顶部（优先于全局按键）加模态分支，Esc 关闭 / Enter 提交 → `ui.rs` 加 `render_xxx_modal`（`centered_rect` + `Clear` widget）。

### 新增 REST 调用

`api.rs` 加方法（照抄 `set_mode`：构造 URL → `request_builder` 附认证 → `send` → 检查 status）→ `main.rs` 用 `app.client.clone()` 移入 `tokio::spawn`。

### 新增持久化数据

新建 `src/xxx.rs`，模式照抄 `RuleManager`：`new()` 载入 + `load/save/add/remove`；在 `main.rs` 声明 `mod xxx;`；若数据需注入 `data/config.yaml`，提供无状态 `apply_to_config()` 函数并在 `subscriptions.rs` 写配置点之后调用。
