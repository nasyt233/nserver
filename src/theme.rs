//! Color themes shared by every screen.

use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeName {
    Midnight,
    Forest,
    Ember,
    Mono,
}

impl ThemeName {
    pub const ALL: [ThemeName; 4] = [
        ThemeName::Midnight,
        ThemeName::Forest,
        ThemeName::Ember,
        ThemeName::Mono,
    ];

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "midnight" | "午夜" => Some(Self::Midnight),
            "forest" | "森林" => Some(Self::Forest),
            "ember" | "余烬" | "sunset" => Some(Self::Ember),
            "mono" | "单色" => Some(Self::Mono),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub name: &'static str,
    pub bg: Color,
    pub panel_bg: Color,
    pub header_bg: Color,
    pub footer_bg: Color,
    pub fg: Color,
    pub muted: Color,
    pub faint: Color,
    pub accent: Color,
    pub accent2: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub info: Color,
    pub border: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
}

impl Theme {
    pub fn new(name: ThemeName) -> Self {
        match name {
            ThemeName::Midnight => Self {
                name: "午夜",
                bg: Color::Rgb(9, 13, 24),
                panel_bg: Color::Rgb(13, 20, 35),
                header_bg: Color::Rgb(12, 19, 33),
                footer_bg: Color::Rgb(15, 24, 41),
                fg: Color::Rgb(216, 228, 245),
                muted: Color::Rgb(128, 148, 174),
                faint: Color::Rgb(83, 100, 124),
                accent: Color::Rgb(76, 201, 240),
                accent2: Color::Rgb(179, 136, 255),
                success: Color::Rgb(74, 222, 128),
                warning: Color::Rgb(251, 191, 36),
                danger: Color::Rgb(251, 113, 133),
                info: Color::Rgb(96, 165, 250),
                border: Color::Rgb(35, 50, 74),
                selection_bg: Color::Rgb(23, 59, 92),
                selection_fg: Color::Rgb(240, 248, 255),
            },
            ThemeName::Forest => Self {
                name: "森林",
                bg: Color::Rgb(8, 18, 15),
                panel_bg: Color::Rgb(12, 27, 23),
                header_bg: Color::Rgb(11, 25, 21),
                footer_bg: Color::Rgb(14, 32, 27),
                fg: Color::Rgb(216, 238, 226),
                muted: Color::Rgb(125, 158, 143),
                faint: Color::Rgb(75, 105, 91),
                accent: Color::Rgb(94, 234, 212),
                accent2: Color::Rgb(163, 230, 53),
                success: Color::Rgb(134, 239, 172),
                warning: Color::Rgb(250, 204, 21),
                danger: Color::Rgb(248, 113, 113),
                info: Color::Rgb(125, 211, 252),
                border: Color::Rgb(35, 62, 52),
                selection_bg: Color::Rgb(27, 69, 58),
                selection_fg: Color::Rgb(240, 255, 248),
            },
            ThemeName::Ember => Self {
                name: "余烬",
                bg: Color::Rgb(22, 14, 12),
                panel_bg: Color::Rgb(30, 19, 16),
                header_bg: Color::Rgb(28, 18, 15),
                footer_bg: Color::Rgb(35, 22, 18),
                fg: Color::Rgb(245, 228, 214),
                muted: Color::Rgb(171, 142, 122),
                faint: Color::Rgb(112, 89, 74),
                accent: Color::Rgb(255, 158, 100),
                accent2: Color::Rgb(186, 156, 255),
                success: Color::Rgb(158, 206, 106),
                warning: Color::Rgb(224, 175, 104),
                danger: Color::Rgb(247, 118, 142),
                info: Color::Rgb(125, 207, 255),
                border: Color::Rgb(70, 45, 35),
                selection_bg: Color::Rgb(83, 47, 31),
                selection_fg: Color::Rgb(255, 245, 235),
            },
            ThemeName::Mono => Self {
                name: "单色",
                bg: Color::Rgb(10, 10, 11),
                panel_bg: Color::Rgb(17, 17, 19),
                header_bg: Color::Rgb(15, 15, 17),
                footer_bg: Color::Rgb(22, 22, 25),
                fg: Color::Rgb(232, 232, 232),
                muted: Color::Rgb(150, 150, 150),
                faint: Color::Rgb(95, 95, 95),
                accent: Color::Rgb(210, 210, 210),
                accent2: Color::Rgb(170, 170, 170),
                success: Color::Rgb(210, 210, 210),
                warning: Color::Rgb(190, 190, 190),
                danger: Color::Rgb(255, 255, 255),
                info: Color::Rgb(190, 190, 190),
                border: Color::Rgb(55, 55, 58),
                selection_bg: Color::Rgb(48, 48, 52),
                selection_fg: Color::Rgb(255, 255, 255),
            },
        }
    }

    pub fn cycle(&mut self) {
        let current = ThemeName::ALL
            .iter()
            .position(|name| Theme::new(*name).name == self.name)
            .unwrap_or(0);
        *self = Theme::new(ThemeName::ALL[(current + 1) % ThemeName::ALL.len()]);
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new(ThemeName::Midnight)
    }
}
