use std::path::Path;

use config::{
    Action, Config, Indexed, KeyCombo, KeyPress, Keymap, KeymapBuild, NavigateKey, PrefixKey,
    ResizeKey, TerminalKey,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const NONE: KeyModifiers = KeyModifiers::NONE;
const SHIFT: KeyModifiers = KeyModifiers::SHIFT;
const CTRL: KeyModifiers = KeyModifiers::CONTROL;
const ALT: KeyModifiers = KeyModifiers::ALT;

fn build(keys: &str) -> KeymapBuild {
    let loaded = Config::parse(&format!("[keys]\n{keys}"), Path::new("/"));
    assert!(
        loaded
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.starts_with("config parse error")),
        "{:?}",
        loaded.diagnostics
    );
    loaded.config.keymap()
}

fn keymap(keys: &str) -> Keymap {
    build(keys).keymap
}

fn diagnostics(keys: &str) -> Vec<String> {
    build(keys).all_diagnostics()
}

fn combo(text: &str) -> Option<KeyCombo> {
    KeyCombo::parse(text)
}

fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyPress {
    KeyPress::new(code, modifiers)
}

fn ch(c: char) -> KeyPress {
    key(KeyCode::Char(c), NONE)
}

#[test]
fn modifiers_and_aliases() {
    let x = |modifiers| Some(KeyCombo::new(KeyCode::Char('x'), modifiers));
    assert_eq!(combo("ctrl+x"), x(CTRL));
    assert_eq!(combo("control+x"), x(CTRL));
    assert_eq!(combo(" Ctrl + x "), x(CTRL));
    assert_eq!(combo("alt+x"), x(ALT));
    assert_eq!(combo("option+x"), x(ALT));
    assert_eq!(combo("meta+x"), x(ALT));
    assert_eq!(combo("cmd+x"), x(KeyModifiers::SUPER));
    assert_eq!(combo("command+x"), x(KeyModifiers::SUPER));
    assert_eq!(combo("super+x"), x(KeyModifiers::SUPER));
    assert_eq!(combo("hyper+x"), x(KeyModifiers::HYPER));
    assert_eq!(combo("shift+x"), x(SHIFT));
    assert_eq!(combo("ctrl+alt+shift+x"), x(CTRL | ALT | SHIFT));
    assert_eq!(combo("x+ctrl"), x(CTRL));
    assert_eq!(combo("CTRL+X"), x(CTRL | SHIFT));
}

#[test]
fn named_keys() {
    let named = [
        ("space", KeyCode::Char(' ')),
        ("enter", KeyCode::Enter),
        ("return", KeyCode::Enter),
        ("esc", KeyCode::Esc),
        ("escape", KeyCode::Esc),
        ("tab", KeyCode::Tab),
        ("backspace", KeyCode::Backspace),
        ("bs", KeyCode::Backspace),
        ("left", KeyCode::Left),
        ("right", KeyCode::Right),
        ("up", KeyCode::Up),
        ("down", KeyCode::Down),
        ("minus", KeyCode::Char('-')),
        ("comma", KeyCode::Char(',')),
        ("period", KeyCode::Char('.')),
        ("slash", KeyCode::Char('/')),
        ("backslash", KeyCode::Char('\\')),
        ("quote", KeyCode::Char('\'')),
        ("double_quote", KeyCode::Char('"')),
        ("double-quote", KeyCode::Char('"')),
        ("semicolon", KeyCode::Char(';')),
        ("colon", KeyCode::Char(':')),
        ("percent", KeyCode::Char('%')),
        ("ampersand", KeyCode::Char('&')),
        ("backtick", KeyCode::Char('`')),
        ("plus", KeyCode::Char('+')),
        ("ENTER", KeyCode::Enter),
        ("f1", KeyCode::F(1)),
        ("F12", KeyCode::F(12)),
        ("f0", KeyCode::F(0)),
        ("f255", KeyCode::F(255)),
        ("é", KeyCode::Char('é')),
        ("[", KeyCode::Char('[')),
        ("?", KeyCode::Char('?')),
        ("f", KeyCode::Char('f')),
    ];
    for (text, code) in named {
        assert_eq!(combo(text), Some(KeyCombo::plain(code)), "{text}");
    }
}

#[test]
fn shifted_and_special_forms() {
    assert_eq!(combo("N"), Some(KeyCombo::new(KeyCode::Char('n'), SHIFT)));
    assert_eq!(combo("N"), combo("shift+n"));
    assert_eq!(combo("É"), Some(KeyCombo::plain(KeyCode::Char('É'))));
    let backtab = Some(KeyCombo::plain(KeyCode::BackTab));
    assert_eq!(combo("shift+tab"), backtab);
    assert_eq!(combo("tab+shift"), backtab);
    assert_eq!(
        combo("ctrl+shift+tab"),
        Some(KeyCombo::new(KeyCode::BackTab, CTRL))
    );
    assert_eq!(combo("+"), Some(KeyCombo::plain(KeyCode::Char('+'))));
    assert_eq!(
        combo("ctrl++"),
        Some(KeyCombo::new(KeyCode::Char('+'), CTRL))
    );
    assert_eq!(
        combo("ctrl+alt++"),
        Some(KeyCombo::new(KeyCode::Char('+'), CTRL | ALT))
    );
    assert_eq!(
        combo("ctrl+plus"),
        Some(KeyCombo::new(KeyCode::Char('+'), CTRL))
    );
}

