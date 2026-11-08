mod common;

use common::{Harness, run};
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use drover::overlay::Kind;
use drover::view::Mode;

fn kind(h: &Harness) -> Option<Kind> {
    h.app().view.overlay.as_ref().map(|o| o.kind())
}

#[test]
fn keybind_help_filters_scrolls_and_closes() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        h.prefix_char('?').await;
        h.until("the help", |h| h.screen().contains("keybinds"))
            .await;
        assert!(h.screen().contains("prefix+shift+r"));
        h.press(KeyCode::Char('j'), KeyModifiers::NONE).await;
        h.mouse(MouseEventKind::ScrollDown, 60, 20).await;
        assert!(!h.screen().contains("prefix mode"), "scrolled past the top");
        h.typed("/zoom").await;
        let screen = h.screen();
        assert!(screen.contains("zoom pane"), "{screen}");
        assert!(!screen.contains("split vertical"), "{screen}");
        assert!(screen.contains("esc back"), "{screen}");
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        assert!(h.screen().contains("prefix mode"));
        h.press(KeyCode::Char('?'), KeyModifiers::SHIFT).await;
        assert_eq!(kind(&h), None);
        h.prefix_char('?').await;
        h.click(1, 1).await;
        assert_eq!(kind(&h), None, "a click outside closes the help");
    });
}

#[test]
fn goto_lists_terminals_and_jumps_to_one() {
    run(|| async {
        let mut h = Harness::with_config("[ui]\nprompt_new_tab_name = false\n").await;
        h.ready().await;
        let first = h.focused().expect("a pane");
        h.prefix_char('c').await;
        h.until("a second tab", |h| {
            h.focused().is_some_and(|f| f != first) && h.tab_panes().len() == 1
        })
        .await;
        h.prefix_char('g').await;
        h.until("the picker", |h| h.screen().contains(" Go to "))
            .await;
        let screen = h.screen();
        assert!(screen.contains("2 terminals"), "{screen}");
        assert!(screen.contains("◆"), "{screen}");
        h.press(KeyCode::Char('w'), KeyModifiers::NONE).await;
        assert!(h.screen().contains("/ working"));
        assert!(h.screen().contains("No matching agents or terminals"));
        h.press(KeyCode::Char('a'), KeyModifiers::NONE).await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        assert_eq!(kind(&h), None);
        assert_eq!(h.focused(), Some(first));
    });
}

#[test]
fn pane_menu_splits_the_clicked_pane() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let pane = h.app().hits.panes[0].clone();
        let (x, y) = (pane.content.x + 5, pane.content.y + 5);
        h.mouse(MouseEventKind::Down(MouseButton::Right), x, y)
            .await;
        let screen = h.screen();
        assert!(screen.contains("Rename pane"), "{screen}");
        assert!(screen.contains("Send right-clicks to pane"), "{screen}");
        assert!(!screen.contains("Swap with focused pane"), "{screen}");
        h.press(KeyCode::Down, KeyModifiers::NONE).await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        h.until_stored("a split", |d| d.addresses().len() == 2)
            .await;
        assert_eq!(kind(&h), None);
    });
}

#[test]
fn tab_and_workspace_menus_rename() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        h.screen();
        let (rect, _) = h.app().hits.tabs.tabs[0].clone();
        h.mouse(MouseEventKind::Down(MouseButton::Right), rect.x + 1, rect.y)
            .await;
        let screen = h.screen();
        assert!(screen.contains("New tab"), "{screen}");
        h.mouse(MouseEventKind::Down(MouseButton::Left), 0, 39)
            .await;
        assert_eq!(kind(&h), None, "a click outside closes the menu");
        let rect = h.app().hits.sidebar.spaces[0].rect;
        h.mouse(MouseEventKind::Down(MouseButton::Right), rect.x + 1, rect.y)
            .await;
        assert!(h.screen().contains("Close"));
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        assert_eq!(kind(&h), Some(Kind::Rename));
        h.press(KeyCode::Char('c'), KeyModifiers::CONTROL).await;
        h.typed("api\r").await;
        let doc = h
            .until_stored("a renamed space", |d| d.spaces[0].name == "api")
            .await;
        assert_eq!(doc.spaces.len(), 1);
    });
}

#[test]
fn global_menu_opens_keybinds() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        h.screen();
        let launcher = h.app().hits.sidebar.menu.expect("the menu button");
        h.click(launcher.x + 1, launcher.y).await;
        assert_eq!(kind(&h), Some(Kind::GlobalMenu));
        let screen = h.screen();
        for item in ["settings", "keybinds", "reload config", "detach"] {
            assert!(screen.contains(item), "{screen}");
        }
        h.press(KeyCode::Char('j'), KeyModifiers::NONE).await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        assert_eq!(kind(&h), Some(Kind::Help));
    });
}

#[test]
fn settings_preview_themes_and_write_choices() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let original = h.app().palette;
        h.prefix_char('s').await;
        h.until("the settings", |h| h.screen().contains("catppuccin ✓"))
            .await;
        h.press(KeyCode::Down, KeyModifiers::NONE).await;
        assert_ne!(h.app().palette, original, "moving previews the theme");
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        assert_eq!(h.app().palette, original, "esc restores the theme");
        h.prefix_char('s').await;
        h.press(KeyCode::Tab, KeyModifiers::NONE).await;
        h.press(KeyCode::Tab, KeyModifiers::NONE).await;
        h.press(KeyCode::Char('j'), KeyModifiers::NONE).await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        assert!(!h.app().config.ui.sound.enabled);
        assert_eq!(kind(&h), Some(Kind::Settings), "only the theme closes");
        let text = std::fs::read_to_string(&h.config_path).expect("the config");
        assert!(text.contains("enabled = false"), "{text}");
        h.press(KeyCode::BackTab, KeyModifiers::SHIFT).await;
        h.press(KeyCode::BackTab, KeyModifiers::SHIFT).await;
        h.press(KeyCode::Char('j'), KeyModifiers::NONE).await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        assert_eq!(kind(&h), None);
        let text = std::fs::read_to_string(&h.config_path).expect("the config");
        assert!(text.contains("catppuccin-latte"), "{text}");
        assert_eq!(
            h.app().palette,
            config::builtin_palette("catppuccin-latte").expect("latte")
        );
    });
}

#[test]
fn mobile_switcher_opens_from_the_header() {
    run(|| async {
        let mut h = Harness::sized(60, 30, "").await;
        h.ready().await;
        h.screen();
        h.click(55, 1).await;
        assert_eq!(h.app().view.mode, Mode::Navigate);
        let screen = h.screen();
        for part in [
            " switch",
            "close",
            " spaces ",
            "+ new workspace",
            " tabs ",
            " menu ",
        ] {
            assert!(screen.contains(part), "{part}\n{screen}");
        }
        let row = screen
            .lines()
            .position(|l| l.contains("tab 1") && !l.contains("·"))
            .expect("the tab row");
        h.click(4, u16::try_from(row).expect("a row")).await;
        assert_eq!(h.app().view.mode, Mode::Terminal);
        h.click(55, 1).await;
        h.click(55, 0).await;
        assert_eq!(
            h.app().view.mode,
            Mode::Terminal,
            "close leaves the switcher"
        );
    });
}
