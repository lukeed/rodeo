use std::path::{Path, PathBuf};

use config::{
    AgentPanelSort, Config, Loaded, PaneBorders, ReloadStatus, ToastDelivery, check, strip_bom,
    summary,
};

fn dir() -> &'static Path {
    Path::new("/nonexistent")
}

fn parse(text: &str) -> Loaded {
    Config::parse(text, dir())
}

fn hard_error(text: &str) {
    let loaded = parse(text);
    assert_eq!(loaded.config, Config::default(), "{text}");
    assert_eq!(loaded.diagnostics.len(), 1, "{:?}", loaded.diagnostics);
    let diagnostic = &loaded.diagnostics[0];
    assert!(
        diagnostic.starts_with("config parse error: "),
        "{diagnostic}"
    );
    assert!(diagnostic.ends_with("; using defaults"), "{diagnostic}");
}

fn sample_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join("oss/thuic-ref/samples/herdr-tui-config.mac.toml")
}

const SAMPLE: &str = r#"onboarding = false

[keys]
prefix = "ctrl+b"
previous_workspace = "alt+up"
next_workspace = "alt+down"
previous_tab = "alt+["
next_tab = "alt+]"

[ui.sound]
enabled = true

[ui.toast]
enabled = true

[ui]
agent_panel_sort = "spaces"

[experimental]
pane_history = false

[theme]
# name = "rose-pine"
name = "tokyo-night"
# name = "vesper"

[terminal]
new_cwd = "follow"
"#;

fn assert_sample(loaded: &Loaded) {
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    let config = &loaded.config;
    assert_eq!(config.onboarding, Some(false));
    assert_eq!(config.theme.name.as_deref(), Some("tokyo-night"));
    assert_eq!(config.ui.toast.delivery, ToastDelivery::Herdr);
    assert_eq!(config.ui.agent_panel_sort, AgentPanelSort::Spaces);
    let keymap = config.keymap().keymap;
    assert_eq!(
        keymap.label(config::Action::PreviousTab).as_deref(),
        Some("alt+[")
    );
    assert_eq!(
        keymap.label(config::Action::NextWorkspace).as_deref(),
        Some("alt+down")
    );
}

#[test]
fn user_sample_loads_cleanly() {
    assert_sample(&parse(SAMPLE));
    let path = sample_path();
    if path.exists() {
        assert_sample(&Config::load(&path));
        assert_eq!(check(&path), Ok(()));
    }
}

#[test]
fn missing_file_is_defaults() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.toml");
    let loaded = Config::load(&path);
    assert_eq!(loaded.config, Config::default());
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    assert_eq!(check(&path), Ok(()));
}

#[test]
fn unreadable_file_is_defaults() {
    let temp = tempfile::tempdir().unwrap();
    let loaded = Config::load(temp.path());
    assert_eq!(loaded.config, Config::default());
    assert_eq!(loaded.diagnostics.len(), 1);
    assert!(loaded.diagnostics[0].starts_with("config read error: "));
    assert!(loaded.diagnostics[0].ends_with("; using defaults"));
    assert_eq!(
        summary(&loaded.diagnostics, Path::new("/x/config.toml")).as_deref(),
        Some("config.toml unreadable; using defaults; rodeo config check")
    );
}