#[test]
fn rejected_forms() {
    for text in [
        "",
        "ctrl+",
        "+ctrl",
        "++",
        "a+b",
        "ctrl+a+b",
        "ctrl",
        "ctrl+shift",
        "delete",
        "home",
        "end",
        "pageup",
        "pagedown",
        "insert",
        "f256",
        "fx",
        "foo",
        "x++",
        "ctrl+ +x",
    ] {
        assert_eq!(combo(text), None, "{text:?}");
    }
}

#[test]
fn labels() {
    let label = |text: &str| combo(text).unwrap().label_for(false);
    assert_eq!(label("ctrl+b"), "ctrl+b");
    assert_eq!(label("shift+alt+ctrl+x"), "ctrl+alt+shift+x");
    assert_eq!(label("super+hyper+alt+x"), "alt+super+hyper+x");
    assert_eq!(label("shift+tab"), "shift+tab");
    assert_eq!(label("ctrl+shift+tab"), "ctrl+shift+tab");
    assert_eq!(label("space"), "space");
    assert_eq!(label("minus"), "-");
    assert_eq!(label("N"), "shift+n");
    assert_eq!(label("f5"), "f5");
    assert_eq!(label("ctrl++"), "ctrl++");
    assert_eq!(label("BS"), "backspace");
    assert_eq!(combo("cmd+k").unwrap().label_for(true), "cmd+k");
    assert_eq!(combo("cmd+k").unwrap().label_for(false), "super+k");
    let meta = KeyCombo::new(KeyCode::Char('m'), KeyModifiers::META | CTRL);
    assert_eq!(meta.label_for(false), "ctrl+meta+m");
    assert_eq!(meta.to_string(), meta.label());

    let keymap = keymap(
        "prefix = [\"ctrl+a\", \"f12\"]\nhelp = [\"prefix+?\", \"prefix+h\", \"ctrl+alt+h\"]\nfocus_pane_left = \"\"\n",
    );
    assert_eq!(keymap.prefix_label(), "ctrl+a");
    assert_eq!(keymap.prefixes_label(), "ctrl+a / f12");
    assert_eq!(
        keymap.label(Action::Help).as_deref(),
        Some("prefix+? / prefix+h / ctrl+alt+h")
    );
    assert_eq!(keymap.rhs_label(Action::Help).as_deref(), Some("? / h"));
    assert_eq!(keymap.rhs_label(Action::OpenWorktree), None);
    assert_eq!(
        Keymap::default().label(Action::SplitHorizontal).as_deref(),
        Some("prefix+-")
    );
}

#[test]
fn unbind_and_arrays() {
    let keymap = keymap(
        "new_tab = \"\"\nclose_tab = []\nsplit_vertical = [\"prefix+v\", \"prefix+|\", \"\"]\n",
    );
    assert!(
        keymap.bindings(Action::NewTab).is_empty(),
        "{:?}",
        keymap.bindings(Action::NewTab)
    );
    assert!(
        keymap.bindings(Action::CloseTab).is_empty(),
        "{:?}",
        keymap.bindings(Action::CloseTab)
    );
    assert_eq!(
        keymap.label(Action::SplitVertical).as_deref(),
        Some("prefix+v / prefix+|")
    );
    let blank = self::keymap("switch_tab = \" \"\n");
    assert!(
        blank.indexed(Indexed::Tab).is_empty(),
        "{:?}",
        blank.indexed(Indexed::Tab)
    );
}

#[test]
fn indexed_ranges() {
    let keymap = keymap(
        "switch_tab = \"ctrl+alt+1..9\"\nswitch_workspace = [\"prefix+shift+1..9\", \"prefix+0\"]\nfocus_agent = \"prefix+alt+3\"\n",
    );
    let tabs = keymap.indexed(Indexed::Tab);
    assert_eq!(tabs.len(), 9);
    assert_eq!(tabs[0].label, "ctrl+alt+1");
    assert_eq!(tabs[8].label, "ctrl+alt+9");
    assert_eq!(
        keymap.indexed_label(Indexed::Tab).as_deref(),
        Some("ctrl+alt+1..9")
    );
    assert_eq!(
        keymap.indexed_label(Indexed::Workspace).as_deref(),
        Some("prefix+shift+1..9")
    );
    assert_eq!(
        keymap.indexed_label(Indexed::Agent).as_deref(),
        Some("prefix+alt+3")
    );
    assert_eq!(
        build("switch_workspace = [\"prefix+shift+1..9\", \"prefix+0\"]\n").diagnostics,
        vec![
            "indexed keybinding must use 1..9: keys.switch_workspace = \"prefix+0\"; disabling binding"
        ]
    );
    assert_eq!(
        diagnostics("new_tab = \"prefix+1..9\"\n"),
        vec![
            "range keybinding is only valid for indexed actions: keys.new_tab = \"prefix+1..9\"; disabling binding"
        ]
    );
    assert_eq!(
        diagnostics("navigate_pane_left = \"1..9\"\n"),
        vec![
            "range keybinding is only valid for indexed actions: keys.navigate_pane_left = \"1..9\"; disabling binding"
        ]
    );
    assert_eq!(
        diagnostics("switch_tab = \"prefix+1..9+1..9\"\n"),
        vec!["invalid keybinding: keys.switch_tab = \"prefix+1..9+1..9\"; disabling binding"]
    );
}

