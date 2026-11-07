use std::path::Path;

use config::{
    Appearance, COLOR_ROLES, Config, Palette, THEME_NAMES, ThemeConfig, builtin_palette,
    canonical_theme_name, parse_color, sibling_theme_names,
};
use ratatui::style::Color;

fn rgb(hex: u32) -> Color {
    let [_, r, g, b] = hex.to_be_bytes();
    Color::Rgb(r, g, b)
}

fn roles(palette: &Palette) -> [Color; 19] {
    [
        palette.accent,
        palette.panel_bg,
        palette.sidebar_bg,
        palette.active_row_bg,
        palette.selection_bg,
        palette.surface0,
        palette.surface1,
        palette.surface_dim,
        palette.overlay0,
        palette.overlay1,
        palette.text,
        palette.subtext0,
        palette.mauve,
        palette.green,
        palette.yellow,
        palette.red,
        palette.blue,
        palette.teal,
        palette.peach,
    ]
}

fn theme(text: &str) -> Config {
    let loaded = Config::parse(text, Path::new("/"));
    assert!(
        loaded
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.starts_with("config parse error")),
        "{:?}",
        loaded.diagnostics
    );
    loaded.config
}

#[test]
fn nineteen_roles() {
    assert_eq!(COLOR_ROLES.len(), 19);
    assert_eq!(COLOR_ROLES[0], "accent");
    assert_eq!(COLOR_ROLES[18], "peach");
}

#[test]
fn every_builtin_resolves() {
    assert_eq!(THEME_NAMES.len(), 18);
    for name in THEME_NAMES {
        assert_eq!(canonical_theme_name(name), Some(*name));
        let palette = builtin_palette(name).unwrap();
        assert_eq!(palette.sidebar_bg, Color::Reset, "{name}");
    }
}

#[test]
fn palette_values_match_herdr() {
    let spot = [
        ("catppuccin", 0, rgb(0x0089_b4fa)),
        ("catppuccin", 1, rgb(0x0018_1825)),
        ("catppuccin", 10, rgb(0x00cd_d6f4)),
        ("catppuccin", 18, rgb(0x00fa_b387)),
        ("catppuccin-latte", 4, rgb(0x00bd_d0f5)),
        ("catppuccin-latte", 15, rgb(0x00d2_0f39)),
        ("tokyo-night", 0, rgb(0x007a_a2f7)),
        ("tokyo-night", 3, rgb(0x0023_2636)),
        ("tokyo-night", 17, rgb(0x007d_cfff)),
        ("tokyo-night-day", 9, rgb(0x0068_709a)),
        ("tokyo-night-day", 18, rgb(0x00b1_5c00)),
        ("dracula", 4, rgb(0x0046_3f5d)),
        ("dracula", 16, rgb(0x008b_e9fd)),
        ("nord", 0, rgb(0x0088_c0d0)),
        ("nord", 9, rgb(0x0064_6e82)),
        ("gruvbox", 4, rgb(0x004b_3f27)),
        ("gruvbox", 13, rgb(0x00b8_bb26)),
        ("gruvbox-light", 0, rgb(0x0007_6678)),
        ("gruvbox-light", 15, rgb(0x009d_0006)),
        ("one-dark", 4, rgb(0x0033_4659)),
        ("one-light", 7, rgb(0x00f5_f5f6)),
        ("one-light", 11, rgb(0x0068_6b77)),
        ("solarized", 1, rgb(0x0000_2b36)),
        ("solarized", 3, rgb(0x0016_4b57)),
        ("solarized-light", 4, rgb(0x00c9_dcdf)),
        ("kanagawa", 8, rgb(0x0072_7169)),
        ("kanagawa-lotus", 11, rgb(0x0043_436c)),
        ("kanagawa-lotus", 18, rgb(0x00cc_6d00)),
        ("rose-pine", 13, rgb(0x0031_748f)),
        ("rose-pine", 7, rgb(0x0026_233a)),
        ("rose-pine-dawn", 6, rgb(0x00ff_faf3)),
        ("rose-pine-dawn", 10, rgb(0x0046_4261)),
        ("vesper", 0, rgb(0x00ff_c799)),
        ("vesper", 10, rgb(0x00ff_ffff)),
        ("vesper", 13, rgb(0x0099_ffe4)),
    ];
    for (name, role, color) in spot {
        let palette = builtin_palette(name).unwrap();
        assert_eq!(roles(&palette)[role], color, "{name} {}", COLOR_ROLES[role]);
    }
}