#[test]
fn hard_errors_drop_everything() {
    hard_error("[ui\n");
    hard_error("[ui]\npane_borders = \"framed\"\n[theme]\nname = \"nord\"\n");
    hard_error("[ui]\nmouse_scroll_lines = 0\n");
    hard_error("[ui.toast]\ndelay_seconds = 5000\n");
    hard_error("[ui.sidebar.spaces]\nrows = [[\"nope\"]]\n");
    hard_error("[ui.sidebar.agents.rows_by_agent]\nnobody = [[\"agent\"]]\n");
    hard_error("[ui]\nright_click_passthrough_modifier = \"shift\"\n");
    hard_error("[ui]\nagent_panel_scope = \"some\"\n");
    hard_error("[ui]\ntab_bar_right = [{ type = \"clock\" }]\n");
    hard_error("[ui]\ntab_bar_right = [{ type = \"text\", text = \"a\", extra = 1 }]\n");
    hard_error("[keys]\nprefix = 5\n");
    hard_error("[[keys.command]]\ncommand = \"x\"\nwidth = \"80\"\n");
    hard_error("[[keys.command]]\ncommand = \"x\"\nwidth = \"0%\"\n");
    hard_error("[[keys.command]]\ncommand = \"x\"\ntype = \"script\"\n");
    hard_error("[terminal]\nshell_mode = \"sometimes\"\n");
    hard_error("onboarding = \"yes\"\n");
    let summary_text = summary(&parse("[ui\n").diagnostics, Path::new("config.toml"));
    assert_eq!(
        summary_text.as_deref(),
        Some("config.toml invalid; using defaults; rodeo config check")
    );
}

#[test]
fn legacy_values_still_parse() {
    let config = parse(
        "[ui]\npane_borders = false\nagent_panel_sort = \"workspaces\"\nagent_panel_scope = \"all\"\n\
         [ui.toast]\nenabled = true\ndelivery = \"system\"\n\
         [keys]\nfullscreen = \"prefix+f\"\n\
         [advanced]\nscrollback_lines = 5\n",
    );
    assert!(config.diagnostics.is_empty(), "{:?}", config.diagnostics);
    let config = config.config;
    assert_eq!(config.ui.pane_borders, PaneBorders::Off);
    assert_eq!(config.ui.agent_panel_sort, AgentPanelSort::Spaces);
    assert_eq!(config.ui.toast.delivery, ToastDelivery::System);
    assert_eq!(config.advanced.scrollback_limit_bytes, 5);
    assert_eq!(
        config
            .keymap()
            .keymap
            .label(config::Action::Zoom)
            .as_deref(),
        Some("prefix+f")
    );
    let disabled = parse("[ui.toast]\nenabled = false\n").config;
    assert_eq!(disabled.ui.toast.delivery, ToastDelivery::Off);
    let borders = parse("[ui]\npane_borders = true\n").config;
    assert_eq!(borders.ui.pane_borders, PaneBorders::Auto);
}

#[test]
fn unknown_sections_and_keys_are_soft() {
    let loaded = parse(
        "mystery = 1\n[toast]\nenabled = true\n[[plugins]]\nname = \"x\"\n\
         [ui]\nwidth = 3\n[ui.toast]\nsize = 2\n[ui.sidebar.spaces]\n\"odd key\" = 1\n\
         [keys]\nnope = \"prefix+z\"\n[[keys.command]]\ncommand = \"ls\"\nkey = \"prefix+alt+l\"\nbogus = true\n",
    );
    assert_eq!(
        loaded.diagnostics,
        vec![
            "unknown config section [[plugins]]; ignoring section",
            "unknown config section [toast]; did you mean [ui.toast]? ignoring section",
            "unknown config key keys.command.0.bogus; ignoring key",
            "unknown config key keys.nope; ignoring key",
            "unknown config key mystery; ignoring key",
            "unknown config key ui.sidebar.spaces.\"odd key\"; ignoring key",
            "unknown config key ui.toast.size; ignoring key",
            "unknown config key ui.width; ignoring key",
        ]
    );
    let only_keys = parse("[ui]\nwidth = 3\n").diagnostics;
    assert_eq!(
        summary(&only_keys, Path::new("/a/config.toml")).as_deref(),
        Some("config.toml has unknown keys; rodeo config check")
    );
    assert_eq!(
        summary(&loaded.diagnostics, Path::new("/a/my.toml")).as_deref(),
        Some("my.toml; rodeo config check")
    );
    assert_eq!(summary(&[], Path::new("config.toml")), None);
}

