mod common;

use common::{Harness, run};
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use doc::{Kind, Size};
use drover::view::Mode;

fn root_kind(doc: &doc::Document) -> Vec<String> {
    doc.spaces[0].tabs[0]
        .root
        .children()
        .iter()
        .map(|c| match &c.kind {
            Kind::Block { .. } => "block".to_owned(),
            Kind::Row { .. } => "row".to_owned(),
            Kind::Column { .. } => "column".to_owned(),
            Kind::Stack { .. } => "stack".to_owned(),
        })
        .collect()
}

#[test]
fn an_empty_doc_gets_a_space_with_a_running_shell() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let doc = h.until_stored("one space", |d| d.spaces.len() == 1).await;
        let space = &doc.spaces[0];
        assert!(doc::is_id(&space.id, doc::Prefix::Space));
        assert_eq!(space.name, "");
        assert!(space.folder.starts_with('/'));
        let tab = &space.tabs[0];
        assert_eq!(tab.root.id, tab.id);
        let address = tab.addresses()[0].clone();
        let terminal = h.outside.get(&address).await.expect("the shell");
        assert!(terminal.is_live());
        assert!(terminal.keep);
        assert_eq!(terminal.label("herdr.space"), Some(space.id.as_str()));
        assert_eq!(terminal.label("herdr.tab"), Some(tab.id.as_str()));
        assert_eq!(terminal.label("herdr.seen"), Some("0"));
        let screen = h.screen();
        assert!(screen.contains(" spaces"), "{screen}");
        assert!(screen.contains(" agents"), "{screen}");
    });
}

#[test]
fn typing_reaches_the_shell_and_output_renders() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        h.typed("echo hi-$((40+2))\r").await;
        h.until("the echo output", |h| h.screen().contains("hi-42"))
            .await;
        let address = h.focused_address().expect("a pane");
        h.until("the where label", |h| {
            h.app()
                .terminals
                .get(&address)
                .and_then(|t| t.label("where"))
                .is_some_and(|w| w.ends_with(" / 1"))
        })
        .await;
    });
}

#[test]
fn splits_move_focus_and_zoom_hides_the_others() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let first = h.focused().expect("a pane");
        h.prefix_char('v').await;
        h.until("a split to the right", |h| h.tab_panes().len() == 2)
            .await;
        h.until("focus on the new pane", |h| {
            h.focused().as_deref() != Some(first.as_str())
        })
        .await;
        let doc = h
            .until_stored("two panes", |d| d.addresses().len() == 2)
            .await;
        assert_eq!(root_kind(&doc), ["block", "block"]);
        h.prefix(KeyCode::Char('-'), KeyModifiers::NONE).await;
        h.until("a split down", |h| h.tab_panes().len() == 3).await;
        let doc = h
            .until_stored("three panes", |d| d.addresses().len() == 3)
            .await;
        assert_eq!(root_kind(&doc), ["block", "column"]);
        h.prefix_char('h').await;
        h.until("focus back on the left", |h| {
            h.focused().as_deref() == Some(first.as_str())
        })
        .await;
        h.prefix_char('z').await;
        let tab = h.app().view.active_tab(&h.app().machines).expect("a tab");
        assert!(h.app().view.is_zoomed(&tab));
        h.until("the zoom marker", |h| h.screen().contains("1 Z"))
            .await;
        assert_eq!(h.app().hits.panes.len(), 1);
        h.prefix_char('z').await;
        h.until("all panes back", |h| h.app().hits.panes.len() == 3)
            .await;
        h.prefix(KeyCode::Tab, KeyModifiers::NONE).await;
        assert_ne!(h.focused().as_deref(), Some(first.as_str()));
        h.prefix(KeyCode::BackTab, KeyModifiers::NONE).await;
        assert_eq!(h.focused().as_deref(), Some(first.as_str()));
    });
}

