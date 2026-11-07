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
    let now = config::time::OffsetDateTime::now_local()
        .unwrap_or_else(|_| config::time::OffsetDateTime::now_utc());
    now.format(&items).unwrap_or_default()
}

pub fn render(app: &mut App, snapshot: &Snapshot, buf: &mut Buffer, area: Rect, hits: &mut Hits) {
    let p = app.palette;
    buf.set_style(area, Style::new().bg(p.panel_bg));
    hits.tabs.row = Some(area);
    let mouse = app.config.ui.mouse_capture;
    let mut content = area;
    let segments = status_segments(app, snapshot);
    if !segments.is_empty() {
        let separator: String = app
            .config
            .ui
            .tab_bar_right_separator
            .chars()
            .filter(|c| !c.is_control())
            .collect();
        let total: u16 = segments.iter().map(|(t, _)| width(t)).sum::<u16>()
            + width(&separator) * u16::try_from(segments.len() - 1).unwrap_or(0);
        if area.width.saturating_sub(total + 1) >= MIN_STRIP {
            let mut x = area.right() - total;
            for (i, (text, accent)) in segments.iter().enumerate() {
                if i > 0 {
                    buf.set_string(
                        x,
                        area.y,
                        &separator,
                        Style::new().fg(p.overlay0).bg(p.panel_bg),
                    );
                    x += width(&separator);
                }
                let style = if *accent {
                    Style::new()
                        .fg(contrast(&p))
                        .bg(p.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::new().fg(p.overlay1).bg(p.panel_bg)
                };
                buf.set_string(x, area.y, text, style);
                x += width(text);
            }
            content.width -= total + 1;
        }
    }
    let tabs = &snapshot.tabs;
    let widths: Vec<u16> = tabs.iter().map(tab_width).collect();
    let gaps = u16::try_from(tabs.len().saturating_sub(1)).unwrap_or(0);
    let desired = widths.iter().sum::<u16>() + gaps + if mouse { NEW_TAB } else { 0 };
    let overflow = desired > content.width && (!mouse || content.width >= MIN_STRIP);
    if !overflow {
        app.view.tab_scroll = 0;
        let mut x = content.x;
        for (tab, w) in tabs.iter().zip(&widths) {
            if x >= content.right() {
                break;
            }
            let shown = (*w).min(content.right() - x);
            draw_tab(buf, &p, tab, Rect::new(x, area.y, shown, 1));
            hits.tabs
                .tabs
                .push((Rect::new(x, area.y, shown, 1), tab.key.clone()));
            x += shown + 1;
        }
        if mouse && x + NEW_TAB <= content.right() + 1 {
            let x = x.min(content.right().saturating_sub(NEW_TAB));
            let rect = Rect::new(x, area.y, NEW_TAB, 1);
            buf.set_string(x, area.y, " + ", Style::new().fg(p.overlay1).bg(p.panel_bg));
            hits.tabs.new_tab = Some(rect);
        }
        app.view.reveal_tab = false;
        return;
    }
    let strip = if mouse {
        Rect::new(
            content.x + BUTTON,
            area.y,
            content.width.saturating_sub(BUTTON * 2 + NEW_TAB),
            1,
        )
    } else {
        content
    };
    let focused = tabs.iter().position(|t| t.focused).unwrap_or(0);
    let max_scroll = max_scroll(&widths, strip.width);
    let mut first = app.view.tab_scroll;
    if app.view.reveal_tab {
        first = reveal(&widths, focused, strip.width);
        app.view.reveal_tab = false;
    }
    first = first.min(max_scroll);
    app.view.tab_scroll = first;
    let mut x = strip.x;
    let mut last = first;
    for (i, (tab, w)) in tabs.iter().zip(&widths).enumerate().skip(first) {
        if x >= strip.right() {
            break;
        }
        let shown = (*w).min(strip.right() - x);
        draw_tab(buf, &p, tab, Rect::new(x, area.y, shown, 1));
        hits.tabs
            .tabs
            .push((Rect::new(x, area.y, shown, 1), tab.key.clone()));
        last = i;
        x += shown + 1;
        if shown < *w {
            break;
        }
    }
    let ellipsis = Style::new().fg(p.overlay0);
    if first > 0 {
        buf[(strip.x, area.y)].set_symbol("…").set_style(ellipsis);
    }
    let hidden_right = last + 1 < tabs.len() || x > strip.right() + 1;
    if hidden_right && strip.width > 0 {
        buf[(strip.right() - 1, area.y)]
            .set_symbol("…")
            .set_style(ellipsis);
    }
    if mouse {
        let left = Rect::new(content.x, area.y, BUTTON, 1);
        let right = Rect::new(
            content.right().saturating_sub(BUTTON + NEW_TAB),
            area.y,
            BUTTON,
            1,
        );
        let can = |yes: bool| {
            Style::new()
                .fg(if yes { p.overlay1 } else { p.overlay0 })
                .bg(p.surface0)
        };
        buf.set_string(left.x, area.y, " < ", can(first > 0));
        buf.set_string(right.x, area.y, " > ", can(first < max_scroll));
        hits.tabs.scroll_left = Some(left);
        hits.tabs.scroll_right = Some(right);
        let plus = Rect::new(content.right().saturating_sub(NEW_TAB), area.y, NEW_TAB, 1);
        buf.set_string(
            plus.x,
            area.y,
            " + ",
            Style::new().fg(p.overlay1).bg(p.panel_bg),
        );
        hits.tabs.new_tab = Some(plus);
    }
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
    let full = tab_width(tab);
    let left = (full - width(&text)) / 2;
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
    let w = widths.get(focused).copied().unwrap_or(0);
    let mut budget = room.saturating_sub(w) / 2;
    let mut first = focused;
    while first > 0 {
        let prev = widths[first - 1] + 1;
        if prev > budget {
            break;
        }
        budget -= prev;
        first -= 1;
    }
    first
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
        assert_eq!(max_scroll(&widths, 100), 0);
    }
}
