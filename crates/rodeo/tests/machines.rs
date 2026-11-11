mod common;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::rc::Rc;

use common::{Harness, run};
use crossterm::event::{KeyCode, KeyModifiers};
use rodeo::overlay::Kind;

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
    line.iter()
        .map(|part| format!("'{}'", part.replace('\'', r"'\''")))
        .collect::<Vec<_>>()
        .join(" ")
}

struct Second {
    env: hc::Env,
    bin: std::path::PathBuf,
    name: String,
    session: String,
}

impl Second {
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
        self.hc("t", &["machine", "rm", &self.name]);
        self.hc(&self.session, &["server", "stop"]);
    }
}

fn add_core(h: &Harness, name: &str, session: &str, temp: bool) -> Second {
    hc(h, session, &["server", "start"]);
    let line = via(h);
    let mut args = vec![
        "machine",
        "add",
        "--via",
        &line,
        "--session",
        session,
        "--name",
        name,
    ];
    if temp {
        args.push("--temp");
    }
    hc(h, "t", &args);
    Second {
        env: h.core.env().clone(),
        bin: h.core.bin().to_path_buf(),
        name: name.to_owned(),
        session: session.to_owned(),
    }
}

fn add_box(h: &Harness) -> Second {
    add_core(h, "box", SECOND, false)
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

async fn pick_box(h: &mut Harness) {
    h.prefix_char('w').await;
    h.press(KeyCode::Down, KeyModifiers::NONE).await;
    assert_eq!(
        h.app().view.nav,
        Some(rodeo::view::Nav::Machine(Some("box".into())))
    );
    h.press(KeyCode::Esc, KeyModifiers::NONE).await;
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
        pick_box(&mut h).await;
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
        pick_box(&mut h).await;
        h.prefix(KeyCode::Char('N'), KeyModifiers::SHIFT).await;
        h.until("a second workspace on box", |h| {
            spaces_on(h, Some("box")).len() == 2
        })
        .await;
        assert_eq!(spaces_on(&h, None), [(local.clone(), 1)]);
        let second = spaces_on(&h, Some("box"))[1].0.clone();
        h.until("its focus", |h| {
            h.app().view.space.as_deref() == Some(second.as_str())
        })
        .await;
        h.app_mut().focus_space(&local);
        h.prefix_char('w').await;
        h.press(KeyCode::Down, KeyModifiers::NONE).await;
        h.press(KeyCode::Char('c'), KeyModifiers::NONE).await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        h.until("a tab in the last box workspace", |h| {
            spaces_on(h, Some("box")) == [(first.clone(), 2), (second.clone(), 2)]
        })
        .await;
        assert_eq!(spaces_on(&h, None), [(local, 1)]);
    });
}

fn kind(h: &Harness) -> Option<Kind> {
    h.app().view.overlay.as_ref().map(|o| o.kind())
}

fn picked(h: &mut Harness) -> Option<String> {
    h.screen();
    h.app_mut()
        .machine_manager()
        .expect("the machine manager")
        .picked()
        .map(str::to_owned)
}

fn manager_error(h: &mut Harness) -> Option<String> {
    h.app_mut()
        .machine_manager()
        .and_then(|m| m.error().map(str::to_owned))
}

