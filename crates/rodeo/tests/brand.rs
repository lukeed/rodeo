use doc::{Document, Node, Tab};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use rodeo::app::{App, ConnState, HostInfo, Options};
use rodeo::view::Mode;

const HAT: [&str; 6] = [
    "    ██▄▄██    ",
    "█▄  ▀▀▀▀▀▀  ▄█",
    " ▀▀▀▀▀▀▀▀▀▀▀▀ ",
    "     ████     ",
    "     ████     ",
    "     ████     ",
];

fn app(cols: u16, rows: u16, config: &str) -> App {
    let dir = std::env::temp_dir().join(format!("rodeo-brand-{}", std::process::id()));
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
    app.screen = Rect::new(0, 0, cols, rows);
    app.relayout();
    app
}

fn empty(cols: u16, rows: u16, config: &str) -> App {
    let mut app = app(cols, rows, config);
    app.conn.state = ConnState::Online;
    let slot = &mut app.machines.iter_mut().next().expect("local").slot;
    assert!(slot.command().is_some());
    slot.read_done(Ok((Document::default().to_value(), 1)));
    app.after_docs();
    app.relayout();
    app
}

fn frame(app: &mut App) -> Buffer {
    let Rect { width, height, .. } = app.screen;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("a terminal");
    terminal
        .draw(|frame| rodeo::render::draw(app, frame))
        .expect("a frame");
    terminal.backend().buffer().clone()
}

fn rows(buf: &Buffer, area: Rect) -> Vec<String> {
    (area.top()..area.bottom())
        .map(|y| {
            (area.left()..area.right())
                .map(|x| buf[(x, y)].symbol())
                .collect()
        })
        .collect()
}

fn splash_area(app: &App) -> Rect {
    let surface = app.regions.surface;
    if app.view.mode == Mode::Terminal {
        return surface;
    }
    Rect {
        height: surface.height - 1,
        ..surface
    }
}

fn centered(text: &str, width: u16) -> String {
    let w = u16::try_from(text.chars().count()).expect("a short line");
    let left = usize::from((width - w) / 2);
    let right = usize::from(width - w) - left;
    format!("{}{text}{}", " ".repeat(left), " ".repeat(right))
}

fn expected(area: Rect, blocks: &[&[&str]]) -> Vec<String> {
    let mut lines: Vec<&str> = Vec::new();
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            lines.push("");
        }
        lines.extend(block.iter());
    }
    let count = u16::try_from(lines.len()).expect("a few lines");
    let top = usize::from((area.height - count) / 2);
    let blank = " ".repeat(usize::from(area.width));
    let mut out = vec![blank.clone(); usize::from(area.height)];
    for (i, line) in lines.iter().enumerate() {
        out[top + i] = if line.is_empty() {
            blank.clone()
        } else {
            centered(line, area.width)
        };
    }
    out
}

fn find(buf: &Buffer, area: Rect, text: &str) -> (u16, u16) {
    rows(buf, area)
        .iter()
        .enumerate()
        .find_map(|(y, row)| {
            let at = row.find(text)?;
            let x = u16::try_from(row[..at].chars().count()).ok()?;
            Some((area.x + x, area.y + u16::try_from(y).ok()?))
        })
        .unwrap_or_else(|| panic!("{text:?} in {:#?}", rows(buf, area)))
}

fn hat_at(buf: &Buffer, area: Rect) -> (u16, u16) {
    let (x, y) = find(buf, area, HAT[1]);
    (x, y - 1)
}

fn assert_hat_colors(buf: &Buffer, x: u16, y: u16, text: Color, accent: Color, bg: Color) {
    let cell = |dx: u16, dy: u16| {
        let c = &buf[(x + dx, y + dy)];
        (c.symbol().to_owned(), c.fg, c.bg)
    };
    assert_eq!(cell(4, 0), ("█".into(), text, bg));
    assert_eq!(cell(6, 0), ("▄".into(), text, bg));
    assert_eq!(cell(0, 1), ("█".into(), text, bg));
    assert_eq!(cell(1, 1), ("▄".into(), text, bg));
    for dx in 4..10 {
        assert_eq!(cell(dx, 1), ("▀".into(), text, accent), "band at {dx}");
    }
    assert_eq!(cell(12, 1), ("▄".into(), text, bg));
    assert_eq!(cell(13, 1), ("█".into(), text, bg));
    for dx in 1..13 {
        assert_eq!(cell(dx, 2), ("▀".into(), text, bg), "brim at {dx}");
    }
    for dy in 3..6 {
        for dx in 5..9 {
            assert_eq!(
                cell(dx, dy),
                ("█".into(), accent, bg),
                "cursor at {dx},{dy}"
            );
        }
    }
}

