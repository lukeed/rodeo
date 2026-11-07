use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use hc::TestCore;

const WITHIN: Duration = Duration::from_secs(20);
const COLS: u16 = 120;
const ROWS: u16 = 40;

struct Tmux {
    socket: String,
    path: Option<std::path::PathBuf>,
}

impl Tmux {
    fn start(core: &TestCore) -> Self {
        let root = core
            .root()
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let socket = format!("drover-e2e-{}-{root}", std::process::id());
        let mut command = vec!["env".to_owned(), "-i".to_owned()];
        for (key, value) in core.env().iter() {
            command.push(format!(
                "{}={}",
                key.to_string_lossy(),
                value.to_string_lossy()
            ));
        }
        command.push("TERM=xterm-256color".to_owned());
        command.push(env!("CARGO_BIN_EXE_drover").to_owned());
        let line = command
            .iter()
            .map(|part| format!("'{}'", part.replace('\'', r"'\''")))
            .collect::<Vec<_>>()
            .join(" ");
        let mut tmux = Self { socket, path: None };
        let started = tmux.run(&[
            "-f",
            "/dev/null",
            "new-session",
            "-d",
            "-s",
            "e2e",
            "-x",
            &COLS.to_string(),
            "-y",
            &ROWS.to_string(),
            &line,
        ]);
        assert!(
            started.status.success(),
            "tmux new-session: {}",
            String::from_utf8_lossy(&started.stderr)
        );
        tmux.run(&["set-option", "-t", "e2e", "remain-on-exit", "on"]);
        let path = tmux.run(&["display-message", "-p", "#{socket_path}"]);
        tmux.path = Some(String::from_utf8_lossy(&path.stdout).trim().into())
            .filter(|p: &std::path::PathBuf| !p.as_os_str().is_empty());
        tmux
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new("tmux")
            .arg("-L")
            .arg(&self.socket)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .expect("tmux runs")
    }

    fn capture(&self) -> String {
        let out = self.run(&["capture-pane", "-p", "-e", "-t", "e2e"]);
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn text(&self) -> String {
        let out = self.run(&["capture-pane", "-p", "-t", "e2e"]);
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn keys(&self, keys: &[&str]) {
        let mut args = vec!["send-keys", "-t", "e2e"];
        args.extend_from_slice(keys);
        let out = self.run(&args);
        assert!(
            out.status.success(),
            "send-keys: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn literal(&self, text: &str) {
        let out = self.run(&["send-keys", "-t", "e2e", "-l", text]);
        assert!(out.status.success());
    }

    fn dead(&self) -> bool {
        let out = self.run(&["display-message", "-p", "-t", "e2e", "#{pane_dead}"]);
        String::from_utf8_lossy(&out.stdout).trim() == "1"
    }

    async fn until(&self, what: &str, done: impl Fn(&Self) -> bool) {
        let started = Instant::now();
        while !done(self) {
            assert!(
                started.elapsed() < WITHIN,
                "timed out waiting for {what}\n{}",
                self.text()
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

impl Drop for Tmux {
    fn drop(&mut self) {
        let _ = self.run(&["kill-server"]);
        if let Some(path) = &self.path {
            let _ = std::fs::remove_file(path);
        }
    }
}

struct Fixture {
    tmux: Tmux,
    core: TestCore,
}

impl Fixture {
    async fn start() -> Self {
        let core = TestCore::start().await;
        let tmux = Tmux::start(&core);
        Self { tmux, core }
    }
}

async fn live_terminals(client: &hc::Client) -> Vec<hc::Terminal> {
    client
        .list(&hc::ListParams::default())
        .await
        .expect("terminal.list")
        .terminals
        .into_iter()
        .filter(hc::Terminal::is_live)
        .collect()
}

#[tokio::test]
async fn drover_starts_splits_types_and_detaches() {
    let fixture = Fixture::start().await;
    let tmux = &fixture.tmux;
    let client = fixture.core.client().await;
    tmux.until("the sidebar and a prompt", |t| {
        let text = t.text();
        text.contains(" spaces") && text.contains(" agents") && text.contains('$')
    })
    .await;
    let screen = tmux.capture();
    assert!(screen.contains("\x1b["), "the capture carries colors");
    tmux.literal("echo e2e-$((6*7))");
    tmux.keys(&["Enter"]);
    tmux.until("the echo output", |t| t.text().contains("e2e-42"))
        .await;
    tmux.keys(&["C-b", "v"]);
    tmux.until("two framed panes", |t| t.text().matches('┌').count() == 2)
        .await;
    tmux.keys(&["C-b", "-"]);
    tmux.until("three framed panes", |t| t.text().matches('┌').count() == 3)
        .await;
    let terminals = live_terminals(&client).await;
    assert_eq!(terminals.len(), 3);
    assert!(terminals.iter().all(|t| t.keep));
    tmux.keys(&["C-b", "q"]);
    tmux.until("drover to exit", Tmux::dead).await;
    let terminals = live_terminals(&client).await;
    assert_eq!(terminals.len(), 3, "terminals outlive drover");
    let stored = client.doc_get(doc::NAME, None).await.expect("doc.get");
    let doc = doc::Document::parse(&stored.value).expect("a doc");
    assert_eq!(doc.addresses().len(), 3);
}

#[tokio::test]
async fn drover_shows_the_same_layout_after_a_restart() {
    let fixture = Fixture::start().await;
    let tmux = &fixture.tmux;
    tmux.until("a prompt", |t| t.text().contains('$')).await;
    tmux.keys(&["C-b", "v"]);
    tmux.until("two framed panes", |t| t.text().matches('┌').count() == 2)
        .await;
    tmux.keys(&["C-b", "q"]);
    tmux.until("drover to exit", Tmux::dead).await;
    tmux.run(&["respawn-pane", "-t", "e2e"]);
    tmux.until("the same two panes", |t| {
        !t.dead() && t.text().matches('┌').count() == 2 && t.text().contains('$')
    })
    .await;
}