#[test]
fn closing_panes_tabs_and_workspaces() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        h.prefix_char('v').await;
        h.until("two panes", |h| h.tab_panes().len() == 2).await;
        let doomed = h.focused_address().expect("the new pane");
        h.prefix_char('x').await;
        h.until("one pane left", |h| h.tab_panes().len() == 1).await;
        h.until_stored("one pane stored", |d| d.addresses().len() == 1)
            .await;
        let mut killed = false;
        for _ in 0..50 {
            let t = h.outside.get(&doomed).await.expect("the record");
            if t.is_finished() {
                killed = true;
                break;
            }
            h.pump().await;
        }
        assert!(killed, "closing a pane kills its terminal");
        h.prefix_char('x').await;
        assert!(h.app().view.overlay.is_some(), "the last pane asks first");
        h.until("the confirm dialog", |h| {
            h.screen().contains("Close workspace?")
        })
        .await;
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        assert!(h.app().view.overlay.is_none());
        assert_eq!(h.app().view.mode, Mode::Navigate);
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        h.prefix_char('D').await;
        h.until("the confirm dialog", |h| {
            h.screen().contains("Close workspace?")
        })
        .await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        h.until_stored("no spaces", |d| d.spaces.is_empty()).await;
        assert_eq!(h.app().view.mode, Mode::Navigate);
    });
}

#[test]
fn closing_without_confirm_and_closing_tabs() {
    run(|| async {
        let mut h =
            Harness::with_config("[ui]\nconfirm_close = false\nprompt_new_tab_name = false\n")
                .await;
        h.ready().await;
        h.prefix_char('c').await;
        h.until_stored("two tabs", |d| d.spaces[0].tabs.len() == 2)
            .await;
        h.until("the second tab shown", |h| h.screen().contains("   2    "))
            .await;
        h.prefix_char('X').await;
        h.until_stored("one tab", |d| d.spaces[0].tabs.len() == 1)
            .await;
        h.prefix_char('x').await;
        h.until_stored("no spaces", |d| d.spaces.is_empty()).await;
        assert!(h.app().view.overlay.is_none());
    });
}

#[test]
fn new_tabs_switch_and_rename() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        h.prefix_char('c').await;
        h.until("the new tab dialog", |h| h.screen().contains("new tab"))
            .await;
        h.typed("logs\r").await;
        let doc = h
            .until_stored("a named tab", |d| d.spaces[0].tabs.len() == 2)
            .await;
        assert_eq!(doc.spaces[0].tabs[1].name, "logs");
        h.until("the tab label", |h| h.screen().contains("logs"))
            .await;
        h.until("the new tab attached", |h| {
            h.focused_address()
                .and_then(|a| {
                    h.app()
                        .panes
                        .get(&a)
                        .map(|p| p.link == drover::panes::Link::Attached)
                })
                .unwrap_or(false)
        })
        .await;
        h.prefix_char('1').await;
        let first_tab = doc.spaces[0].tabs[0].id.clone();
        assert_eq!(
            h.app().view.active_tab(&h.app().machines).as_deref(),
            Some(first_tab.as_str())
        );
        h.prefix_char('n').await;
        assert_ne!(
            h.app().view.active_tab(&h.app().machines).as_deref(),
            Some(first_tab.as_str())
        );
        h.prefix_char('T').await;
        h.until("the rename tab dialog", |h| {
            h.screen().contains("rename tab")
        })
        .await;
        h.press(KeyCode::Char('u'), KeyModifiers::CONTROL).await;
        h.typed("build\r").await;
        let doc = h
            .until_stored("a renamed tab", |d| d.spaces[0].tabs[1].name == "build")
            .await;
        assert_eq!(doc.spaces[0].tabs[0].name, "");
        h.prefix_char('W').await;
        h.until("the rename workspace dialog", |h| {
            h.screen().contains("rename workspace")
        })
        .await;
        h.press(KeyCode::Char('u'), KeyModifiers::CONTROL).await;
        h.typed("work\r").await;
        h.until_stored("a renamed space", |d| d.spaces[0].name == "work")
            .await;
        h.until("the space name in the sidebar", |h| {
            h.screen().contains("work")
        })
        .await;
        h.prefix_char('P').await;
        h.until("the rename pane dialog", |h| {
            h.screen().contains("rename pane")
        })
        .await;
        h.typed("server\r").await;
        let doc = h
            .until_stored("a pane title", |d| {
                d.spaces[0].tabs[1]
                    .root
                    .panes()
                    .first()
                    .and_then(|p| p.title.clone())
                    .as_deref()
                    == Some("server")
            })
            .await;
        assert_eq!(doc.spaces[0].tabs[1].panes().len(), 1);
    });
}

