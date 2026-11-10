mod common;

use std::time::{Duration, Instant};

use common::{Harness, WITHIN, run};

async fn echoing() -> (Harness, String) {
    let mut h = Harness::start().await;
    h.ready().await;
    let address = h.focused_address().expect("a focused pane");
    h.typed("stty raw -echo; printf '\\122AW\\r\\n'; cat\r")
        .await;
    h.until("the tty is raw", |h| h.pane_text(&address).contains("RAW"))
        .await;
    send(&mut h, &address, b"\r\nready\r\n");
    steps_until(&mut h, "the marker is drawn", |h| {
        h.drawn().contains("ready")
    })
    .await;
    (h, address)
}

fn send(h: &mut Harness, address: &str, bytes: &[u8]) {
    h.app_mut()
        .panes
        .get_mut(address)
        .expect("the pane")
        .input(bytes);
}

fn synchronized(h: &Harness, address: &str) -> bool {
    h.app()
        .panes
        .get(address)
        .is_some_and(|p| p.term.synchronized_output())
}

async fn steps_until(h: &mut Harness, what: &str, done: impl Fn(&Harness) -> bool) {
    let started = Instant::now();
    while !done(h) {
        assert!(
            started.elapsed() < WITHIN,
            "timed out waiting for {what}\n{}",
            h.drawn()
        );
        let _ = tokio::time::timeout(Duration::from_millis(20), h.runner.step()).await;
    }
}

async fn steps_for(h: &mut Harness, span: Duration) {
    let started = Instant::now();
    while started.elapsed() < span {
        let _ = tokio::time::timeout(Duration::from_millis(20), h.runner.step()).await;
    }
}

#[test]
fn a_pane_inside_a_synchronized_update_keeps_its_last_frame() {
    run(|| async {
        let (mut h, address) = Box::pin(echoing()).await;
        send(&mut h, &address, b"\x1b[?2026h\x1b[1A\x1b[2K\rhalf");
        steps_until(&mut h, "the vt to take the half frame", |h| {
            h.pane_text(&address).contains("half") && synchronized(h, &address)
        })
        .await;
        steps_for(&mut h, Duration::from_millis(200)).await;
        let held = h.drawn();
        assert!(held.contains("ready"), "{held}");
        assert!(!held.contains("half"), "{held}");

        send(&mut h, &address, b" done\x1b[?2026l");
        let sent = Instant::now();
        steps_until(&mut h, "the finished frame", |h| {
            h.drawn().contains("half done")
        })
        .await;
        assert!(sent.elapsed() < Duration::from_millis(500));
        let shown = h.drawn();
        assert!(!shown.contains("ready"), "{shown}");
    });
}

#[test]
fn a_synchronized_update_that_never_ends_shows_after_a_second() {
    run(|| async {
        let (mut h, address) = Box::pin(echoing()).await;
        send(&mut h, &address, b"\x1b[?2026hstuck");
        steps_until(&mut h, "the vt to take the stuck frame", |h| {
            h.pane_text(&address).contains("stuck") && synchronized(h, &address)
        })
        .await;
        let seen = Instant::now();
        steps_until(&mut h, "the stuck frame to show", |h| {
            h.drawn().contains("stuck")
        })
        .await;
        assert!(
            seen.elapsed() >= Duration::from_millis(700),
            "{:?}",
            seen.elapsed()
        );
        assert!(!synchronized(&h, &address));

        send(&mut h, &address, b" more");
        steps_until(&mut h, "later output", |h| h.drawn().contains("stuck more")).await;
    });
}
