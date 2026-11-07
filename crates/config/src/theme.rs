use ratatui::style::Color;
use serde::Deserialize;
use tracing::warn;

pub const THEME_NAMES: &[&str] = &[
    "catppuccin",
    "catppuccin-latte",
    "terminal",
    "tokyo-night",
    "tokyo-night-day",
    "dracula",
    "nord",
    "gruvbox",
    "gruvbox-light",
    "one-dark",
    "one-light",
    "solarized",
    "solarized-light",
    "kanagawa",
    "kanagawa-lotus",
    "rose-pine",
    "rose-pine-dawn",
    "vesper",
];

const DEFAULT_DARK: &str = "catppuccin";
const DEFAULT_LIGHT: &str = "catppuccin-latte";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Appearance {
    #[default]
    Dark,
    Light,
}

macro_rules! roles {
    ($($role:ident),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct Palette {
            $(pub $role: Color,)+
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
        #[serde(default)]
        pub struct ModeColors {
            $(pub $role: Option<String>,)+
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
        #[serde(default)]
        pub struct CustomColors {
            $(pub $role: Option<String>,)+
            pub light: Option<ModeColors>,
            pub dark: Option<ModeColors>,
        }

        pub const COLOR_ROLES: &[&str] = &[$(stringify!($role)),+];

        impl ModeColors {
            pub fn apply(&self, palette: &mut Palette) {
                $(if let Some(value) = &self.$role {
                    palette.$role = parse_color(value);
                })+
            }
        }

        impl CustomColors {
            pub fn apply(&self, palette: &mut Palette) {
                $(if let Some(value) = &self.$role {
                    palette.$role = parse_color(value);
                })+
            }
        }
    };
}

roles!(
    accent,
    panel_bg,
    sidebar_bg,
    active_row_bg,
    selection_bg,
    surface0,
    surface1,
    surface_dim,
    overlay0,
    overlay1,
    text,
    subtext0,
    mauve,
    green,
    yellow,
    red,
    blue,
    teal,
    peach,
);

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub name: Option<String>,
    pub auto_switch: bool,
    pub dark_name: Option<String>,
    pub light_name: Option<String>,
    pub custom: Option<CustomColors>,
}

impl ThemeConfig {
    pub fn diagnostics(&self) -> Vec<String> {
        let valid = THEME_NAMES.join(", ");
        [
            ("theme.name", &self.name, DEFAULT_DARK),
            ("theme.dark_name", &self.dark_name, DEFAULT_DARK),
            ("theme.light_name", &self.light_name, DEFAULT_LIGHT),
        ]
        .into_iter()
        .filter_map(|(field, value, fallback)| {
            let value = value.as_deref()?;
            canonical_theme_name(value).is_none().then(|| {
                format!(
                    "unknown theme name {field} = {value:?}; using {fallback:?}; valid themes: {valid}"
                )
            })
        })
        .collect()
    }

    pub fn manual_name(&self) -> &str {
        self.name.as_deref().unwrap_or(DEFAULT_DARK)
    }

    pub fn dark_name(&self) -> String {
        self.dark_name
            .clone()
            .unwrap_or_else(|| sibling_theme_names(self.manual_name()).0)
    }

    pub fn light_name(&self) -> String {
        self.light_name
            .clone()
            .unwrap_or_else(|| sibling_theme_names(self.manual_name()).1)
    }

    pub fn active_name(&self, appearance: Option<Appearance>) -> String {
        if !self.auto_switch {
            return self.manual_name().to_string();
        }
        match appearance.unwrap_or_default() {
            Appearance::Dark => self.dark_name(),
            Appearance::Light => self.light_name(),
        }
    }

    pub fn palette(&self, legacy_accent: Option<&str>, appearance: Option<Appearance>) -> Palette {
        let (name, fallback, mode) = if self.auto_switch {
            match appearance.unwrap_or_default() {
                Appearance::Dark => (self.dark_name(), DEFAULT_DARK, self.mode(Appearance::Dark)),
                Appearance::Light => (
                    self.light_name(),
                    DEFAULT_LIGHT,
                    self.mode(Appearance::Light),
                ),
            }
        } else {
            (self.manual_name().to_string(), DEFAULT_DARK, None)
        };
        let mut palette = builtin_palette(&name).unwrap_or_else(|| {
            warn!(theme = name, fallback, "unknown theme, falling back");
            builtin_palette(fallback).unwrap_or(CATPPUCCIN)
        });
        if let Some(custom) = &self.custom {
            custom.apply(&mut palette);
        }
        let custom_accent = self
            .custom
            .as_ref()
            .is_some_and(|custom| custom.accent.is_some());
        if let Some(accent) = legacy_accent.filter(|accent| *accent != "cyan" && !custom_accent) {
            palette.accent = parse_color(accent);
        }
        if let Some(mode) = mode {
            mode.apply(&mut palette);
        }
        palette
    }

    fn mode(&self, appearance: Appearance) -> Option<&ModeColors> {
        let custom = self.custom.as_ref()?;
        match appearance {
            Appearance::Dark => custom.dark.as_ref(),
            Appearance::Light => custom.light.as_ref(),
        }
    }
}

pub fn canonical_theme_name(name: &str) -> Option<&'static str> {
    Some(match normalize(name).as_str() {
        "catppuccin" | "catppuccin-mocha" => "catppuccin",
        "catppuccin-latte" | "latte" | "light" => "catppuccin-latte",
        "terminal" => "terminal",
        "tokyo-night" | "tokyonight" => "tokyo-night",
        "tokyo-night-day" | "tokyo-day" | "tokyonight-day" => "tokyo-night-day",
        "dracula" => "dracula",
        "nord" => "nord",
        "gruvbox" | "gruvbox-dark" => "gruvbox",
        "gruvbox-light" => "gruvbox-light",
        "one-dark" | "onedark" => "one-dark",
        "one-light" | "onelight" => "one-light",
        "solarized" | "solarized-dark" => "solarized",
        "solarized-light" => "solarized-light",
        "kanagawa" => "kanagawa",
        "kanagawa-lotus" | "lotus" => "kanagawa-lotus",
        "rose-pine" | "rosepine" => "rose-pine",
        "rose-pine-dawn" | "rosepine-dawn" | "dawn" => "rose-pine-dawn",
        "vesper" => "vesper",
        _ => return None,
    })
}