#[test]
fn swap_and_resize_write_the_doc() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        h.prefix_char('v').await;
        h.until_stored("two panes", |d| d.addresses().len() == 2)
            .await;
        let before = h.doc().spaces[0].tabs[0].addresses();
        h.prefix_char('H').await;
        let after = h
            .until_stored("swapped panes", |d| {
                d.spaces[0].tabs[0].addresses() != before
            })
            .await;
        assert_eq!(
            after.spaces[0].tabs[0].addresses(),
            [before[1].clone(), before[0].clone()]
        );
        h.prefix_char('r').await;
        assert_eq!(h.app().view.mode, Mode::Resize);
        h.until("the resize bar", |h| h.screen().contains(" RESIZE "))
            .await;
        h.press(KeyCode::Char('l'), KeyModifiers::NONE).await;
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        assert_eq!(h.app().view.mode, Mode::Terminal);
        let doc = h
            .until_stored("weights", |d| {
                d.spaces[0].tabs[0]
                    .root
                    .children()
                    .iter()
                    .any(|c| c.size.is_some())
            })
            .await;
        let sizes: Vec<_> = doc.spaces[0].tabs[0]
            .root
            .children()
            .iter()
            .map(|c| c.size)
            .collect();
        let Some(Size::Weight { weight: left }) = sizes[0] else {
            panic!("a weight: {sizes:?}");
        };
        assert!(left > 0.5, "the left pane grew: {sizes:?}");
    });
}

#[test]
fn dragging_a_border_writes_weights_on_release() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        h.prefix_char('v').await;
        h.until_stored("two panes", |d| d.addresses().len() == 2)
            .await;
        h.until("a split hit", |h| !h.app().hits.splits.is_empty())
            .await;
        let split = h.app().hits.splits[0].clone();
        let (x, y) = (split.pos, split.hit.y + 5);
        let version = h.stored().await.1;
        h.mouse(MouseEventKind::Down(MouseButton::Left), x, y).await;
        h.mouse(MouseEventKind::Drag(MouseButton::Left), x - 10, y)
            .await;
        h.settle().await;
        assert_eq!(h.stored().await.1, version, "no write while dragging");
        h.screen();
        let left = h.app().hits.panes[0].rect.width;
        assert!(left < split.pos, "the drag previews live: {left}");
        h.mouse(MouseEventKind::Up(MouseButton::Left), x - 10, y)
            .await;
        let doc = h
            .until_stored("weights", |d| {
                d.spaces[0].tabs[0]
                    .root
                    .children()
                    .iter()
                    .any(|c| c.size.is_some())
            })
            .await;
        let Some(Size::Weight { weight }) = doc.spaces[0].tabs[0].root.children()[0].size else {
            panic!("a weight");
        };
        assert!(weight < 1.0, "the left pane shrank: {weight}");
    });
}

#[test]
fn the_sidebar_toggles_and_navigate_mode_picks_a_space() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        h.prefix_char('b').await;
        assert_eq!(h.app().prefs.sidebar_collapsed, Some(true));
        h.until("the compact sidebar", |h| {
            h.app().regions.sidebar.width == 4
        })
        .await;
        h.until("the compact rail", |h| h.screen().contains("»"))
            .await;
        h.prefix_char('b').await;
        h.until("the full sidebar", |h| h.app().regions.sidebar.width == 26)
            .await;
        h.prefix_char('N').await;
        h.until_stored("two spaces", |d| d.spaces.len() == 2).await;
        h.until("the second space focused", |h| {
            h.app().view.space.as_deref() == Some(h.doc().spaces[1].id.as_str())
        })
        .await;
        h.prefix_char('w').await;
        assert_eq!(h.app().view.mode, Mode::Navigate);
        h.until("the navigate bar", |h| h.screen().contains(" NAVIGATE "))
            .await;
        h.press(KeyCode::Up, KeyModifiers::NONE).await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        assert_eq!(h.app().view.mode, Mode::Terminal);
        let first = h.doc().spaces[0].id.clone();
        assert_eq!(h.app().view.space.as_deref(), Some(first.as_str()));
        h.prefix_char('w').await;
        h.press(KeyCode::Char('2'), KeyModifiers::NONE).await;
        let second = h.doc().spaces[1].id.clone();
        assert_eq!(h.app().view.space.as_deref(), Some(second.as_str()));
        assert_eq!(h.app().view.mode, Mode::Terminal);
    });
}