fn overlay_text(h: &mut Harness) -> String {
    let screen = h.screen();
    let area = h.app().hits.covers.last().copied().expect("an overlay");
    screen
        .lines()
        .skip(usize::from(area.y))
        .take(usize::from(area.height))
        .map(|line| {
            line.chars()
                .skip(usize::from(area.x))
                .take(usize::from(area.width))
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn open_manager(h: &mut Harness) {
    h.prefix_char('m').await;
    h.until("the machine manager", |h| kind(h) == Some(Kind::Machines))
        .await;
}

async fn pick(h: &mut Harness, name: &str) {
    for _ in 0..8 {
        if picked(h).as_deref() == Some(name) {
            return;
        }
        h.press(KeyCode::Char('j'), KeyModifiers::NONE).await;
    }
    panic!("no row for {name}\n{}", h.screen());
}

async fn until_online(h: &mut Harness, names: &[&str]) {
    let names: Vec<String> = names.iter().map(|n| (*n).to_owned()).collect();
    h.until("the machines online", |h| {
        let list = h.app().snapshot().machine_list;
        names.iter().all(|name| {
            list.iter().any(|m| {
                m.name.as_deref() == Some(name.as_str())
                    && m.status == rodeo::model::MachineStatus::Online
            })
        })
    })
    .await;
}

async fn listed(h: &Harness) -> Vec<String> {
    let mut names: Vec<String> = h
        .outside
        .machines()
        .await
        .expect("machine.list")
        .into_iter()
        .map(|m| m.name)
        .collect();
    names.sort();
    names
}

type Seen = Rc<RefCell<BTreeMap<String, (Vec<String>, Option<i32>)>>>;

async fn watch_popups(h: &Harness) -> Seen {
    let client = h.core.client().await;
    let filter = hc::Filter {
        labels: BTreeMap::from([(rodeo::popup::LABEL.to_owned(), "1".to_owned())]),
        ..hc::Filter::default()
    };
    let (_, mut events) = client.subscribe(&filter).await.expect("a subscription");
    let seen = Seen::default();
    let out = seen.clone();
    tokio::task::spawn_local(async move {
        let _client = client;
        while let Some(Ok(notice)) = events.next().await {
            let hc::Notice::Event(event) = notice else {
                continue;
            };
            let Some(address) = event.terminal else {
                continue;
            };
            match event.change {
                hc::Change::Created(terminal) => {
                    out.borrow_mut().insert(address, (terminal.argv, None));
                }
                hc::Change::Finished(ending) => {
                    if let Some(entry) = out.borrow_mut().get_mut(&address) {
                        entry.1 = ending.exit_code;
                    }
                }
                _ => {}
            }
        }
    });
    seen
}

fn hc_args(argv: &[String]) -> &[String] {
    argv.get(4..).unwrap_or_default()
}

#[test]
fn the_manager_lists_every_machine_with_its_state() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let _second = add_box(&h);
        let _temp = add_core(&h, "sandbox", "m3", true);
        until_online(&mut h, &["box", "sandbox"]).await;
        assert!(
            !h.app()
                .snapshot()
                .machines
                .iter()
                .any(|m| m.label == "sandbox"),
            "the sidebar hides temporary machines"
        );
        open_manager(&mut h).await;
        assert_eq!(picked(&mut h), None);
        let text = overlay_text(&mut h);
        eprintln!("{text}");
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[1].contains("machines") && !lines[1].contains("esc close"));
        assert!(text.contains("a add  esc close"), "{text}");
        let local = lines
            .iter()
            .position(|l| l.contains(" Local "))
            .expect("Local");
        let boxed = lines.iter().position(|l| l.contains(" box ")).expect("box");
        let temp = lines
            .iter()
            .position(|l| l.contains(" sandbox "))
            .expect("sandbox");
        assert!(local < boxed && boxed < temp, "{text}");
        assert!(lines[local].contains("● online"), "{text}");
        assert!(lines[boxed].contains("via 'env'"), "{text}");
        assert!(lines[boxed].contains("● online"), "{text}");
        assert!(lines[temp].contains("temp"), "{text}");
        assert!(!text.contains("actions"), "local rows only add\n{text}");
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        assert!(
            !overlay_text(&mut h).contains("reconnect"),
            "no menu for Local"
        );
        h.press(KeyCode::Down, KeyModifiers::NONE).await;
        let text = overlay_text(&mut h);
        assert!(text.contains("↵ actions  a add  esc close"), "{text}");
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        let text = overlay_text(&mut h);
        eprintln!("{text}");
        for (action, key) in [
            ("reconnect", "c"),
            ("log in again", "l"),
            ("update hc there", "u"),
            ("rename", "r"),
            ("remove", "x"),
        ] {
            let line = text
                .lines()
                .find(|l| l.contains(action))
                .unwrap_or_else(|| panic!("{action}\n{text}"));
            let line = line.trim_end_matches(['│', ' ']);
            assert!(line.ends_with(&format!(" {key}")), "{line}");
        }
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        assert!(
            !overlay_text(&mut h).contains("update hc there"),
            "esc closes the menu"
        );
        assert_eq!(kind(&h), Some(Kind::Machines));
        let screen = h.screen();
        let (row, line) = screen
            .lines()
            .enumerate()
            .find(|(_, l)| l.contains(" sandbox "))
            .expect("the sandbox row");
        let column = line[..line.find(" sandbox ").expect("sandbox")]
            .chars()
            .count()
            + 2;
        h.click(
            u16::try_from(column).expect("a column"),
            u16::try_from(row).expect("a row"),
        )
        .await;
        assert_eq!(picked(&mut h).as_deref(), Some("sandbox"));
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        assert_eq!(kind(&h), None);
    });
}

