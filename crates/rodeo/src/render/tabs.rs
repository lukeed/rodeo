use config::TabBarEntry;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::app::App;
use crate::hits::Hits;
use crate::model::{Snapshot, TabRow};
use crate::overlay::frame::contrast;
use crate::text::width;

const MIN_TAB: u16 = 8;
const NEW_TAB: u16 = 3;
const BUTTON: u16 = 3;
const MIN_STRIP: u16 = 17;

fn label(tab: &TabRow) -> String {
    if tab.zoomed {
        format!("{} Z", tab.label)
    } else {
        tab.label.clone()
    }
}

fn tab_width(tab: &TabRow) -> u16 {
    (width(&label(tab)) + 4).max(MIN_TAB)
}

pub fn status_segments(app: &App, snapshot: &Snapshot) -> Vec<(String, bool)> {
    let zoomed = snapshot.tabs.iter().any(|t| t.focused && t.zoomed);
    let machine = app.active_machine();
    app.config
        .ui
        .tab_bar_right
        .iter()
        .enumerate()
        .filter(|(_, e)| e.is_shown())
        .take(16)
        .filter_map(|(i, entry)| {
            let text = match entry {
                TabBarEntry::Zoom => return zoomed.then(|| ("ZOOM".to_owned(), true)),
                TabBarEntry::Hostname => machine
                    .as_deref()
                    .and_then(|m| app.machines.get(Some(m)))
                    .and_then(|m| m.info.as_ref().map(|i| i.host.clone()))
                    .unwrap_or_else(|| app.hostname.clone()),
                TabBarEntry::Datetime { format } => datetime(format),
                TabBarEntry::Text { text } => text.clone(),
                TabBarEntry::Command { .. } => app
                    .status
                    .commands
                    .get(&i)
                    .map(|c| c.0.clone())
                    .unwrap_or_default(),
            };
            let text = crate::app::strip_controls(&text);
            (!text.is_empty()).then_some((text, false))
        })
        .collect()
}

fn datetime(format: &str) -> String {
    let Ok(items) = config::parse_datetime_format(format) else {
        return String::new();
    };
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    now.format(&items).unwrap_or_default()
}

pub fn render(app: &mut App, snapshot: &Snapshot, buf: &mut Buffer, area: Rect, hits: &mut Hits) {
    buf.set_style(area, Style::new().bg(app.palette.panel_bg));
    hits.tabs.row = Some(area);
    let mut content = area;
    content.width -= status_area(app, snapshot, buf, area);
    let widths: Vec<u16> = snapshot.tabs.iter().map(tab_width).collect();
    let mouse = app.config.ui.mouse_capture;
    let gaps = u16::try_from(widths.len().saturating_sub(1)).unwrap_or(0);
    let desired = widths.iter().sum::<u16>() + gaps + if mouse { NEW_TAB } else { 0 };
    if desired > content.width && (!mouse || content.width >= MIN_STRIP) {
        overflowing(app, snapshot, buf, content, &widths, hits);
    } else {
        fitting(app, snapshot, buf, content, &widths, hits);
    }
    app.view.reveal_tab = false;
    drop_indicator(app, buf, hits);
}

fn drop_indicator(app: &App, buf: &mut Buffer, hits: &Hits) {
    let Some(crate::view::Drag::Press {
        target: crate::view::Press::Tab(dragged),
        at,
        now,
    }) = &app.view.drag
    else {
        return;
    };
    if at.0.abs_diff(now.0).max(at.1.abs_diff(now.1)) < 1 {
        return;
    }
    let Some((rect, key)) = hits
        .tabs
        .tabs
        .iter()
        .find(|(r, _)| crate::hits::inside(*r, now.0, now.1))
    else {
        return;
    };
    if key == dragged {
        return;
    }
    let row = hits.tabs.row.unwrap_or(*rect);
    let x = if now.0 >= at.0 {
        rect.right()
    } else {
        rect.x.saturating_sub(1).max(row.x)
    };
    if buf.area.contains((x, rect.y).into()) {
        buf[(x, rect.y)]
            .set_symbol("│")
            .set_style(Style::new().fg(app.palette.accent).bg(app.palette.panel_bg));
    }
}

