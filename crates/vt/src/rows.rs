use libghostty_vt::screen::{Screen, TrackedGridRef};
use libghostty_vt::terminal::{Point, PointCoordinate, PointSpace};

use crate::term::Vt;

#[derive(Default)]
struct Track {
    base: u64,
    total: u64,
    anchor: Option<TrackedGridRef>,
    anchor_row: u64,
}

#[derive(Default)]
pub(crate) struct Rows {
    primary: Track,
    alternate: Track,
    active: Screen,
}

impl Rows {
    pub(crate) fn base(&self) -> u64 {
        self.track().base
    }

    fn track(&self) -> &Track {
        match self.active {
            Screen::Primary => &self.primary,
            Screen::Alternate => &self.alternate,
        }
    }

    pub(crate) fn sync(&mut self, vt: &mut Vt) {
        self.active = vt.active_screen().unwrap_or_default();
        let track = match self.active {
            Screen::Primary => &mut self.primary,
            Screen::Alternate => &mut self.alternate,
        };
        let total = vt.total_rows().map_or(0, |rows| rows as u64);
        let tracked = track
            .anchor
            .as_ref()
            .and_then(|anchor| anchor.point(PointSpace::Screen).ok().flatten());
        track.base = match (tracked, &track.anchor) {
            (Some(point), _) => track.anchor_row.saturating_sub(u64::from(point.y)),
            (None, Some(_)) => track.base + track.total,
            (None, None) => track.base,
        };
        track.total = total;
        let Some(last) = total.checked_sub(1) else {
            return;
        };
        let point = Point::Screen(PointCoordinate {
            x: 0,
            y: u32::try_from(last).unwrap_or(u32::MAX),
        });
        let anchored = match track.anchor.as_mut() {
            Some(anchor) => anchor.set(vt, point).is_ok(),
            None => match vt.track_grid_ref(point) {
                Ok(anchor) => {
                    track.anchor = Some(anchor);
                    true
                }
                Err(_) => false,
            },
        };
        if anchored {
            track.anchor_row = track.base + last;
        } else {
            track.anchor = None;
        }
    }
}