#[test]
fn invalid_bindings() {
    assert_eq!(
        diagnostics("help = [\"prefix+home\", \"prefix+?\"]\n"),
        vec!["invalid keybinding: keys.help = \"prefix+home\"; disabling binding"]
    );
    assert_eq!(
        diagnostics("prefix = [\"ctrl+a\", \"ctrl+a\", \"bogus+x\"]\n"),
        vec!["invalid keybinding: keys.prefix = \"bogus+x\"; ignoring prefix"]
    );
    let prefixes = keymap("prefix = [\"ctrl+a\", \"ctrl+a\"]\n");
    assert_eq!(prefixes.prefixes().len(), 1);
    assert_eq!(
        diagnostics("prefix = \"\"\n"),
        vec![
            "invalid keybinding: keys.prefix; using fallback ctrl+b",
            "invalid keybinding: keys.prefix = \"\"; ignoring prefix",
        ]
    );
    assert_eq!(
        diagnostics("remote_image_paste = \"prefix+v\"\n"),
        vec!["invalid keybinding: keys.remote_image_paste = \"prefix+v\"; disabling binding"]
    );
    assert!(
        keymap("remote_image_paste = \"\"\n")
            .image_paste()
            .is_none()
    );
}

#[test]
fn extra_prefixes_merge() {
    let keymap = keymap("prefix = \"ctrl+a\"\nextra_prefixes = [\"ctrl+space\"]\n");
    assert_eq!(keymap.prefixes_label(), "ctrl+a / ctrl+space");
    assert!(keymap.is_prefix(key(KeyCode::Char(' '), CTRL)));
    let only_extra = self::keymap("extra_prefixes = \"f12\"\n");
    assert_eq!(only_extra.prefixes_label(), "f12");
}

#[test]
fn unsafe_direct_bindings() {
    assert_eq!(
        diagnostics("help = \"shift+a\"\n"),
        vec![
            "unsafe direct keybinding: keys.help = \"shift+a\" would intercept typing; use \"prefix+shift+a\" to require the prefix; disabling binding"
        ]
    );
    assert!(
        diagnostics("help = [\"ctrl+a\", \"f2\", \"alt+?\", \"enter\"]\n").is_empty(),
        "{:?}",
        diagnostics("help = [\"ctrl+a\", \"f2\", \"alt+?\", \"enter\"]\n")
    );
    assert_eq!(diagnostics("switch_tab = \"1..9\"\n").len(), 9);
    assert_eq!(
        diagnostics("[[keys.command]]\ncommand = \"ls\"\nkey = \"z\"\n"),
        vec![
            "unsafe direct keybinding: keys.command[0].key = \"z\" would intercept typing; use \"prefix+z\" to require the prefix; disabling binding"
        ]
    );
    assert!(
        diagnostics("navigate_pane_left = \"a\"\n").is_empty(),
        "{:?}",
        diagnostics("navigate_pane_left = \"a\"\n")
    );
}

#[test]
fn conflicts_follow_registration_order() {
    assert_eq!(
        diagnostics("new_tab = \"prefix+t\"\nhelp = \"prefix+t\"\n"),
        vec!["prefix+t: kept keys.help, disabled keys.new_tab"]
    );
    assert_eq!(
        diagnostics(
            "toggle_sidebar = \"ctrl+alt+s\"\ncopy_mode = \"ctrl+alt+s\"\nswitch_tab = \"ctrl+alt+1..9\"\nfocus_agent = \"ctrl+alt+1\"\n"
        ),
        vec![
            "ctrl+alt+1: kept keys.focus_agent, disabled keys.switch_tab",
            "ctrl+alt+s: kept keys.copy_mode, disabled keys.toggle_sidebar",
        ]
    );
    let swapped = keymap("help = \"prefix+c\"\n");
    assert!(
        swapped.bindings(Action::NewTab).is_empty(),
        "{:?}",
        swapped.bindings(Action::NewTab)
    );
    assert!(
        diagnostics("help = \"prefix+c\"\n").is_empty(),
        "{:?}",
        diagnostics("help = \"prefix+c\"\n")
    );
    assert_eq!(
        diagnostics("help = \"ctrl+b\"\n"),
        vec!["ctrl+b: kept keys.prefix, disabled keys.help"]
    );
}

#[test]
fn prefix_reservations() {
    assert_eq!(
        diagnostics("help = \"prefix+ctrl+b\"\n"),
        vec![
            "reserved keybinding: keys.help = \"prefix+ctrl+b\" uses keys.prefix as the prefix-mode key; pressing the prefix twice sends a literal prefix key, so this binding is disabled"
        ]
    );
    let user_prefix = build("prefix = \"c\"\n");
    assert!(
        user_prefix.all_diagnostics().is_empty(),
        "{:?}",
        user_prefix.all_diagnostics()
    );
    assert!(
        user_prefix.keymap.bindings(Action::NewTab).is_empty(),
        "{:?}",
        user_prefix.keymap.bindings(Action::NewTab)
    );
    let direct_default = build("prefix = \"up\"\n");
    assert!(
        direct_default.all_diagnostics().is_empty(),
        "{:?}",
        direct_default.all_diagnostics()
    );
    assert_eq!(
        direct_default
            .keymap
            .navigate_label(Action::NavigateWorkspaceUp),
        None
    );
}

