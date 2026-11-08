use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};

use base64::Engine;
use ratatui::layout::Rect;
use vt::{ImageFormat, Placement};

use crate::app::App;

const CHUNK: usize = 3072;
const ID_BASE: u32 = 10_000;
const ID_SPAN: u32 = 900_000;
const MAX_PIECES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cell {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Piece {
    pub x: u16,
    pub y: u16,
    pub cols: u16,
    pub rows: u16,
    pub source_x: u32,
    pub source_y: u32,
    pub source_width: u32,
    pub source_height: u32,
    pub x_offset: u32,
    pub y_offset: u32,
    pub z: i32,
}

#[derive(Debug, Default)]
pub struct Graphics {
    images: HashMap<u32, (String, u32, u64)>,
    placed: HashMap<(u32, u32), Piece>,
}

struct Wanted {
    address: String,
    image: u32,
    generation: u64,
    host: u32,
    placement: u32,
    piece: Piece,
}

fn hash_id(parts: impl Hash, base: u32) -> u32 {
    let mut hasher = DefaultHasher::new();
    parts.hash(&mut hasher);
    base + u32::try_from(hasher.finish() % u64::from(ID_SPAN)).unwrap_or(0)
}

pub fn host_image_id(address: &str, image: u32) -> u32 {
    hash_id((address, image), ID_BASE)
}

pub fn host_placement_id(address: &str, image: u32, placement: u32, piece: usize) -> u32 {
    hash_id((address, image, placement, piece), 1)
}

fn scale(value: u32, source: u32, dest: u32) -> u32 {
    let scaled = u64::from(value) * u64::from(source) / u64::from(dest.max(1));
    u32::try_from(scaled).unwrap_or(u32::MAX)
}

fn subtract(rect: Rect, cover: Rect) -> Vec<Rect> {
    let hit = rect.intersection(cover);
    if hit.is_empty() {
        return vec![rect];
    }
    let mut out = Vec::new();
    if hit.y > rect.y {
        out.push(Rect::new(rect.x, rect.y, rect.width, hit.y - rect.y));
    }
    if hit.bottom() < rect.bottom() {
        out.push(Rect::new(
            rect.x,
            hit.bottom(),
            rect.width,
            rect.bottom() - hit.bottom(),
        ));
    }
    if hit.x > rect.x {
        out.push(Rect::new(rect.x, hit.y, hit.x - rect.x, hit.height));
    }
    if hit.right() < rect.right() {
        out.push(Rect::new(
            hit.right(),
            hit.y,
            rect.right() - hit.right(),
            hit.height,
        ));
    }
    out
}

pub fn visible(rect: Rect, covers: &[Rect]) -> Vec<Rect> {
    let mut pieces = vec![rect];
    for cover in covers {
        pieces = pieces
            .into_iter()
            .flat_map(|p| subtract(p, *cover))
            .collect();
        if pieces.len() > MAX_PIECES {
            return Vec::new();
        }
    }
    pieces
}

pub fn pieces(
    placement: &Placement,
    image: (u32, u32),
    area: Rect,
    cell: Cell,
    covers: &[Rect],
) -> Vec<Piece> {
    if area.is_empty() || placement.cols == 0 || placement.rows == 0 {
        return Vec::new();
    }
    let gx = i64::from(area.x) + i64::from(placement.col);
    let gy = i64::from(area.y) + i64::from(placement.row);
    let left = gx.max(i64::from(area.x));
    let top = gy.max(i64::from(area.y));
    let right = (gx + i64::from(placement.cols)).min(i64::from(area.right()));
    let bottom = (gy + i64::from(placement.rows)).min(i64::from(area.bottom()));
    if right <= left || bottom <= top {
        return Vec::new();
    }
    let to16 = |v: i64| u16::try_from(v).unwrap_or(u16::MAX);
    let shown = Rect::new(
        to16(left),
        to16(top),
        to16(right - left),
        to16(bottom - top),
    );
    let source_width = if placement.source_width == 0 {
        image.0
    } else {
        placement.source_width
    };
    let source_height = if placement.source_height == 0 {
        image.1
    } else {
        placement.source_height
    };
    let pixel_width = placement
        .width
        .max(placement.cols.saturating_mul(cell.width))
        .max(1);
    let pixel_height = placement
        .height
        .max(placement.rows.saturating_mul(cell.height))
        .max(1);
    visible(shown, covers)
        .into_iter()
        .filter_map(|part| {
            let left_cells = u32::try_from(i64::from(part.x) - gx).ok()?;
            let top_cells = u32::try_from(i64::from(part.y) - gy).ok()?;
            let source_x =
                placement.source_x + scale(left_cells * cell.width, source_width, pixel_width);
            let source_y =
                placement.source_y + scale(top_cells * cell.height, source_height, pixel_height);
            let width = scale(
                u32::from(part.width) * cell.width,
                source_width,
                pixel_width,
            )
            .max(1)
            .min(image.0.saturating_sub(source_x));
            let height = scale(
                u32::from(part.height) * cell.height,
                source_height,
                pixel_height,
            )
            .max(1)
            .min(image.1.saturating_sub(source_y));
            (width > 0 && height > 0).then_some(Piece {
                x: part.x,
                y: part.y,
                cols: part.width,
                rows: part.height,
                source_x,
                source_y,
                source_width: width,
                source_height: height,
                x_offset: if left_cells == 0 {
                    placement.x_offset
                } else {
                    0
                },
                y_offset: if top_cells == 0 {
                    placement.y_offset
                } else {
                    0
                },
                z: placement.z,
            })
        })
        .collect()
}

fn delete_image(out: &mut String, id: u32) {
    let _ = write!(out, "\x1b_Ga=d,d=I,i={id},q=2;\x1b\\");
}

fn delete_placement(out: &mut String, id: u32, placement: u32) {
    let _ = write!(out, "\x1b_Ga=d,d=i,i={id},p={placement},q=2;\x1b\\");
}

pub fn upload(out: &mut String, id: u32, image: &vt::Image) {
    let (format, data) = match image.format {
        ImageFormat::Rgb => (24, image.data.clone()),
        ImageFormat::Rgba => (32, image.data.clone()),
        ImageFormat::Png => (100, image.data.clone()),
        ImageFormat::Gray => (24, image.data.iter().flat_map(|&g| [g, g, g]).collect()),
        ImageFormat::GrayAlpha => (
            32,
            image
                .data
                .as_chunks::<2>()
                .0
                .iter()
                .flat_map(|&[g, a]| [g, g, g, a])
                .collect(),
        ),
    };
    let control = format!(
        "a=t,t=d,f={format},s={},v={},i={id},q=2",
        image.width, image.height
    );
    let engine = base64::engine::general_purpose::STANDARD;
    let mut chunks = data.chunks(CHUNK).peekable();
    let mut first = true;
    while let Some(chunk) = chunks.next() {
        let more = u8::from(chunks.peek().is_some());
        let payload = engine.encode(chunk);
        if first {
            let _ = write!(out, "\x1b_G{control},m={more};{payload}\x1b\\");
            first = false;
        } else {
            let _ = write!(out, "\x1b_Gm={more};{payload}\x1b\\");
        }
    }
}

pub fn place(out: &mut String, id: u32, placement: u32, piece: &Piece) {
    let _ = write!(
        out,
        "\x1b[{};{}H\x1b_Ga=p,i={id},p={placement},c={},r={},z={},C=1,q=2",
        piece.y + 1,
        piece.x + 1,
        piece.cols,
        piece.rows,
        piece.z
    );
    for (key, value) in [
        ('x', piece.source_x),
        ('y', piece.source_y),
        ('w', piece.source_width),
        ('h', piece.source_height),
        ('X', piece.x_offset),
        ('Y', piece.y_offset),
    ] {
        if value > 0 {
            let _ = write!(out, ",{key}={value}");
        }
    }
    out.push_str(";\x1b\\");
}

impl Graphics {
    pub fn clear(&mut self) -> String {
        let mut out = String::new();
        let mut ids: Vec<u32> = self.images.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            delete_image(&mut out, id);
        }
        self.images.clear();
        self.placed.clear();
        out
    }
}