#[test]
fn the_manager_fits_a_narrow_screen() {
    run(|| async {
        let mut h = Harness::sized(60, 20, "").await;
        h.ready().await;
        let _second = add_box(&h);
        until_online(&mut h, &["box"]).await;
        open_manager(&mut h).await;
        h.press(KeyCode::Down, KeyModifiers::NONE).await;
        let text = overlay_text(&mut h);
        eprintln!("{text}");
        let lines: Vec<&str> = text.lines().collect();
        let wide = lines[0].chars().count();
        assert!(wide <= 56, "{text}");
        assert!(lines.iter().all(|l| l.chars().count() == wide), "{text}");
        assert!(
            lines[0].starts_with('┌') && lines[0].ends_with('┐'),
            "{text}"
        );
        let last = lines.last().expect("a bottom border");
        assert!(last.starts_with('└') && last.ends_with('┘'), "{text}");
        assert!(text.contains("● online"), "{text}");
        assert!(
            text.contains("↵ actions  a add  esc close"),
            "one footer line\n{text}"
        );
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        let text = overlay_text(&mut h);
        eprintln!("{text}");
        let screen = h.screen();
        let menu: Vec<&str> = screen
            .lines()
            .filter(|l| l.contains("update hc there"))
            .collect();
        assert_eq!(menu.len(), 1, "{screen}");
        assert!(text.contains("remove"), "{text}");
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        h.press(KeyCode::Char('x'), KeyModifiers::NONE).await;
        let text = overlay_text(&mut h);
        eprintln!("{text}");
        assert!(text.contains("Remove machine?"), "{text}");
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        h.press(KeyCode::Char('a'), KeyModifiers::NONE).await;
        let text = overlay_text(&mut h);
        eprintln!("{text}");
        assert!(
            text.contains("ssh target") && text.contains("↵ add"),
            "{text}"
        );
    });
}

#[test]
fn navigate_and_the_menu_open_the_manager() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let _second = add_box(&h);
        until_online(&mut h, &["box"]).await;
        h.prefix_char('w').await;
        h.press(KeyCode::Down, KeyModifiers::NONE).await;
        assert_eq!(
            h.app().view.nav,
            Some(rodeo::view::Nav::Machine(Some("box".into())))
        );
        h.press(KeyCode::Char('m'), KeyModifiers::NONE).await;
        assert_eq!(kind(&h), Some(Kind::Machines));
        assert_eq!(picked(&mut h).as_deref(), Some("box"));
        assert_eq!(h.app().view.mode, rodeo::view::Mode::Terminal);
        assert_eq!(h.app().view.nav, None);
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        h.screen();
        let launcher = h.app().hits.sidebar.menu.expect("the menu button");
        h.click(launcher.x + 1, launcher.y).await;
        h.press(KeyCode::Char('j'), KeyModifiers::NONE).await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        assert_eq!(kind(&h), Some(Kind::Machines));
        assert_eq!(picked(&mut h), None);
    });
}

#[test]
fn rename_remove_and_reconnect_go_to_hc() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let _second = add_box(&h);
        until_online(&mut h, &["box"]).await;
        open_manager(&mut h).await;
        pick(&mut h, "box").await;
        h.press(KeyCode::Char('c'), KeyModifiers::NONE).await;
        h.until("the reconnect", |h| {
            h.app_mut().machine_manager().is_some_and(|m| !m.is_busy())
        })
        .await;
        assert_eq!(manager_error(&mut h), None);
        assert!(overlay_text(&mut h).contains("box: connected"));
        h.press(KeyCode::Char('r'), KeyModifiers::NONE).await;
        assert!(overlay_text(&mut h).contains("rename machine"));
        h.press(KeyCode::Char('c'), KeyModifiers::CONTROL).await;
        h.typed("crate\r").await;
        h.until("the renamed machine", |h| {
            h.app().machines.get(Some("crate")).is_some()
        })
        .await;
        assert_eq!(listed(&h).await, ["crate"]);
        assert_eq!(picked(&mut h).as_deref(), Some("crate"));
        assert_eq!(manager_error(&mut h), None);
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        h.until("the sidebar row", |h| h.screen().contains(" ▾ crate"))
            .await;
        open_manager(&mut h).await;
        pick(&mut h, "crate").await;
        h.press(KeyCode::Char('x'), KeyModifiers::NONE).await;
        assert!(overlay_text(&mut h).contains("its terminals keep running there"));
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        h.until("the machine gone", |h| {
            h.app().snapshot().machine_list.len() == 1
        })
        .await;
        assert_eq!(listed(&h).await, Vec::<String>::new());
        assert_eq!(manager_error(&mut h), None);
        assert_eq!(picked(&mut h), None);
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        assert!(!h.screen().contains(" ▾ crate"));
    });
}