fn assert_wordmark(buf: &Buffer, x: u16, y: u16, text: Color, accent: Color) {
    for (i, ch) in "rodeo".chars().enumerate() {
        let cell = &buf[(x + u16::try_from(i).expect("short"), y)];
        assert_eq!(cell.symbol(), ch.to_string());
        assert_eq!(cell.fg, text);
        assert!(cell.modifier.contains(Modifier::BOLD));
    }
    let cursor = &buf[(x + 5, y)];
    assert_eq!(cursor.symbol(), "▍");
    assert_eq!(cursor.fg, accent);
}

const HINT: &str = "prefix+shift+n  new workspace";

#[test]
fn the_empty_state_shows_the_hat_wordmark_and_hint() {
    let mut app = empty(100, 30, "");
    let buf = frame(&mut app);
    let area = splash_area(&app);
    assert_eq!(
        rows(&buf, area),
        expected(area, &[&HAT, &["rodeo▍"], &[HINT]])
    );
    let p = app.palette;
    let (x, y) = hat_at(&buf, area);
    assert_hat_colors(&buf, x, y, p.text, p.accent, Color::Reset);
    let (wx, wy) = find(&buf, area, "rodeo▍");
    assert_eq!(wy, y + 7);
    assert_eq!(wx, x + 4);
    assert_wordmark(&buf, wx, wy, p.text, p.accent);
    let (hx, hy) = find(&buf, area, HINT);
    assert_eq!(hy, y + 9);
    assert_eq!(buf[(hx, hy)].fg, p.overlay0);
    assert_eq!(buf[(hx + 20, hy)].fg, p.overlay0);
}

#[test]
fn the_empty_hint_uses_the_real_binding() {
    let mut app = empty(100, 30, "[keys]\nnew_workspace = \"ctrl+alt+w\"\n");
    let buf = frame(&mut app);
    find(&buf, app.regions.surface, "ctrl+alt+w  new workspace");
    let mut app = empty(100, 30, "[keys]\nnew_workspace = \"\"\n");
    let buf = frame(&mut app);
    find(&buf, app.regions.surface, "unset  new workspace");
}

#[test]
fn the_empty_state_drops_the_hint_then_the_wordmark_then_the_hat() {
    let mut app = empty(100, 30, "");
    let chrome = 30 - splash_area(&app).height;
    for (height, blocks) in [
        (10, vec![&HAT[..], &["rodeo▍"], &[HINT]]),
        (9, vec![&HAT[..], &["rodeo▍"]]),
        (8, vec![&HAT[..], &["rodeo▍"]]),
        (7, vec![&HAT[..]]),
        (6, vec![&HAT[..]]),
        (5, vec![]),
        (1, vec![]),
    ] {
        app.screen = Rect::new(0, 0, 100, height + chrome);
        app.relayout();
        let buf = frame(&mut app);
        let area = splash_area(&app);
        assert_eq!(area.height, height);
        assert_eq!(rows(&buf, area), expected(area, &blocks), "height {height}");
    }
}

#[test]
fn a_narrow_empty_state_drops_what_does_not_fit_across() {
    let desktop = "[ui]\nmobile_width_threshold = 1\n";
    let mut app = empty(40, 30, desktop);
    let area = splash_area(&app);
    assert_eq!(area.width, 14);
    let buf = frame(&mut app);
    assert_eq!(rows(&buf, area), expected(area, &[&HAT, &["rodeo▍"]]));
    app.screen = Rect::new(0, 0, 39, 30);
    app.relayout();
    let buf = frame(&mut app);
    let area = splash_area(&app);
    assert_eq!(area.width, 13);
    assert_eq!(rows(&buf, area), expected(area, &[&["rodeo▍"]]));
}

#[test]
fn the_brand_follows_the_terminal_theme() {
    let mut app = empty(100, 30, "[theme]\nname = \"terminal\"\n");
    let buf = frame(&mut app);
    let area = splash_area(&app);
    let (x, y) = hat_at(&buf, area);
    assert_eq!(app.palette.text, Color::Reset);
    assert_hat_colors(&buf, x, y, Color::Reset, Color::Blue, Color::Reset);
    let (wx, wy) = find(&buf, area, "rodeo▍");
    assert_wordmark(&buf, wx, wy, Color::Reset, Color::Blue);
}

#[test]
fn connecting_shows_the_brand_above_the_status() {
    let mut app = app(100, 30, "");
    let buf = frame(&mut app);
    let area = splash_area(&app);
    let status = "Local: connecting. Select a connected machine.";
    assert_eq!(
        rows(&buf, area),
        expected(area, &[&HAT, &["rodeo▍"], &[status]])
    );
    let p = app.palette;
    let (x, y) = hat_at(&buf, area);
    assert_hat_colors(&buf, x, y, p.text, p.accent, p.panel_bg);
    let (wx, wy) = find(&buf, area, "rodeo▍");
    assert_wordmark(&buf, wx, wy, p.text, p.accent);
    let (sx, sy) = find(&buf, area, status);
    assert_eq!(sy, y + 9);
    assert_eq!(
        (buf[(sx, sy)].fg, buf[(sx, sy)].bg),
        (p.overlay0, p.panel_bg)
    );
    assert_eq!(buf[(area.x, area.y)].bg, p.panel_bg);
}