#[test]
fn navigate_field_rules() {
    assert_eq!(
        diagnostics("navigate_pane_left = \"prefix+h\"\n"),
        vec![
            "navigate keybinding must not include prefix: keys.navigate_pane_left = \"prefix+h\"; disabling binding"
        ]
    );
    assert_eq!(
        diagnostics("navigate_pane_left = \"esc\"\n"),
        vec![
            "navigate keybinding cannot use esc: keys.navigate_pane_left = \"esc\"; disabling binding"
        ]
    );
    for (value, label) in [
        ("enter", "enter"),
        ("tab", "tab"),
        ("shift+tab", "shift+tab"),
        ("left", "left"),
        ("right", "right"),
        ("5", "5"),
    ] {
        assert_eq!(
            diagnostics(&format!("navigate_pane_up = \"{value}\"\n")),
            vec![format!(
                "{label}: kept navigate reserved keys, disabled keys.navigate_pane_up"
            )]
        );
    }
    assert_eq!(
        diagnostics("navigate_pane_up = \"ctrl+b\"\n"),
        vec!["ctrl+b: kept keys.prefix, disabled keys.navigate_pane_up"]
    );
    assert_eq!(
        diagnostics("navigate_pane_left = \"x\"\nnavigate_pane_right = \"x\"\n"),
        vec!["x: kept keys.navigate_pane_left, disabled keys.navigate_pane_right"]
    );
    let swapped = build("navigate_pane_down = \"k\"\n");
    assert!(
        swapped.all_diagnostics().is_empty(),
        "{:?}",
        swapped.all_diagnostics()
    );
    assert_eq!(swapped.keymap.navigate_label(Action::FocusPaneUp), None);
    let keymap = Keymap::default();
    assert_eq!(
        keymap.label(Action::FocusPaneDown).as_deref(),
        Some("prefix+j")
    );
    assert_eq!(
        keymap.navigate_label(Action::FocusPaneDown).as_deref(),
        Some("j")
    );
}

#[test]
fn legacy_indexed() {
    let keymap = keymap("[keys.indexed]\ntabs = \"ctrl\"\nagents = \"ctrl+shift\"\n");
    let tabs = keymap.indexed(Indexed::Tab);
    assert_eq!(tabs.len(), 9);
    assert!(tabs.iter().all(config::KeyBinding::is_direct));
    assert_eq!(
        keymap.indexed_label(Indexed::Tab).as_deref(),
        Some("ctrl+1..9")
    );
    assert_eq!(
        keymap.indexed_label(Indexed::Agent).as_deref(),
        Some("ctrl+shift+1..9")
    );
    assert_eq!(
        keymap.resolve_terminal(key(KeyCode::Char('4'), CTRL)),
        Some(TerminalKey::Run(Action::SwitchTab(3)))
    );
    assert_eq!(keymap.prefixed(ch('1')), None);

    let invalid = build("[keys.indexed]\ntabs = \"ctrl+x\"\n");
    assert_eq!(
        invalid.diagnostics,
        vec!["invalid indexed keybinding: keys.indexed.tabs = \"ctrl+x\"; disabling binding"]
    );
    assert!(
        invalid.keymap.indexed(Indexed::Tab).is_empty(),
        "{:?}",
        invalid.keymap.indexed(Indexed::Tab)
    );

    let combined = self::keymap("switch_tab = \"prefix+1..9\"\n[keys.indexed]\ntabs = \"alt\"\n");
    assert_eq!(
        combined.indexed_label(Indexed::Tab).as_deref(),
        Some("prefix+1..9 / alt+1..9")
    );
}

#[test]
fn custom_commands() {
    let build = build(
        "[[keys.command]]\nkey = \"prefix+alt+g\"\ncommand = \"lazygit\"\ntype = \"popup\"\ndescription = \"git\"\nwidth = \"80%\"\nheight = 20\n\
         [[keys.command]]\nkey = [\"ctrl+alt+t\", \"prefix+T\"]\ncommand = \"top\"\ntype = \"pane\"\nwidth = 3\n\
         [[keys.command]]\nkey = \"\"\ncommand = \"unbound\"\n",
    );
    assert_eq!(
        build.diagnostics,
        vec!["popup size on non-popup custom command: keys.command[1]; ignoring width and height"]
    );
    let commands = build.keymap.commands();
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].label, "prefix+alt+g");
    assert_eq!(commands[0].kind, config::CommandKind::Popup);
    assert_eq!(commands[0].width, Some(config::PopupSize::Percent(80)));
    assert_eq!(commands[0].height, Some(config::PopupSize::Cells(20)));
    assert_eq!(config::PopupSize::Percent(80).resolve(50), 40);
    assert_eq!(commands[1].label, "ctrl+alt+t / prefix+shift+t");
    assert_eq!(commands[1].width, None);
    assert_eq!(
        build
            .keymap
            .resolve_terminal(key(KeyCode::Char('t'), CTRL | ALT)),
        Some(TerminalKey::Run(Action::Command(1)))
    );
    assert_eq!(build.keymap.command(1).unwrap().command, "top");
    assert_eq!(
        build.keymap.label(Action::Command(0)).as_deref(),
        Some("prefix+alt+g")
    );

    assert_eq!(
        diagnostics(
            "new_tab = \"prefix+t\"\n[[keys.command]]\ncommand = \"x\"\nkey = \"prefix+t\"\n"
        ),
        vec!["prefix+t: kept keys.new_tab, disabled keys.command[0].key"]
    );
    let over_default = self::build("[[keys.command]]\ncommand = \"x\"\nkey = \"prefix+c\"\n");
    assert!(
        over_default.all_diagnostics().is_empty(),
        "{:?}",
        over_default.all_diagnostics()
    );
    assert!(
        over_default.keymap.bindings(Action::NewTab).is_empty(),
        "{:?}",
        over_default.keymap.bindings(Action::NewTab)
    );
    assert_eq!(
        diagnostics("[[keys.command]]\ncommand = \"x\"\nkey = \"prefix+1..9\"\n"),
        vec![
            "range keybinding is only valid for indexed actions: keys.command[0].key = \"prefix+1..9\"; disabling binding"
        ]
    );
}