#[test]
fn a_failed_rename_shows_hcs_error_in_the_manager() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let _second = add_box(&h);
        let _temp = add_core(&h, "sandbox", "m3", true);
        until_online(&mut h, &["box", "sandbox"]).await;
        open_manager(&mut h).await;
        pick(&mut h, "box").await;
        h.press(KeyCode::Char('r'), KeyModifiers::NONE).await;
        h.press(KeyCode::Char('c'), KeyModifiers::CONTROL).await;
        h.typed("sandbox\r").await;
        h.until("the error", |h| manager_error(h).is_some()).await;
        let error = manager_error(&mut h).expect("an error");
        assert!(error.starts_with("cannot rename box"), "{error}");
        let text = overlay_text(&mut h);
        assert!(text.contains("cannot rename box"), "{text}");
        assert_eq!(listed(&h).await, ["box", "sandbox"]);
    });
}

#[test]
fn login_and_update_run_this_hc_against_this_core() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let _second = add_box(&h);
        until_online(&mut h, &["box"]).await;
        let seen = watch_popups(&h).await;
        let bin = h.core.bin().display().to_string();
        for (key, verb) in [('l', "login"), ('u', "update")] {
            open_manager(&mut h).await;
            pick(&mut h, "box").await;
            h.press(KeyCode::Char(key), KeyModifiers::NONE).await;
            let ran = |argv: &[String]| hc_args(argv).get(2).map(String::as_str) == Some(verb);
            h.until("its exit", |_| {
                seen.borrow()
                    .values()
                    .any(|(argv, code)| ran(argv) && code.is_some())
            })
            .await;
            h.until("the popup to close", |h| {
                h.app().popup.is_none() && kind(h) == Some(Kind::Machines)
            })
            .await;
            let (argv, code) = seen
                .borrow()
                .values()
                .find(|(argv, _)| ran(argv))
                .cloned()
                .expect("the popup terminal");
            assert_eq!(&argv[..2], ["/bin/sh", "-c"]);
            assert_eq!(argv[3], "sh");
            assert_eq!(hc_args(&argv), [bin.as_str(), "machine", verb, "box"]);
            assert_eq!(code, Some(0), "hc {verb} box ran against the test core");
            assert_eq!(picked(&mut h).as_deref(), Some("box"));
            assert_eq!(manager_error(&mut h), None);
            h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        }
        until_online(&mut h, &["box"]).await;
    });
}

#[test]
fn a_failed_add_keeps_hcs_words_on_screen() {
    run(|| async {
        let mut h = Harness::start().await;
        h.ready().await;
        let seen = watch_popups(&h).await;
        open_manager(&mut h).await;
        h.press(KeyCode::Char('a'), KeyModifiers::NONE).await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        assert!(overlay_text(&mut h).contains("ssh target is required"));
        h.typed("bad target").await;
        h.press(KeyCode::Tab, KeyModifiers::NONE).await;
        h.typed("odd").await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        h.until("hc's error in the popup", |h| {
            h.screen().contains("Press enter to close.")
        })
        .await;
        let screen = h.screen();
        assert!(screen.contains("add machine"), "{screen}");
        assert!(screen.contains("bad ssh target"), "{screen}");
        let address = h
            .app()
            .popup
            .as_ref()
            .and_then(rodeo::popup::Popup::address)
            .expect("a popup");
        let argv = seen.borrow().get(&address).map(|(argv, _)| argv.clone());
        let bin = h.core.bin().display().to_string();
        assert_eq!(
            argv.as_deref().map(hc_args),
            Some(
                &[
                    bin,
                    "machine".into(),
                    "add".into(),
                    "bad target".into(),
                    "--name".into(),
                    "odd".into()
                ][..]
            )
        );
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        h.until("the manager again", |h| kind(h) == Some(Kind::Machines))
            .await;
        let error = manager_error(&mut h).expect("the exit code");
        assert_eq!(
            error,
            "hc machine add 'bad target' --name odd exited with code 6"
        );
        let text = overlay_text(&mut h);
        assert!(text.contains("exited with code 6"), "{text}");
    });
}