#[test]
fn a_down_core_shows_the_error_under_the_status() {
    let mut app = app(100, 30, "");
    app.conn.state = ConnState::Down;
    app.conn.error = Some("no socket".into());
    let buf = frame(&mut app);
    let area = splash_area(&app);
    let status = "Local: reconnecting. Select a connected machine.";
    assert_eq!(
        rows(&buf, area),
        expected(area, &[&HAT, &["rodeo▍"], &[status, "no socket"]])
    );
}

#[test]
fn a_short_connecting_screen_keeps_the_status() {
    let mut app = app(100, 30, "");
    let chrome = 30 - splash_area(&app).height;
    let status = "Local: connecting. Select a connected machine.";
    for (height, blocks) in [
        (10, vec![&HAT[..], &["rodeo▍"], &[status]]),
        (9, vec![&HAT[..], &[status]]),
        (8, vec![&HAT[..], &[status]]),
        (7, vec![&[status]]),
        (2, vec![&[status]]),
        (1, vec![&[status]]),
    ] {
        app.screen = Rect::new(0, 0, 100, height + chrome);
        app.relayout();
        let buf = frame(&mut app);
        let area = splash_area(&app);
        assert_eq!(area.height, height);
        assert_eq!(rows(&buf, area), expected(area, &blocks), "height {height}");
    }
}

#[test]
fn a_narrow_connecting_screen_cuts_the_status_at_the_edge() {
    let mut app = app(30, 30, "");
    let buf = frame(&mut app);
    let area = splash_area(&app);
    let rows = rows(&buf, area);
    let line = rows
        .iter()
        .find(|r| r.contains("Local:"))
        .unwrap_or_else(|| panic!("{:#?}", self::rows(&buf, app.screen)));
    assert_eq!(
        line,
        &format!(
            " {}",
            &"Local: connecting. Select a connected machine."[..usize::from(area.width) - 1]
        )
    );
    assert!(rows.iter().any(|r| r.contains("rodeo▍")));
}

fn machines_app() -> App {
    let mut app = empty(100, 30, "");
    let mut doc = Document::default();
    let tab = Tab::new("tb1", "", Node::terminal("p1", "t1"));
    doc.new_space("s1", "", "/w", tab).expect("a space");
    let slot = &mut app.machines.iter_mut().next().expect("local").slot;
    slot.read_done(Ok((doc.to_value(), 2)));
    app.machines.update(vec![hc::MachineView {
        name: "box".into(),
        state: "online".into(),
        ..hc::MachineView::default()
    }]);
    app.after_docs();
    app.relayout();
    app
}

#[test]
fn the_machines_sidebar_header_is_the_wordmark() {
    let mut app = machines_app();
    let buf = frame(&mut app);
    let sidebar = app.regions.sidebar;
    let header = rows(&buf, Rect::new(sidebar.x, sidebar.y, sidebar.width, 2));
    let width = usize::from(sidebar.width) - 8;
    assert_eq!(header[0], format!(" rodeo▍{}│", " ".repeat(width)));
    assert_eq!(header[1].trim_end_matches('│').trim(), "");
    let p = app.palette;
    assert_wordmark(&buf, sidebar.x + 1, sidebar.y, p.text, p.accent);
}

#[test]
fn the_single_machine_sidebar_header_is_the_wordmark() {
    let mut app = empty(100, 30, "");
    let buf = frame(&mut app);
    let sidebar = app.regions.sidebar;
    let header = rows(&buf, Rect::new(sidebar.x, sidebar.y, sidebar.width, 2));
    let width = usize::from(sidebar.width) - 8;
    assert_eq!(header[0], format!(" rodeo▍{}│", " ".repeat(width)));
    assert_eq!(header[1].trim_end_matches('│').trim(), "");
    let p = app.palette;
    assert_wordmark(&buf, sidebar.x + 1, sidebar.y, p.text, p.accent);
}

#[test]
fn a_mobile_connecting_screen_stays_below_the_lifecycle_banner() {
    let mut app = app(60, 14, "");
    assert!(app.regions.mobile);
    let buf = frame(&mut app);
    let surface = app.regions.surface;
    let banner = find(&buf, app.screen, "◐ Local · connecting");
    assert_eq!(banner.1, surface.y);
    let below = Rect {
        y: surface.y + 1,
        height: surface.height - 1,
        ..surface
    };
    let status = "Local: connecting. Select a connected machine.";
    assert_eq!(
        rows(&buf, below),
        expected(below, &[&HAT, &["rodeo▍"], &[status]])
    );
}
