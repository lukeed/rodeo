mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::{Harness, run};
use crossterm::event::{KeyCode, KeyModifiers};

const SECOND: &str = "m2";

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.email=a@b", "-c", "user.name=a"])
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

fn hc(h: &Harness, session: &str, args: &[&str]) -> String {
    let out = h
        .core
        .hc()
        .env("HC_SESSION", session)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("hc runs");
    assert!(
        out.status.success(),
        "hc {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn via(h: &Harness) -> String {
    let mut line = vec!["env".to_owned(), "-i".to_owned()];
    for (key, value) in h.core.env().iter() {
        if key != "HC_SESSION" {
            line.push(format!(
                "{}={}",
                key.to_string_lossy(),
                value.to_string_lossy()
            ));
        }
    }
    line.push("sh".to_owned());
    line.push("-c".to_owned());
    line.join(" ")
}

struct Second {
    env: hc::Env,
    bin: std::path::PathBuf,
}

impl Second {
    fn new(h: &Harness) -> Self {
        Self {
            env: h.core.env().clone(),
            bin: h.core.bin().to_path_buf(),
        }
    }

    fn hc(&self, session: &str, args: &[&str]) {
        let _ = Command::new(&self.bin)
            .env_clear()
            .envs(self.env.iter())
            .env("HC_SESSION", session)
            .args(args)
            .stdin(Stdio::null())
            .output();
    }
}

impl Drop for Second {
    fn drop(&mut self) {
        self.hc("t", &["machine", "rm", "box"]);
        self.hc(SECOND, &["server", "stop"]);
    }
}

fn add_box(h: &Harness) -> Second {
    hc(h, SECOND, &["server", "start"]);
    let line = via(h);
    hc(
        h,
        "t",
        &[
            "machine",
            "add",
            "--via",
            &line,
            "--session",
            SECOND,
            "--name",
            "box",
        ],
    );
    Second::new(h)
}

fn machine_row(h: &mut Harness, name: Option<&str>) -> (u16, u16) {
    h.screen();
    let hit = h
        .app()
        .hits
        .sidebar
        .machines
        .iter()
        .find(|m| m.name.as_deref() == name)
        .map(|m| m.rect)
        .expect("a machine row");
    (hit.x + 4, hit.y)
}

fn spaces_on(h: &Harness, machine: Option<&str>) -> Vec<(String, usize)> {
    h.app()
        .machines
        .spaces()
        .into_iter()
        .filter(|s| s.machine == machine)
        .map(|s| (s.key(), s.space.tabs.len()))
        .collect()
}

#[test]
fn a_second_core_shows_as_a_machine() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let _second = add_box(&h);
        let agent = h
            .outside
            .run(&hc::RunParams {
                on: Some("box".into()),
                labels: [
                    ("herdr.space".to_owned(), "rs1".to_owned()),
                    ("herdr.tab".to_owned(), "rt1".to_owned()),
                ]
                .into(),
                ..hc::RunParams::default()
            })
            .await
            .expect("a terminal on box");
        h.until("the machines sidebar", |h| {
            let screen = h.screen();
            screen.contains(" rodeo▍") && screen.contains(" ▾ box")
        })
        .await;
        h.until("the remote space", |h| {
            h.screen();
            h.app()
                .hits
                .sidebar
                .spaces
                .iter()
                .any(|s| s.key == "box/rs1")
        })
        .await;
        let row = h
            .app()
            .hits
            .sidebar
            .spaces
            .iter()
            .find(|s| s.key == "box/rs1")
            .map(|s| s.rect)
            .expect("a row for the remote space");
        h.click(row.x + 3, row.y).await;
        assert_eq!(h.app().view.space.as_deref(), Some("box/rs1"));
        h.until("the footer names box", |h| {
            h.screen().contains(" new · box")
        })
        .await;
        let _ = h.outside.kill(&agent.address).await;
    });
}

#[test]
fn linked_worktrees_group_under_their_repo() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let w = h.core.root().join("w");
        std::fs::create_dir_all(w.join("repo")).expect("a repo folder");
        git(&w.join("repo"), &["init", "-q"]);
        git(
            &w.join("repo"),
            &["commit", "-q", "--allow-empty", "-m", "one"],
        );
        git(
            &w.join("repo"),
            &["worktree", "add", "-q", "../wt", "-b", "feat"],
        );
        let repo = w.join("repo").display().to_string();
        let wt = w.join("wt").display().to_string();
        h.typed(&format!("cd {repo}\r")).await;
        h.prefix(KeyCode::Char('N'), KeyModifiers::SHIFT).await;
        h.until("a second space", |h| h.app().snapshot().spaces.len() == 2)
            .await;
        h.until("its shell", |h| {
            let app = h.app();
            app.focused_address()
                .and_then(|a| app.panes.get(&a))
                .is_some_and(|p| p.link == rodeo::panes::Link::Attached)
        })
        .await;
        h.typed(&format!("cd {wt}\r")).await;
        h.until("the worktree group", |h| {
            let screen = h.screen();
            screen.contains("· repo") && screen.contains("└─ · feat")
        })
        .await;
        let toggle = h
            .app()
            .hits
            .sidebar
            .spaces
            .iter()
            .find_map(|s| s.group.clone())
            .expect("a group toggle");
        h.click(toggle.0.x, toggle.0.y).await;
        assert_eq!(h.app().prefs.collapsed_groups.len(), 1);
        h.until("the folded group", |h| h.screen().contains("▸│"))
            .await;
    });
}

#[test]
fn new_tabs_and_workspaces_follow_a_picked_machine() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let _second = add_box(&h);
        h.until("the box doc", |h| {
            h.app()
                .machines
                .get(Some("box"))
                .is_some_and(|m| m.slot.is_ready())
        })
        .await;
        let (x, y) = machine_row(&mut h, Some("box"));
        h.click(x, y).await;
        h.until("the footer names box", |h| {
            h.screen().contains(" new · box")
        })
        .await;
        h.prefix_char('c').await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        h.until("a first workspace on box", |h| {
            spaces_on(h, Some("box")).len() == 1
        })
        .await;
        let first = spaces_on(&h, Some("box"))[0].0.clone();
        h.until("its focus", |h| {
            h.app().view.space.as_deref() == Some(first.as_str())
        })
        .await;
        assert_eq!(spaces_on(&h, None)[0].1, 1);
        let local = spaces_on(&h, None)[0].0.clone();
        h.app_mut().focus_space(&local);
        h.until("the footer names Local", |h| {
            h.screen().contains(" new · Local")
        })
        .await;
        h.app_mut().view.sidebar.picked = Some((Some("box".into()), Some(local.clone())));
        h.until("the footer names box", |h| {
            h.screen().contains(" new · box")
        })
        .await;
        h.prefix_char('c').await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        h.until("a second tab in the box workspace", |h| {
            spaces_on(h, Some("box")) == [(first.clone(), 2)]
        })
        .await;
        assert_eq!(spaces_on(&h, None), [(local.clone(), 1)]);
        h.app_mut().focus_space(&local);
        h.app_mut().view.sidebar.picked = Some((Some("box".into()), Some(local.clone())));
        h.prefix(KeyCode::Char('N'), KeyModifiers::SHIFT).await;
        h.until("a second workspace on box", |h| {
            spaces_on(h, Some("box")).len() == 2
        })
        .await;
        assert_eq!(spaces_on(&h, None), [(local, 1)]);
    });
}
