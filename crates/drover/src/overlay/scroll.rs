use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Bar {
    pub track: Rect,
    pub max: usize,
    pub viewport: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thumb {
    pub top: u16,
    pub len: u16,
}

fn round_div(num: usize, den: usize) -> usize {
    (num * 2 + den) / (den * 2)
}

impl Bar {
    pub fn new(track: Rect, max: usize, viewport: usize) -> Self {
        Self {
            track,
            max,
            viewport,
        }
    }

    pub fn thumb(&self, scroll: usize) -> Option<Thumb> {
        let height = usize::from(self.track.height);
        if self.max == 0 || height == 0 {
            return None;
        }
        let total = self.max + self.viewport;
        let len = round_div(self.viewport * height, total).clamp(1, height);
        let room = height - len;
        let top = if room == 0 {
            0
        } else {
            round_div(scroll.min(self.max) * room, self.max).min(room)
        };
        Some(Thumb {
            top: self.track.y + u16::try_from(top).unwrap_or(0),
            len: u16::try_from(len).unwrap_or(1),
        })
    }

    pub fn grab(&self, scroll: usize, row: u16) -> Option<u16> {
        let thumb = self.thumb(scroll)?;
        (row >= thumb.top && row < thumb.top + thumb.len).then(|| row - thumb.top)
    }

    fn from_top(&self, top: usize, scroll: usize) -> usize {
        let len = self.thumb(scroll).map_or(1, |t| usize::from(t.len));
        let room = usize::from(self.track.height).saturating_sub(len);
        if self.max == 0 || room == 0 {
            return 0;
        }
        round_div(top.min(room) * self.max, room)
    }

    fn offset(&self, row: u16) -> usize {
        let last = self.track.y + self.track.height.saturating_sub(1);
        usize::from(row.clamp(self.track.y, last) - self.track.y)
    }

    pub fn jump(&self, scroll: usize, row: u16) -> usize {
        let half = self.thumb(scroll).map_or(0, |t| usize::from(t.len) / 2);
        self.from_top(self.offset(row).saturating_sub(half), scroll)
    }

    pub fn drag(&self, scroll: usize, row: u16, grab: u16) -> usize {
        self.from_top(self.offset(row).saturating_sub(usize::from(grab)), scroll)
    }

    pub fn render(
        &self,
        buf: &mut Buffer,
        scroll: usize,
        track: (&str, Color),
        thumb: (&str, Color),
        bg: Color,
    ) {
        let Some(at) = self.thumb(scroll) else {
            return;
        };
        let x = self.track.x;
        for y in self.track.top()..self.track.bottom() {
            let (symbol, fg) = if y >= at.top && y < at.top + at.len {
                thumb
            } else {
                track
            };
            buf[(x, y)]
                .set_symbol(symbol)
                .set_style(Style::new().fg(fg).bg(bg));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_thumb_tracks_the_scroll() {
        let bar = Bar::new(Rect::new(9, 2, 1, 10), 10, 10);
        assert_eq!(bar.thumb(0), Some(Thumb { top: 2, len: 5 }));
        assert_eq!(bar.thumb(10), Some(Thumb { top: 7, len: 5 }));
        assert_eq!(Bar::new(Rect::new(0, 0, 1, 10), 0, 10).thumb(0), None);
    }

    #[test]
    fn rows_map_back_to_scroll() {
        let bar = Bar::new(Rect::new(9, 2, 1, 10), 10, 10);
        assert_eq!(bar.jump(0, 11), 10);
        assert_eq!(bar.jump(0, 2), 0);
        assert_eq!(bar.grab(0, 4), Some(2));
        assert_eq!(bar.drag(0, 7, 2), 6);
    }
}