#[test]
fn ignored_sections_are_silent() {
    let loaded = parse(
        "onboarding = true\n[session]\nresume_agents_on_restore = false\nstartup_per_agent_delay_ms = 0\n\
         [server]\nheadless_cols = 0\nheadless_rows = 0\n[update]\nchannel = \"preview\"\n\
         [remote]\nmanage_ssh_config = false\n[experimental]\npane_history = true\n",
    );
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
}

#[test]
fn soft_diagnostics() {
    let loaded = parse(
        "[theme]\nname = \"neon\"\nlight_name = \"glow\"\n\
         [ui]\nsidebar_min_width = 40\nsidebar_max_width = 20\nwindow_title = \"{oops}\"\n\
         tab_bar_right = [{ type = \"datetime\", format = \"\" }, { type = \"command\", command = \" \", interval_seconds = 0, timeout_seconds = 4000 }]\n\
         [ui.sound]\npath = \"ding.wav\"\ndone_path = \"/definitely/missing.mp3\"\n\
         [keys]\nprefix = \"ctrl+b\"\nhelp = \"q\"\nremote_image_paste = \"ctrl+nope\"\n\
         [[keys.command]]\ncommand = \"\"\nkey = \"prefix+alt+x\"\n\
         [[keys.command]]\ncommand = \"do-thing\"\ntype = \"plugin_action\"\nkey = \"prefix+alt+p\"\n",
    );
    let valid = config::THEME_NAMES.join(", ");
    assert_eq!(
        loaded.diagnostics,
        vec![
            "unsafe direct keybinding: keys.help = \"q\" would intercept typing; use \"prefix+q\" to require the prefix; disabling binding".to_string(),
            "empty custom command: keys.command[0].command; disabling custom command".into(),
            "unsupported custom command: keys.command[1].type = \"plugin_action\"; rodeo has no plugins; disabling custom command".into(),
            "invalid keybinding: keys.remote_image_paste = \"ctrl+nope\"; disabling binding".into(),
            format!("unknown theme name theme.name = \"neon\"; using \"catppuccin\"; valid themes: {valid}"),
            format!("unknown theme name theme.light_name = \"glow\"; using \"catppuccin-latte\"; valid themes: {valid}"),
            "unsupported sound file format: ui.sound.path = ding.wav resolves to /nonexistent/ding.wav; expected an mp3 file; using default sound".into(),
            "missing sound file: ui.sound.done_path = /definitely/missing.mp3 resolves to /definitely/missing.mp3; using default sound".into(),
            "ui.tab_bar_right[0] datetime format is empty; hiding entry".into(),
            "ui.tab_bar_right[1] command is empty; hiding entry".into(),
            "ui.tab_bar_right[1] interval_seconds must be at least 1; hiding entry".into(),
            "ui.tab_bar_right[1] timeout_seconds may be at most 3600; hiding entry".into(),
            "ui.window_title has unknown token '{oops}'; leaving the outer terminal title alone".into(),
            "ui.sidebar_min_width (40) is greater than sidebar_max_width (20)".into(),
        ]
    );
    let config = loaded.config;
    assert_eq!(config.theme.name.as_deref(), Some("neon"));
    assert_eq!(config.ui.sidebar_bounds_or_default(), (18, 36));
    assert!(
        config.keymap().keymap.commands().is_empty(),
        "{:?}",
        config.keymap().keymap.commands()
    );
}

#[test]
fn sound_file_checks() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("done.mp3"), b"id3").unwrap();
    std::fs::create_dir(temp.path().join("folder.mp3")).unwrap();
    let loaded = Config::parse(
        "[ui.sound]\ndone_path = \"done.mp3\"\nrequest_path = \"folder.mp3\"\npath = \"DONE.MP3\"\n",
        temp.path(),
    );
    assert_eq!(loaded.diagnostics.len(), 2, "{:?}", loaded.diagnostics);
    assert!(loaded.diagnostics[0].starts_with("missing sound file: ui.sound.path = DONE.MP3"));
    assert!(loaded.diagnostics[1].starts_with("invalid sound file: ui.sound.request_path"));
    let sound = loaded.config.ui.sound;
    assert_eq!(
        sound.path_for(config::Sound::Done, temp.path()),
        Some(temp.path().join("done.mp3"))
    );
    assert_eq!(sound.path_for(config::Sound::Request, temp.path()), None);
}

