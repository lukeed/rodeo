mod common;

use common::{Harness, run};
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::Color;

#[test]
fn opening_a_space_from_navigate_mode_leaves_no_selection() {
    run(|| async {
        let mut h = Harness::with_config("[ui]\nprompt_new_workspace_name = false\n").await;
        h.ready().await;
        h.prefix_char('N').await;
        h.until_stored("two spaces", |d| d.spaces.len() == 2).await;
        h.prefix_char('w').await;
        h.press(KeyCode::Up, KeyModifiers::NONE).await;
        assert!(h.app().snapshot().spaces.iter().any(|s| s.selected));
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        let spaces = h.app().snapshot().spaces;
        assert!(spaces[0].focused);
        assert!(spaces.iter().all(|s| !s.selected), "{spaces:?}");
    });
}

#[test]
fn selections_use_true_colors_like_herdr() {
    run(|| async {
        let h = Harness::start().await;
        let style = drover::render::selection_style(h.app());
        assert_eq!(style.fg, Some(Color::Rgb(255, 255, 255)));
        assert_eq!(style.bg, Some(Color::Rgb(89, 89, 98)));
    });
}

#[test]
fn a_smaller_host_keeps_the_prompt_without_scrollback() {
    run(|| async {
        let mut h = Harness::sized(120, 36, "").await;
        h.ready().await;
        let address = h.focused_address().expect("a pane");
        h.send(crossterm::event::Event::Resize(60, 30));
        h.pump().await;
        let bar = h
            .app()
            .panes
            .get(&address)
            .expect("the pane")
            .term
            .scrollbar();
        assert_eq!(bar.total, bar.len, "{bar:?}");
        let screen = h.screen();
        assert!(!screen.contains('▕'), "{screen}");
    });
}