pub fn sibling_theme_names(name: &str) -> (String, String) {
    let pair = match normalize(name).as_str() {
        "catppuccin" | "catppuccin-mocha" | "catppuccin-latte" | "latte" | "light" => {
            Some(("catppuccin", "catppuccin-latte"))
        }
        "tokyo-night" | "tokyonight" | "tokyo-night-day" | "tokyo-day" | "tokyonight-day" => {
            Some(("tokyo-night", "tokyo-night-day"))
        }
        "gruvbox" | "gruvbox-dark" | "gruvbox-light" => Some(("gruvbox", "gruvbox-light")),
        "one-dark" | "onedark" | "one-light" | "onelight" => Some(("one-dark", "one-light")),
        "solarized" | "solarized-dark" | "solarized-light" => {
            Some(("solarized", "solarized-light"))
        }
        "kanagawa" | "kanagawa-lotus" | "lotus" => Some(("kanagawa", "kanagawa-lotus")),
        "rose-pine" | "rosepine" | "rose-pine-dawn" | "rosepine-dawn" | "dawn" => {
            Some(("rose-pine", "rose-pine-dawn"))
        }
        _ => None,
    };
    pair.map_or_else(
        || (name.to_string(), name.to_string()),
        |(dark, light)| (dark.to_string(), light.to_string()),
    )
}

fn normalize(name: &str) -> String {
    name.to_lowercase().replace([' ', '_'], "-")
}

