use doc::Content;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Borders;
use vt::{Highlight, Point, RenderOptions};

use super::Cursor;
use super::mods;
use crate::app::App;
use crate::hits::{Hits, PaneHit, SplitHit};
use crate::ids::qualify;
use crate::layout::{self, PaneGeom};
use crate::overlay::frame::contrast;
use crate::view::Mode;

pub fn render(app: &App, buf: &mut Buffer, hits: &mut Hits) -> Option<Cursor> {
    let tab_key = app.view.active_tab(&app.machines)?;
    let geom = app.geoms.get(&tab_key)?;
    let found = app.machines.tab(&tab_key)?;
    let machine = found.machine;
    let focused_key = app.view.focused_in(&app.machines, &tab_key);
    let focused_id = focused_key
        .as_deref()
        .map(|k| crate::ids::local(k, machine).to_owned());
    let p = &app.palette;
    let gaps = app.config.ui.pane_gaps;
    let focused_rect = geom
        .panes
        .iter()
        .find(|g| Some(&g.id) == focused_id.as_ref())
        .map(|g| g.rect);
    if app.config.ui.pane_borders.draws_borders() {
        for ((x, y), line) in layout::border_lines(geom, gaps) {
            let symbol = line.symbol();
            if symbol.is_empty() || !buf.area.contains((x, y).into()) {
                continue;
            }
            let lit = focused_rect.is_some_and(|r| layout::touches(x, y, r, gaps));
            let color = if lit { p.accent } else { p.overlay0 };
            buf[(x, y)]
                .set_symbol(symbol)
                .set_style(Style::new().fg(color));
        }
    }
    let mut cursor = None;
    for pane in &geom.panes {
        let Some(node) = found.tab.root.find(&pane.id) else {
            continue;
        };
        let focused = Some(&pane.id) == focused_id.as_ref();
        let address = node.address().map(|a| qualify(a, machine));
        title(
            app,
            buf,
            pane,
            node.title.as_deref(),
            address.as_deref(),
            focused,
        );
        match node.content() {
            Some(Content::Terminal { .. }) => {
                if let Some(c) = terminal(
                    app,
                    buf,
                    pane,
                    address.as_deref().unwrap_or_default(),
                    focused,
                    &mut hits.covers,
                ) {
                    cursor = Some(c);
                }
            }
            Some(Content::Mod { name, .. }) => {
                mods::render(app, buf, pane.inner, name, &qualify(&pane.id, machine));
            }
            _ => mods::placeholder(app, buf, pane.inner, "layout block"),
        }
        hits.panes.push(PaneHit {
            key: qualify(&pane.id, machine),
            address,
            rect: pane.rect,
            inner: pane.inner,
            content: pane.content,
            gutter: pane.gutter,
        });
    }
    for split in &geom.splits {
        if let Some(hit) = split.hit {
            hits.splits.push(SplitHit {
                machine: machine.map(str::to_owned),
                tab: tab_key.clone(),
                container: split.container.clone(),
                index: split.index,
                axis: split.axis,
                hit,
                pos: split.pos,
                origin: split.origin(),
            });
        }
    }
    cursor
}

fn title(
    app: &App,
    buf: &mut Buffer,
    pane: &PaneGeom,
    manual: Option<&str>,
    address: Option<&str>,
    focused: bool,
) {
    if !pane.borders.contains(Borders::TOP) || pane.rect.width <= 4 {
        return;
    }
    let agent = address
        .filter(|_| app.config.ui.show_agent_labels_on_pane_borders)
        .and_then(|a| app.terminals.get(a))
        .and_then(crate::agents::label);
    let Some(label) = manual
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .or(agent)
    else {
        return;
    };
    let p = &app.palette;
    let text = format!(
        " {} ",
        crate::text::truncate(&label, usize::from(pane.rect.width - 4))
    );
    let style = if focused {
        Style::new().fg(p.accent).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(p.overlay0)
    };
    let start = pane.rect.x + 1;
    let end = pane.rect.right() - 1;
    buf.set_stringn(start, pane.rect.y, text, usize::from(end - start), style);
}

