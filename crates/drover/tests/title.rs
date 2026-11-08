mod common;

use common::{Harness, run};

fn titles(h: &Harness) -> Vec<String> {
    h.host_output()
        .split("\x1b]0;")
        .skip(1)
        .filter_map(|rest| rest.split_once('\x07').map(|(title, _)| title.to_owned()))
        .collect()
}

#[test]
fn the_window_title_follows_the_focused_terminal() {
    run(|| async {
        let mut h = Harness::with_config(
            "[ui]\nwindow_title = \"{hostname}: {workspace} / {tab} / {terminal_title}{{x}}\"\n",
        )
        .await;
        h.ready().await;
        h.until("a first title", |h| {
            h.screen();
            titles(h)
                .last()
                .is_some_and(|t| t.starts_with("testhost: ") && t.contains(" / 1 / "))
        })
        .await;
        h.typed("printf '\\033]2;\\342\\240\\213 building\\007'\r")
            .await;
        h.until("the terminal title", |h| {
            h.screen();
            titles(h)
                .last()
                .is_some_and(|t| t.ends_with(" / 1 / building{x}"))
        })
        .await;
        let count = titles(&h).len();
        h.settle().await;
        h.screen();
        assert_eq!(
            titles(&h).len(),
            count,
            "unchanged titles are not written again"
        );
    });
}

#[test]
fn an_empty_window_title_leaves_the_host_alone_and_bells_pass() {
    run(|| async {
        let mut h = Harness::with_config("[ui]\nwindow_title = \"\"\n").await;
        h.ready().await;
        h.screen();
        assert!(titles(&h).is_empty());
        assert!(!h.host_output().contains('\x07'));
        h.typed("printf '\\007'\r").await;
        h.until("the bell", |h| h.host_output().contains('\x07'))
            .await;
        assert!(titles(&h).is_empty());
    });
}