#[test]
fn exact_modifiers() {
    let ctrl_x = combo("ctrl+x").unwrap();
    assert!(ctrl_x.matches(key(KeyCode::Char('x'), CTRL)));
    assert!(!ctrl_x.matches(key(KeyCode::Char('x'), CTRL | ALT)));
    assert!(!ctrl_x.matches(ch('x')));
    let f5 = combo("f5").unwrap();
    assert!(f5.matches(key(KeyCode::F(5), NONE)));
    assert!(!f5.matches(key(KeyCode::F(5), SHIFT)));
    let backtab = combo("shift+tab").unwrap();
    assert!(backtab.matches(key(KeyCode::BackTab, NONE)));
    assert!(backtab.matches(key(KeyCode::BackTab, SHIFT)));
    assert!(backtab.matches(key(KeyCode::Tab, SHIFT)));
    assert!(!backtab.matches(key(KeyCode::Tab, NONE)));
}

#[test]
fn shifted_letters() {
    let upper = combo("shift+h").unwrap();
    assert!(upper.matches(ch('H')));
    assert!(upper.matches(key(KeyCode::Char('H'), SHIFT)));
    assert!(upper.matches(key(KeyCode::Char('h'), SHIFT)));
    assert!(!upper.matches(ch('h')));
    let ctrl_upper = combo("ctrl+shift+h").unwrap();
    assert!(ctrl_upper.matches(key(KeyCode::Char('H'), CTRL)));
    let lower = combo("h").unwrap();
    assert!(lower.matches(ch('h')));
    assert!(!lower.matches(ch('H')));
    assert!(!lower.matches(key(KeyCode::Char('h'), SHIFT)));
    assert!(!lower.matches(key(KeyCode::Char('H'), SHIFT)));
}

#[test]
fn shifted_punctuation() {
    let question = combo("?").unwrap();
    assert!(question.matches(ch('?')));
    assert!(question.matches(key(KeyCode::Char('?'), SHIFT)));
    assert!(question.matches(key(KeyCode::Char('/'), SHIFT).with_shifted('?')));
    assert!(!question.matches(key(KeyCode::Char('/'), SHIFT)));
    assert!(!question.matches(key(KeyCode::Char('/'), SHIFT).with_shifted('!')));
    let slash = combo("/").unwrap();
    assert!(!slash.matches(key(KeyCode::Char('/'), SHIFT)));
    let bracket = combo("[").unwrap();
    assert!(!bracket.matches(key(KeyCode::Char('['), SHIFT)));
    let ctrl_question = combo("ctrl+?").unwrap();
    assert!(ctrl_question.matches(key(KeyCode::Char('/'), CTRL | SHIFT).with_shifted('?')));
}

#[test]
fn shifted_digit_symbols() {
    let keymap = keymap("switch_workspace = \"prefix+shift+1..9\"\nswitch_tab = \"prefix+1..9\"\n");
    assert_eq!(keymap.prefixed(ch('!')), Some(Action::SwitchWorkspace(0)));
    assert_eq!(keymap.prefixed(ch('(')), Some(Action::SwitchWorkspace(8)));
    assert_eq!(
        keymap.prefixed(key(KeyCode::Char('1'), SHIFT)),
        Some(Action::SwitchWorkspace(0))
    );
    assert_eq!(keymap.prefixed(ch('1')), Some(Action::SwitchTab(0)));
    assert_eq!(keymap.prefixed(ch(')')), None);
    let plain = self::keymap("switch_tab = \"prefix+1..9\"\n");
    assert_eq!(plain.prefixed(ch('!')), None);
}

#[test]
fn resolution_order() {
    let keymap = keymap("help = \"prefix+?\"\nsettings = \"prefix+shift+/\"\n");
    let shifted_slash = key(KeyCode::Char('/'), SHIFT).with_shifted('?');
    assert_eq!(keymap.prefixed(shifted_slash), Some(Action::Help));
    assert_eq!(
        keymap.prefixed(key(KeyCode::Char('/'), SHIFT)),
        Some(Action::Settings)
    );
    let both = self::keymap(
        "switch_tab = \"prefix+alt+1..9\"\n[[keys.command]]\ncommand = \"x\"\nkey = \"prefix+alt+shift+1\"\n",
    );
    assert_eq!(
        both.prefixed(key(KeyCode::Char('1'), ALT | SHIFT)),
        Some(Action::Command(0))
    );
}

#[test]
fn prefix_fallback_uses_typed_char() {
    let keymap = Keymap::default();
    let typed = key(KeyCode::Char('ß'), SHIFT).with_text('?');
    assert_eq!(keymap.resolve_prefix(typed), PrefixKey::Run(Action::Help));
    assert_eq!(
        keymap.resolve_prefix(key(KeyCode::Char('ß'), SHIFT)),
        PrefixKey::Drop
    );
    let control_text = key(KeyCode::Char('x'), CTRL).with_text('\u{18}');
    assert_eq!(keymap.resolve_prefix(control_text), PrefixKey::Drop);
    assert_eq!(keymap.direct(typed), None);
}