#[test]
fn bom_is_removed() {
    assert_eq!(strip_bom("\u{feff}a = 1"), "a = 1");
    let displaced = "[ui]\n\u{feff}sidebar_width = 30\n";
    assert_eq!(strip_bom(displaced), "[ui]\nsidebar_width = 30\n");
    let kept = "[ui]\nwindow_title = \"\u{feff}x\"\n";
    assert_eq!(strip_bom(kept), kept);

    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.toml");
    std::fs::write(&path, "\u{feff}[ui]\n\u{feff}sidebar_width = 30\n").unwrap();
    let loaded = Config::load(&path);
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    assert_eq!(loaded.config.ui.sidebar_width, 30);
}

#[test]
fn check_lists_every_diagnostic() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.toml");
    std::fs::write(&path, "[ui]\nwidth = 1\n[theme]\nname = \"x\"\n").unwrap();
    let issues = check(&path).unwrap_err();
    assert_eq!(issues.len(), 2);
}

#[test]
fn prefix_fallback_at_startup() {
    let loaded = parse("[keys]\nprefix = []\n");
    assert_eq!(
        loaded.diagnostics,
        vec!["invalid keybinding: keys.prefix; using fallback ctrl+b"]
    );
    let keymap = loaded.config.keymap().keymap;
    assert_eq!(keymap.prefixes_label(), "ctrl+b");
}

#[test]
fn reload_applies_valid_sections() {
    let current = parse("[theme]\nname = \"nord\"\n[ui]\nsidebar_width = 30\n").config;
    let reload = current.reload_text("[ui]\nsidebar_width = 20\n", dir());
    assert_eq!(reload.status, ReloadStatus::Applied);
    assert!(reload.diagnostics.is_empty(), "{:?}", reload.diagnostics);
    assert_eq!(reload.config.ui.sidebar_width, 20);
    assert_eq!(reload.config.theme.name, None);
}

#[test]
fn reload_keeps_invalid_sections() {
    let current = parse(
        "[theme]\nname = \"nord\"\n[ui]\nsidebar_width = 30\n[terminal]\ndefault_shell = \"zsh\"\n",
    )
    .config;
    let reload = current.reload_text(
        "[theme]\nname = \"dracula\"\n[ui]\nsidebar_width = 22\npane_borders = \"framed\"\n\
         [terminal]\ndefault_shell = \"fish\"\nmystery = 1\n[toast]\nx = 1\n",
        dir(),
    );
    assert_eq!(reload.status, ReloadStatus::Partial);
    assert_eq!(reload.config.theme.name.as_deref(), Some("dracula"));
    assert_eq!(reload.config.ui.sidebar_width, 30);
    assert_eq!(reload.config.terminal.default_shell, "fish");
    assert_eq!(reload.diagnostics.len(), 3, "{:?}", reload.diagnostics);
    assert_eq!(
        reload.diagnostics[0],
        "unknown config section [toast]; did you mean [ui.toast]? ignoring section"
    );
    assert_eq!(
        reload.diagnostics[1],
        "unknown config key terminal.mystery; ignoring key"
    );
    assert!(reload.diagnostics[2].starts_with("invalid ui config: "));
    assert!(reload.diagnostics[2].ends_with("; keeping current ui settings"));
}

#[test]
fn reload_syntax_error_keeps_everything() {
    let current = parse("[theme]\nname = \"nord\"\n").config;
    let reload = current.reload_text("[theme\n", dir());
    assert_eq!(reload.status, ReloadStatus::Failed);
    assert_eq!(reload.config, current);
    assert!(reload.diagnostics[0].starts_with("config parse error: "));
    assert!(reload.diagnostics[0].ends_with("; keeping current config"));
    assert_eq!(
        summary(&reload.diagnostics, Path::new("config.toml")).as_deref(),
        Some("config.toml invalid; keeping current config; rodeo config check")
    );
}

