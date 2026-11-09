use std::process::Stdio;

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::style::{Modifier, Style};
use vt::{Highlight, Point};

use crate::app::App;
use crate::hits::inside;
use crate::view::Mode;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Links {
    pub hover: Option<Hover>,
    pub consume: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hover {
    pub address: String,
    pub col: u16,
    pub row: u16,
}

pub fn is_safe(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

pub fn link_at(term: &vt::Terminal, point: Point) -> Option<vt::Link> {
    match term.hyperlink_at(point) {
        Some(link) => is_safe(&link.uri).then_some(link),
        None => term.url_at(point).filter(|l| is_safe(&l.uri)),
    }
}

pub fn opener() -> &'static str {
    if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    }
}

impl App {
    fn link_allowed(&self) -> bool {
        self.config.ui.mouse_capture
            && self.view.mode == Mode::Terminal
            && self.view.overlay.is_none()
            && self.popup.is_none()
            && self.view.host_focused
    }

    fn link_point(&self, event: MouseEvent) -> Option<(String, u16, u16)> {
        let hit = self.hits.pane_at(event.column, event.row)?;
        if !inside(hit.content, event.column, event.row) {
            return None;
        }
        Some((
            hit.address.clone()?,
            event.column - hit.content.x,
            event.row - hit.content.y,
        ))
    }

    pub fn update_hover(&mut self, event: MouseEvent) {
        let wanted = event.kind == MouseEventKind::Moved
            && event.modifiers.contains(KeyModifiers::CONTROL)
            && self.link_allowed()
            && self.view.drag.is_none();
        self.links.hover = wanted
            .then(|| self.link_point(event))
            .flatten()
            .map(|(address, col, row)| Hover { address, col, row });
    }

    pub fn link_click(&mut self, event: MouseEvent) -> bool {
        if self.links.consume {
            match event.kind {
                MouseEventKind::Drag(MouseButton::Left) => return true,
                MouseEventKind::Up(MouseButton::Left) => {
                    self.links.consume = false;
                    return true;
                }
                MouseEventKind::Down(MouseButton::Left) => self.links.consume = false,
                _ => {}
            }
        }
        if event.kind != MouseEventKind::Down(MouseButton::Left)
            || !event.modifiers.contains(KeyModifiers::CONTROL)
            || !self.link_allowed()
        {
            return false;
        }
        let Some((address, col, row)) = self.link_point(event) else {
            return false;
        };
        let Some(link) = self.panes.get(&address).and_then(|pane| {
            let point = pane.term.viewport_point(col, row);
            link_at(&pane.term, point)
        }) else {
            return false;
        };
        self.links.consume = true;
        self.links.hover = None;
        self.open_url(&link.uri);
        true
    }

    pub fn open_url(&self, url: &str) {
        let spawned = self
            .env
            .command(opener())
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(false)
            .spawn();
        match spawned {
            Ok(mut child) => {
                tokio::task::spawn_local(async move {
                    let _ = child.wait().await;
                });
            }
            Err(error) => tracing::warn!(%error, url, "cannot open the link"),
        }
    }

    pub fn hover_highlight(&self, address: &str) -> Option<Highlight> {
        let hover = self.links.hover.as_ref().filter(|h| h.address == address)?;
        if !self.link_allowed() {
            return None;
        }
        let term = &self.panes.get(address)?.term;
        let link = link_at(term, term.viewport_point(hover.col, hover.row))?;
        Some(Highlight {
            start: link.start,
            end: link.end,
            style: Style::new().add_modifier(Modifier::UNDERLINED),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term(bytes: &[u8]) -> vt::Terminal {
        let mut term = vt::Terminal::new(40, 4, vt::Options::default()).expect("a vt");
        term.feed(bytes);
        term
    }

    #[test]
    fn finds_osc8_and_plain_urls() {
        let t = term(b"go \x1b]8;;https://a.dev/x\x07here\x1b]8;;\x07 or https://b.dev/y now");
        let osc = link_at(&t, Point::new(t.viewport_top(), 4)).expect("an OSC 8 link");
        assert_eq!(osc.uri, "https://a.dev/x");
        assert_eq!((osc.start.col, osc.end.col), (3, 6));
        let plain = link_at(&t, Point::new(t.viewport_top(), 15)).expect("a plain url");
        assert_eq!(plain.uri, "https://b.dev/y");
        assert!(link_at(&t, Point::new(t.viewport_top(), 0)).is_none());
    }

    #[test]
    fn unsafe_osc8_targets_are_not_links() {
        let t = term(b"\x1b]8;;file:///etc/passwd\x07open\x1b]8;;\x07");
        assert!(link_at(&t, Point::new(t.viewport_top(), 1)).is_none());
        assert!(is_safe("http://x") && !is_safe("javascript:x"));
    }
}
