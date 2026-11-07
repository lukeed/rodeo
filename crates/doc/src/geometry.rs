use std::cmp::Reverse;

use crate::schema::{Axis, Node, Size};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    pub fn length(self, axis: Axis) -> u16 {
        match axis {
            Axis::Row => self.width,
            Axis::Column => self.height,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    pub fn axis(self) -> Axis {
        match self {
            Direction::Left | Direction::Right => Axis::Row,
            Direction::Up | Direction::Down => Axis::Column,
        }
    }

    pub fn forward(self) -> bool {
        matches!(self, Direction::Right | Direction::Down)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cell {
    pub width: u16,
    pub height: u16,
}

impl Cell {
    pub fn px(self, axis: Axis) -> f32 {
        f32::from(match axis {
            Axis::Row => self.width,
            Axis::Column => self.height,
        })
        .max(1.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Slot {
    pub size: Option<Size>,
    pub min: Option<f32>,
    pub max: Option<f32>,
}

impl Slot {
    pub fn of(node: &Node) -> Slot {
        Slot {
            size: node.size,
            min: node.min,
            max: node.max,
        }
    }
}

pub fn split_lengths(total: u16, slots: &[Slot], px_per_cell: f32) -> Vec<u16> {
    if slots.is_empty() {
        return Vec::new();
    }
    let total_f = f32::from(total);
    let cells = |px: f32| px / px_per_cell.max(f32::EPSILON);
    let bounds = |s: &Slot| {
        let min = s.min.map_or(0.0, cells).max(0.0);
        (min, s.max.map_or(f32::INFINITY, cells).max(min))
    };
    let mut sizes = vec![0.0f32; slots.len()];
    let mut flex = Vec::new();
    let mut fixed = 0.0;
    for (i, slot) in slots.iter().enumerate() {
        match slot.size {
            Some(Size::Px { px }) => {
                let (min, max) = bounds(slot);
                sizes[i] = sane(cells(px)).clamp(min, max);
                fixed += sizes[i];
            }
            _ => flex.push(i),
        }
    }
    let weight = |i: usize| match slots[i].size {
        Some(Size::Weight { weight }) => sane(weight),
        _ => 1.0,
    };
    let mut free = (total_f - fixed).max(0.0);
    let mut open = flex;
    while !open.is_empty() {
        let sum: f32 = open.iter().map(|&i| weight(i)).sum();
        let mut moved = Vec::new();
        let mut overflow = 0.0;
        for &i in &open {
            let share = if sum > 0.0 {
                free * weight(i) / sum
            } else {
                free / count(open.len())
            };
            let (min, max) = bounds(&slots[i]);
            sizes[i] = share.clamp(min, max);
            overflow += sizes[i] - share;
            moved.push((i, sizes[i] - share));
        }
        if overflow == 0.0 {
            break;
        }
        let frozen: Vec<usize> = moved
            .into_iter()
            .filter(|&(_, d)| (overflow > 0.0 && d > 0.0) || (overflow < 0.0 && d < 0.0))
            .map(|(i, _)| i)
            .collect();
        if frozen.is_empty() {
            break;
        }
        for i in &frozen {
            free = (free - sizes[*i]).max(0.0);
        }
        open.retain(|i| !frozen.contains(i));
    }
    let sum: f32 = sizes.iter().sum();
    if sum > 0.0 {
        for s in &mut sizes {
            *s *= total_f / sum;
        }
    } else {
        sizes.fill(total_f / count(slots.len()));
    }
    apportion(total, &sizes)
}

#[expect(clippy::cast_precision_loss, reason = "a node has few children")]
fn count(n: usize) -> f32 {
    n as f32
}

fn sane(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "every share is finite, at least 0 and at most total"
)]
fn apportion(total: u16, shares: &[f32]) -> Vec<u16> {
    let mut out: Vec<u16> = shares
        .iter()
        .map(|s| (s.floor().max(0.0) as u16).min(total))
        .collect();
    let given: u32 = out.iter().map(|&c| u32::from(c)).sum();
    let mut left = u32::from(total).saturating_sub(given);
    let mut order: Vec<usize> = (0..shares.len()).collect();
    order.sort_by(|&a, &b| {
        let fa = shares[a] - shares[a].floor();
        let fb = shares[b] - shares[b].floor();
        fb.total_cmp(&fa).then(a.cmp(&b))
    });
    for &i in order.iter().cycle() {
        if left == 0 {
            break;
        }
        out[i] += 1;
        left -= 1;
    }
    let mut over = out
        .iter()
        .map(|&c| u32::from(c))
        .sum::<u32>()
        .saturating_sub(u32::from(total));
    for c in out.iter_mut().rev() {
        let cut = over.min(u32::from(*c));
        *c -= u16::try_from(cut).expect("cut is at most a u16");
        over -= cut;
    }
    out
}

pub fn split_rect(area: Rect, axis: Axis, slots: &[Slot], cell: Cell) -> Vec<Rect> {
    let lengths = split_lengths(area.length(axis), slots, cell.px(axis));
    let mut at = 0u16;
    lengths
        .into_iter()
        .map(|len| {
            let rect = match axis {
                Axis::Row => Rect::new(area.x.saturating_add(at), area.y, len, area.height),
                Axis::Column => Rect::new(area.x, area.y.saturating_add(at), area.width, len),
            };
            at = at.saturating_add(len);
            rect
        })
        .collect()
}

pub fn neighbor<'a>(
    from: &str,
    direction: Direction,
    panes: &[(&'a str, Rect)],
) -> Option<&'a str> {
    let (_, origin) = panes.iter().find(|(id, _)| *id == from)?;
    let o = *origin;
    let (o_start, o_len, o_cross, o_cross_len) = spans(o, direction);
    panes
        .iter()
        .enumerate()
        .filter(|(_, (id, _))| *id != from)
        .filter_map(|(index, (id, r))| {
            let (start, len, cross, cross_len) = spans(*r, direction);
            let gap = if direction.forward() {
                i32::from(start) - (i32::from(o_start) + i32::from(o_len))
            } else {
                i32::from(o_start) - (i32::from(start) + i32::from(len))
            };
            let overlap = (i32::from(cross) + i32::from(cross_len))
                .min(i32::from(o_cross) + i32::from(o_cross_len))
                - i32::from(cross).max(i32::from(o_cross));
            (gap >= 0 && overlap > 0).then(|| {
                let centers = (2 * i32::from(cross) + i32::from(cross_len))
                    .abs_diff(2 * i32::from(o_cross) + i32::from(o_cross_len));
                ((gap, Reverse(overlap), centers, index), *id)
            })
        })
        .min_by_key(|(rank, _)| *rank)
        .map(|(_, id)| id)
}

fn spans(r: Rect, direction: Direction) -> (u16, u16, u16, u16) {
    match direction.axis() {
        Axis::Row => (r.x, r.width, r.y, r.height),
        Axis::Column => (r.y, r.height, r.x, r.width),
    }
}
