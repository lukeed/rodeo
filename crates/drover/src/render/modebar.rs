use config::{Action, TabBarPosition};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::app::App;
use crate::hits::Hits;
use crate::overlay::frame::contrast;
use crate::text::width;
use crate::view::Mode;

pub fn bar(app: &App) -> Option<Rect> {
    if app.view.mode == Mode::Terminal && app.error.is_none() {
        return None;
    }
    let regions = app.regions;
    let bottom_tabs = regions
        .tab_bar
        .filter(|_| app.config.ui.tab_bar_position == TabBarPosition::Bottom && !regions.mobile);
    if let Some(tabs) = bottom_tabs {
        return Some(tabs);
    }
    let surface = regions.surface;
    (surface.height > 0 && surface.width > 0)
        .then(|| Rect::new(surface.x, surface.bottom() - 1, surface.width, 1))
}

fn rhs(app: &App, action: Action) -> String {
    app.keymap
        .rhs_label(action)
        .unwrap_or_else(|| "unset".to_owned())
}

#[derive(Clone, Copy)]
struct Styles {
    base: Style,
    key: Style,
    badge: Style,
}

pub fn render(app: &App, buf: &mut Buffer, hits: &mut Hits) {
    let Some(bar) = bar(app) else {
        return;
    };
    if hits.tabs.row == Some(bar) {
        hits.tabs = crate::hits::TabHits::default();
    }
    hits.covers.push(bar);
    let p = &app.palette;
    let styles = Styles {
        base: Style::new().fg(p.overlay0).bg(p.panel_bg),
        key: Style::new()
            .fg(p.accent)
            .bg(p.panel_bg)
            .add_modifier(Modifier::BOLD),
        badge: Style::new()
            .fg(contrast(p))
            .bg(if app.view.mode == Mode::Resize {
                p.mauve
            } else {
                p.accent
            })
            .add_modifier(Modifier::BOLD),
    };
    buf.set_style(bar, styles.base);
    for x in bar.left()..bar.right() {
        buf[(x, bar.y)].set_symbol(" ");
    }
    if app.error.is_none()
        && app.view.mode == Mode::Copy
        && app.view.copy.as_ref().is_some_and(|c| c.prompt.is_some())
    {
        prompt(app, buf, bar, &styles);
        return;
    }
    let mut x = bar.x;
    for (text, style) in segments(app, &styles) {
        if x >= bar.right() {
            break;
        }
        let room = bar.right() - x;
        buf.set_stringn(x, bar.y, &text, usize::from(room), style);
        x += width(&text).min(room);
    }
}

fn segments(app: &App, styles: &Styles) -> Vec<(String, Style)> {
    let Styles { base, key, badge } = *styles;
    if let Some((error, _)) = &app.error {
        return vec![(" ERROR ".into(), badge), (format!(" {error}"), base)];
    }
    match app.view.mode {
        Mode::Terminal => Vec::new(),
        Mode::Prefix => vec![
            (" PREFIX ".into(), badge),
            (" ".into(), base),
            ("esc".into(), key),
            (" cancel  ".into(), base),
            (app.keymap.prefix_label(), key),
            (" send prefix  ".into(), base),
            (rhs(app, Action::WorkspacePicker), key),
            (" workspace nav  ".into(), base),
            (rhs(app, Action::Help), key),
            (" keybinds".into(), base),
        ],
        Mode::Navigate => vec![
            (" NAVIGATE ".into(), badge),
            (" esc back  ".into(), base),
            ("↑/↓".into(), key),
            (" workspace  ".into(), base),
            ("tab".into(), key),
            (" pane  ".into(), base),
            (rhs(app, Action::Help), key),
            (" keybinds".into(), base),
        ],
        Mode::Resize => vec![
            (" RESIZE ".into(), badge),
            ("  ".into(), base),
            ("h/l".into(), key),
            (" width  ".into(), base),
            ("j/k".into(), key),
            (" height  ".into(), base),
            ("esc".into(), key),
            (" done".into(), base),
        ],
        Mode::Copy => copy_segments(app, styles),
    }
}

fn copy_segments(app: &App, styles: &Styles) -> Vec<(String, Style)> {
    let Styles { base, key, badge } = *styles;
    let Some(copy) = &app.view.copy else {
        return Vec::new();
    };
    let search = copy.mode.search();
    let status = search
        .map(|s| match s.current {
            Some(_) => format!(" {}/{}", s.index + 1, s.total),
            None => " 0/0".to_owned(),
        })
        .unwrap_or_default();
    let selecting = copy.mode.is_selecting();
    let select = if selecting { "selecting" } else { "select" };
    let (exit_keys, exit_label) = if search.is_none() && !selecting {
        ("q/esc", " exit")
    } else {
        ("esc", " clear  q exit")
    };
    vec![
        (" COPY ".into(), badge),
        (" ".into(), base),
        ("h/j/k/l w/b/e { }".into(), key),
        (" move  ".into(), base),
        ("/ ?".into(), key),
        (" search  ".into(), base),
        ("n/N".into(), key),
        (format!(" repeat{status}  "), base),
        ("v/space".into(), key),
        (format!(" {select}  "), base),
        ("y/enter".into(), key),
        (" copy  ".into(), base),
        (exit_keys.into(), key),
        (exit_label.into(), base),
    ]
}

fn prompt(app: &App, buf: &mut Buffer, bar: Rect, styles: &Styles) {
    let Styles { base, key, badge } = *styles;
    let Some(prompt) = app.view.copy.as_ref().and_then(|c| c.prompt.as_ref()) else {
        return;
    };
    let p = &app.palette;
    buf.set_stringn(bar.x, bar.y, " COPY ", usize::from(bar.width), badge);
    let lead = 8.min(bar.width);
    if bar.width >= 8 {
        let marker = match prompt.direction {
            vt::Direction::Forward => "/",
            vt::Direction::Backward => "?",
        };
        buf.set_string(bar.x + 7, bar.y, marker, key);
    }
    let footer = "  enter search  esc cancel";
    let footer_width = if bar.width >= 50 { width(footer) } else { 0 };
    let field = Rect::new(
        bar.x + lead,
        bar.y,
        bar.width.saturating_sub(lead + footer_width),
        1,
    );
    if let Some((cx, cy)) = prompt
        .field
        .render(field, buf, Style::new().fg(p.text).bg(p.panel_bg))
    {
        buf[(cx, cy)].set_style(Style::new().fg(p.panel_bg).bg(p.text));
    }
    if footer_width > 0 {
        buf.set_string(bar.right() - footer_width, bar.y, footer, base);
    }
}