#[test]
fn prefix_twice_sends_the_prefix() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        h.typed("od -c\r").await;
        h.settle().await;
        h.press(KeyCode::Char('b'), KeyModifiers::CONTROL).await;
        assert_eq!(h.app().view.mode, Mode::Prefix);
        h.until("the prefix bar", |h| h.screen().contains(" PREFIX "))
            .await;
        h.press(KeyCode::Char('b'), KeyModifiers::CONTROL).await;
        assert_eq!(h.app().view.mode, Mode::Terminal);
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        h.press(KeyCode::Char('d'), KeyModifiers::CONTROL).await;
        h.until("od shows ctrl-b", |h| h.screen().contains("002"))
            .await;
    });
}

#[test]
fn copy_mode_selects_and_copies() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        h.typed("echo copy-me-please\r").await;
        h.until("the output", |h| {
            h.screen().matches("copy-me-please").count() >= 2
        })
        .await;
        h.prefix(KeyCode::Char('['), KeyModifiers::NONE).await;
        assert_eq!(h.app().view.mode, Mode::Copy);
        h.until("the copy bar", |h| h.screen().contains(" COPY "))
            .await;
        h.press(KeyCode::Char('?'), KeyModifiers::NONE).await;
        h.until("the search prompt", |h| h.screen().contains("enter search"))
            .await;
        h.typed("copy-me").await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        h.press(KeyCode::Char('v'), KeyModifiers::NONE).await;
        h.press(KeyCode::Char('E'), KeyModifiers::SHIFT).await;
        h.until("the selecting bar", |h| h.screen().contains("selecting"))
            .await;
        h.press(KeyCode::Char('y'), KeyModifiers::NONE).await;
        assert_eq!(h.app().view.mode, Mode::Terminal);
        h.until("the copied toast", |h| {
            h.screen().contains("copied to clipboard")
        })
        .await;
        let expected = base64_of("copy-me-please");
        assert!(
            h.host_output()
                .contains(&format!("\x1b]52;c;{expected}\x07")),
            "{:?}",
            h.host_output()
        );
    });
}

fn base64_of(text: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(text)
}

#[test]
fn outside_doc_changes_show_up() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let (mut doc, version) = h.stored().await;
        doc.spaces[0].name = "from-outside".into();
        h.outside
            .doc_set(doc::NAME, &doc.to_value(), version, None)
            .await
            .expect("an outside write");
        h.until("the outside name", |h| h.screen().contains("from-outside"))
            .await;
    });
}

#[test]
fn a_conflict_keeps_both_edits() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        h.until_stored("the first space", |d| d.spaces.len() == 1)
            .await;
        h.prefix_char('W').await;
        h.until("the rename dialog", |h| {
            h.screen().contains("rename workspace")
        })
        .await;
        h.press(KeyCode::Char('u'), KeyModifiers::CONTROL).await;
        h.typed("mine").await;
        h.send(crossterm::event::Event::Key(
            crossterm::event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        ));
        let (mut doc, version) = h.stored().await;
        doc.spaces[0].tabs[0].name = "theirs".into();
        h.outside
            .doc_set(doc::NAME, &doc.to_value(), version, None)
            .await
            .expect("an outside write");
        let merged = h
            .until_stored("both edits", |d| {
                d.spaces[0].name == "mine" && d.spaces[0].tabs[0].name == "theirs"
            })
            .await;
        assert_eq!(merged.spaces.len(), 1);
        assert!(
            h.app().machines.local().slot.conflicts() >= 1,
            "the write met a conflict"
        );
    });
}

#[test]
fn outside_labelled_terminals_are_adopted() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let doc = h
            .until_stored("the first space", |d| d.spaces.len() == 1)
            .await;
        let (space, tab) = (doc.spaces[0].id.clone(), doc.spaces[0].tabs[0].id.clone());
        let terminal = h
            .outside
            .run(&hc::RunParams {
                line: Some("echo adopted; sleep 60".into()),
                labels: [
                    ("herdr.space".to_owned(), space),
                    ("herdr.tab".to_owned(), tab),
                ]
                .into(),
                ..hc::RunParams::default()
            })
            .await
            .expect("an outside terminal");
        let doc = h
            .until_stored("the adopted pane", |d| {
                d.addresses().contains(&terminal.address)
            })
            .await;
        assert_eq!(doc.spaces[0].tabs[0].addresses().len(), 2);
        let record = h.outside.get(&terminal.address).await.expect("the record");
        assert!(record.keep, "adoption turns keep on");
        h.until("the adopted output", |h| h.screen().contains("adopted"))
            .await;
        h.outside.kill(&terminal.address).await.expect("kill");
    });
}

