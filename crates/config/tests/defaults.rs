use std::path::Path;

use config::{
    Action, AgentPanelSort, AgentSound, AgentToken, ClipboardToastPosition, Config, CursorShape,
    HostCursor, Indexed, NewCwd, PaneBorders, ShellMode, SidebarCollapsedMode, SpaceToken,
    StatusIndicators, TabBarPosition, ToastDelivery, ToastPosition, Token, UpdateChannel,
    binding_fields, default_binding, default_config_text,
};

#[test]
fn top_level_and_runtime_sections() {
    let config = Config::default();
    assert_eq!(config.onboarding, None);
    assert!(config.show_onboarding());

    assert_eq!(config.terminal.default_shell, "");
    assert_eq!(config.terminal.shell_mode, ShellMode::Auto);
    assert_eq!(config.terminal.new_cwd, NewCwd::Follow);
    assert_eq!(config.terminal.kitty_graphics, None);
    assert!(config.kitty_graphics());

    assert!(config.session.resume_agents_on_restore);
    assert_eq!(config.session.startup_per_agent_delay_ms, 100);
    assert_eq!(config.server.headless_cols, 120);
    assert_eq!(config.server.headless_rows, 40);
    assert_eq!(config.update.channel, UpdateChannel::Stable);
    assert!(config.update.version_check);
    assert!(config.update.manifest_check);
    assert_eq!(config.worktrees.directory, "~/.herdr/worktrees");
    assert_eq!(config.advanced.scrollback_limit_bytes, 10_000_000);
    assert!(config.remote.manage_ssh_config);

    let experimental = &config.experimental;
    assert!(!experimental.allow_nested);
    assert_eq!(experimental.kitty_graphics, None);
    assert!(!experimental.pane_history);
    assert!(!experimental.reveal_hidden_cursor_for_cjk_ime);
    assert!(
        experimental.cjk_ime_agents.is_empty(),
        "{:?}",
        experimental.cjk_ime_agents
    );
    assert_eq!(experimental.cjk_ime_cursor_shape, CursorShape::SteadyBlock);
    assert_eq!(experimental.cjk_ime_cursor_shape.decscusr(), 2);
    assert!(!experimental.switch_ascii_input_source_in_prefix);
}

#[test]
fn theme_section() {
    let theme = Config::default().theme;
    assert_eq!(theme.name, None);
    assert_eq!(theme.manual_name(), "catppuccin");
    assert!(!theme.auto_switch);
    assert_eq!(theme.dark_name, None);
    assert_eq!(theme.light_name, None);
    assert_eq!(theme.dark_name(), "catppuccin");
    assert_eq!(theme.light_name(), "catppuccin-latte");
    assert_eq!(theme.custom, None);
}