fn fake_ssh(root: &Path, env: &mut hc::Env) {
    let mut line = vec!["env".to_owned(), "-i".to_owned()];
    for (key, value) in env.iter() {
        if key != "HC_SESSION" {
            line.push(format!(
                "{}={}",
                key.to_string_lossy(),
                value.to_string_lossy()
            ));
        }
    }
    line.extend([
        "HC_SESSION=far".to_owned(),
        "sh".to_owned(),
        "-c".to_owned(),
    ]);
    let line = line
        .iter()
        .map(|part| format!("'{}'", part.replace('\'', r"'\''")))
        .collect::<Vec<_>>()
        .join(" ");
    let bin = root.join("fakebin");
    std::fs::create_dir_all(&bin).expect("a bin dir");
    let ssh = bin.join("ssh");
    std::fs::write(
        &ssh,
        format!("#!/bin/sh\nfor last; do :; done\nexec {line} \"$last\"\n"),
    )
    .expect("a fake ssh");
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let path = env
        .get("PATH")
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    env.set("PATH", format!("{}:{path}", bin.display()));
}

#[test]
fn add_reaches_a_machine_through_the_form() {
    run(|| async {
        let mut h = Harness::prepared(fake_ssh).await;
        h.ready().await;
        let _far = Second {
            env: h.core.env().clone(),
            bin: h.core.bin().to_path_buf(),
            name: "far".to_owned(),
            session: "far".to_owned(),
        };
        let seen = watch_popups(&h).await;
        open_manager(&mut h).await;
        h.press(KeyCode::Char('a'), KeyModifiers::NONE).await;
        h.typed("fakehost").await;
        h.press(KeyCode::Tab, KeyModifiers::NONE).await;
        h.typed("far").await;
        h.press(KeyCode::Enter, KeyModifiers::NONE).await;
        h.until("the add to exit", |_| {
            seen.borrow().values().any(|(argv, code)| {
                hc_args(argv)
                    .get(1..)
                    .is_some_and(|a| a == ["machine", "add", "fakehost", "--name", "far"])
                    && code.is_some()
            })
        })
        .await;
        h.until("the manager again", |h| {
            h.app().popup.is_none() && kind(h) == Some(Kind::Machines)
        })
        .await;
        let error = manager_error(&mut h);
        assert_eq!(error, None, "{}", h.screen());
        assert_eq!(listed(&h).await, ["far"]);
        until_online(&mut h, &["far"]).await;
        assert_eq!(picked(&mut h).as_deref(), Some("far"));
        let text = overlay_text(&mut h);
        eprintln!("{text}");
        assert!(
            text.contains(" far ") && text.contains("fakehost"),
            "{text}"
        );
        h.press(KeyCode::Esc, KeyModifiers::NONE).await;
        h.until("the sidebar row", |h| h.screen().contains(" ▾ far"))
            .await;
    });
}

#[test]
fn the_prefix_bar_names_the_machines_key() {
    run(|| async {
        for (config, shown) in [("", true), ("[keys]\nmachines = \"\"\n", false)] {
            let mut h = Harness::with_config(config).await;
            h.ready().await;
            h.press(KeyCode::Char('b'), KeyModifiers::CONTROL).await;
            let screen = h.screen();
            let bar = screen
                .lines()
                .find(|l| l.contains(" PREFIX "))
                .expect("the prefix bar")
                .to_owned();
            assert_eq!(
                bar.contains("w workspace nav  m machines  ? keybinds"),
                shown,
                "{bar}"
            );
            assert_eq!(bar.contains("machines"), shown, "{bar}");
        }
    });
}
