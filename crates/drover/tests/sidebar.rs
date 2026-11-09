use std::collections::BTreeMap;

use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use doc::{Document, Node, Tab};
use drover::app::{App, ConnState, Git, HostInfo, Options};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

const W: u16 = 100;
const H: u16 = 30;

fn space(doc: &mut Document, id: &str, name: &str, address: &str) {
    let tab = Tab::new(
        format!("tb{id}"),
        "",
        Node::terminal(format!("p{id}"), address),
    );
    doc.new_space(id, name, "/w", tab).expect("a space");
}

fn local_doc() -> Document {
    let mut doc = Document::default();
    space(&mut doc, "s1", "", "t1");
    space(&mut doc, "s2", "", "t2");
    space(&mut doc, "s3", "notes", "t3");
    doc
}

fn git(top: &str, branch: &str, linked: bool, ahead: u32, behind: u32) -> Git {
    Git {
        cwd: "/w".into(),
        toplevel: Some(top.into()),
        branch: Some(branch.into()),
        common: Some("/w/repo/.git".into()),
        linked,
        ahead,
        behind,
        ..Git::default()
    }
}

fn agent(address: &str, state: &str, labels: &[(&str, &str)]) -> hc::Terminal {
    hc::Terminal {
        address: address.into(),
        status: Some(hc::Status {
            state: state.into(),
            app: "claude-code".into(),
            ..hc::Status::default()
        }),
        labels: labels
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect::<BTreeMap<_, _>>(),
        ..hc::Terminal::default()
    }
}

fn app_with(config: &str) -> App {
    let dir = std::env::temp_dir().join(format!(
        "drover-sidebar-{}-{}",
        std::process::id(),
        rand_suffix()
    ));
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(
        Options {
            env: hc::Env::new().with("HOME", "/home/me"),
            config_path: dir.join("config.toml"),
            prefs_path: dir.join("prefs.json"),
            loaded: config::Config::parse(config, &dir),
            host: HostInfo {
                cell_px: (8, 16),
                ..HostInfo::default()
            },
            caller: None,
            notes: Vec::new(),
            hostname: "box".into(),
            launch_dir: None,
        },
        tx,
    );
    app.conn.state = ConnState::Online;
    app.screen = Rect::new(0, 0, W, H);
    let slot = &mut app.machines.iter_mut().next().expect("local").slot;
    assert!(slot.command().is_some());
    slot.read_done(Ok((local_doc().to_value(), 1)));
    app.git
        .insert("s1".into(), git("/w/repo", "main", false, 2, 1));
    app.git
        .insert("s2".into(), git("/w/wt", "worktree/feat", true, 0, 0));
    app.terminals = hc::Terminals::new(
        hc::Snapshot {
            seq: 1,
            terminals: vec![
                agent("t1", "blocked", &[("name", "fixer")]),
                agent("t2", "working", &[]),
            ],
        },
        &hc::Filter::default(),
    );
    app.after_docs();
    app.relayout();
    app
}

fn rand_suffix() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

fn app() -> App {
    app_with("")
}

fn add_remote(app: &mut App, state: &str, why: Option<&str>) {
    app.machines.update(vec![hc::MachineView {
        name: "box".into(),
        state: state.into(),
        why: why.map(str::to_owned),
        ..hc::MachineView::default()
    }]);
    let mut doc = Document::default();
    space(&mut doc, "r1", "api", "t9");
    let slot = &mut app.machines.get_mut(Some("box")).expect("box").slot;
    let _ = slot.command();
    slot.read_done(Ok((doc.to_value(), 4)));
    app.after_docs();
    app.relayout();
}

fn frame(app: &mut App) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(W, H)).expect("a terminal");
    terminal
        .draw(|frame| drover::render::draw(app, frame))
        .expect("a frame");
    terminal.backend().buffer().clone()
}

fn line(buf: &Buffer, y: u16, width: u16) -> String {
    (0..width).map(|x| buf[(x, y)].symbol()).collect()
}

fn column(buf: &Buffer, width: u16) -> Vec<String> {
    (0..H).map(|y| line(buf, y, width)).collect()
}

fn mouse(app: &mut App, kind: MouseEventKind, column: u16, row: u16) {
    app.host_event(Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }));
}

fn click(app: &mut App, x: u16, y: u16) {
    mouse(app, MouseEventKind::Down(MouseButton::Left), x, y);
    mouse(app, MouseEventKind::Up(MouseButton::Left), x, y);
}

