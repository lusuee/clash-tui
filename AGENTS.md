# AGENTS.md — AI 代理开发指南

## 项目速览

**Clash TUI**：Rust 编写的 Mihomo/Clash 终端控制面板（Ratatui TUI）。自身不实现代理，而是管理 Mihomo 内核守护进程（`bin/mihomo(.exe)`），通过本地 YAML（`data/config.yaml`）+ REST API（`127.0.0.1:9090`）控制内核。

- 平台：Windows（主）、macOS（sysproxy/envproxy 已适配）
- 端口约定：mixed-port 默认 **7897**，外部控制器 **9090**
- 详细架构见 [ARCHITECTURE.md](./ARCHITECTURE.md)（模块地图、事件循环、数据流、API 清单、扩展 How-to）

## 常用命令

```powershell
cargo check                    # 类型检查（快速反馈）
cargo build --release          # 构建 target/release/clash-tui.exe
.\target\release\clash-tui.exe # 运行（可附 -u <api-url> --secret <secret>）
clash-tui install              # 全局安装到用户 PATH（可选 --autostart）
clash-tui autostart <on|off>   # 管理开机自启
clash-tui uninstall            # 卸载（移除 PATH，清理自启服务）
```

无测试框架。验证标准见文末「完成定义」。

注意：构建 release 前需先退出正在运行的 `clash-tui.exe`（Windows 链接器无法覆盖被占用的 exe，报 os error 5）。

## 代码地图（改动前先看这里，避免盲扫）

| 需求 | 去哪改 |
|---|---|
| 键盘按键、事件分发、CLI命令 | `src/main.rs`（`handle_key_event`，模态分支在函数顶部优先处理） |
| Tab/光标/状态/弹窗数据模型 | `src/app.rs`（`App`、`ActiveTab`、导航方法） |
| 界面渲染 | `src/ui.rs`（`render` 分发 + `render_*_tab/modal`，纯渲染无 I/O） |
| REST 调用 | `src/api.rs`（`ClashClient`，可 Clone 进 tokio task） |
| 全局安装/PATH管理 | `src/installer.rs`（纯原生注册/注销 PATH） |
| 内核进程启停/开机自启/UAC 提权 | `src/core.rs`（静默 VBS + 计划任务/LaunchAgents/systemd） |
| 订阅下载/激活/生效配置写入 | `src/subscriptions.rs` |
| 自定义代理域名规则 | `src/rules.rs`（持久化 rules.json + YAML 注入） |
| 系统代理/环境变量代理 | `src/sysproxy.rs` / `src/envproxy.rs` |
| 颜色样式 | `src/theme.rs`（统一走 `Theme::*`，勿硬编码色值） |

## 必须遵守的架构约定

1. **事件驱动**：所有异步操作用 `tokio::spawn`，结果经 `mpsc` 的 `AppEvent`（定义在 `main.rs`）回主循环更新 `App` 状态；禁止在渲染路径或按键处理里同步阻塞/做网络请求。
2. **单一可变状态**：业务状态只放 `App`；渲染函数只读（`ui::render(f, &mut app)` 中仅更新 TableState 光标）。
3. **配置生效流水线**（让任何配置改动生效的唯一正确姿势）：
   写 `data/config.yaml` → 调 `crate::rules::apply_rules_to_config()`（重注入自定义规则）→ `client.reload_config(&get_clean_config_path()).await`。直接改内核其他路径无效。
4. **YAML 操作用 `serde_yaml::Value`**：注入字段（external-controller、mixed-port、rules）而非重建结构，保留订阅其余内容。
5. **状态提示**：成功前缀 `✔`、失败前缀 `✘`（`app.set_status()`），TUI 中可混用中英文。
6. **gitignore 文件禁提交**：`bin/`、`data/`、`profiles/`、`subscriptions.json`、`rules.json`、`target/`（订阅 URL 与规则含用户隐私）。
7. **路径全部相对 cwd**（`bin/`、`data/`、`profiles/`、两个 json），正确性由 `main()` 开头的 `resolve_working_dir()`（main.rs）保证：cwd 有数据则用 cwd，否则切到 exe 目录（便携全局安装）。勿在模块中自行引入绝对路径或绕过该机制。

## 环境坑（重要）

- **工具链为 `stable-x86_64-pc-windows-gnu`**，其精简 mingw 缺 `libwininet.a` 与 `dlltool.exe`：
  - `sysproxy.rs` 中 WinINet 必须通过 `LoadLibraryW`/`GetProcAddress` 运行时动态解析 —— **勿改回 `#[link(name = "wininet")]` 或 `raw-dylib`**，都会链接失败；
  - `kernel32`/`user32` 静态链接正常；
  - MSVC 工具链已装但缺 Build Tools 链接器，不可用。
- `run.bat` 优先运行 `target/release/clash-tui.exe`，改完代码必须重新 `cargo build --release` 才能在 `run.bat` 中看到效果。
- YAML dump 会丢注释；`data/config.yaml` 是生成物，勿手工编辑后指望持久。

## 常见任务配方

- **新增 Tab / 快捷键 / 弹窗 / REST 封装 / 持久化数据**：按 `ARCHITECTURE.md` §9 的分步指南执行（以 Rules 页为现成范本，照抄结构最稳）。
- **新增功能涉及让流量规则生效**：走约定 3 的流水线，规则注入逻辑放 `rules.rs`。
- **README 同步**：改动快捷键或新增功能后，更新 `README.md` 的「快捷键速查表」与「核心特性」。

## 完成定义（DoD）

1. `cargo check` 通过且无 warning；
2. `cargo build --release` 成功（工具链坑见上）；
3. 若涉及 UI/按键：README 快捷键表已同步；
4. 若涉及配置写入：确认规则注入（`apply_rules_to_config`）与热重载路径未被绕过。
