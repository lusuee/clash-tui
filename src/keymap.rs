//! 快捷键帮助与页脚提示的唯一数据源。
//!
//! 帮助浮层（`?`）里的键位表、“共 N 项”计数，以及底部两行页脚提示都从这里生成，
//! 避免新增按键时只改了界面、忘了文档（或反过来），也避免帮助与页脚写出两套按键名。
//! 实际按键分发仍在 `main.rs::handle_key_event`，新增按键时请同时更新本表与分发逻辑。

use ratatui::style::Color;

use crate::theme::Theme;

/// 一条键位说明：`keys` 是展示文本（可含组合，如 `q / Ctrl+C`），`desc` 是作用。
#[derive(Debug, Clone, Copy)]
pub struct Binding {
    pub keys: &'static str,
    pub desc: &'static str,
}

/// 一组键位：`title` 同时决定帮助浮层里的分组标题与配色（见 `ui::help_section`）。
#[derive(Debug, Clone, Copy)]
pub struct Section {
    pub title: &'static str,
    pub bindings: &'static [Binding],
}

const fn b(keys: &'static str, desc: &'static str) -> Binding {
    Binding { keys, desc }
}

const fn s(title: &'static str, bindings: &'static [Binding]) -> Section {
    Section { title, bindings }
}

/// 一条页脚提示：`keys` 是页脚上展示的按键，`label` 是短说明（页脚空间有限，
/// 可以和帮助浮层里的完整描述不同，但按键名必须与帮助表一致），`color` 决定按键高亮色。
#[derive(Debug, Clone, Copy)]
pub struct FooterHint {
    pub keys: &'static str,
    pub label: &'static str,
    pub color: Color,
}

const fn f(keys: &'static str, label: &'static str, color: Color) -> FooterHint {
    FooterHint { keys, label, color }
}

const GLOBAL: &[Binding] = &[
    b("q / Ctrl+C", "退出（内核继续后台运行）"),
    b("Q", "退出并停止内核（关闭代理与 TUN）"),
    b("? / Esc", "打开 / 关闭本帮助"),
    b("Tab / Shift+Tab", "切换标签页"),
    b("1 … 5", "直达对应标签页"),
    b("j k ↑ ↓", "上下移动选中项"),
    b("g G Home End", "跳到首项 / 末项"),
    b("PgUp / PgDn", "整页滚动"),
    b("h l ← →", "左右切换代理组 / 字段"),
    b("Enter", "激活订阅 / 选择节点"),
    b("m", "循环切换代理模式"),
    b("p / e", "系统代理 / 环境代理"),
    b("n", "TUN 虚拟网卡开关"),
    b("P", "修改混合代理端口"),
    b("r", "重载本地订阅与配置"),
];

const CONNECTIONS: &[Binding] = &[b("d / D", "关闭所选 / 全部连接")];

const PROXIES: &[Binding] = &[
    b("Enter", "切换所选组内节点"),
    b("t / T", "测速节点 / 整组节点"),
    b("o", "切换节点排序"),
];

const SUBSCRIPTIONS: &[Binding] = &[
    b("a", "新增订阅"),
    b("e", "编辑名称 / URL"),
    b("u / U", "更新所选 / 全部订阅"),
    b("i", "切换自动更新间隔"),
    b("Enter", "激活并重载内核"),
    b("x", "删除所选订阅"),
];

const SETTINGS: &[Binding] = &[
    b("d", "DNS 覆盖编辑器"),
    b("C", "运行配置覆盖（mode / 日志级别 / LAN）"),
    b("B", "本地配置备份 / 恢复"),
    b("V", "proxy / rule providers 查看与更新"),
    b("Y", "流量历史（峰值 / 会话总量）"),
    b("H", "内核体检（进程/版本/端口）"),
    b("L", "内核日志查看 / 过滤"),
    b("K", "下载 / 更新 Mihomo 内核"),
    b("U", "检查并安装自身更新"),
    b("s / S", "停止 / 重启内核"),
    b("A", "以管理员运行内核"),
];