pub fn builtin_palette(name: &str) -> Option<Palette> {
    Some(match canonical_theme_name(name)? {
        "catppuccin" => CATPPUCCIN,
        "catppuccin-latte" => CATPPUCCIN_LATTE,
        "terminal" => TERMINAL,
        "tokyo-night" => TOKYO_NIGHT,
        "tokyo-night-day" => TOKYO_NIGHT_DAY,
        "dracula" => DRACULA,
        "nord" => NORD,
        "gruvbox" => GRUVBOX,
        "gruvbox-light" => GRUVBOX_LIGHT,
        "one-dark" => ONE_DARK,
        "one-light" => ONE_LIGHT,
        "solarized" => SOLARIZED,
        "solarized-light" => SOLARIZED_LIGHT,
        "kanagawa" => KANAGAWA,
        "kanagawa-lotus" => KANAGAWA_LOTUS,
        "rose-pine" => ROSE_PINE,
        "rose-pine-dawn" => ROSE_PINE_DAWN,
        _ => VESPER,
    })
}

pub fn parse_color(value: &str) -> Color {
    let value = value.trim().to_lowercase();
    if let Some(color) = parse_known_color(&value) {
        return color;
    }
    warn!(color = value, "unknown color, defaulting to cyan");
    Color::Cyan
}

fn parse_known_color(value: &str) -> Option<Color> {
    if matches!(value, "reset" | "default" | "none" | "transparent") {
        return Some(Color::Reset);
    }
    if let Some(color) = value.strip_prefix('#').and_then(parse_hex) {
        return Some(color);
    }
    if let Some(inner) = value
        .strip_prefix("rgb(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let channels: Vec<_> = inner
            .split(',')
            .map(|part| part.trim().parse::<u8>().ok())
            .collect();
        if let [Some(r), Some(g), Some(b)] = channels[..] {
            return Some(Color::Rgb(r, g, b));
        }
    }
    Some(match value {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" | "purple" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        _ => return None,
    })
}

pub(crate) fn parse_hex(hex: &str) -> Option<Color> {
    if !hex.is_ascii() {
        return None;
    }
    let digit =
        |index: usize, width: usize| u8::from_str_radix(hex.get(index..index + width)?, 16).ok();
    match hex.len() {
        6 => Some(Color::Rgb(digit(0, 2)?, digit(2, 2)?, digit(4, 2)?)),
        3 => Some(Color::Rgb(
            digit(0, 1)? * 17,
            digit(1, 1)? * 17,
            digit(2, 1)? * 17,
        )),
        _ => None,
    }
}

const fn hex(value: &str) -> Color {
    const fn nibble(byte: u8) -> u8 {
        match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => panic!("palette colors are lowercase hex"),
        }
    }
    let bytes = value.as_bytes();
    assert!(bytes.len() == 7 && bytes[0] == b'#');
    Color::Rgb(
        nibble(bytes[1]) * 16 + nibble(bytes[2]),
        nibble(bytes[3]) * 16 + nibble(bytes[4]),
        nibble(bytes[5]) * 16 + nibble(bytes[6]),
    )
}

const fn rgb_palette(colors: &[&str; 18]) -> Palette {
    Palette {
        accent: hex(colors[0]),
        panel_bg: hex(colors[1]),
        sidebar_bg: Color::Reset,
        active_row_bg: hex(colors[2]),
        selection_bg: hex(colors[3]),
        surface0: hex(colors[4]),
        surface1: hex(colors[5]),
        surface_dim: hex(colors[6]),
        overlay0: hex(colors[7]),
        overlay1: hex(colors[8]),
        text: hex(colors[9]),
        subtext0: hex(colors[10]),
        mauve: hex(colors[11]),
        green: hex(colors[12]),
        yellow: hex(colors[13]),
        red: hex(colors[14]),
        blue: hex(colors[15]),
        teal: hex(colors[16]),
        peach: hex(colors[17]),
    }
}

