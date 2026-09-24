use ratatui::style::{Color, Modifier, Style};

/// Catppuccin Mocha Palette
#[allow(dead_code)]
pub struct Theme;

#[allow(dead_code)]
impl Theme {
    // Backgrounds & Surfaces
    pub const CRUST: Color = Color::Rgb(17, 17, 27);       // #11111b
    pub const MANTLE: Color = Color::Rgb(24, 24, 37);      // #181825
    pub const BASE: Color = Color::Rgb(30, 30, 46);        // #1e1e2e
    pub const SURFACE0: Color = Color::Rgb(49, 50, 68);    // #313244
    pub const SURFACE1: Color = Color::Rgb(69, 71, 90);    // #45475a
    pub const SURFACE2: Color = Color::Rgb(88, 91, 112);   // #585b70
    pub const OVERLAY0: Color = Color::Rgb(108, 112, 134); // #6c7086

    // Foreground & Text
    pub const TEXT: Color = Color::Rgb(205, 214, 244);     // #cdd6f4
    pub const SUBTEXT0: Color = Color::Rgb(166, 173, 200); // #a6adc8
    pub const MUTED: Color = Color::Rgb(127, 132, 156);    // #7f849c

    // Accent Colors
    pub const LAVENDER: Color = Color::Rgb(180, 190, 254); // #b4befe
    pub const MAUVE: Color = Color::Rgb(203, 166, 247);    // #cba6f7
    pub const SAPPHIRE: Color = Color::Rgb(116, 199, 236); // #74c7ec
    pub const BLUE: Color = Color::Rgb(137, 180, 250);     // #89b4fa
    pub const TEAL: Color = Color::Rgb(148, 226, 213);     // #94e2d5
    pub const GREEN: Color = Color::Rgb(166, 227, 161);    // #a6e3a1
    pub const YELLOW: Color = Color::Rgb(249, 226, 175);   // #f9e2af
    pub const PEACH: Color = Color::Rgb(250, 179, 135);    // #fab387
    pub const RED: Color = Color::Rgb(243, 139, 168);      // #f38ba8

    // Common UI Styles
    pub fn title_style() -> Style {
        Style::default()
            .fg(Self::MAUVE)
            .add_modifier(Modifier::BOLD)
    }

    pub fn active_border() -> Style {
        Style::default().fg(Self::LAVENDER)
    }

    pub fn inactive_border() -> Style {
        Style::default().fg(Self::SURFACE1)
    }

    pub fn selected_row_focused() -> Style {
        Style::default()
            .bg(Self::SURFACE1)
            .fg(Self::TEXT)
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
        if ms < 150 {
            Self::GREEN
        } else if ms < 300 {
            Self::YELLOW
        } else if ms < 700 {
            Self::PEACH
        } else {
            Self::RED
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
            "rule" => Self::BLUE,
            "global" => Self::MAUVE,
            "direct" => Self::YELLOW,
            _ => Self::TEXT,
        }
    }
}
