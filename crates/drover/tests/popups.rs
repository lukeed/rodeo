mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use common::{Harness, run};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use drover::view::Mode;
use ratatui::style::Modifier;

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    std::fs::create_dir_all(dir).expect("a bin dir");
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("a script");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

fn with_bin(h: &mut Harness, bin: &Path) {
    let path = format!(
        "{}:{}",
        bin.display(),
        h.app()
            .env
            .get("PATH")
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
    );
    h.app_mut().env.set("PATH", path);
}

async fn until_file(
    h: &mut Harness,
    path: &Path,
    what: &str,
    done: impl Fn(&str) -> bool,
) -> String {
    h.until(what, |_| {
        std::fs::read_to_string(path).is_ok_and(|text| done(&text))
    })
    .await;
    std::fs::read_to_string(path).unwrap_or_default()
}

fn find(screen: &str, text: &str) -> Option<(u16, u16)> {
    screen.lines().enumerate().find_map(|(y, line)| {
        let at = line.find(text)?;
        let x = line[..at].chars().count();
        Some((u16::try_from(x).ok()?, u16::try_from(y).ok()?))
    })
}

#[test]
fn popup_commands_take_every_key_until_they_exit() {
    run(|| async {
        let config = r#"
[[keys.command]]
key = "prefix+y"
type = "popup"
width = 40
height = 10
command = "printf 'in-popup\n'; read x; echo \"got-$x\" > popup.txt"
"#;
        let mut h = Harness::with_config(config).await;
        h.ready().await;
        h.prefix_char('y').await;
        h.until("the popup", |h| {
            let screen = h.screen();
            screen.contains("in-popup") && screen.contains("┌popup")
        })
        .await;
        let address = h.app().paste_target().expect("the popup terminal");
        assert_ne!(Some(address.clone()), h.focused_address());
        let record = h.outside.get(&address).await.expect("the popup record");
        assert_eq!(record.label(drover::popup::LABEL), Some("1"));
        assert!(record.label("herdr.space").is_none());
        assert_eq!((record.cols, record.rows), (37, 8));
        h.press(KeyCode::Char('b'), KeyModifiers::CONTROL).await;
        assert_eq!(h.app().view.mode, Mode::Terminal);
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        h.typed("abc\r").await;
        let out = h.core.root().join("popup.txt");
        let text = until_file(&mut h, &out, "the popup output", |t| t.contains("abc")).await;
        assert!(text.starts_with("got-"), "{text:?}");
        h.until("the popup to close", |h| h.app().popup.is_none())
            .await;
        assert!(!h.screen().contains("┌popup"));
        assert_eq!(h.doc().addresses().len(), 1);
        h.typed("echo back-$((1+1))\r").await;
        h.until("keys back in the pane", |h| h.screen().contains("back-2"))
            .await;
    });
}

#[test]
fn edit_scrollback_opens_the_editor_in_a_popup() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        h.typed("echo marker-$((6*7))\r").await;
        h.until("the marker", |h| h.screen().contains("marker-42"))
            .await;
        let root = h.core.root().to_path_buf();
        let seen = root.join("seen.txt");
        let editor = script(
            &root.join("bin"),
            "ed",
            &format!(
                "cat \"$1\" > {seen}; echo \"$1\" > {seen}.path",
                seen = seen.display()
            ),
        );
        h.app_mut().env.set("EDITOR", editor.display().to_string());
        h.prefix_char('e').await;
        let text = until_file(&mut h, &seen, "the editor input", |t| {
            t.contains("marker-42")
        })
        .await;
        assert!(text.contains("echo marker-$((6*7))"), "{text}");
        h.until("the popup to close", |h| h.app().popup.is_none())
            .await;
        let path = std::fs::read_to_string(root.join("seen.txt.path")).expect("the temp path");
        assert!(!Path::new(path.trim()).exists(), "the temp file is removed");
    });
}

#[test]
fn ctrl_v_pastes_a_clipboard_image_path() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let root = h.core.root().to_path_buf();
        let bin = root.join("bin");
        script(&bin, "wl-paste", r"printf '\211PNG\r\n\032\nfake'");
        with_bin(&mut h, &bin);
        h.app_mut().env.set("WAYLAND_DISPLAY", "wayland-test");
        h.app_mut().env.remove("DISPLAY");
        h.typed("read -r p; printf '%s' \"$p\" > got.txt\r").await;
        h.settle().await;
        h.press(KeyCode::Char('v'), KeyModifiers::CONTROL).await;
        let got = root.join("got.txt");
        h.until("the pasted path", |h| h.screen().contains("clipboard.png"))
            .await;
        h.typed("\r").await;
        let text = until_file(&mut h, &got, "the read path", |t| {
            t.contains("clipboard.png")
        })
        .await;
        assert!(text.starts_with('\'') && text.ends_with('\''), "{text:?}");
        let path = text.trim_matches('\'');
        let bytes = std::fs::read(path).expect("the put file");
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    });
}

