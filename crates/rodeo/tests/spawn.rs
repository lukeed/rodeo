mod common;

use std::time::Duration;

use common::{Harness, run};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
    Event::Key(KeyEvent::new(code, modifiers))
}

fn queue_prefix(h: &Harness, code: KeyCode, modifiers: KeyModifiers) {
    h.send(key(KeyCode::Char('b'), KeyModifiers::CONTROL));
    h.send(key(code, modifiers));
}

fn queue_text(h: &Harness, text: &str) {
    for ch in text.chars() {
        let event = match ch {
            '\r' => key(KeyCode::Enter, KeyModifiers::NONE),
            ch => key(KeyCode::Char(ch), KeyModifiers::NONE),
        };
        h.send(event);
    }
}

fn has_text(h: &Harness, address: &str, text: &str) -> bool {
    h.pane_text(address).contains(text)
}

async fn live_shells(h: &Harness) -> usize {
    h.outside
        .list(&hc::ListParams::default())
        .await
        .expect("terminal.list")
        .terminals
        .iter()
        .filter(|t| t.is_live())
        .count()
}

#[test]
fn keys_right_after_a_split_reach_the_new_pane() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let first = h.focused_address().expect("a pane");
        queue_prefix(&h, KeyCode::Char('v'), KeyModifiers::NONE);
        queue_text(&h, "echo one-$((1+1)); echo two-$((1+2))\r");
        h.pump().await;
        let second = h.focused().expect("a focused pane");
        h.until("the new pane's shell", |h| {
            h.focused_address()
                .is_some_and(|a| a != first && has_text(h, &a, "two-3"))
        })
        .await;
        assert_eq!(h.focused().as_deref(), Some(second.as_str()));
        let address = h.focused_address().expect("an address");
        let text = h.pane_text(&address);
        let one = text.find("one-2").expect("the first output");
        let two = text.find("two-3").expect("the second output");
        assert!(one < two, "{text}");
        assert!(!has_text(&h, &first, "one-2"), "{}", h.pane_text(&first));
        h.until_stored("two panes", |d| d.addresses().len() == 2)
            .await;
    });
}

#[test]
fn keys_right_after_a_new_tab_reach_its_shell() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let first = h.focused_address().expect("a pane");
        h.prefix_char('c').await;
        h.until("the new tab dialog", |h| h.screen().contains("new tab"))
            .await;
        queue_text(&h, "logs\recho tab-$((2+2))\r");
        h.pump().await;
        h.until("the new tab's shell", |h| {
            h.focused_address()
                .is_some_and(|a| a != first && has_text(h, &a, "tab-4"))
        })
        .await;
        assert!(!has_text(&h, &first, "tab-4"));
        let doc = h
            .until_stored("two tabs", |d| d.spaces[0].tabs.len() == 2)
            .await;
        assert_eq!(doc.spaces[0].tabs[1].name, "logs");
    });
}

#[test]
fn keys_right_after_a_new_workspace_reach_its_shell() {
    run(|| async {
        let mut h = Harness::with_config("[ui]\nprompt_new_workspace_name = false\n").await;
        h.ready().await;
        let first = h.focused_address().expect("a pane");
        queue_prefix(&h, KeyCode::Char('N'), KeyModifiers::SHIFT);
        queue_text(&h, "echo space-$((3+3))\r");
        h.pump().await;
        h.until("the new workspace's shell", |h| {
            h.focused_address()
                .is_some_and(|a| a != first && has_text(h, &a, "space-6"))
        })
        .await;
        assert!(!has_text(&h, &first, "space-6"));
        h.until_stored("two spaces", |d| d.spaces.len() == 2).await;
    });
}

#[test]
fn a_pane_closed_before_its_shell_starts_leaves_no_shell() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        queue_prefix(&h, KeyCode::Char('v'), KeyModifiers::NONE);
        queue_prefix(&h, KeyCode::Char('x'), KeyModifiers::NONE);
        h.pump().await;
        assert_eq!(h.tab_panes().len(), 1);
        h.until("the new shell ended", |h| {
            h.app().machines.local().slot.is_settled()
        })
        .await;
        let started = std::time::Instant::now();
        while started.elapsed() < Duration::from_secs(3) {
            h.pump().await;
        }
        let doc = h
            .until_stored("one pane", |d| d.addresses().len() == 1)
            .await;
        assert_eq!(doc.addresses().len(), 1);
        assert_eq!(live_shells(&h).await, 1);
    });
}