#[test]
fn ui_section() {
    let ui = Config::default().ui;
    assert_eq!(ui.sidebar_width, 26);
    assert_eq!(ui.sidebar_min_width, 18);
    assert_eq!(ui.sidebar_max_width, 36);
    assert!(!ui.sidebar_start_collapsed);
    assert_eq!(ui.sidebar_collapsed_mode, SidebarCollapsedMode::Compact);
    assert_eq!(ui.mobile_width_threshold, 64);
    assert!(ui.mouse_capture);
    assert!(ui.copy_on_select);
    assert_eq!(ui.host_cursor, HostCursor::Auto);
    assert_eq!(ui.right_click_passthrough_modifier.0, None);
    assert!(ui.redraw_on_focus_gained);
    assert_eq!(ui.mouse_scroll_lines, None);
    assert_eq!(ui.mouse_scroll_lines(), 3);
    assert!(ui.confirm_close);
    assert!(ui.prompt_new_tab_name);
    assert!(!ui.prompt_new_workspace_name);
    assert_eq!(ui.pane_borders, PaneBorders::Auto);
    assert!(ui.pane_outer_borders);
    assert!(ui.pane_scrollbars);
    assert!(ui.pane_gaps);
    assert!(!ui.show_agent_labels_on_pane_borders);
    assert!(!ui.hide_tab_bar_when_single_tab);
    assert_eq!(ui.tab_bar_position, TabBarPosition::Top);
    assert!(ui.tab_bar_right.is_empty(), "{:?}", ui.tab_bar_right);
    assert_eq!(ui.tab_bar_right_separator, " ");
    assert_eq!(ui.window_title, "{hostname}: {workspace}");
    assert_eq!(ui.agent_panel_sort, AgentPanelSort::Spaces);
    assert_eq!(ui.agent_panel_scope, None);
    assert_eq!(ui.status_indicators, StatusIndicators::Dots);
    assert_eq!(ui.accent, "cyan");
    assert_eq!(ui.sidebar_bounds(), Some((18, 36)));

    assert_eq!(ui.toast.delivery, ToastDelivery::Off);
    assert_eq!(ui.toast.delay_seconds, 1);
    assert_eq!(ui.toast.herdr.position, ToastPosition::BottomRight);
    assert!(ui.toast.clipboard.enabled);
    assert_eq!(
        ui.toast.clipboard.position,
        ClipboardToastPosition::BottomCenter
    );

    assert!(ui.sound.enabled);
    assert_eq!(ui.sound.path, None);
    assert_eq!(ui.sound.done_path, None);
    assert_eq!(ui.sound.request_path, None);
    assert_eq!(ui.sound.agents.droid, AgentSound::Off);
    assert_eq!(ui.sound.agents.claude, AgentSound::Default);
    assert_eq!(ui.sound.agents.open_code, AgentSound::Default);
    assert!(ui.sound.allows(Some("claude")));
    assert!(!ui.sound.allows(Some("droid")));
    assert!(ui.sound.allows(None));
}

#[test]
fn sidebar_rows() {
    let sidebar = Config::default().ui.sidebar;
    assert_eq!(
        sidebar.agents.rows,
        vec![
            vec![
                Token::plain(AgentToken::StateIcon),
                Token::plain(AgentToken::Machine),
                Token::plain(AgentToken::Workspace),
                Token::plain(AgentToken::Tab),
            ],
            vec![Token::plain(AgentToken::Agent)],
        ]
    );
    assert!(
        sidebar.agents.rows_by_agent.is_empty(),
        "{:?}",
        sidebar.agents.rows_by_agent
    );
    assert_eq!(sidebar.agents.row_gap, 0);
    assert_eq!(
        sidebar.spaces.rows,
        vec![
            vec![
                Token::plain(SpaceToken::StateIcon),
                Token::plain(SpaceToken::Workspace),
            ],
            vec![
                Token::plain(SpaceToken::Branch),
                Token::plain(SpaceToken::GitStatus),
            ],
        ]
    );
    assert_eq!(sidebar.spaces.row_gap, 0);
}