fn find(buf: &Buffer, text: &str) -> Option<(u16, u16)> {
    (0..H).find_map(|y| {
        let row = line(buf, y, W);
        let at = row.find(text)?;
        Some((u16::try_from(row[..at].chars().count()).ok()?, y))
    })
}

#[test]
fn spaces_show_tokens_groups_and_the_footer() {
    let mut app = app();
    let buf = frame(&mut app);
    let rows = column(&buf, 26);
    assert_eq!(rows[0], " spaces                  │");
    assert_eq!(rows[2], " ● repo                 ▾│");
    assert_eq!(rows[3], "   main ↑2 ↓1            │");
    assert_eq!(rows[4], "   └─ ● feat             │");
    assert_eq!(rows[5], " ○ notes                 │".replace('○', "·"));
    assert_eq!(rows[14], " new                 menu│");
    assert_eq!(rows[15], "─────────────────────────│");
    assert_eq!(rows[16], " agents           grouped│");
    assert_eq!(rows[18], " ● repo                  │");
    assert_eq!(rows[19], "   claude                │");
    assert_eq!(rows[20], " ● wt                    │");
    assert_eq!(rows[29], "                        «│");
    let toggle = app.hits.sidebar.spaces[0]
        .group
        .clone()
        .expect("a group toggle");
    assert_eq!(toggle.0, Rect::new(24, 2, 1, 1));
    assert_eq!(buf[(24, 2)].fg, app.palette.accent);
    assert_eq!(buf[(1, 2)].fg, app.palette.red);
    assert!(buf[(3, 2)].modifier.contains(Modifier::BOLD));
    assert_eq!(buf[(3, 3)].fg, app.palette.mauve);
    assert_eq!(buf[(8, 3)].fg, app.palette.green);
    assert_eq!(buf[(11, 3)].fg, app.palette.red);
    assert_eq!(buf[(5, 4)].fg, app.palette.overlay0);
    assert_eq!(buf[(0, 2)].bg, app.palette.active_row_bg);
    assert_eq!(buf[(0, 4)].bg, app.palette.sidebar_bg);
}

#[test]
fn groups_collapse_roll_up_and_persist() {
    let mut app = app();
    let _ = frame(&mut app);
    let (toggle, key) = app.hits.sidebar.spaces[0].group.clone().expect("a toggle");
    assert_eq!(key, "/w/repo/.git");
    click(&mut app, toggle.x, toggle.y);
    assert_eq!(app.prefs.collapsed_groups, ["/w/repo/.git"]);
    let saved = config::Preferences::load(&app.prefs_path);
    assert_eq!(saved.collapsed_groups, ["/w/repo/.git"]);
    let buf = frame(&mut app);
    let rows = column(&buf, 26);
    assert_eq!(rows[2], " ● repo                 ▸│");
    assert_eq!(rows[4], " · notes                 │");
    assert_eq!(
        buf[(1, 2)].fg,
        app.palette.red,
        "the group shows its worst state"
    );
    app.focus_space("s2");
    let rows = column(&frame(&mut app), 26);
    assert_eq!(rows[4], "   └─ ● feat             │");
}

#[test]
fn custom_tokens_rules_and_rows_by_agent() {
    let mut app = app_with(
        r##"
[ui.sidebar.agents]
row_gap = 1
rows = [["state_icon", "$name", "state_text"]]
[ui.sidebar.agents.rows_by_agent]
claude = [["state_icon", { token = "$name", rules = [{ equals = "fixer", fg = "#ff0000" }] }, "agent"]]
[ui.sidebar.spaces]
rows = [["state_text", "workspace", "$name"]]
"##,
    );
    let buf = frame(&mut app);
    let rows = column(&buf, 26);
    assert_eq!(rows[2], " blocked · repo         ▾│");
    assert_eq!(rows[18], " ● fixer · claude        │");
    assert_eq!(rows[19], "                         │");
    assert_eq!(rows[20], " ● claude                │");
    assert_eq!(buf[(3, 18)].fg, ratatui::style::Color::Rgb(255, 0, 0));
}

#[test]
fn the_sort_toggle_orders_by_priority_and_saves() {
    let mut app = app();
    app.terminals = hc::Terminals::new(
        hc::Snapshot {
            seq: 1,
            terminals: vec![agent("t1", "working", &[]), agent("t2", "blocked", &[])],
        },
        &hc::Filter::default(),
    );
    let rows = column(&frame(&mut app), 26);
    assert_eq!(rows[18], " ● repo                  │");
    let sort = app.hits.sidebar.agent_sort.expect("the sort toggle");
    click(&mut app, sort.x, sort.y);
    let rows = column(&frame(&mut app), 26);
    assert_eq!(rows[16], " agents          priority│");
    assert_eq!(rows[18], " ● wt                    │");
    assert_eq!(
        config::Preferences::load(&app.prefs_path).agent_panel_sort,
        Some(config::AgentPanelSort::Priority)
    );
}

