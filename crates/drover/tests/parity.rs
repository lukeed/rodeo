mod common;

use std::time::{Duration, Instant};

use common::{Harness, Sink, run};
use crossterm::event::{KeyCode, KeyModifiers};
use drover::app::{HostInfo, Options};
use drover::runtime::Runner;
use ratatui::backend::TestBackend;
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
fn the_mobile_layout_shows_every_pane_of_the_tab() {
    run(|| async {
        let mut h = Harness::sized(60, 20, "").await;
        h.ready().await;
        h.prefix_char('v').await;
        h.until("two panes side by side", |h| {
            h.screen();
            h.app().hits.panes.len() == 2
        })
        .await;
        assert!(h.app().regions.mobile);
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

#[test]
fn a_restart_keeps_the_focused_pane_and_zoom() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        h.prefix_char('v').await;
        h.until_stored("two panes", |d| d.addresses().len() == 2)
            .await;
        let focused = h.focused().expect("the new pane");
        h.prefix_char('z').await;
        h.app_mut().shutdown();
        let options = Options {
            env: h.core.env().clone(),
            config_path: h.config_path.clone(),
            prefs_path: h
                .core
                .root()
                .join("state")
                .join("drover")
                .join("prefs.json"),
            loaded: config::Config::load(&h.config_path),
            host: HostInfo {
                cell_px: (8, 16),
                ..HostInfo::default()
            },
            caller: None,
            notes: Vec::new(),
            hostname: "testhost".into(),
        };
        let mut again = Runner::new(
            options,
            TestBackend::new(120, 40),
            Box::new(Sink::default()),
        )
        .expect("a runner");
        let started = Instant::now();
        while !again.app.machines.local().slot.is_ready() {
            assert!(started.elapsed() < common::WITHIN, "the doc never loaded");
            let _ = tokio::time::timeout(Duration::from_millis(100), again.step()).await;
        }
        let app = &again.app;
        assert_eq!(app.view.focused_pane(&app.machines), Some(focused));
        let tab = app.view.active_tab(&app.machines).expect("a tab");
        assert!(app.view.is_zoomed(&tab));
    });
}

#[test]
fn two_escapes_in_one_read_leave_copy_mode() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        h.prefix_char('[').await;
        h.press(KeyCode::Char('v'), KeyModifiers::NONE).await;
        assert_eq!(h.app().view.mode, drover::view::Mode::Copy);
        h.press(KeyCode::Esc, KeyModifiers::ALT).await;
        assert_eq!(h.app().view.mode, drover::view::Mode::Terminal);
    });
}