const RULES: &[Binding] = &[
    b("a", "新增域名/IP/GEOSITE 规则"),
    b("t", "循环 DIRECT / 代理组"),
    b("K / J", "上移 / 下移规则优先级"),
    b("f", "域名命中推演"),
    b("x", "删除所选规则"),
];

const DIALOGS: &[Binding] = &[
    b("Esc", "关闭弹窗"),
    b("Tab / Shift+Tab", "切换字段"),
    b("Space", "切换开关 / 循环取值"),
    b("Enter", "保存并热重载"),
];

/// 左栏分组（全局 + 连接 + 代理）。
pub const LEFT: &[Section] = &[s("GLOBAL", GLOBAL), s("CONNECTIONS", CONNECTIONS), s("PROXIES", PROXIES)];

/// 右栏分组（订阅、设置、规则、弹窗）。
pub const RIGHT: &[Section] = &[
    s("SUBSCRIPTIONS", SUBSCRIPTIONS),
    s("SETTINGS", SETTINGS),
    s("RULES", RULES),
    s("DIALOGS", DIALOGS),
];

/// 页脚第一行（全局命令行）的提示。
pub const FOOTER_GLOBAL: &[FooterHint] = &[
    f("q", "Quit", Theme::RED),
    f("Q", "Quit+Stop", Theme::RED),
    f("Tab", "Next Tab", Theme::CYAN),
    f("p", "SysProxy", Theme::GREEN),
    f("e", "EnvProxy", Theme::TEAL),
    f("n", "TUN", Theme::SAPPHIRE),
    f("P", "Port", Theme::YELLOW),
    f("m", "Mode", Theme::LAVENDER),
    f("?", "All Keys", Theme::MAUVE),
];

/// 页脚第二行按当前标签页切换的提示。
pub const FOOTER_PROXIES: &[FooterHint] = &[
    f("Enter", "Select", Theme::LAVENDER),
    f("t", "Ping", Theme::YELLOW),
    f("T", "Ping All", Theme::YELLOW),
    f("o", "Sort", Theme::PEACH),
    f("←/→", "Switch Col", Theme::BLUE),
    f("PgUp/PgDn", "Scroll", Theme::TEAL),
];

pub const FOOTER_CONNECTIONS: &[FooterHint] = &[
    f("d", "Close Selected", Theme::RED),
    f("D", "Close All", Theme::RED),
    f("PgUp/PgDn", "Page Scroll", Theme::TEAL),
];

pub const FOOTER_SUBSCRIPTIONS: &[FooterHint] = &[
    f("a", "Add", Theme::GREEN),
    f("e", "Edit", Theme::TEAL),
    f("u", "Update", Theme::YELLOW),
    f("U", "Update All", Theme::YELLOW),
    f("i", "Auto", Theme::SAPPHIRE),
    f("Enter", "Activate", Theme::LAVENDER),
    f("x", "Delete", Theme::RED),
];

pub const FOOTER_SETTINGS: &[FooterHint] = &[
    f("p", "SysProxy", Theme::GREEN),
    f("e", "EnvProxy", Theme::TEAL),
    f("n", "TUN", Theme::SAPPHIRE),
    f("d", "DNS", Theme::TEAL),
    f("C", "Config", Theme::MAUVE),
    f("B", "Backup", Theme::YELLOW),
    f("V", "Providers", Theme::SAPPHIRE),
    f("Y", "Traffic", Theme::PEACH),
    f("H", "Health", Theme::SAPPHIRE),
    f("L", "Logs", Theme::TEAL),
    f("K", "Kernel", Theme::PEACH),
    f("U", "Update", Theme::MAUVE),
    f("P", "Port", Theme::YELLOW),
    f("A", "Admin", Theme::PEACH),
    f("s/S", "Core", Theme::RED),
];