#[test]
fn agent_rows_focus_their_pane() {
    let mut app = app();
    let _ = frame(&mut app);
    let row = app.hits.sidebar.agents[1].rect;
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        row.x + 2,
        row.y,
    );
    assert_eq!(app.view.focused_pane(&app.machines).as_deref(), Some("ps2"));
    let buf = frame(&mut app);
    assert_eq!(buf[(0, row.y)].bg, app.palette.active_row_bg);
}

fn many() -> App {
    let mut app = app();
    let mut doc = Document::default();
    for i in 0..30 {
        space(
            &mut doc,
            &format!("m{i}"),
            &format!("space {i}"),
            &format!("t{}", 100 + i),
        );
    }
    let slot = &mut app.machines.iter_mut().next().expect("local").slot;
    slot.read_done(Ok((doc.to_value(), 2)));
    app.git.clear();
    app.after_docs();
    app.relayout();
    app
}

#[test]
fn the_agent_list_scrolls_too() {
    let mut app = many();
    app.terminals = hc::Terminals::new(
        hc::Snapshot {
            seq: 1,
            terminals: (0..30)
                .map(|i| agent(&format!("t{}", 100 + i), "working", &[]))
                .collect(),
        },
        &hc::Filter::default(),
    );
    let buf = frame(&mut app);
    let track = app.hits.sidebar.agents_bar.expect("an agent scrollbar");
    assert_eq!(track, Rect::new(24, 18, 1, 12));
    assert_eq!(line(&buf, 18, 26), " ● space 0              ▕│");
    mouse(&mut app, MouseEventKind::ScrollDown, 5, 20);
    let buf = frame(&mut app);
    assert_eq!(app.view.agent_scroll, 1);
    assert_eq!(line(&buf, 18, 26), " ● space 1              ▕│");
    assert_eq!(buf[(24, 29)].symbol(), "«");
    click(&mut app, 24, 28);
    let _ = frame(&mut app);
    assert_eq!(app.hits.sidebar.agents_metrics.max, 24);
    assert_eq!(app.view.agent_scroll, 22);
}

#[test]
fn long_lists_scroll_by_wheel_track_and_thumb() {
    let mut app = many();
    let buf = frame(&mut app);
    let track = app.hits.sidebar.spaces_bar.expect("a scrollbar");
    assert_eq!(track, Rect::new(24, 2, 1, 12));
    assert_eq!(buf[(24, 2)].symbol(), "▕");
    assert_eq!(buf[(24, 2)].fg, app.palette.overlay0);
    assert_eq!(buf[(24, 13)].fg, app.palette.surface_dim);
    assert!(line(&buf, 2, 26).starts_with(" · space 0"));
    mouse(&mut app, MouseEventKind::ScrollDown, 5, 5);
    let buf = frame(&mut app);
    assert!(line(&buf, 2, 26).starts_with(" · space 1"));
    click(&mut app, 24, 13);
    let buf = frame(&mut app);
    assert_eq!(app.view.sidebar_scroll, 18);
    assert!(line(&buf, 13, 26).starts_with(" · space 29"));
    let (top, _) =
        drover::sidebar::scroll::thumb(app.hits.sidebar.spaces_metrics, track).expect("a thumb");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 24, top);
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), 24, 2);
    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), 24, 2);
    let buf = frame(&mut app);
    assert_eq!(app.view.sidebar_scroll, 0);
    assert!(line(&buf, 2, 26).starts_with(" · space 0"));
}

#[test]
fn dragging_a_space_shows_the_drop_line_and_reorders_the_doc() {
    let mut app = app();
    let _ = frame(&mut app);
    let notes = app.hits.sidebar.space_at(3, 5).expect("notes").clone();
    assert_eq!(notes.key, "s3");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 3, 5);
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), 3, 2);
    let buf = frame(&mut app);
    assert_eq!(line(&buf, 1, 25), "─".repeat(25));
    assert_eq!(buf[(0, 1)].fg, app.palette.accent);
    assert_eq!(buf[(0, 5)].bg, app.palette.surface1);
    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), 3, 2);
    let order: Vec<String> = app
        .machines
        .local()
        .slot
        .doc()
        .spaces
        .iter()
        .map(|s| s.id.clone())
        .collect();
    assert_eq!(order, ["s3", "s1", "s2"]);
    assert_eq!(
        app.view.space.as_deref(),
        Some("s1"),
        "a drop does not focus"
    );
    let _ = frame(&mut app);
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 3, 3);
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), 3, 1);
    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), 3, 1);
    let order: Vec<String> = app
        .machines
        .local()
        .slot
        .doc()
        .spaces
        .iter()
        .map(|s| s.id.clone())
        .collect();
    assert_eq!(order, ["s1", "s2", "s3"], "a group moves with its children");
}