#[test]
fn terminal_mode() {
    let keymap = keymap("previous_tab = \"alt+[\"\n");
    assert_eq!(
        keymap.resolve_terminal(key(KeyCode::Char('['), ALT)),
        Some(TerminalKey::Run(Action::PreviousTab))
    );
    assert_eq!(
        keymap.resolve_terminal(KeyEvent::new(KeyCode::Char('b'), CTRL)),
        Some(TerminalKey::EnterPrefix)
    );
    assert_eq!(keymap.resolve_terminal(ch('a')), None);
    assert_eq!(keymap.resolve_terminal(ch('?')), None);
    assert!(keymap.is_image_paste(key(KeyCode::Char('v'), CTRL)));
    assert!(!keymap.is_image_paste(ch('v')));
}

#[test]
fn prefix_mode() {
    let keymap = keymap("[[keys.command]]\ncommand = \"x\"\nkey = \"prefix+alt+x\"\n");
    assert_eq!(
        keymap.resolve_prefix(key(KeyCode::Char('b'), CTRL)),
        PrefixKey::SendPrefix
    );
    assert_eq!(
        keymap.resolve_prefix(key(KeyCode::Esc, NONE)),
        PrefixKey::Cancel
    );
    assert_eq!(
        keymap.resolve_prefix(ch('c')),
        PrefixKey::Run(Action::NewTab)
    );
    assert_eq!(keymap.resolve_prefix(ch('?')), PrefixKey::Run(Action::Help));
    assert_eq!(
        keymap.resolve_prefix(ch('N')),
        PrefixKey::Run(Action::NewWorkspace)
    );
    assert_eq!(
        keymap.resolve_prefix(ch('3')),
        PrefixKey::Run(Action::SwitchTab(2))
    );
    assert_eq!(
        keymap.resolve_prefix(key(KeyCode::Tab, NONE)),
        PrefixKey::Run(Action::CyclePaneNext)
    );
    assert_eq!(
        keymap.resolve_prefix(key(KeyCode::BackTab, SHIFT)),
        PrefixKey::Run(Action::CyclePanePrevious)
    );
    assert_eq!(
        keymap.resolve_prefix(key(KeyCode::Char('x'), ALT)),
        PrefixKey::Run(Action::Command(0))
    );
    assert_eq!(keymap.resolve_prefix(ch('y')), PrefixKey::Drop);
    let every = [
        ('s', Action::Settings),
        ('m', Action::Machines),
        ('w', Action::WorkspacePicker),
        ('g', Action::Goto),
        ('q', Action::Detach),
        ('o', Action::OpenNotificationTarget),
        ('p', Action::PreviousTab),
        ('n', Action::NextTab),
        ('e', Action::EditScrollback),
        ('[', Action::CopyMode),
        ('h', Action::FocusPaneLeft),
        ('j', Action::FocusPaneDown),
        ('k', Action::FocusPaneUp),
        ('l', Action::FocusPaneRight),
        ('H', Action::SwapPaneLeft),
        ('J', Action::SwapPaneDown),
        ('K', Action::SwapPaneUp),
        ('L', Action::SwapPaneRight),
        ('v', Action::SplitVertical),
        ('-', Action::SplitHorizontal),
        ('x', Action::ClosePane),
        ('z', Action::Zoom),
        ('r', Action::ResizeMode),
        ('b', Action::ToggleSidebar),
        ('G', Action::NewWorktree),
        ('W', Action::RenameWorkspace),
        ('D', Action::CloseWorkspace),
        ('R', Action::ReloadConfig),
        ('T', Action::RenameTab),
        ('X', Action::CloseTab),
        ('P', Action::RenamePane),
    ];
    for (c, action) in every {
        assert_eq!(keymap.resolve_prefix(ch(c)), PrefixKey::Run(action), "{c}");
    }
}

