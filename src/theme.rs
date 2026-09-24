use ratatui::style::{Color, Modifier, Style};

/// Neon HUD Palette — high-contrast sci-fi terminal look
/// 常量名保持与旧版（Catppuccin Mocha）一致，调用点无需改动
#[allow(dead_code)]
pub struct Theme;

#[allow(dead_code)]
impl Theme {
    // Backgrounds & Surfaces (deep space / cyber abyss slate)
    pub const CRUST: Color = Color::Rgb(6, 9, 15);          // #06090f
    pub const MANTLE: Color = Color::Rgb(10, 15, 24);       // #0a0f18
    pub const BASE: Color = Color::Rgb(13, 20, 32);         // #0d1420
    pub const SURFACE0: Color = Color::Rgb(20, 32, 48);     // #142030
    pub const SURFACE1: Color = Color::Rgb(30, 47, 71);     // #1e2f47
    pub const SURFACE2: Color = Color::Rgb(45, 68, 99);     // #2d4463
    pub const OVERLAY0: Color = Color::Rgb(75, 100, 132);   // #4b6484

    // Foreground & Text (icy crisp white & cool slates)
    pub const TEXT: Color = Color::Rgb(226, 237, 255);      // #e2edff
    pub const SUBTEXT0: Color = Color::Rgb(145, 168, 198);  // #91a8c6
    pub const MUTED: Color = Color::Rgb(92, 112, 138);      // #5c708a

    // Accent Colors (cyber neon spectrum)
    pub const CYAN: Color = Color::Rgb(0, 240, 255);        // #00f0ff electric cyan
    pub const LAVENDER: Color = Color::Rgb(0, 229, 255);    // #00e5ff neon cyan (primary HUD)
    pub const MAUVE: Color = Color::Rgb(180, 130, 255);     // #b482ff electric violet
    pub const SAPPHIRE: Color = Color::Rgb(56, 189, 248);   // #38bdf8 ice blue
    pub const BLUE: Color = Color::Rgb(96, 165, 250);       // #60a5fa cyber blue
    pub const TEAL: Color = Color::Rgb(45, 212, 191);       // #2dd4bf neon aqua
    pub const GREEN: Color = Color::Rgb(52, 211, 153);      // #34d399 matrix emerald
    pub const YELLOW: Color = Color::Rgb(251, 191, 36);     // #fbbf24 cyber amber
    pub const PEACH: Color = Color::Rgb(251, 146, 60);      // #fb923c neon orange
    pub const RED: Color = Color::Rgb(244, 63, 94);         // #f43f5e neon crimson

    // Common UI Styles
    pub fn title_style() -> Style {
        Style::default()
            .fg(Self::LAVENDER)
            .add_modifier(Modifier::BOLD)
    }

    pub fn active_border() -> Style {
        Style::default()
            .fg(Self::CYAN)
            .add_modifier(Modifier::BOLD)
    }

    pub fn inactive_border() -> Style {
        Style::default().fg(Self::SURFACE2)
    }

    pub fn selected_row_focused() -> Style {
        Style::default()
            .bg(Self::SURFACE1)
            .fg(Self::CYAN)
            .add_modifier(Modifier::BOLD)
    }

    pub fn selected_row_unfocused() -> Style {
        Style::default()
            .bg(Self::SURFACE0)
            .fg(Self::SUBTEXT0)
    }

    pub fn normal_row() -> Style {
        Style::default().fg(Self::TEXT)
    }

    pub fn latency_color(ms: u64) -> Color {
        if ms < 100 {
            Self::GREEN
        } else if ms < 200 {
            Self::TEAL
        } else if ms < 350 {
            Self::YELLOW
        } else if ms < 600 {
            Self::PEACH
        } else {
            Self::RED
        }
    }

    pub fn latency_badge(ms: u64) -> (&'static str, Color) {
        if ms < 100 {
            ("⚡", Self::GREEN)
        } else if ms < 200 {
            ("●", Self::TEAL)
        } else if ms < 350 {
            ("▲", Self::YELLOW)
        } else if ms < 600 {
            ("◆", Self::PEACH)
        } else {
            ("■", Self::RED)
        }
    }

    pub fn protocol_color(proto: &str) -> Color {
        match proto.to_lowercase().as_str() {
            "ss" | "shadowsocks" => Self::SAPPHIRE,
            "vmess" => Self::MAUVE,
            "vless" => Self::LAVENDER,
            "trojan" => Self::PEACH,
            "hysteria" | "hysteria2" | "hy2" => Self::GREEN,
            "wireguard" => Self::TEAL,
            "snell" => Self::YELLOW,
            "socks5" | "http" => Self::BLUE,
            _ => Self::SUBTEXT0,
        }
    }

    pub fn mode_color(mode: &str) -> Color {
        match mode.to_lowercase().as_str() {
            "rule" => Self::LAVENDER,
            "global" => Self::MAUVE,
            "direct" => Self::YELLOW,
            _ => Self::TEXT,
        }
    }
}