#[test]
fn reload_prefix_rejection_keeps_keymap() {
    let current = parse("[keys]\nprefix = \"ctrl+a\"\nnew_tab = \"prefix+t\"\n").config;
    let reload = current.reload_text(
        "[keys]\nprefix = \"ctrl+nope\"\nnew_tab = \"prefix+y\"\n",
        dir(),
    );
    assert_eq!(reload.config.keys, current.keys);
    assert_eq!(
        reload.diagnostics,
        vec![
            "invalid keybinding: keys.prefix; using fallback ctrl+b; kept current keybinds",
            "invalid keybinding: keys.prefix = \"ctrl+nope\"; ignoring prefix; kept current keybinds",
        ]
    );
    let other = current.reload_text("[keys]\nprefix = \"ctrl+a\"\nhelp = \"x\"\n", dir());
    assert_eq!(other.config.keys.help, Some("x".into()));
    assert_eq!(other.diagnostics.len(), 1);
    assert!(other.diagnostics[0].starts_with("unsafe direct keybinding"));
}

#[test]
fn reload_inverted_bounds_keep_ui() {
    let current = parse("[ui]\nsidebar_width = 30\n").config;
    let reload = current.reload_text("[ui]\nsidebar_width = 20\nsidebar_min_width = 50\n", dir());
    assert_eq!(reload.config.ui, current.ui);
    assert_eq!(
        reload.diagnostics,
        vec![
            "ui.sidebar_min_width (50) is greater than sidebar_max_width (36); keeping previous [ui] settings"
        ]
    );
}

#[test]
fn reload_ui_soft_diagnostics() {
    let current = Config::default();
    let reload = current.reload_text("[ui]\nwindow_title = \"{\"\n", dir());
    assert_eq!(reload.config.ui.window_title, "{");
    assert_eq!(
        reload.diagnostics,
        vec!["ui.window_title has an unclosed '{'; leaving the outer terminal title alone"]
    );
}

#[test]
fn reload_kitty_graphics_needs_restart() {
    let current = Config::default();
    let reload = current.reload_text(
        "[terminal]\nkitty_graphics = false\nnew_cwd = \"home\"\n",
        dir(),
    );
    assert!(reload.config.kitty_graphics());
    assert_eq!(reload.config.terminal.new_cwd, config::NewCwd::Home);
    assert_eq!(
        reload.diagnostics,
        vec!["terminal.kitty_graphics changes require restarting rodeo; kept current setting"]
    );
    let legacy = current.reload_text("[experimental]\nkitty_graphics = false\n", dir());
    assert!(legacy.config.kitty_graphics());
    assert_eq!(legacy.diagnostics.len(), 1);
}

#[test]
fn reload_invalid_onboarding_keeps_state() {
    let current = parse("onboarding = false\n").config;
    let reload = current.reload_text("onboarding = 3\n", dir());
    assert_eq!(reload.config.onboarding, Some(false));
    assert!(reload.diagnostics[0].starts_with("invalid onboarding setting: "));
}

#[test]
fn reload_from_disk() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.toml");
    let current = parse("[theme]\nname = \"nord\"\n").config;
    let missing = current.reload(&path);
    assert_eq!(missing.status, ReloadStatus::Applied);
    assert_eq!(missing.config, Config::default());
    std::fs::write(&path, "[theme]\nname = \"vesper\"\n").unwrap();
    assert_eq!(
        current.reload(&path).config.theme.name.as_deref(),
        Some("vesper")
    );
    let unreadable = current.reload(temp.path());
    assert_eq!(unreadable.status, ReloadStatus::Failed);
    assert_eq!(unreadable.config, current);
    assert_eq!(
        summary(&unreadable.diagnostics, &path).as_deref(),
        Some("config.toml unreadable; keeping current config; rodeo config check")
    );
}
