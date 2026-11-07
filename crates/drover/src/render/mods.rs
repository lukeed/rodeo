use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::app::App;
use crate::text::{truncate, width};

pub fn render(app: &App, buf: &mut Buffer, area: Rect, name: &str, pane: &str) {
    if name == "files" {
        files(app, buf, area, pane);
    } else {
        placeholder(app, buf, area, &format!("{name} mod"));
    }
}

pub fn placeholder(app: &App, buf: &mut Buffer, area: Rect, label: &str) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let p = &app.palette;
    let text = truncate(label, usize::from(area.width));
    let x = area.x + area.width.saturating_sub(width(&text)) / 2;
    let y = area.y + area.height / 2;
    buf.set_string(x, y, &text, Style::new().fg(p.overlay0));
    if area.height > 2 {
        let note = truncate("shown in the Herdr app", usize::from(area.width));
        let x = area.x + area.width.saturating_sub(width(&note)) / 2;
        buf.set_string(
            x,
            y + 1,
            note,
            Style::new().fg(p.overlay0).add_modifier(Modifier::DIM),
        );
    }
}

fn files(app: &App, buf: &mut Buffer, area: Rect, pane: &str) {
    if area.width < 2 || area.height == 0 {
        return;
    }
    let p = &app.palette;
    let Some(target) = app.files_dir(pane) else {
        return;
    };
    let home = app.home();
    let title = if target.1 == home {
        "~".to_owned()
    } else {
        target
            .1
            .replacen(&home, "~", usize::from(target.1.starts_with(&home)))
    };
    let right = usize::from(area.width - 1);
    buf.set_stringn(
        area.x + 1,
        area.y,
        truncate(&title, right),
        right,
        Style::new().fg(p.text).add_modifier(Modifier::BOLD),
    );
    let Some(listing) = app.files.map.get(&target) else {
        return;
    };
    if let Some(error) = &listing.error {
        buf.set_stringn(
            area.x + 1,
            area.y + 1,
            truncate(error, right),
            right,
            Style::new().fg(p.red),
        );
        return;
    }
    let rows = usize::from(area.height.saturating_sub(1));
    let count = listing.entries.len();
    for (i, entry) in listing.entries.iter().take(rows).enumerate() {
        let last = i + 1 == count || i + 1 == rows && count > rows;
        let branch = if last { "└─ " } else { "├─ " };
        let shown = if i + 1 == rows && count > rows {
            format!("… {} more", count - i)
        } else {
            entry.clone()
        };
        let y = area.y + 1 + u16::try_from(i).unwrap_or(0);
        let x = area.x + 1;
        buf.set_stringn(x, y, branch, right, Style::new().fg(p.overlay0));
        let style = if entry.ends_with('/') {
            Style::new().fg(p.blue)
        } else {
            Style::new().fg(p.subtext0)
        };
        let room = right.saturating_sub(3);
        buf.set_stringn(x + 3, y, truncate(&shown, room), room, style);
    }
}