pub const CATPPUCCIN: Palette = rgb_palette(&[
    "#89b4fa", "#181825", "#1e1e2e", "#313244", "#313244", "#45475a", "#1e1e2e", "#6c7086",
    "#7f849c", "#cdd6f4", "#a6adc8", "#cba6f7", "#a6e3a1", "#f9e2af", "#f38ba8", "#89b4fa",
    "#94e2d5", "#fab387",
]);

pub const CATPPUCCIN_LATTE: Palette = rgb_palette(&[
    "#1e66f5", "#eff1f5", "#e6e9ef", "#bdd0f5", "#ccd0da", "#bcc0cc", "#e6e9ef", "#9ca0b0",
    "#8c8fa1", "#4c4f69", "#6c6f85", "#8839ef", "#40a02b", "#df8e1d", "#d20f39", "#1e66f5",
    "#179299", "#fe640b",
]);

pub const TERMINAL: Palette = Palette {
    accent: Color::Blue,
    panel_bg: Color::Reset,
    sidebar_bg: Color::Reset,
    active_row_bg: Color::DarkGray,
    selection_bg: Color::Reset,
    surface0: Color::Reset,
    surface1: Color::DarkGray,
    surface_dim: Color::DarkGray,
    overlay0: Color::Gray,
    overlay1: Color::White,
    text: Color::Reset,
    subtext0: Color::Gray,
    mauve: Color::Gray,
    green: Color::Green,
    yellow: Color::Yellow,
    red: Color::LightRed,
    blue: Color::Blue,
    teal: Color::Cyan,
    peach: Color::Yellow,
};

pub const TOKYO_NIGHT: Palette = rgb_palette(&[
    "#7aa2f7", "#1a1b26", "#232636", "#2d3650", "#24283b", "#414868", "#1a1b26", "#565f89",
    "#697196", "#c0caf5", "#a9b1d6", "#bb9af7", "#9ece6a", "#e0af68", "#f7768e", "#7aa2f7",
    "#7dcfff", "#ff9e64",
]);

pub const TOKYO_NIGHT_DAY: Palette = rgb_palette(&[
    "#2e7de9", "#e1e2e7", "#d2d3da", "#b6cae7", "#c4c8da", "#a8aecb", "#d2d3da", "#8990b3",
    "#68709a", "#3760bf", "#6172b0", "#7847bd", "#587539", "#8c6c3e", "#f52a65", "#2e7de9",
    "#118c74", "#b15c00",
]);

pub const DRACULA: Palette = rgb_palette(&[
    "#bd93f9", "#282a36", "#373c52", "#463f5d", "#44475a", "#6272a4", "#282a36", "#6272a4",
    "#828cb4", "#f8f8f2", "#d2d2dc", "#ff79c6", "#50fa7b", "#f1fa8c", "#ff5555", "#8be9fd",
    "#8be9fd", "#ffb86c",
]);

pub const NORD: Palette = rgb_palette(&[
    "#88c0d0", "#2e3440", "#434c5e", "#40505d", "#3b4252", "#434c5e", "#2e3440", "#4c566a",
    "#646e82", "#eceff4", "#d8dee9", "#b48ead", "#a3be8c", "#ebcb8b", "#bf616a", "#81a1c1",
    "#8fbcbb", "#d08770",
]);

pub const GRUVBOX: Palette = rgb_palette(&[
    "#d79921", "#282828", "#323130", "#4b3f27", "#3c3836", "#504945", "#282828", "#928374",
    "#a89984", "#ebdbb2", "#d5c4a1", "#d3869b", "#b8bb26", "#fabd2f", "#fb4934", "#83a598",
    "#8ec07c", "#fe8019",
]);