impl App {
    pub fn compose_graphics(&mut self, covers: &[Rect]) {
        let (w, h) = self.host.cell_px;
        let hidden = self.view.overlay.is_some() || !self.conn.is_online();
        if !self.config.kitty_graphics() || w == 0 || h == 0 {
            let bytes = self.graphics.clear();
            self.out.extend_from_slice(bytes.as_bytes());
            return;
        }
        let cell = Cell {
            width: w,
            height: h,
        };
        let mut wanted = Vec::new();
        let mut sizes: HashMap<(String, u32), (u32, u32)> = HashMap::new();
        for hit in self.hits.panes.iter().filter(|_| !hidden) {
            let Some(address) = &hit.address else {
                continue;
            };
            let Some(pane) = self.panes.get(address) else {
                continue;
            };
            for placement in pane.term.graphics().placements {
                let key = (address.clone(), placement.image);
                let size = if let Some(size) = sizes.get(&key) {
                    *size
                } else {
                    let Some(image) = pane.term.image(placement.image) else {
                        continue;
                    };
                    sizes.insert(key, (image.width, image.height));
                    (image.width, image.height)
                };
                let host = host_image_id(address, placement.image);
                for (index, piece) in pieces(&placement, size, hit.content, cell, covers)
                    .into_iter()
                    .enumerate()
                {
                    wanted.push(Wanted {
                        address: address.clone(),
                        image: placement.image,
                        generation: placement.image_generation,
                        host,
                        placement: host_placement_id(
                            address,
                            placement.image,
                            placement.placement,
                            index,
                        ),
                        piece,
                    });
                }
            }
        }
        let bytes = self.graphics_bytes(&wanted);
        if !bytes.is_empty() {
            self.out.extend_from_slice(b"\x1b7");
            self.out.extend_from_slice(bytes.as_bytes());
            self.out.extend_from_slice(b"\x1b8");
        }
    }