#[test]
fn navigate_mode() {
    let keymap = keymap("[[keys.command]]\ncommand = \"x\"\nkey = \"prefix+y\"\n");
    let nav = |key: KeyPress| keymap.resolve_navigate(key, false);
    assert_eq!(nav(key(KeyCode::Esc, NONE)), Some(NavigateKey::Cancel));
    assert_eq!(
        nav(key(KeyCode::Char('b'), CTRL)),
        Some(NavigateKey::Cancel)
    );
    assert_eq!(
        nav(key(KeyCode::Up, NONE)),
        Some(NavigateKey::Stay(Action::NavigateWorkspaceUp))
    );
    assert_eq!(
        nav(key(KeyCode::Down, NONE)),
        Some(NavigateKey::Stay(Action::NavigateWorkspaceDown))
    );
    assert_eq!(nav(key(KeyCode::Enter, NONE)), Some(NavigateKey::Open));
    assert_eq!(
        nav(ch('4')),
        Some(NavigateKey::Run(Action::SwitchWorkspace(3)))
    );
    assert_eq!(
        nav(key(KeyCode::Tab, NONE)),
        Some(NavigateKey::Run(Action::CyclePaneNext))
    );
    assert_eq!(
        nav(key(KeyCode::Tab, SHIFT)),
        Some(NavigateKey::Run(Action::CyclePanePrevious))
    );
    assert_eq!(
        nav(key(KeyCode::Left, NONE)),
        Some(NavigateKey::Stay(Action::FocusPaneLeft))
    );
    assert_eq!(
        nav(key(KeyCode::Right, NONE)),
        Some(NavigateKey::Stay(Action::FocusPaneRight))
    );
    assert_eq!(nav(ch('j')), Some(NavigateKey::Stay(Action::FocusPaneDown)));
    assert_eq!(nav(ch('c')), Some(NavigateKey::Run(Action::NewTab)));
    assert_eq!(nav(ch('L')), Some(NavigateKey::Run(Action::SwapPaneRight)));
    assert_eq!(nav(ch('y')), Some(NavigateKey::Run(Action::Command(0))));
    assert_eq!(
        nav(key(KeyCode::Char('['), CTRL)),
        Some(NavigateKey::Cancel)
    );
    assert_eq!(nav(ch('m')), Some(NavigateKey::Run(Action::Machines)));
    assert_eq!(nav(ch('u')), None);

    assert_eq!(
        keymap.resolve_navigate(ch('c'), true),
        Some(NavigateKey::Blocked)
    );
    assert_eq!(
        keymap.resolve_navigate(key(KeyCode::Char('['), CTRL), true),
        Some(NavigateKey::Cancel)
    );
    assert_eq!(
        keymap.resolve_navigate(key(KeyCode::Up, NONE), true),
        Some(NavigateKey::Stay(Action::NavigateWorkspaceUp))
    );
    assert_eq!(
        keymap.resolve_navigate(key(KeyCode::Enter, NONE), true),
        Some(NavigateKey::Open)
    );

    let moved =
        self::keymap("navigate_pane_left = \"ctrl+h\"\nswitch_workspace = \"prefix+alt+1..9\"\n");
    assert_eq!(moved.resolve_navigate(ch('h'), false), None);
    assert_eq!(
        moved.resolve_navigate(key(KeyCode::Char('h'), CTRL), false),
        Some(NavigateKey::Stay(Action::FocusPaneLeft))
    );
    assert_eq!(
        moved.resolve_navigate(key(KeyCode::Char('2'), ALT), false),
        Some(NavigateKey::Run(Action::SwitchWorkspace(1)))
    );
}

#[test]
fn resize_mode() {
    let keymap = keymap("resize_mode = [\"prefix+r\", \"ctrl+alt+r\"]\n");
    assert_eq!(
        keymap.resolve_resize(key(KeyCode::Esc, NONE)),
        Some(ResizeKey::Leave)
    );
    assert_eq!(
        keymap.resolve_resize(key(KeyCode::Enter, NONE)),
        Some(ResizeKey::Leave)
    );
    assert_eq!(keymap.resolve_resize(ch('r')), Some(ResizeKey::Leave));
    assert_eq!(
        keymap.resolve_resize(key(KeyCode::Char('r'), CTRL | ALT)),
        Some(ResizeKey::Leave)
    );
    for (press, action) in [
        (ch('h'), Action::ResizePaneLeft),
        (key(KeyCode::Left, NONE), Action::ResizePaneLeft),
        (key(KeyCode::Char('h'), CTRL), Action::ResizePaneLeft),
        (ch('j'), Action::ResizePaneDown),
        (key(KeyCode::Down, NONE), Action::ResizePaneDown),
        (ch('k'), Action::ResizePaneUp),
        (key(KeyCode::Up, SHIFT), Action::ResizePaneUp),
        (ch('l'), Action::ResizePaneRight),
        (key(KeyCode::Right, NONE), Action::ResizePaneRight),
    ] {
        assert_eq!(keymap.resolve_resize(press), Some(ResizeKey::Step(action)));
    }
    assert_eq!(keymap.resolve_resize(ch('x')), None);
    assert_eq!(keymap.resolve_resize(key(KeyCode::Char('b'), CTRL)), None);
}

#[test]
fn key_event_conversion() {
    let event = KeyEvent::new(KeyCode::Char('c'), CTRL);
    assert_eq!(KeyPress::from(event), key(KeyCode::Char('c'), CTRL));
    assert_eq!(KeyPress::from(&event).text, None);
    assert_eq!(Keymap::default().direct(event), None);
}

fn help_keymap() -> Keymap {
    keymap(
        "prefix = [\"ctrl+b\", \"ctrl+a\"]\nswitch_workspace = \"ctrl+alt+1..9\"\n\
         [[keys.command]]\ncommand = \"x\"\nkey = \"prefix+alt+x\"\ndescription = \"run x\"\n\
         [[keys.command]]\ncommand = \"y\"\nkey = \"prefix+alt+y\"\n",
    )
}

fn help_rows(index: usize) -> Vec<(String, String)> {
    help_keymap().help_groups()[index]
        .entries
        .iter()
        .map(|entry| (entry.keys.clone(), entry.label.clone()))
        .collect()
}

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(keys, label)| ((*keys).to_string(), (*label).to_string()))
        .collect()
}

#[test]
fn help_group_titles() {
    let titles: Vec<_> = help_keymap()
        .help_groups()
        .iter()
        .map(|group| group.title)
        .collect();
    assert_eq!(
        titles,
        [
            "global",
            "navigation",
            "workspaces / tabs",
            "panes",
            "custom"
        ]
    );
    assert_eq!(Keymap::default().help_groups().len(), 4);
}