pub const GRUVBOX_LIGHT: Palette = rgb_palette(&[
    "#076678", "#fbf1c7", "#f2e5bc", "#ebdbb2", "#ebdbb2", "#d5c4a1", "#f2e5bc", "#928374",
    "#7c6f64", "#3c3836", "#504945", "#8f3f71", "#79740e", "#b57614", "#9d0006", "#076678",
    "#427b58", "#af3a03",
]);

pub const ONE_DARK: Palette = rgb_palette(&[
    "#61afef", "#282c34", "#313640", "#334659", "#2c313a", "#3e4451", "#282c34", "#5c6370",
    "#737a87", "#abb2bf", "#969ca8", "#c678dd", "#98c379", "#e5c07b", "#e06c75", "#61afef",
    "#56b6c2", "#d19a66",
]);

pub const ONE_LIGHT: Palette = rgb_palette(&[
    "#4078f2", "#fafafa", "#d8dbe2", "#cddbf8", "#f0f0f1", "#e5e5e6", "#f5f5f6", "#a0a1a7",
    "#686b77", "#383a42", "#686b77", "#a626a4", "#50a14f", "#c18401", "#e45649", "#4078f2",
    "#0184bc", "#986801",
]);

pub const SOLARIZED: Palette = rgb_palette(&[
    "#268bd2", "#002b36", "#164b57", "#083e55", "#073642", "#586e75", "#002b36", "#586e75",
    "#657b83", "#93a1a1", "#839496", "#d33682", "#859900", "#b58900", "#dc322f", "#268bd2",
    "#2aa198", "#cb4b16",
]);

pub const SOLARIZED_LIGHT: Palette = rgb_palette(&[
    "#268bd2", "#fdf6e3", "#eee8d5", "#c9dcdf", "#eee8d5", "#93a1a1", "#eee8d5", "#93a1a1",
    "#586e75", "#657b83", "#839496", "#d33682", "#859900", "#b58900", "#dc322f", "#268bd2",
    "#2aa198", "#cb4b16",
]);

pub const KANAGAWA: Palette = rgb_palette(&[
    "#7e9cd8", "#1f1f28", "#363646", "#32384b", "#2a2a37", "#363646", "#1f1f28", "#727169",
    "#87867d", "#dcd7ba", "#c8c3aa", "#957fb8", "#76946a", "#c0a36e", "#c34043", "#7e9cd8",
    "#7fb4ca", "#ffa066",
]);

pub const KANAGAWA_LOTUS: Palette = rgb_palette(&[
    "#4d699b", "#f2ecbc", "#d5cea3", "#dcd5ac", "#dcd5ac", "#c9cbd1", "#d5cea3", "#a09cac",
    "#8a8980", "#545464", "#43436c", "#624c83", "#6f894e", "#77713f", "#c84053", "#4d699b",
    "#4e8ca2", "#cc6d00",
]);

pub const ROSE_PINE: Palette = rgb_palette(&[
    "#c4a7e7", "#191724", "#26233a", "#3b344b", "#1f1d2e", "#26233a", "#26233a", "#6e6a86",
    "#908caa", "#e0def4", "#c8c5dc", "#c4a7e7", "#31748f", "#f6c177", "#eb6f92", "#31748f",
    "#9ccfd8", "#ea9a97",
]);

pub const ROSE_PINE_DAWN: Palette = rgb_palette(&[
    "#907aa9", "#faf4ed", "#e3d9cf", "#f2e9e1", "#f2e9e1", "#fffaf3", "#f2e9e1", "#9893a5",
    "#797593", "#464261", "#797593", "#907aa9", "#286983", "#ea9d34", "#b4637a", "#286983",
    "#56949f", "#d7827e",
]);

pub const VESPER: Palette = rgb_palette(&[
    "#ffc799", "#1a1a1a", "#101010", "#232323", "#232323", "#282828", "#101010", "#5c5c5c",
    "#7e7e7e", "#ffffff", "#a0a0a0", "#ffd1a8", "#99ffe4", "#ffc799", "#ff8080", "#b0b0b0",
    "#66ddcc", "#ffc799",
]);