fn status_area(app: &App, snapshot: &Snapshot, buf: &mut Buffer, area: Rect) -> u16 {
    let p = &app.palette;
    let segments = status_segments(app, snapshot);
    if segments.is_empty() {
        return 0;
    }
    let separator: String = app
        .config
        .ui
        .tab_bar_right_separator
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    let total: u16 = segments.iter().map(|(t, _)| width(t)).sum::<u16>()
        + width(&separator) * u16::try_from(segments.len() - 1).unwrap_or(0);
    if area.width.saturating_sub(total + 1) < MIN_STRIP {
        return 0;
    }
    let mut x = area.right() - total;
    for (i, (text, accent)) in segments.iter().enumerate() {
        if i > 0 {
            let style = Style::new().fg(p.overlay0).bg(p.panel_bg);
            buf.set_string(x, area.y, &separator, style);
            x += width(&separator);
        }
        let style = if *accent {
            Style::new()
                .fg(contrast(p))
                .bg(p.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(p.overlay1).bg(p.panel_bg)
        };
        buf.set_string(x, area.y, text, style);
        x += width(text);
    }
    total + 1
}

fn new_tab_button(app: &App, buf: &mut Buffer, rect: Rect, hits: &mut Hits) {
    let style = Style::new()
        .fg(app.palette.overlay1)
        .bg(app.palette.panel_bg);
    buf.set_string(rect.x, rect.y, " + ", style);
    hits.tabs.new_tab = Some(rect);
}

fn fitting(
    app: &mut App,
    snapshot: &Snapshot,
    buf: &mut Buffer,
    content: Rect,
    widths: &[u16],
    hits: &mut Hits,
) {
    app.view.tab_scroll = 0;
    let mut x = content.x;
    for (tab, w) in snapshot.tabs.iter().zip(widths) {
        if x >= content.right() {
            break;
        }
        let rect = Rect::new(x, content.y, (*w).min(content.right() - x), 1);
        draw_tab(buf, &app.palette, tab, rect);
        hits.tabs.tabs.push((rect, tab.key.clone()));
        x += rect.width + 1;
    }
    if app.config.ui.mouse_capture && x + NEW_TAB <= content.right() + 1 {
        let x = x.min(content.right().saturating_sub(NEW_TAB));
        new_tab_button(app, buf, Rect::new(x, content.y, NEW_TAB, 1), hits);
    }
}

fn overflowing(
    app: &mut App,
    snapshot: &Snapshot,
    buf: &mut Buffer,
    content: Rect,
    widths: &[u16],
    hits: &mut Hits,
) {
    let mouse = app.config.ui.mouse_capture;
    let tabs = &snapshot.tabs;
    let strip = if mouse {
        let room = content.width.saturating_sub(BUTTON * 2 + NEW_TAB);
        Rect::new(content.x + BUTTON, content.y, room, 1)
    } else {
        content
    };
    let most = max_scroll(widths, strip.width);
    let mut first = app.view.tab_scroll;
    if app.view.reveal_tab {
        let focused = tabs.iter().position(|t| t.focused).unwrap_or(0);
        first = reveal(widths, focused, strip.width);
    }
    first = first.min(most);
    app.view.tab_scroll = first;
    let mut x = strip.x;
    let mut last = first;
    for (i, (tab, w)) in tabs.iter().zip(widths).enumerate().skip(first) {
        if x >= strip.right() {
            break;
        }
        let rect = Rect::new(x, strip.y, (*w).min(strip.right() - x), 1);
        draw_tab(buf, &app.palette, tab, rect);
        hits.tabs.tabs.push((rect, tab.key.clone()));
        last = i;
        x += rect.width + 1;
        if rect.width < *w {
            break;
        }
    }
    let p = app.palette;
    let ellipsis = Style::new().fg(p.overlay0);
    if first > 0 {
        buf[(strip.x, strip.y)].set_symbol("…").set_style(ellipsis);
    }
    if (last + 1 < tabs.len() || x > strip.right() + 1) && strip.width > 0 {
        buf[(strip.right() - 1, strip.y)]
            .set_symbol("…")
            .set_style(ellipsis);
    }
    if !mouse {
        return;
    }
    let can = |yes: bool| {
        Style::new()
            .fg(if yes { p.overlay1 } else { p.overlay0 })
            .bg(p.surface0)
    };
    let left = Rect::new(content.x, content.y, BUTTON, 1);
    let right = Rect::new(
        content.right().saturating_sub(BUTTON + NEW_TAB),
        content.y,
        BUTTON,
        1,
    );
    buf.set_string(left.x, left.y, " < ", can(first > 0));
    buf.set_string(right.x, right.y, " > ", can(first < most));
    hits.tabs.scroll_left = Some(left);
    hits.tabs.scroll_right = Some(right);
    let plus = Rect::new(
        content.right().saturating_sub(NEW_TAB),
        content.y,
        NEW_TAB,
        1,
    );
    new_tab_button(app, buf, plus, hits);
}

fn draw_tab(buf: &mut Buffer, p: &config::Palette, tab: &TabRow, rect: Rect) {
    let style = match (tab.focused, tab.custom) {
        (true, false) => Style::new().fg(contrast(p)).bg(p.accent),
        (true, true) => Style::new()
            .fg(contrast(p))
            .bg(p.accent)
            .add_modifier(Modifier::BOLD),
        (false, true) => Style::new().fg(p.overlay1).bg(p.surface0),
        (false, false) => Style::new().fg(p.overlay0).bg(p.surface0),
    };
    buf.set_style(rect, style);
    let text = label(tab);
    let left = rect.width.saturating_sub(width(&text)) / 2;
    if left < rect.width {
        buf.set_stringn(
            rect.x + left,
            rect.y,
            &text,
            usize::from(rect.width - left),
            style,
        );
    }
}

fn max_scroll(widths: &[u16], room: u16) -> usize {
    let mut used = 0u16;
    for (i, w) in widths.iter().enumerate().rev() {
        let next = used + w + u16::from(used > 0);
        if next > room {
            return (i + 1).min(widths.len().saturating_sub(1));
        }
        used = next;
    }
    0
}

fn reveal(widths: &[u16], focused: usize, room: u16) -> usize {
    let Some(&own) = widths.get(focused) else {
        return 0;
    };
    let mut best = (u16::MAX, focused);
    for first in 0..=focused {
        let before: u16 = widths[first..focused]
            .iter()
            .fold(0u16, |sum, w| sum.saturating_add(w + 1));
        if before >= room {
            continue;
        }
        let center = before
            .saturating_mul(2)
            .saturating_add(own.min(room - before));
        let distance = center.abs_diff(room);
        if distance <= best.0 {
            best = (distance, first);
        }
    }
    best.1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_scrolls_to_reveal_the_focused_tab() {
        let widths = [8, 8, 8, 8, 8, 8];
        assert_eq!(max_scroll(&widths, 26), 3);
        assert_eq!(reveal(&widths, 5, 26), 4);
        assert_eq!(reveal(&widths, 0, 26), 0);
        let mixed = [8, 8, 17, 17, 17, 17, 17, 17];
        assert_eq!(reveal(&mixed, 4, 85), 2);
        assert_eq!(max_scroll(&widths, 100), 0);
    }

    #[test]
    fn a_partial_tab_centers_its_label_in_the_cells_left() {
        let tab = TabRow {
            key: "tb1".into(),
            label: "longer-name-6".into(),
            custom: true,
            zoomed: false,
            focused: false,
            state: None,
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 16, 1));
        draw_tab(
            &mut buf,
            &config::Config::default().palette(None),
            &tab,
            Rect::new(0, 0, 15, 1),
        );
        let text: String = (0..16).map(|x| buf[(x, 0)].symbol().to_owned()).collect();
        assert_eq!(text, " longer-name-6  ");
    }
}