#[test]
fn help_global_navigation_custom() {
    let rows = help_rows;
    assert_eq!(
        rows(0),
        pairs(&[
            ("ctrl+b / ctrl+a", "prefix mode"),
            ("prefix+?", "keybinds"),
            ("prefix+s", "settings"),
            ("prefix+m", "machines"),
            ("prefix+q", "detach"),
            ("prefix+shift+r", "reload config"),
            ("prefix+o", "open notification target"),
        ])
    );
    assert_eq!(
        rows(1),
        pairs(&[
            ("esc", "back"),
            ("up / down", "workspace list"),
            ("h / j / k / l / left / right", "move focus"),
            ("tab / shift+tab", "cycle pane"),
            ("enter", "open workspace"),
            ("1..9", "switch workspace"),
        ])
    );
    assert_eq!(
        rows(4),
        pairs(&[
            ("prefix+alt+x", "run x"),
            ("prefix+alt+y", "custom command")
        ])
    );
}

#[test]
fn help_workspaces_and_tabs() {
    let rows = help_rows;
    assert_eq!(
        rows(2),
        pairs(&[
            ("prefix+w", "workspace navigation"),
            ("prefix+g", "session navigator"),
            ("prefix+shift+n", "new workspace"),
            ("prefix+shift+g", "new worktree"),
            ("unset", "open worktree"),
            ("unset", "delete worktree checkout"),
            ("prefix+shift+w", "rename workspace"),
            ("prefix+shift+d", "close workspace"),
            ("unset", "previous workspace"),
            ("unset", "next workspace"),
            ("ctrl+alt+1..9", "switch workspace 1-9"),
            ("unset", "previous agent"),
            ("unset", "next agent"),
            ("unset", "focus agent 1-9"),
            ("prefix+c", "new tab"),
            ("prefix+shift+t", "rename tab"),
            ("prefix+p", "previous tab"),
            ("prefix+n", "next tab"),
            ("unset", "move tab left"),
            ("unset", "move tab right"),
            ("prefix+1..9", "switch tab 1-9"),
            ("prefix+shift+x", "close tab"),
        ])
    );
}

#[test]
fn help_panes() {
    let rows = help_rows;
    assert_eq!(
        rows(3),
        pairs(&[
            ("prefix+v", "split vertical"),
            ("prefix+-", "split horizontal"),
            ("prefix+x", "close pane"),
            ("prefix+shift+p", "rename pane"),
            ("prefix+e", "edit scrollback"),
            ("unset", "clear pane"),
            ("prefix+[", "copy mode"),
            ("prefix+z", "zoom pane"),
            ("prefix+r", "resize mode"),
            ("unset", "resize pane left"),
            ("unset", "resize pane down"),
            ("unset", "resize pane up"),
            ("unset", "resize pane right"),
            ("prefix+b", "toggle sidebar"),
            ("prefix+h", "focus pane left"),
            ("prefix+j", "focus pane down"),
            ("prefix+k", "focus pane up"),
            ("prefix+l", "focus pane right"),
            ("prefix+tab", "cycle pane next"),
            ("prefix+shift+tab", "cycle pane previous"),
            ("unset", "last pane"),
        ])
    );
}

#[test]
fn help_filter() {
    let keymap = help_keymap();
    let groups = keymap.help_groups();
    assert!(
        groups
            .iter()
            .flat_map(|group| &group.entries)
            .all(|entry| !entry.label.contains("swap"))
    );

    let filtered = config::filter_help(keymap.help_groups(), "ZOOM");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "panes");
    assert_eq!(filtered[0].entries[0].label, "zoom pane");
    let by_key = config::filter_help(keymap.help_groups(), "ctrl+a");
    assert_eq!(by_key[0].entries[0].label, "prefix mode");
    assert!(
        config::filter_help(keymap.help_groups(), "nothing-here").is_empty(),
        "{:?}",
        config::filter_help(keymap.help_groups(), "nothing-here")
    );
    assert_eq!(config::filter_help(keymap.help_groups(), "").len(), 5);
}

#[test]
fn machines_binding_moves_and_unbinds() {
    let keymap = Keymap::default();
    assert_eq!(keymap.label(Action::Machines).as_deref(), Some("prefix+m"));
    assert_eq!(
        keymap.resolve_prefix(ch('m')),
        PrefixKey::Run(Action::Machines)
    );
    assert_eq!(
        keymap.resolve_navigate(ch('m'), false),
        Some(NavigateKey::Run(Action::Machines))
    );

    let moved = self::keymap("machines = \"prefix+shift+m\"\n");
    assert_eq!(moved.resolve_prefix(ch('m')), PrefixKey::Drop);
    assert_eq!(
        moved.resolve_prefix(ch('M')),
        PrefixKey::Run(Action::Machines)
    );
    assert_eq!(
        moved.resolve_navigate(ch('M'), false),
        Some(NavigateKey::Run(Action::Machines))
    );
    assert_eq!(moved.resolve_navigate(ch('m'), false), None);

    let direct = self::keymap("machines = \"ctrl+alt+m\"\n");
    assert_eq!(
        direct.resolve_terminal(key(KeyCode::Char('m'), CTRL | ALT)),
        Some(TerminalKey::Run(Action::Machines))
    );

    let unbound = self::keymap("machines = \"\"\n");
    assert_eq!(unbound.label(Action::Machines), None);
    assert_eq!(unbound.resolve_prefix(ch('m')), PrefixKey::Drop);
    assert_eq!(unbound.resolve_navigate(ch('m'), false), None);
}
