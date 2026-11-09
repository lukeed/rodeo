use config::Palette;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Metrics {
    pub start: usize,
    pub max: usize,
    pub visible: usize,
}

impl Metrics {
    pub fn overflows(self) -> bool {
        self.max > 0
    }
}

pub fn metrics(heights: &[u16], gaps: &[u16], room: u16, requested: usize) -> Metrics {
    if heights.is_empty() || room == 0 {
        return Metrics::default();
    }
    let gap = |i: usize| gaps.get(i).copied().unwrap_or(0);
    let mut used = 0u16;
    let mut max = heights.len();
    for i in (0..heights.len()).rev() {
        let next = used
            .saturating_add(heights[i].clamp(1, room))
            .saturating_add(gap(i));
        if next > room {
            break;
        }
        used = next;
        max = i;
    }
    let max = max.min(heights.len() - 1);
    let start = requested.min(max);
    let mut visible = 0;
    let mut used = 0u16;
    for (i, height) in heights.iter().enumerate().skip(start) {
        let height = (*height).clamp(1, room);
        if used.saturating_add(height) > room {
            break;
        }
        used += height;
        visible += 1;
        if used.saturating_add(gap(i)) > room {
            break;
        }
        used += gap(i);
    }
    Metrics {
        start,
        max,
        visible,
    }
}

pub fn reveal(heights: &[u16], gaps: &[u16], room: u16, requested: usize, target: usize) -> usize {
    let mut m = metrics(heights, gaps, room, requested);
    if target < m.start {
        return target;
    }
    while target >= m.start + m.visible && m.start < m.max {
        m = metrics(heights, gaps, room, m.start + 1);
    }
    m.start
}

pub fn thumb(m: Metrics, track: Rect) -> Option<(u16, u16)> {
    if m.max == 0 || track.height == 0 {
        return None;
    }
    let h = usize::from(track.height);
    let total = m.max + m.visible;
    let len = (m.visible * h + total / 2) / total;
    let len = len.clamp(1, h);
    let room = h - len;
    let top = (m.start * room + m.max / 2) / m.max;
    let top = top.min(room);
    Some((
        track.y + u16::try_from(top).unwrap_or(0),
        u16::try_from(len).unwrap_or(1),
    ))
}

fn start_from_top(m: Metrics, track: Rect, top: usize) -> usize {
    let Some((_, len)) = thumb(m, track) else {
        return 0;
    };
    let room = usize::from(track.height - len);
    if room == 0 {
        return 0;
    }
    (top.min(room) * m.max + room / 2) / room
}

fn offset_in(track: Rect, row: u16) -> usize {
    usize::from(row.clamp(track.y, track.bottom().saturating_sub(1)) - track.y)
}

pub fn start_at_row(m: Metrics, track: Rect, row: u16) -> usize {
    let Some((_, len)) = thumb(m, track) else {
        return 0;
    };
    let top = offset_in(track, row).saturating_sub(usize::from(len) / 2);
    start_from_top(m, track, top)
}

pub fn start_dragged(m: Metrics, track: Rect, row: u16, grab: u16) -> usize {
    let top = offset_in(track, row).saturating_sub(usize::from(grab));
    start_from_top(m, track, top)
}

pub fn grab(m: Metrics, track: Rect, row: u16) -> Option<u16> {
    let (top, len) = thumb(m, track)?;
    (row >= top && row < top + len).then(|| row - top)
}

pub fn draw(buf: &mut Buffer, track: Rect, m: Metrics, palette: &Palette) {
    let Some((top, len)) = thumb(m, track) else {
        return;
    };
    for y in track.top()..track.bottom() {
        let color = if y >= top && y < top + len {
            palette.overlay0
        } else {
            palette.surface_dim
        };
        buf[(track.x, y)]
            .set_symbol("▕")
            .set_style(Style::new().fg(color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_keep_variable_rows_and_gaps() {
        let m = metrics(&[1, 3, 2], &[1, 1, 0], 5, 0);
        assert_eq!(
            m,
            Metrics {
                start: 0,
                max: 2,
                visible: 2
            }
        );
        let m = metrics(&[1; 20], &[0; 20], 5, 40);
        assert_eq!(
            m,
            Metrics {
                start: 15,
                max: 15,
                visible: 5
            }
        );
        assert_eq!(metrics(&[1, 1], &[0, 0], 5, 3).max, 0);
    }

    #[test]
    fn reveal_scrolls_just_enough() {
        let heights = [1; 20];
        let gaps = [0; 20];
        assert_eq!(reveal(&heights, &gaps, 5, 0, 19), 15);
        assert_eq!(reveal(&heights, &gaps, 5, 15, 2), 2);
        assert_eq!(reveal(&heights, &gaps, 5, 3, 5), 3);
        assert_eq!(reveal(&[2, 2, 2], &[1, 1, 0], 5, 0, 2), 1);
    }

    #[test]
    fn the_thumb_tracks_the_start_both_ways() {
        let track = Rect::new(0, 10, 1, 10);
        let m = metrics(&[1; 40], &[0; 40], 10, 0);
        assert_eq!(thumb(m, track), Some((10, 3)));
        let end = Metrics { start: 30, ..m };
        assert_eq!(thumb(end, track), Some((17, 3)));
        assert_eq!(start_at_row(m, track, 19), 30);
        assert_eq!(start_at_row(m, track, 10), 0);
        assert_eq!(grab(m, track, 11), Some(1));
        assert_eq!(grab(m, track, 14), None);
        assert_eq!(start_dragged(m, track, 14, 1), 13);
    }
}