#[test]
fn an_exiting_shell_removes_its_pane() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        h.prefix_char('v').await;
        h.until_stored("two panes", |d| d.addresses().len() == 2)
            .await;
        h.until("the new pane attached", |h| {
            h.focused_address()
                .and_then(|a| {
                    h.app()
                        .panes
                        .get(&a)
                        .map(|p| p.link == drover::panes::Link::Attached)
                })
                .unwrap_or(false)
        })
        .await;
        let leaving = h.focused_address().expect("the new pane");
        h.typed("exit\r").await;
        let doc = h
            .until_stored("one pane", |d| !d.addresses().contains(&leaving))
            .await;
        assert_eq!(doc.addresses().len(), 1);
        h.until("one pane shown", |h| h.app().hits.panes.len() == 1)
            .await;
    });
}

#[test]
fn clicks_focus_panes_and_tabs() {
    run(|| async {
        let mut h = Harness::with_config("[ui]\nprompt_new_tab_name = false\n").await;
        h.ready().await;
        let first = h.focused().expect("a pane");
        h.prefix_char('v').await;
        h.until("two panes", |h| h.app().hits.panes.len() == 2)
            .await;
        h.until("focus moved", |h| {
            h.focused().as_deref() != Some(first.as_str())
        })
        .await;
        let left = h
            .app()
            .hits
            .panes
            .iter()
            .find(|p| p.key == first)
            .expect("the left pane")
            .clone();
        h.click(left.content.x + 2, left.content.y + 2).await;
        assert_eq!(h.focused().as_deref(), Some(first.as_str()));
        h.prefix_char('c').await;
        h.until("two tabs", |h| h.app().hits.tabs.tabs.len() == 2)
            .await;
        let (rect, key) = h.app().hits.tabs.tabs[0].clone();
        h.click(rect.x + 1, rect.y).await;
        assert_eq!(
            h.app().view.active_tab(&h.app().machines).as_deref(),
            Some(key.as_str())
        );
        let toggle = h.app().hits.sidebar.toggle.expect("the sidebar toggle");
        h.click(toggle.x, toggle.y).await;
        assert_eq!(h.app().prefs.sidebar_collapsed, Some(true));
    });
}

#[test]
fn agents_toast_and_get_marked_seen() {
    run(|| async {
        let mut h =
            Harness::with_config("[ui.toast]\ndelivery = \"herdr\"\ndelay_seconds = 0\n").await;
        h.ready().await;
        let doc = h
            .until_stored("the first space", |d| d.spaces.len() == 1)
            .await;
        let space = doc.spaces[0].id.clone();
        let tab = doc::fresh_id(doc::Prefix::Tab);
        let script = r"sleep 3; printf '\033]7501;state=working:app=claude-code\033\\'; sleep 0.5; printf '\033]7501;state=idle:app=claude-code\033\\'; sleep 60";
        let agent = h
            .outside
            .run(&hc::RunParams {
                line: Some(script.into()),
                labels: [
                    ("herdr.space".to_owned(), space),
                    ("herdr.tab".to_owned(), tab.clone()),
                    ("herdr.seen".to_owned(), "0".to_owned()),
                ]
                .into(),
                ..hc::RunParams::default()
            })
            .await
            .expect("an agent");
        h.until_stored("a tab for the agent", |d| d.spaces[0].tabs.len() == 2)
            .await;
        h.until("the finished toast", |h| {
            h.screen().contains("claude finished")
        })
        .await;
        let row = h
            .app()
            .snapshot()
            .agents
            .into_iter()
            .find(|a| a.address == agent.address);
        assert_eq!(row.map(|r| r.state), Some(drover::agents::State::Done));
        assert!(h.screen().contains("claude"), "the agents list shows it");
        h.prefix_char('o').await;
        assert_eq!(h.focused_address().as_deref(), Some(agent.address.as_str()));
        let mut seen = None;
        for _ in 0..100 {
            let record = h.outside.get(&agent.address).await.expect("the agent");
            if record.label("herdr.seen") == Some("1") {
                seen = Some(record);
                break;
            }
            h.pump().await;
        }
        assert!(seen.is_some(), "drover marks the turn seen");
        h.until("the agent idle", |h| {
            h.app().agent_state(&agent.address) == Some(drover::agents::State::Idle)
        })
        .await;
        h.outside.kill(&agent.address).await.expect("kill");
    });
}