#[test]
fn linked_children_do_not_drag() {
    let mut app = app();
    let _ = frame(&mut app);
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 8, 4);
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), 8, 6);
    let buf = frame(&mut app);
    assert_ne!(buf[(0, 4)].bg, app.palette.surface1);
    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), 8, 6);
    assert_eq!(app.machines.local().slot.doc().spaces[0].id, "s1");
}

#[test]
fn the_compact_sidebar_numbers_rows() {
    let mut app = app_with("[ui]\nsidebar_start_collapsed = true\n");
    let buf = frame(&mut app);
    let rows = column(&buf, 4);
    assert_eq!(rows[0], "1 ●│");
    assert_eq!(rows[1], "2 ●│");
    assert_eq!(rows[2], "3 ·│");
    assert_eq!(rows[15], "───│");
    assert_eq!(rows[16], "1 ●│");
    assert_eq!(rows[17], "2 ●│");
    assert_eq!(rows[29], " » │");
    assert_eq!(buf[(0, 0)].bg, app.palette.active_row_bg);
    click(&mut app, 1, 29);
    let rows = column(&frame(&mut app), 26);
    assert_eq!(rows[0], " spaces                  │");
}

#[test]
fn hidden_mode_draws_nothing() {
    let mut app =
        app_with("[ui]\nsidebar_start_collapsed = true\nsidebar_collapsed_mode = \"hidden\"\n");
    let _ = frame(&mut app);
    assert_eq!(app.hits.sidebar.area, None);
}

#[test]
fn the_sidebar_edge_drags_and_resets() {
    let mut app = app();
    let _ = frame(&mut app);
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 25, 10);
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), 30, 10);
    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), 30, 10);
    assert_eq!(app.prefs.sidebar_width, Some(31));
    assert_eq!(app.regions.sidebar.width, 31);
    let _ = frame(&mut app);
    click(&mut app, 30, 10);
    click(&mut app, 30, 10);
    assert_eq!(app.prefs.sidebar_width, None);
    assert_eq!(app.regions.sidebar.width, 26);
    let _ = frame(&mut app);
    let divider = app.hits.sidebar.section_divider.expect("a section divider");
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        3,
        divider.y,
    );
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), 3, 9);
    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), 3, 9);
    assert_eq!(app.prefs.section_split(), 0.3);
    let rows = column(&frame(&mut app), 26);
    assert_eq!(rows[9], "─────────────────────────│");
}

#[test]
fn machines_get_rows_states_and_nested_spaces() {
    let mut app = app();
    add_remote(&mut app, "connected", None);
    app.terminals = hc::Terminals::new(
        hc::Snapshot {
            seq: 1,
            terminals: vec![agent("t1", "blocked", &[]), agent("box/t9", "working", &[])],
        },
        &hc::Filter::default(),
    );
    let buf = frame(&mut app);
    let rows = column(&buf, 26);
    assert_eq!(rows[0], " machines                │");
    assert_eq!(rows[2], " ▾ Local ────────────────│");
    assert_eq!(rows[3], "   ● repo               ▾│");
    assert_eq!(rows[4], "     main ↑2 ↓1          │");
    assert_eq!(rows[5], "     └─ · feat           │");
    assert_eq!(rows[6], "   · notes               │");
    assert_eq!(rows[7], " ▾ box ● ────────────────│");
    assert_eq!(rows[8], "   ● api                 │");
    assert_eq!(rows[14], " new · Local         menu│");
    assert_eq!(rows[18], " ● Local · repo          │");
    assert_eq!(rows[20], " ● box · api             │");
    assert_eq!(buf[(7, 7)].fg, app.palette.green);
    assert_eq!(buf[(9, 7)].fg, app.palette.surface_dim);
    assert!(buf[(3, 7)].modifier.contains(Modifier::BOLD));
    click(&mut app, 5, 7);
    assert_eq!(app.view.space.as_deref(), Some("box/r1"));
    let rows = column(&frame(&mut app), 26);
    assert_eq!(rows[14], " new · box           menu│");
    click(&mut app, 5, 7);
    let rows = column(&frame(&mut app), 26);
    assert_eq!(rows[7], " ▸ box ● ────────────────│");
    assert_eq!(rows[8], "                         │");
    click(&mut app, 1, 2);
    let rows = column(&frame(&mut app), 26);
    assert_eq!(rows[2], " ▸ Local ────────────────│");
    assert_eq!(app.view.space.as_deref(), Some("box/r1"));
}