#[test]
fn full_palettes() {
    let tokyo = [
        0x007a_a2f7,
        0x001a_1b26,
        0,
        0x0023_2636,
        0x002d_3650,
        0x0024_283b,
        0x0041_4868,
        0x001a_1b26,
        0x0056_5f89,
        0x0069_7196,
        0x00c0_caf5,
        0x00a9_b1d6,
        0x00bb_9af7,
        0x009e_ce6a,
        0x00e0_af68,
        0x00f7_768e,
        0x007a_a2f7,
        0x007d_cfff,
        0x00ff_9e64,
    ];
    let palette = roles(&builtin_palette("tokyo-night").unwrap());
    for (index, hex) in tokyo.into_iter().enumerate() {
        let expected = if index == 2 { Color::Reset } else { rgb(hex) };
        assert_eq!(palette[index], expected, "{}", COLOR_ROLES[index]);
    }
    assert_eq!(
        roles(&builtin_palette("terminal").unwrap()),
        [
            Color::Blue,
            Color::Reset,
            Color::Reset,
            Color::DarkGray,
            Color::Reset,
            Color::Reset,
            Color::DarkGray,
            Color::DarkGray,
            Color::Gray,
            Color::White,
            Color::Reset,
            Color::Gray,
            Color::Gray,
            Color::Green,
            Color::Yellow,
            Color::LightRed,
            Color::Blue,
            Color::Cyan,
            Color::Yellow,
        ]
    );
}

#[test]
fn aliases() {
    let aliases = [
        ("catppuccin-mocha", "catppuccin"),
        ("latte", "catppuccin-latte"),
        ("light", "catppuccin-latte"),
        ("tokyonight", "tokyo-night"),
        ("tokyo-day", "tokyo-night-day"),
        ("tokyonight-day", "tokyo-night-day"),
        ("gruvbox-dark", "gruvbox"),
        ("onedark", "one-dark"),
        ("onelight", "one-light"),
        ("solarized-dark", "solarized"),
        ("lotus", "kanagawa-lotus"),
        ("rosepine", "rose-pine"),
        ("rosepine-dawn", "rose-pine-dawn"),
        ("dawn", "rose-pine-dawn"),
        ("Tokyo Night", "tokyo-night"),
        ("ROSE_PINE", "rose-pine"),
    ];
    for (alias, name) in aliases {
        assert_eq!(canonical_theme_name(alias), Some(name), "{alias}");
    }
    assert_eq!(canonical_theme_name("neon"), None);
}

#[test]
fn siblings() {
    let pair = |dark: &str, light: &str| (dark.to_string(), light.to_string());
    assert_eq!(
        sibling_theme_names("catppuccin"),
        pair("catppuccin", "catppuccin-latte")
    );
    assert_eq!(
        sibling_theme_names("latte"),
        pair("catppuccin", "catppuccin-latte")
    );
    assert_eq!(
        sibling_theme_names("tokyo-night-day"),
        pair("tokyo-night", "tokyo-night-day")
    );
    assert_eq!(
        sibling_theme_names("gruvbox"),
        pair("gruvbox", "gruvbox-light")
    );
    assert_eq!(
        sibling_theme_names("onelight"),
        pair("one-dark", "one-light")
    );
    assert_eq!(
        sibling_theme_names("solarized-light"),
        pair("solarized", "solarized-light")
    );
    assert_eq!(
        sibling_theme_names("lotus"),
        pair("kanagawa", "kanagawa-lotus")
    );
    assert_eq!(
        sibling_theme_names("dawn"),
        pair("rose-pine", "rose-pine-dawn")
    );
    assert_eq!(sibling_theme_names("dracula"), pair("dracula", "dracula"));
    assert_eq!(sibling_theme_names("Vesper"), pair("Vesper", "Vesper"));
    assert_eq!(
        sibling_theme_names("terminal"),
        pair("terminal", "terminal")
    );
}