pub const FOOTER_RULES: &[FooterHint] = &[
    f("a", "Add", Theme::GREEN),
    f("t", "Cycle Target", Theme::YELLOW),
    f("K/J", "Move", Theme::BLUE),
    f("f", "Probe", Theme::MAUVE),
    f("x", "Delete", Theme::RED),
    f("PgUp/PgDn", "Scroll", Theme::TEAL),
];

/// 键位条目总数（帮助浮层标题里的“共 N 项”）。
pub fn binding_count() -> usize {
    LEFT.iter().chain(RIGHT).map(|section| section.bindings.len()).sum()
}

/// 一组分组在帮助浮层里占用的行数（分组标题 + 各键位行，无空行）。
pub fn line_count(sections: &[Section]) -> usize {
    sections.iter().map(|section| 1 + section.bindings.len()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_tables_are_complete() {
        assert!(binding_count() > 0, "帮助表不能为空");
        assert_eq!(binding_count(), 45, "帮助条目数变化时请同步测试与标题");

        for section in LEFT.iter().chain(RIGHT) {
            assert!(!section.title.is_empty());
            assert!(!section.bindings.is_empty(), "{} 分组没有键位", section.title);
            for binding in section.bindings {
                assert!(!binding.keys.trim().is_empty(), "{} 存在空键位", section.title);
                assert!(!binding.desc.trim().is_empty(), "{} 的 {} 缺少说明", section.title, binding.keys);
            }
        }
    }

    #[test]
    fn scope_titles_are_unique() {
        let mut titles: Vec<&str> = LEFT.iter().chain(RIGHT).map(|s| s.title).collect();
        let count = titles.len();
        titles.sort_unstable();
        titles.dedup();
        assert_eq!(titles.len(), count, "分组标题不应重复");
    }

    /// 把 `q / Ctrl+C`、`PgUp/PgDn` 这样的展示文本拆成单个按键名。
    fn key_tokens(keys: &str) -> impl Iterator<Item = &str> {
        keys.split(|c: char| c == '/' || c == '|' || c.is_whitespace())
            .filter(|token| !token.is_empty())
    }

    fn help_tokens(bindings: &[Binding]) -> Vec<&str> {
        bindings.iter().flat_map(|binding| key_tokens(binding.keys)).collect()
    }

    #[test]
    fn footer_hints_are_backed_by_help_entries() {
        let global = help_tokens(GLOBAL);

        for hint in FOOTER_GLOBAL {
            for token in key_tokens(hint.keys) {
                assert!(
                    global.contains(&token),
                    "全局页脚按键 [{token}]（{}）在帮助表 GLOBAL 里找不到对应项",
                    hint.keys
                );
            }
            assert!(!hint.label.trim().is_empty(), "全局页脚 {} 缺少说明", hint.keys);
        }

        let scopes: [(&str, &[FooterHint], &[Binding]); 5] = [
            ("PROXIES", FOOTER_PROXIES, PROXIES),
            ("CONNS", FOOTER_CONNECTIONS, CONNECTIONS),
            ("SUBS", FOOTER_SUBSCRIPTIONS, SUBSCRIPTIONS),
            ("SETTINGS", FOOTER_SETTINGS, SETTINGS),
            ("RULES", FOOTER_RULES, RULES),
        ];
        for (name, footer, section) in scopes {
            let mut allowed = global.clone();
            allowed.extend(help_tokens(section));
            for hint in footer {
                for token in key_tokens(hint.keys) {
                    assert!(
                        allowed.contains(&token),
                        "{} 页脚按键 [{token}]（{}）在帮助表里找不到对应项",
                        name,
                        hint.keys
                    );
                }
                assert!(!hint.label.trim().is_empty(), "{} 页脚 {} 缺少说明", name, hint.keys);
            }
        }

        assert_eq!(scopes.len(), 5, "标签页数量变化时请同步页脚表");
        for (title, footer, _) in scopes {
            assert!(!title.is_empty());
            assert!(!footer.is_empty(), "{} 页脚没有提示项", title);
        }
    }
}