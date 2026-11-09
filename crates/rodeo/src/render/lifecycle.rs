use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::app::App;
use crate::model::MachineStatus;
use crate::sidebar::status_look;
use crate::text::{truncate, width};

pub fn render(app: &App, buf: &mut Buffer, area: Rect, top: u16) -> Option<Rect> {
    let machine = app
        .active_machine_row()
        .filter(|m| m.status != MachineStatus::Online)?;
    if area.is_empty() {
        return None;
    }
    let (symbol, state, color) = status_look(machine.status, &app.palette);
    let text = format!("{symbol} {} · {state}", machine.label);
    let w = (width(&text) + 2).min(area.width);
    let y = (area.y + top).min(area.bottom() - 1);
    let rect = Rect::new(area.right() - w, y, w, 1);
    let style = Style::new().fg(color).bg(app.palette.surface0);
    buf.set_style(rect, style);
    for x in rect.left()..rect.right() {
        buf[(x, y)].set_symbol(" ");
    }
    let room = usize::from(w.saturating_sub(2));
    buf.set_string(rect.x + 1, y, truncate(&text, room), style);
    Some(rect)
}