#[test]
fn color_parsing() {
    for reset in ["reset", "default", "NONE", " transparent "] {
        assert_eq!(parse_color(reset), Color::Reset);
    }
    assert_eq!(parse_color("#89b4fa"), Color::Rgb(0x89, 0xb4, 0xfa));
    assert_eq!(parse_color("#89B4FA"), Color::Rgb(0x89, 0xb4, 0xfa));
    assert_eq!(parse_color("#f0a"), Color::Rgb(0xff, 0x00, 0xaa));
    assert_eq!(parse_color("rgb(1, 2, 3)"), Color::Rgb(1, 2, 3));
    assert_eq!(parse_color("RGB(10,20,30)"), Color::Rgb(10, 20, 30));
    let names = [
        ("black", Color::Black),
        ("red", Color::Red),
        ("green", Color::Green),
        ("yellow", Color::Yellow),
        ("blue", Color::Blue),
        ("magenta", Color::Magenta),
        ("purple", Color::Magenta),
        ("cyan", Color::Cyan),
        ("white", Color::White),
        ("gray", Color::Gray),
        ("grey", Color::Gray),
        ("darkgray", Color::DarkGray),
        ("darkgrey", Color::DarkGray),
        ("lightred", Color::LightRed),
        ("lightgreen", Color::LightGreen),
        ("lightyellow", Color::LightYellow),
        ("lightblue", Color::LightBlue),
        ("lightmagenta", Color::LightMagenta),
        ("LightCyan", Color::LightCyan),
    ];
    for (name, color) in names {
        assert_eq!(parse_color(name), color, "{name}");
    }
    for unknown in [
        "#12345",
        "#ggg",
        "rgb(1,2)",
        "rgb(256,0,0)",
        "teal",
        "#ééé",
        "",
    ] {
        assert_eq!(parse_color(unknown), Color::Cyan, "{unknown:?}");
    }
}

#[test]
fn custom_overrides_and_legacy_accent() {
    let config = theme(
        "[theme]\nname = \"nord\"\n[theme.custom]\nred = \"#010203\"\nsidebar_bg = \"#111\"\n[ui]\naccent = \"#abcdef\"\n",
    );
    let palette = config.palette(None);
    assert_eq!(palette.red, Color::Rgb(1, 2, 3));
    assert_eq!(palette.sidebar_bg, Color::Rgb(0x11, 0x11, 0x11));
    assert_eq!(palette.accent, Color::Rgb(0xab, 0xcd, 0xef));
    assert_eq!(palette.text, builtin_palette("nord").unwrap().text);

    let custom_wins = theme("[theme.custom]\naccent = \"red\"\n[ui]\naccent = \"#abcdef\"\n");
    assert_eq!(custom_wins.palette(None).accent, Color::Red);

    let cyan = theme("[ui]\naccent = \"cyan\"\n");
    assert_eq!(
        cyan.palette(None).accent,
        builtin_palette("catppuccin").unwrap().accent
    );
}

#[test]
fn unknown_theme_falls_back() {
    let manual = theme("[theme]\nname = \"neon\"\n");
    assert_eq!(manual.palette(None), builtin_palette("catppuccin").unwrap());
    let light = theme("[theme]\nauto_switch = true\nlight_name = \"neon\"\n");
    assert_eq!(
        light.palette(Some(Appearance::Light)),
        builtin_palette("catppuccin-latte").unwrap()
    );
    let dark = theme("[theme]\nauto_switch = true\ndark_name = \"neon\"\n");
    assert_eq!(
        dark.palette(Some(Appearance::Dark)),
        builtin_palette("catppuccin").unwrap()
    );
}

#[test]
fn auto_switch() {
    let config = theme(
        "[theme]\nname = \"gruvbox\"\nauto_switch = true\n\
         [theme.custom]\ntext = \"#000001\"\n\
         [theme.custom.light]\ntext = \"#000002\"\n\
         [theme.custom.dark]\npeach = \"#000003\"\n\
         [ui]\naccent = \"#000004\"\n",
    );
    let theme = &config.theme;
    assert_eq!(theme.active_name(None), "gruvbox");
    assert_eq!(theme.active_name(Some(Appearance::Light)), "gruvbox-light");
    let light = config.palette(Some(Appearance::Light));
    assert_eq!(
        light.panel_bg,
        builtin_palette("gruvbox-light").unwrap().panel_bg
    );
    assert_eq!(light.text, Color::Rgb(0, 0, 2));
    assert_eq!(light.accent, Color::Rgb(0, 0, 4));
    let dark = config.palette(None);
    assert_eq!(dark.panel_bg, builtin_palette("gruvbox").unwrap().panel_bg);
    assert_eq!(dark.text, Color::Rgb(0, 0, 1));
    assert_eq!(dark.peach, Color::Rgb(0, 0, 3));

    let manual = ThemeConfig {
        name: Some("gruvbox".into()),
        auto_switch: false,
        ..ThemeConfig::default()
    };
    assert_eq!(manual.active_name(Some(Appearance::Light)), "gruvbox");
    assert_eq!(
        manual.palette(None, Some(Appearance::Light)),
        builtin_palette("gruvbox").unwrap()
    );

    let explicit =
        self::theme("[theme]\nname = \"nord\"\nauto_switch = true\nlight_name = \"vesper\"\n");
    assert_eq!(explicit.theme.dark_name(), "nord");
    assert_eq!(explicit.theme.light_name(), "vesper");
    assert_eq!(
        explicit.palette(Some(Appearance::Light)),
        builtin_palette("vesper").unwrap()
    );
}