pub fn selection_style(app: &App) -> Style {
    let base = app
        .host
        .bg
        .map(|c| (c.0, c.1, c.2))
        .or(match contrast(&app.palette) {
            Color::Rgb(r, g, b) => Some((r, g, b)),
            _ => None,
        });
    let Some((r, g, b)) = base else {
        return Style::reset().add_modifier(Modifier::REVERSED);
    };
    let toward = if luminance((r, g, b)) < 0.5 {
        255.0
    } else {
        0.0
    };
    let mix = |c: u8| {
        let c = f32::from(c);
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the mix stays in 0..=255"
        )]
        let mixed = (c + (toward - c) * 0.28).round() as u8;
        mixed
    };
    let (r, g, b) = (mix(r), mix(g), mix(b));
    let l = luminance((r, g, b));
    let fg = if (l + 0.05) / 0.05 > 1.05 / (l + 0.05) {
        Color::Rgb(0, 0, 0)
    } else {
        Color::Rgb(255, 255, 255)
    };
    Style::reset().fg(fg).bg(Color::Rgb(r, g, b))
}

fn luminance((r, g, b): (u8, u8, u8)) -> f32 {
    let channel = |c: u8| {
        let c = f32::from(c) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

fn terminal(
    app: &App,
    buf: &mut Buffer,
    pane: &PaneGeom,
    address: &str,
    focused: bool,
    covers: &mut Vec<Rect>,
) -> Option<Cursor> {
    let entry = app.panes.get(address)?;
    let term = &entry.term;
    let p = &app.palette;
    let mut highlights = Vec::new();
    let copy = app.view.copy.as_ref().filter(|c| c.address == address);
    if let Some(copy) = copy {
        if let Some(search) = copy.mode.search() {
            for found in term.visible_matches(&search.query) {
                highlights.push(Highlight {
                    start: found.start,
                    end: found.end,
                    style: Style::new().fg(p.text).bg(p.surface1),
                });
            }
        }
        if let Some(selection) = copy.mode.selection(term) {
            let (start, end) = term.selection_bounds(&selection);
            highlights.push(Highlight {
                start,
                end,
                style: selection_style(app),
            });
        }
        if let Some(current) = copy.mode.search().and_then(|s| s.current) {
            highlights.push(Highlight {
                start: current.start,
                end: current.end,
                style: Style::new()
                    .fg(contrast(p))
                    .bg(p.accent)
                    .add_modifier(Modifier::BOLD),
            });
        }
        let at: Point = copy.mode.cursor();
        highlights.push(Highlight {
            start: at,
            end: at,
            style: Style::new()
                .fg(contrast(p))
                .bg(p.accent)
                .add_modifier(Modifier::BOLD),
        });
    } else if let Some(selection) = app
        .view
        .selection
        .as_ref()
        .filter(|s| s.address == address && s.visible)
    {
        let (start, end) = term.selection_bounds(&selection.selection);
        highlights.push(Highlight {
            start,
            end,
            style: selection_style(app),
        });
    }
    highlights.extend(app.hover_highlight(address));
    covers.extend(crate::graphics::highlight_covers(
        &highlights,
        term.viewport_top(),
        term.size(),
        pane.content.intersection(buf.area),
    ));
    let rendered = term.render(
        pane.content,
        buf,
        &RenderOptions {
            highlights: &highlights,
        },
    );
    if let Some(gutter) = pane.gutter {
        scrollbar(app, buf, gutter, term.scrollbar(), focused);
    }
    let visible = focused
        && copy.is_none()
        && app.view.mode != Mode::Copy
        && app.view.overlay.is_none()
        && !term.is_scrolled_back();
    let drawn = rendered.cursor.map(|c| Cursor {
        x: c.x,
        y: c.y,
        shape: c.shape,
        blinking: c.blinking,
    });
    app.ime_cursor(address, pane.content, drawn)
        .filter(|_| visible)
}

fn scrollbar(app: &App, buf: &mut Buffer, gutter: Rect, bar: vt::Scrollbar, focused: bool) {
    let max = bar.max_offset();
    if max == 0 || gutter.height == 0 {
        return;
    }
    let p = &app.palette;
    let track_h = u64::from(gutter.height);
    let total = max + bar.len;
    let len = ((bar.len * track_h + total / 2) / total).clamp(1, track_h);
    let top = (bar.offset * (track_h - len) + max / 2) / max;
    let (track, thumb, thumb_symbol) = if focused {
        (p.overlay0, p.overlay1, "▐")
    } else {
        (p.surface_dim, p.overlay0, "▕")
    };
    for i in 0..track_h {
        let y = gutter.y + u16::try_from(i).unwrap_or(0);
        let on = i >= top && i < top + len;
        let (symbol, color) = if on {
            (thumb_symbol, thumb)
        } else {
            ("▕", track)
        };
        buf[(gutter.x, y)]
            .set_symbol(symbol)
            .set_style(Style::new().fg(color));
    }
}