#[test]
fn offline_machines_dim_their_spaces_and_explain_errors() {
    let mut app = app();
    add_remote(&mut app, "connected", None);
    app.machines.update(vec![hc::MachineView {
        name: "box".into(),
        state: "needs_login".into(),
        why: Some("Permission denied (publickey)".into()),
        ..hc::MachineView::default()
    }]);
    app.after_docs();
    let buf = frame(&mut app);
    let rows = column(&buf, 26);
    assert_eq!(rows[7], " ▾ box ! auth ───────────│");
    assert_eq!(rows[8], "   · api                 │");
    assert_eq!(buf[(7, 7)].fg, app.palette.red);
    assert!(buf[(5, 8)].modifier.contains(Modifier::DIM));
    mouse(&mut app, MouseEventKind::Moved, 9, 7);
    let buf = frame(&mut app);
    assert!(buf[(9, 7)].modifier.contains(Modifier::REVERSED));
    assert!(!buf[(14, 7)].modifier.contains(Modifier::REVERSED));
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 9, 7);
    let notice = app.notice.clone().expect("a diagnostic");
    assert_eq!(notice.title, "box: hc machine login box");
    assert_eq!(notice.body, "Permission denied (publickey)");
    app.notice = None;
    click(&mut app, 5, 8);
    let notice = app.notice.clone().expect("a not ready notice");
    assert_eq!(notice.title, "Endpoint unavailable");
    assert_eq!(notice.body, "box is not ready");
    app.machines.update(vec![hc::MachineView {
        name: "box".into(),
        state: "unreachable".into(),
        ..hc::MachineView::default()
    }]);
    let rows = column(&frame(&mut app), 26);
    assert_eq!(rows[7], " ▾ box ◐ reconnecting ───│");
}

#[test]
fn the_compact_machines_view() {
    let mut app = app_with("[ui]\nsidebar_start_collapsed = true\n");
    add_remote(&mut app, "connected", None);
    let buf = frame(&mut app);
    let rows = column(&buf, 4);
    assert_eq!(rows[0], "▾L │");
    assert_eq!(rows[1], " 1●│");
    assert_eq!(rows[4], "▾2●│");
    assert_eq!(rows[5], " 1·│");
    assert_eq!(rows[16], "L●│".replace("L●│", "L● │"));
    assert_eq!(rows[29], " » │");
}

#[test]
fn a_lost_core_shows_the_lifecycle_banner() {
    let mut app = app();
    app.conn.state = ConnState::Down;
    let buf = frame(&mut app);
    let (x, y) = find(&buf, "◐ Local · reconnecting").expect("the banner");
    assert_eq!(y, 0);
    assert_eq!(x, W - 23);
    assert_eq!(buf[(x, y)].fg, app.palette.yellow);
    assert_eq!(buf[(x - 1, y)].bg, app.palette.surface0);
    assert_eq!(line(&buf, 0, 26), " machines                │");
    assert!(app.hits.covers.contains(&Rect::new(x - 1, y, W - x + 1, 1)));
}

#[test]
fn workspace_steps_follow_the_sidebar_order() {
    let mut app = app();
    app.git.clear();
    app.git
        .insert("s1".into(), git("/w/wt", "feat", true, 0, 0));
    app.git
        .insert("s3".into(), git("/w/repo", "main", false, 0, 0));
    assert_eq!(app.space_order(), ["s3", "s1", "s2"]);
    app.focus_space("s3");
    app.run_action(config::Action::NextWorkspace);
    assert_eq!(app.view.space.as_deref(), Some("s1"));
    app.run_action(config::Action::NextWorkspace);
    assert_eq!(app.view.space.as_deref(), Some("s2"));
    app.prefs.collapsed_groups.push("/w/repo/.git".into());
    assert_eq!(app.space_order(), ["s3", "s2"]);
    app.prefs.sidebar_collapsed = Some(true);
    assert_eq!(app.space_order(), ["s1", "s2", "s3"]);
}