#[test]
fn key_defaults() {
    let expected = [
        ("help", "prefix+?"),
        ("settings", "prefix+s"),
        ("new_workspace", "prefix+shift+n"),
        ("new_worktree", "prefix+shift+g"),
        ("open_worktree", ""),
        ("remove_worktree", ""),
        ("rename_workspace", "prefix+shift+w"),
        ("close_workspace", "prefix+shift+d"),
        ("workspace_picker", "prefix+w"),
        ("goto", "prefix+g"),
        ("navigate_workspace_up", "up"),
        ("navigate_workspace_down", "down"),
        ("navigate_pane_left", "h"),
        ("navigate_pane_down", "j"),
        ("navigate_pane_up", "k"),
        ("navigate_pane_right", "l"),
        ("detach", "prefix+q"),
        ("reload_config", "prefix+shift+r"),
        ("open_notification_target", "prefix+o"),
        ("previous_workspace", ""),
        ("next_workspace", ""),
        ("previous_agent", ""),
        ("next_agent", ""),
        ("focus_agent", ""),
        ("new_tab", "prefix+c"),
        ("rename_tab", "prefix+shift+t"),
        ("previous_tab", "prefix+p"),
        ("next_tab", "prefix+n"),
        ("move_tab_previous", ""),
        ("move_tab_next", ""),
        ("switch_tab", "prefix+1..9"),
        ("switch_workspace", ""),
        ("close_tab", "prefix+shift+x"),
        ("rename_pane", "prefix+shift+p"),
        ("edit_scrollback", "prefix+e"),
        ("clear_pane", ""),
        ("copy_mode", "prefix+["),
        ("focus_pane_left", "prefix+h"),
        ("focus_pane_down", "prefix+j"),
        ("focus_pane_up", "prefix+k"),
        ("focus_pane_right", "prefix+l"),
        ("swap_pane_left", "prefix+shift+h"),
        ("swap_pane_down", "prefix+shift+j"),
        ("swap_pane_up", "prefix+shift+k"),
        ("swap_pane_right", "prefix+shift+l"),
        ("cycle_pane_next", "prefix+tab"),
        ("cycle_pane_previous", "prefix+shift+tab"),
        ("last_pane", ""),
        ("split_vertical", "prefix+v"),
        ("split_horizontal", "prefix+minus"),
        ("close_pane", "prefix+x"),
        ("zoom", "prefix+z"),
        ("resize_mode", "prefix+r"),
        ("resize_pane_left", ""),
        ("resize_pane_down", ""),
        ("resize_pane_up", ""),
        ("resize_pane_right", ""),
        ("toggle_sidebar", "prefix+b"),
    ];
    assert_eq!(binding_fields().count(), 58);
    for (field, default) in expected {
        assert_eq!(default_binding(field), Some(default), "{field}");
    }
    let keys = Config::default().keys;
    assert_eq!(keys.remote_image_paste(), "ctrl+v");
    assert_eq!(keys.indexed(), config::IndexedKeys::default());
    assert!(keys.command.is_empty(), "{:?}", keys.command);
    assert!(binding_fields().all(|field| keys.binding(field).is_none()));
}

#[test]
fn default_keymap_has_no_diagnostics() {
    let build = Config::default().keymap();
    assert_eq!(build.prefix_error, None);
    assert!(build.diagnostics.is_empty(), "{:?}", build.diagnostics);
    let keymap = build.keymap;
    assert_eq!(keymap.prefixes_label(), "ctrl+b");
    assert_eq!(keymap.label(Action::Help).as_deref(), Some("prefix+?"));
    assert_eq!(
        keymap.indexed_label(Indexed::Tab).as_deref(),
        Some("prefix+1..9")
    );
    assert_eq!(keymap.indexed_label(Indexed::Workspace), None);
    assert_eq!(keymap.label(Action::OpenWorktree), None);
    assert!(keymap.image_paste().is_some());
}

#[test]
fn default_text_parses_to_defaults() {
    let loaded = Config::parse(default_config_text(), Path::new("/nonexistent"));
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    assert_eq!(loaded.config, Config::default());
}

#[test]
fn default_text_lists_every_key_default() {
    let text = default_config_text();
    let start = text.find("\n[keys]\n").expect("keys section");
    let end = text.find("\n[ui]\n").expect("ui section");
    let keys = text[start..end]
        .lines()
        .filter_map(|line| line.strip_prefix("# "))
        .filter(|line| {
            line.split_once(" = ")
                .is_some_and(|(key, _)| key.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_'))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let table: toml::Table = keys.parse().expect("commented keys are valid TOML");
    for field in binding_fields() {
        let value = table
            .get(field)
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("default text lists keys.{field}"));
        assert_eq!(Some(value), default_binding(field), "{field}");
    }
    assert_eq!(table["prefix"].as_str(), Some("ctrl+b"));
    assert_eq!(table["remote_image_paste"].as_str(), Some("ctrl+v"));
}
