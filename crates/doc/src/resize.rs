use crate::edit::Error;
use crate::geometry::{Cell, Direction, Rect};
use crate::schema::{Axis, Document, Node, Size};

pub const RESIZE_STEP: f32 = 0.05;

const MIN_RATIO: f32 = 0.1;
const MAX_RATIO: f32 = 0.9;

impl Document {
    pub fn resize_pane(
        &mut self,
        pane: &str,
        direction: Direction,
        amount: f32,
        rects: impl Fn(&str) -> Option<Rect>,
        cell: Cell,
    ) -> Result<bool, Error> {
        self.pane(pane)
            .ok_or_else(|| Error::NoPane(pane.to_owned()))?;
        let axis = direction.axis();
        let tab = self.tab_of_mut(pane).expect("the pane is in a tab");
        let path = tab.root.path_to(pane).expect("the tab holds the pane");
        let forward = direction.forward();
        let Some((parent, a, b)) = divider(&tab.root, &path, axis, forward)
            .or_else(|| divider(&tab.root, &path, axis, !forward))
        else {
            return Ok(false);
        };
        let node = tab.root.at_mut(&parent);
        if !node.resizable() {
            return Ok(false);
        }
        let length = |id: &str| {
            rects(id)
                .map(|r| f32::from(r.length(axis)))
                .ok_or_else(|| Error::Unmeasured(id.to_owned()))
        };
        let whole = length(&node.id)?;
        let children = node.children();
        let (la, lb) = (length(&children[a].id)?, length(&children[b].id)?);
        let pair = la + lb;
        let la = weighted_share(children[a].size, children[b].size, pair).unwrap_or(la);
        let lb = pair - la;
        let step = amount.abs().min(0.5) * whole;
        let step = if step.is_finite() { step } else { 0.0 };
        let new_a = if forward { la + step } else { la - step };
        Ok(set_pair(node, a, b, la, lb, new_a, cell.px(axis)))
    }

    pub fn drag_divider(
        &mut self,
        container: &str,
        index: usize,
        new_a: f32,
        rects: impl Fn(&str) -> Option<Rect>,
        cell: Cell,
    ) -> Result<bool, Error> {
        let tab = self
            .tab_of_mut(container)
            .ok_or_else(|| Error::NoNode(container.to_owned()))?;
        let node = tab
            .root
            .find_mut(container)
            .expect("the tab holds the node");
        let axis = node
            .axis()
            .ok_or_else(|| Error::NoNode(container.to_owned()))?;
        if !node.resizable() {
            return Ok(false);
        }
        let shown: Vec<usize> = (0..node.children().len())
            .filter(|&i| !node.children()[i].hidden)
            .collect();
        let (Some(&a), Some(&b)) = (shown.get(index), shown.get(index + 1)) else {
            return Err(Error::NoDivider(container.to_owned(), index));
        };
        let length = |id: &str| {
            rects(id)
                .map(|r| f32::from(r.length(axis)))
                .ok_or_else(|| Error::Unmeasured(id.to_owned()))
        };
        let children = node.children();
        let (la, lb) = (length(&children[a].id)?, length(&children[b].id)?);
        Ok(set_pair(node, a, b, la, lb, new_a, cell.px(axis)))
    }
}

fn divider(
    root: &Node,
    path: &[usize],
    axis: Axis,
    next: bool,
) -> Option<(Vec<usize>, usize, usize)> {
    (0..path.len()).rev().find_map(|depth| {
        let parent = root.at(&path[..depth]);
        if parent.axis() != Some(axis) {
            return None;
        }
        let i = path[depth];
        let children = parent.children();
        let shown = |j: &usize| !children[*j].hidden;
        let pair = if next {
            (i + 1..children.len()).find(shown).map(|j| (i, j))
        } else {
            (0..i).rev().find(shown).map(|j| (j, i))
        };
        pair.map(|(a, b)| (path[..depth].to_vec(), a, b))
    })
}

fn set_pair(
    parent: &mut Node,
    a: usize,
    b: usize,
    la: f32,
    lb: f32,
    new_a: f32,
    px_per_cell: f32,
) -> bool {
    let total = la + lb;
    if !(total > 0.0 && new_a.is_finite()) {
        return false;
    }
    let children = parent.children_mut().expect("a row or column");
    let mut new_a = new_a.clamp(MIN_RATIO * total, MAX_RATIO * total);
    if let Some(min) = children[a].min {
        new_a = new_a.max(min / px_per_cell);
    }
    if let Some(max) = children[a].max {
        new_a = new_a.min(max / px_per_cell);
    }
    let new_a = new_a.clamp(0.0, total);
    let (sa, sb) = (children[a].size, children[b].size);
    let (target, size) = match (sa, sb) {
        (Some(Size::Px { .. }), _) => (
            a,
            Size::Px {
                px: (new_a * px_per_cell).round(),
            },
        ),
        (_, Some(Size::Px { .. })) => (
            b,
            Size::Px {
                px: ((total - new_a) * px_per_cell).round(),
            },
        ),
        _ => {
            let weight = |s: Option<Size>| match s {
                Some(Size::Weight { weight }) => weight,
                _ => 1.0,
            };
            let sum = weight(sa) + weight(sb);
            let wa = Size::Weight {
                weight: sum * new_a / total,
            };
            let wb = Size::Weight {
                weight: sum * (total - new_a) / total,
            };
            if !(finite(wa) && finite(wb)) {
                return false;
            }
            let changed = sa != Some(wa) || sb != Some(wb);
            children[a].size = Some(wa);
            children[b].size = Some(wb);
            return changed;
        }
    };
    if !finite(size) {
        return false;
    }
    let changed = children[target].size != Some(size);
    children[target].size = Some(size);
    changed
}

fn weighted_share(a: Option<Size>, b: Option<Size>, total: f32) -> Option<f32> {
    let weight = |s: Option<Size>| match s {
        None => Some(1.0),
        Some(Size::Weight { weight }) if weight.is_finite() && weight > 0.0 => Some(weight),
        Some(_) => None,
    };
    let (wa, wb) = (weight(a)?, weight(b)?);
    Some(total * wa / (wa + wb))
}

fn finite(size: Size) -> bool {
    match size {
        Size::Px { px } => px.is_finite(),
        Size::Weight { weight } => weight.is_finite(),
    }
}
