use config::Action;
use doc::{Axis, Document, Node, Size, Tab};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use rodeo::app::{App, HostInfo, Options};
use rodeo::view::{Mode, Nav};

fn doc() -> Document {
    let mut doc = Document::default();
    let mut tab = Tab::new("tb1", "", Node::terminal("pa", "t1"));
    tab.root
        .children_mut()
        .expect("a row")
        .push(Node::terminal("pb", "t2"));
    doc.new_space("sp1", "work", "/w", tab).expect("a space");
    doc.new_tab(
        "sp1",
        Tab::new("tb2", "logs", Node::terminal("pc", "t3")),
        None,
    )
    .expect("a tab");
    doc.new_space(
        "sp2",
        "other",
        "/o",
        Tab::new("tb3", "", Node::terminal("pd", "t4")),
    )
    .expect("a space");
    doc
}

fn app() -> App {
    let dir = std::env::temp_dir().join(format!("rodeo-actions-{}", std::process::id()));
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(
        Options {
            env: hc::Env::new().with("HOME", "/home/me"),
            config_path: dir.join("config.toml"),
            prefs_path: dir.join("prefs.json"),
            loaded: config::Config::parse("[ui]\nconfirm_close = false\n", &dir),
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
    app.screen = Rect::new(0, 0, 120, 40);
    let slot = &mut app.machines.iter_mut().next().expect("local").slot;
    assert!(slot.command().is_some());
    slot.read_done(Ok((doc().to_value(), 1)));
    app.after_docs();
    app
}

fn local_doc(app: &App) -> Document {
    app.machines.local().slot.doc().clone()
}

fn focused(app: &App) -> String {
    app.view
        .focused_pane(&app.machines)
        .expect("a focused pane")
}

#[test]
fn the_view_starts_on_the_first_space_tab_and_pane() {
    let app = app();
    assert_eq!(app.view.space.as_deref(), Some("sp1"));
    assert_eq!(app.view.active_tab(&app.machines).as_deref(), Some("tb1"));
    assert_eq!(focused(&app), "pa");
    assert_eq!(app.geoms["tb1"].panes.len(), 2);
}

#[test]
fn focus_moves_by_direction_cycle_and_last() {
    let mut app = app();
    app.run_action(Action::FocusPaneRight);
    assert_eq!(focused(&app), "pb");
    app.run_action(Action::FocusPaneRight);
    assert_eq!(focused(&app), "pb", "no pane further right");
    app.run_action(Action::CyclePaneNext);
    assert_eq!(focused(&app), "pa");
    app.run_action(Action::LastPane);
    assert_eq!(focused(&app), "pb");
}

#[test]
fn swaps_zoom_and_resizes_change_the_doc_or_view() {
    let mut app = app();
    app.run_action(Action::SwapPaneRight);
    let order: Vec<String> = local_doc(&app).spaces[0].tabs[0]
        .panes()
        .iter()
        .map(|p| p.id.clone())
        .collect();
    assert_eq!(order, ["pb", "pa"]);
    assert_eq!(focused(&app), "pa", "focus follows the moved pane");
    app.run_action(Action::Zoom);
    assert!(app.view.is_zoomed("tb1"));
    assert_eq!(app.geoms["tb1"].panes.len(), 1);
    app.run_action(Action::Zoom);
    app.run_action(Action::ResizePaneLeft);
    let sizes: Vec<Option<Size>> = local_doc(&app).spaces[0].tabs[0]
        .root
        .children()
        .iter()
        .map(|c| c.size)
        .collect();
    assert!(
        matches!(sizes[1], Some(Size::Weight { weight }) if weight > 1.0),
        "{sizes:?}"
    );
}

#[test]
fn tabs_and_spaces_switch_move_and_rename() {
    let mut app = app();
    app.run_action(Action::NextTab);
    assert_eq!(app.view.active_tab(&app.machines).as_deref(), Some("tb2"));
    app.run_action(Action::SwitchTab(0));
    assert_eq!(app.view.active_tab(&app.machines).as_deref(), Some("tb1"));
    app.run_action(Action::MoveTabNext);
    let tabs: Vec<String> = local_doc(&app).spaces[0]
        .tabs
        .iter()
        .map(|t| t.id.clone())
        .collect();
    assert_eq!(tabs, ["tb2", "tb1"]);
    app.run_action(Action::NextWorkspace);
    assert_eq!(app.view.space.as_deref(), Some("sp2"));
    app.run_action(Action::SwitchWorkspace(0));
    assert_eq!(app.view.space.as_deref(), Some("sp1"));
    assert_eq!(
        app.view.active_tab(&app.machines).as_deref(),
        Some("tb1"),
        "each space keeps its tab"
    );
    app.finish_rename(
        rodeo::overlay::RenameTarget::Tab {
            tab: "tb1".into(),
            original: "2".into(),
            custom: false,
        },
        "main".into(),
    );
    assert_eq!(local_doc(&app).spaces[0].tabs[1].name, "main");
}

#[test]
fn modes_and_the_sidebar_toggle() {
    let mut app = app();
    app.conn.state = rodeo::app::ConnState::Online;
    app.run_action(Action::WorkspacePicker);
    assert_eq!(app.view.mode, Mode::Navigate);
    app.run_action(Action::NavigateWorkspaceDown);
    assert_eq!(app.view.nav.as_ref().and_then(Nav::space), Some("sp2"));
    app.run_action(Action::NavigateWorkspaceDown);
    assert_eq!(
        app.view.nav.as_ref().and_then(Nav::space),
        Some("sp1"),
        "the desktop list wraps"
    );
    app.leave_mode();
    app.run_action(Action::ResizeMode);
    assert_eq!(app.view.mode, Mode::Resize);
    app.leave_mode();
    let before = app.regions.sidebar.width;
    app.run_action(Action::ToggleSidebar);
    assert_eq!(app.prefs.sidebar_collapsed, Some(true));
    assert!(app.regions.sidebar.width < before);
}

#[test]
fn closing_without_confirm_edits_the_doc() {
    let mut app = app();
    app.run_action(Action::ClosePane);
    assert_eq!(local_doc(&app).spaces[0].tabs[0].panes().len(), 1);
    assert_eq!(focused(&app), "pb");
    app.run_action(Action::CloseTab);
    assert_eq!(local_doc(&app).spaces[0].tabs.len(), 1);
    app.run_action(Action::CloseWorkspace);
    assert_eq!(local_doc(&app).spaces.len(), 1);
    assert_eq!(app.view.space.as_deref(), Some("sp2"));
}

#[test]
fn hits_match_the_drawn_frame() {
    let mut app = app();
    app.conn.state = rodeo::app::ConnState::Online;
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).expect("a terminal");
    terminal
        .draw(|frame| rodeo::render::draw(&mut app, frame))
        .expect("a frame");
    let hits = &app.hits;
    assert_eq!(hits.panes.len(), 2);
    let left = hits.panes[0].clone();
    assert_eq!(
        hits.pane_at(left.rect.x + 2, left.rect.y + 2)
            .map(|p| p.key.as_str()),
        Some("pa")
    );
    let split = hits.splits[0].clone();
    assert_eq!(split.axis, Axis::Row);
    assert_eq!(
        hits.split_at(split.pos, split.hit.y + 1).map(|s| s.index),
        Some(0)
    );
    let (tab, key) = hits.tabs.tabs[1].clone();
    assert_eq!(key, "tb2");
    assert_eq!(hits.tab_at(tab.x, tab.y), Some("tb2"));
    assert!(hits.tabs.new_tab.is_some());
    assert_eq!(hits.sidebar.spaces.len(), 2);
    assert!(hits.sidebar.toggle.is_some());
    assert_eq!(hits.sidebar.divider.map(|d| d.x), Some(25));
    let buffer = terminal.backend().buffer();
    let row: String = (0..120)
        .map(|x| buffer[(x, 0)].symbol().to_owned())
        .collect();
    assert!(row.starts_with(" rodeo▍"), "{row}");
    assert!(row.contains("logs"), "{row}");
}
