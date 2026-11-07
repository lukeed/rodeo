use libghostty_vt::kitty::graphics::{self, PlacementIterator};

use crate::term::{Error, Terminal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Rgb,
    Rgba,
    Gray,
    GrayAlpha,
    Png,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub id: u32,
    pub number: u32,
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
    pub generation: u64,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub image: u32,
    pub placement: u32,
    pub image_generation: u64,
    pub z: i32,
    pub col: i32,
    pub row: i32,
    pub cols: u32,
    pub rows: u32,
    pub width: u32,
    pub height: u32,
    pub x_offset: u32,
    pub y_offset: u32,
    pub source_x: u32,
    pub source_y: u32,
    pub source_width: u32,
    pub source_height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Graphics {
    pub generation: u64,
    pub placements: Vec<Placement>,
}

pub(crate) struct Images {
    iter: PlacementIterator<'static>,
}

impl Images {
    pub(crate) fn new() -> Result<Self, Error> {
        Ok(Self {
            iter: PlacementIterator::new()?,
        })
    }
}

impl Terminal {
    pub fn graphics(&self) -> Graphics {
        let vt = self.vt();
        let Ok(storage) = vt.kitty_graphics() else {
            return Graphics::default();
        };
        let generation = storage.generation().unwrap_or(0);
        if generation == 0 {
            return Graphics::default();
        }
        let mut images = self.images().borrow_mut();
        let Ok(mut iteration) = images.iter.update(&storage) else {
            return Graphics::default();
        };
        let mut placements = Vec::new();
        while let Some(current) = iteration.next() {
            let Ok(id) = current.image_id() else {
                continue;
            };
            let Some(image) = storage.image(id) else {
                continue;
            };
            let Ok(info) = current.placement_render_info(&image, vt) else {
                continue;
            };
            if !info.viewport_visible || current.is_virtual().unwrap_or(true) {
                continue;
            }
            placements.push(Placement {
                image: id,
                placement: current.placement_id().unwrap_or(0),
                image_generation: image.generation().unwrap_or(0),
                z: current.z().unwrap_or(0),
                col: info.viewport_col,
                row: info.viewport_row,
                cols: info.grid_cols,
                rows: info.grid_rows,
                width: info.pixel_width,
                height: info.pixel_height,
                x_offset: current.x_offset().unwrap_or(0),
                y_offset: current.y_offset().unwrap_or(0),
                source_x: info.source_x,
                source_y: info.source_y,
                source_width: info.source_width,
                source_height: info.source_height,
            });
        }
        placements.sort_by_key(|placement| (placement.z, placement.image, placement.placement));
        Graphics {
            generation,
            placements,
        }
    }

    pub fn image(&self, id: u32) -> Option<Image> {
        let storage = self.vt().kitty_graphics().ok()?;
        let image = storage.image(id)?;
        Some(Image {
            id,
            number: image.number().unwrap_or(0),
            width: image.width().ok()?,
            height: image.height().ok()?,
            format: match image.format().ok()? {
                graphics::ImageFormat::Rgb => ImageFormat::Rgb,
                graphics::ImageFormat::Rgba => ImageFormat::Rgba,
                graphics::ImageFormat::Gray => ImageFormat::Gray,
                graphics::ImageFormat::GrayAlpha => ImageFormat::GrayAlpha,
                graphics::ImageFormat::Png => ImageFormat::Png,
                _ => return None,
            },
            generation: image.generation().unwrap_or(0),
            data: image.data().ok()?.to_vec(),
        })
    }
}