    fn graphics_bytes(&mut self, wanted: &[Wanted]) -> String {
        let mut out = String::new();
        let keep: HashSet<(u32, u32)> = wanted.iter().map(|w| (w.host, w.placement)).collect();
        let mut stale: Vec<(u32, u32)> = self
            .graphics
            .placed
            .keys()
            .filter(|k| !keep.contains(k))
            .copied()
            .collect();
        stale.sort_unstable();
        for key in stale {
            delete_placement(&mut out, key.0, key.1);
            self.graphics.placed.remove(&key);
        }
        let used: HashSet<u32> = wanted.iter().map(|w| w.host).collect();
        let mut unused: Vec<u32> = self
            .graphics
            .images
            .iter()
            .filter(|(id, (address, image, _))| {
                !used.contains(id)
                    && self
                        .panes
                        .get(address)
                        .is_none_or(|p| p.term.image(*image).is_none())
            })
            .map(|(id, _)| *id)
            .collect();
        unused.sort_unstable();
        for id in unused {
            delete_image(&mut out, id);
            self.graphics.images.remove(&id);
        }
        for want in wanted {
            let signature = (want.address.clone(), want.image, want.generation);
            if self.graphics.images.get(&want.host) != Some(&signature) {
                let Some(image) = self
                    .panes
                    .get(&want.address)
                    .and_then(|p| p.term.image(want.image))
                else {
                    continue;
                };
                if self.graphics.images.remove(&want.host).is_some() {
                    delete_image(&mut out, want.host);
                    self.graphics.placed.retain(|(id, _), _| *id != want.host);
                }
                upload(&mut out, want.host, &image);
                self.graphics.images.insert(want.host, signature);
            }
            let key = (want.host, want.placement);
            if self.graphics.placed.get(&key) != Some(&want.piece) {
                place(&mut out, want.host, want.placement, &want.piece);
                self.graphics.placed.insert(key, want.piece);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(col: i32, row: i32) -> Placement {
        Placement {
            image: 7,
            placement: 3,
            image_generation: 1,
            z: 0,
            col,
            row,
            cols: 3,
            rows: 3,
            width: 30,
            height: 30,
            x_offset: 0,
            y_offset: 0,
            source_x: 0,
            source_y: 0,
            source_width: 30,
            source_height: 30,
        }
    }

    const CELL: Cell = Cell {
        width: 10,
        height: 10,
    };

    #[test]
    fn placements_sit_at_the_pane_offset() {
        let area = Rect::new(5, 2, 20, 10);
        let got = pieces(&placement(1, 1), (30, 30), area, CELL, &[]);
        assert_eq!(
            got,
            [Piece {
                x: 6,
                y: 3,
                cols: 3,
                rows: 3,
                source_x: 0,
                source_y: 0,
                source_width: 30,
                source_height: 30,
                x_offset: 0,
                y_offset: 0,
                z: 0,
            }]
        );
    }

    #[test]
    fn placements_clip_to_the_pane() {
        let area = Rect::new(0, 0, 4, 4);
        let top = pieces(&placement(0, -1), (30, 30), area, CELL, &[]);
        assert_eq!(top.len(), 1);
        assert_eq!((top[0].y, top[0].rows), (0, 2));
        assert_eq!((top[0].source_y, top[0].source_height), (10, 20));
        let right = pieces(&placement(2, 0), (30, 30), area, CELL, &[]);
        assert_eq!((right[0].x, right[0].cols), (2, 2));
        assert_eq!((right[0].source_x, right[0].source_width), (0, 20));
        assert_eq!(pieces(&placement(4, 0), (30, 30), area, CELL, &[]), []);
        assert_eq!(pieces(&placement(0, -3), (30, 30), area, CELL, &[]), []);
    }

    #[test]
    fn covers_cut_placements_into_visible_pieces() {
        let area = Rect::new(0, 0, 20, 10);
        let cover = Rect::new(1, 1, 1, 1);
        let got = pieces(&placement(0, 0), (30, 30), area, CELL, &[cover]);
        let cells: u16 = got.iter().map(|p| p.cols * p.rows).sum();
        assert_eq!(cells, 8);
        assert!(
            got.iter()
                .all(|p| !Rect::new(p.x, p.y, p.cols, p.rows).intersects(cover))
        );
        let bottom = got.iter().find(|p| p.y == 2).expect("the bottom row");
        assert_eq!((bottom.source_y, bottom.source_height), (20, 10));
        let whole = Rect::new(0, 0, 20, 10);
        assert_eq!(pieces(&placement(0, 0), (30, 30), area, CELL, &[whole]), []);
    }

    #[test]
    fn commands_use_kitty_syntax() {
        let mut out = String::new();
        let piece = pieces(
            &placement(0, -1),
            (30, 30),
            Rect::new(2, 1, 10, 10),
            CELL,
            &[],
        )[0];
        place(&mut out, 12_345, 9, &piece);
        assert_eq!(
            out,
            "\x1b[2;3H\x1b_Ga=p,i=12345,p=9,c=3,r=2,z=0,C=1,q=2,y=10,w=30,h=20;\x1b\\"
        );
        let mut out = String::new();
        let image = vt::Image {
            id: 1,
            number: 0,
            width: 2,
            height: 1,
            format: ImageFormat::Gray,
            generation: 1,
            data: vec![0, 255],
        };
        upload(&mut out, 10_001, &image);
        assert_eq!(
            out,
            "\x1b_Ga=t,t=d,f=24,s=2,v=1,i=10001,q=2,m=0;AAAA////\x1b\\"
        );
        let big = vt::Image {
            data: vec![7; CHUNK + 1],
            format: ImageFormat::Rgba,
            ..image
        };
        let mut out = String::new();
        upload(&mut out, 10_002, &big);
        assert_eq!(out.matches("\x1b_G").count(), 2);
        assert!(out.contains(",m=1;"));
        assert!(out.contains("\x1b_Gm=0;"));
    }

    #[test]
    fn ids_are_stable_and_in_range() {
        let id = host_image_id("t1", 7);
        assert_eq!(id, host_image_id("t1", 7));
        assert_ne!(id, host_image_id("t2", 7));
        assert!((ID_BASE..ID_BASE + ID_SPAN).contains(&id));
        assert!(host_placement_id("t1", 7, 0, 0) >= 1);
    }
}
