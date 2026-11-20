//! 快捷键帮助的唯一数据源。
//!
//! 帮助浮层（`?`）里的键位表与“共 N 项”计数都从这里生成，避免新增按键时
//! 只改了界面、忘了文档（或反过来）。实际按键分发仍在 `main.rs::handle_key_event`，
//! 新增按键时请同时更新本表与分发逻辑。

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

const GLOBAL: &[Binding] = &[
    b("q / Ctrl+C", "退出（内核继续后台运行）"),
    b("Q", "退出并停止内核"),
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
}