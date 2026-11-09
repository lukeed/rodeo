mod common;

use std::time::{Duration, Instant};

use common::{Harness, run};
use crossterm::event::{KeyCode, KeyModifiers};
use doc::Document;
use rodeo::view::Mode;

async fn until_killed(h: &mut Harness, address: &str) {
    let started = Instant::now();
    loop {
        let record = h.outside.get(address).await;
        if record.is_err() || record.is_ok_and(|t| t.is_finished()) {
            return;
        }
        assert!(started.elapsed() < common::WITHIN, "{address} still runs");
        h.pump().await;
    }
}

async fn assert_fresh(h: &mut Harness, before: &Document, version: u64) {
    let old = before.spaces[0].id.clone();
    let old_terminals = before.addresses();
    let doc = h
        .until_stored("a fresh workspace", |d| {
            d.spaces.len() == 1 && d.spaces[0].id != old && d.addresses().len() == 1
        })
        .await;
    let (_, stored) = h.stored().await;
    assert_eq!(stored, version + 1, "one write removed and added");
    let space = &doc.spaces[0];
    assert_eq!(space.folder, h.app().home());
    let address = doc.addresses()[0].clone();
    assert!(!old_terminals.contains(&address));
    let terminal = h.outside.get(&address).await.expect("the new shell");
    assert!(terminal.is_live() && terminal.keep);
    assert_eq!(terminal.label("herdr.space"), Some(space.id.as_str()));
    for gone in old_terminals {
        until_killed(h, &gone).await;
    }
    h.until("the new shell focused", |h| {
        h.focused_address().as_deref() == Some(address.as_str()) && h.screen().contains('$')
    })
    .await;
    assert_eq!(h.app().view.mode, Mode::Terminal);
}

#[test]
fn closing_the_last_workspace_starts_a_fresh_one() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let (before, version) = h.stored().await;
        h.prefix_char('D').await;
        h.until("the confirm dialog", |h| {
            h.screen().contains("Close workspace?")
        })
        .await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        assert_fresh(&mut h, &before, version).await;
    });
}

#[test]
fn closing_the_last_pane_or_tab_starts_a_fresh_workspace() {
    run(|| async {
        let mut h = Harness::with_config("[ui]\nconfirm_close = false\n").await;
        h.ready().await;
        let (before, version) = h.stored().await;
        h.prefix_char('x').await;
        assert_fresh(&mut h, &before, version).await;
        let (before, version) = h.stored().await;
        h.prefix_char('X').await;
        assert_fresh(&mut h, &before, version).await;
    });
}

#[test]
fn a_second_client_writes_nothing_when_the_last_workspace_is_replaced() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let mut other = h.second();
        let started = Instant::now();
        while other.app.view.active_tab(&other.app.machines).is_none() {
            assert!(
                started.elapsed() < common::WITHIN,
                "the second client loads"
            );
            h.both(&mut other).await;
        }
        let (before, version) = h.stored().await;
        h.app_mut().run_action(config::Action::CloseWorkspace);
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        let old = before.spaces[0].id.clone();
        let started = Instant::now();
        loop {
            h.both(&mut other).await;
            let theirs = other.app.machines.local().slot.doc();
            if theirs.spaces.len() == 1 && theirs.spaces[0].id != old {
                break;
            }
            assert!(
                started.elapsed() < common::WITHIN,
                "the second client follows"
            );
        }
        for _ in 0..40 {
            h.both(&mut other).await;
        }
        let (doc, stored) = h.stored().await;
        assert_eq!(doc.spaces.len(), 1, "{doc:#?}");
        assert_eq!(stored, version + 1, "only the closing client wrote");
    });
}

#[test]
fn a_doc_emptied_elsewhere_stays_empty_until_the_user_asks() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let (before, version) = h.stored().await;
        let emptied = h
            .outside
            .doc_set(doc::NAME, &serde_json::json!({"spaces": []}), version, None)
            .await
            .expect("doc.set");
        for address in before.addresses() {
            h.outside.kill(&address).await.expect("kill");
        }
        h.until("the empty state", |h| h.app().machines.spaces().is_empty())
            .await;
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(2500) {
            h.pump().await;
        }
        let (doc, stored) = h.stored().await;
        assert!(doc.spaces.is_empty(), "{doc:#?}");
        assert_eq!(stored, emptied, "rodeo wrote nothing");
        let screen = h.screen();
        assert!(screen.contains(" spaces"), "{screen}");
        assert!(screen.contains("NAVIGATE"), "{screen}");
        assert_eq!(h.app().view.mode, Mode::Navigate);
        h.press(KeyCode::Char('N'), KeyModifiers::SHIFT).await;
        h.until_stored("a workspace on request", |d| d.spaces.len() == 1)
            .await;
    });
}