#[test]
fn ctrl_v_without_an_image_reaches_the_pane() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let root = h.core.root().to_path_buf();
        let bin = root.join("bin");
        script(&bin, "wl-paste", "echo 'no image' >&2; exit 1");
        with_bin(&mut h, &bin);
        h.app_mut().env.set("WAYLAND_DISPLAY", "wayland-test");
        h.app_mut().env.remove("DISPLAY");
        h.typed("stty raw -echo; head -c1 | od -An -tx1 > key.txt; stty sane\r")
            .await;
        h.settle().await;
        h.press(KeyCode::Char('v'), KeyModifiers::CONTROL).await;
        let text = until_file(&mut h, &root.join("key.txt"), "the key byte", |t| {
            !t.trim().is_empty()
        })
        .await;
        assert_eq!(text.trim(), "16");
    });
}

#[test]
fn ctrl_hover_underlines_links_and_ctrl_click_opens_them() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let root = h.core.root().to_path_buf();
        let bin = root.join("bin");
        let opened = root.join("opened.txt");
        script(
            &bin,
            "xdg-open",
            &format!("echo \"$1\" > {}", opened.display()),
        );
        script(&bin, "open", &format!("echo \"$1\" > {}", opened.display()));
        with_bin(&mut h, &bin);
        h.typed("printf '\\033]8;;https://example.com/x\\033\\\\%s\\033]8;;\\033\\\\\\n' CLICK$((1))ME\r")
            .await;
        h.until("the link text", |h| h.screen().contains("CLICK1ME"))
            .await;
        let (x, y) = find(&h.screen(), "CLICK1ME").expect("the link on screen");
        let mouse = |kind, modifiers| {
            Event::Mouse(MouseEvent {
                kind,
                column: x + 2,
                row: y,
                modifiers,
            })
        };
        h.send(mouse(MouseEventKind::Moved, KeyModifiers::CONTROL));
        h.pump().await;
        h.screen();
        let underlined = |h: &Harness, col: u16| {
            h.runner.terminal.backend().buffer()[(col, y)]
                .modifier
                .contains(Modifier::UNDERLINED)
        };
        assert!(underlined(&h, x) && underlined(&h, x + 7));
        assert!(!underlined(&h, x + 8));
        h.send(mouse(MouseEventKind::Moved, KeyModifiers::NONE));
        h.pump().await;
        h.screen();
        assert!(!underlined(&h, x));
        h.send(mouse(
            MouseEventKind::Down(MouseButton::Left),
            KeyModifiers::CONTROL,
        ));
        h.send(mouse(
            MouseEventKind::Up(MouseButton::Left),
            KeyModifiers::CONTROL,
        ));
        h.pump().await;
        let text = until_file(&mut h, &opened, "the opened link", |t| !t.is_empty()).await;
        assert_eq!(text.trim(), "https://example.com/x");
        assert!(h.app().view.selection.is_none());
        h.send(Event::Key(KeyEvent::new(
            KeyCode::Char('x'),
            KeyModifiers::NONE,
        )));
        h.pump().await;
    });
}

const LONG_POPUP: &str = r#"
[[keys.command]]
key = "prefix+y"
type = "popup"
command = "printf 'long-popup\n'; sleep 600"
"#;

async fn ended(h: &Harness, address: &str) -> bool {
    h.outside
        .get(address)
        .await
        .map_or(true, |t| t.is_finished())
}

#[test]
fn quitting_ends_an_open_popup() {
    run(|| async {
        let mut h = Harness::with_config(LONG_POPUP).await;
        h.ready().await;
        h.prefix_char('y').await;
        h.until("the popup", |h| h.screen().contains("long-popup"))
            .await;
        let address = h.app().paste_target().expect("the popup terminal");
        let record = h.outside.get(&address).await.expect("the popup record");
        let owner = format!("testhost:{}", std::process::id());
        assert_eq!(record.label(drover::popup::OWNER), Some(owner.as_str()));
        let _ = h.runner.sender().send(drover::app::Msg::Quit);
        tokio::time::timeout(common::WITHIN, h.runner.run())
            .await
            .expect("drover quits");
        assert!(ended(&h, &address).await, "the popup outlived drover");
    });
}

async fn stray(h: &Harness, owner: &str) -> String {
    let params = hc::RunParams {
        argv: vec!["sleep".into(), "600".into()],
        labels: [
            (drover::popup::LABEL.to_owned(), "1".to_owned()),
            (drover::popup::OWNER.to_owned(), owner.to_owned()),
        ]
        .into(),
        rm: true,
        ..hc::RunParams::default()
    };
    h.outside.run(&params).await.expect("a stray popup").address
}

#[test]
fn a_new_client_ends_popups_whose_owner_is_gone() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let mut child = std::process::Command::new("true").spawn().expect("true");
        let gone = child.id();
        child.wait().expect("true exits");
        let dead = stray(&h, &format!("testhost:{gone}")).await;
        let live = stray(&h, &format!("testhost:{}", std::process::id())).await;
        let elsewhere = stray(&h, &format!("otherhost:{gone}")).await;
        let mut other = h.second();
        let started = std::time::Instant::now();
        while !ended(&h, &dead).await {
            assert!(
                started.elapsed() < common::WITHIN,
                "the orphan popup still runs"
            );
            h.both(&mut other).await;
        }
        for _ in 0..10 {
            h.both(&mut other).await;
        }
        assert!(!ended(&h, &live).await, "a live owner keeps its popup");
        assert!(!ended(&h, &elsewhere).await, "another host keeps its popup");
    });
}