#[test]
fn mod_blocks_show_a_folder_tree_or_a_placeholder() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let (mut doc, version) = h.stored().await;
        let first = doc.spaces[0].tabs[0].panes()[0].id.clone();
        doc.split_pane(
            &first,
            doc::Axis::Row,
            doc::Node::module("pfiles", "files", serde_json::Value::Null),
        )
        .expect("a files split");
        doc.split_pane(
            "pfiles",
            doc::Axis::Column,
            doc::Node::module("pchanges", "changes", serde_json::Value::Null),
        )
        .expect("a changes split");
        h.outside
            .doc_set(doc::NAME, &doc.to_value(), version, None)
            .await
            .expect("an outside write");
        h.until("the folder tree", |h| {
            let screen = h.screen();
            screen.contains("├─ config/") && screen.contains("└─ state/")
        })
        .await;
        h.until("the placeholder", |h| h.screen().contains("changes mod"))
            .await;
    });
}

#[test]
fn two_clients_share_one_layout_without_duplicates() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let mut other = h.second();
        let started = std::time::Instant::now();
        while other.app.view.active_tab(&other.app.machines).is_none() {
            assert!(
                started.elapsed() < common::WITHIN,
                "the second client loads the layout"
            );
            h.both(&mut other).await;
        }
        h.prefix_char('v').await;
        let held = std::time::Instant::now();
        while held.elapsed() < std::time::Duration::from_millis(2600) {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(50), other.step()).await;
        }
        h.prefix(KeyCode::Char('-'), KeyModifiers::NONE).await;
        let doc = h
            .until_stored("three panes", |d| d.addresses().len() == 3)
            .await;
        let started = std::time::Instant::now();
        loop {
            h.both(&mut other).await;
            let theirs = other.app.machines.local().slot.doc().addresses();
            if theirs.len() == 3 && other.app.machines.local().slot.is_settled() {
                break;
            }
            assert!(
                started.elapsed() < common::WITHIN,
                "the second client shows the splits: {theirs:?}"
            );
        }
        for _ in 0..30 {
            h.both(&mut other).await;
        }
        let (stored, _) = h.stored().await;
        let mut addresses = stored.addresses();
        addresses.sort();
        addresses.dedup();
        assert_eq!(
            addresses.len(),
            stored.addresses().len(),
            "no terminal shows twice: {stored:#?}"
        );
        assert_eq!(stored.addresses().len(), doc.addresses().len());
        assert_ne!(
            other.app.view.focused_pane(&other.app.machines),
            h.focused(),
            "each client keeps its own focus"
        );
    });
}

#[test]
fn dragging_a_tab_reorders_the_doc() {
    run(|| async {
        let mut h = Harness::with_config("[ui]\nprompt_new_tab_name = false\n").await;
        h.ready().await;
        h.prefix_char('c').await;
        h.until("two tabs", |h| h.app().hits.tabs.tabs.len() == 2)
            .await;
        let doc = h
            .until_stored("two tabs stored", |d| d.spaces[0].tabs.len() == 2)
            .await;
        let (first, second) = (
            doc.spaces[0].tabs[0].id.clone(),
            doc.spaces[0].tabs[1].id.clone(),
        );
        let (a, _) = h.app().hits.tabs.tabs[0];
        let (b, _) = h.app().hits.tabs.tabs[1];
        h.mouse(MouseEventKind::Down(MouseButton::Left), b.x + 1, b.y)
            .await;
        h.mouse(MouseEventKind::Drag(MouseButton::Left), a.x + 1, a.y)
            .await;
        h.until("the drop indicator", |h| {
            h.screen()
                .lines()
                .next()
                .is_some_and(|l| l.contains('│') && l.matches('│').count() >= 2)
        })
        .await;
        h.mouse(MouseEventKind::Up(MouseButton::Left), a.x + 1, a.y)
            .await;
        let doc = h
            .until_stored("reordered tabs", |d| d.spaces[0].tabs[0].id == second)
            .await;
        assert_eq!(doc.spaces[0].tabs[1].id, first);
    });
}
