use std::cell::Cell;
use std::io::Cursor;

use libghostty_vt::alloc::{Allocator, Bytes};
use libghostty_vt::kitty::graphics::{self, DecodePng, DecodedImage};
use png::{BitDepth, ColorType, Decoder, Transformations};

thread_local! {
    static INSTALLED: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn install() -> Result<(), libghostty_vt::Error> {
    if INSTALLED.get() {
        return Ok(());
    }
    graphics::set_png_decoder(Some(Box::new(PngDecoder)))?;
    INSTALLED.set(true);
    Ok(())
}

struct PngDecoder;

impl DecodePng for PngDecoder {
    fn decode_png<'alloc>(
        &mut self,
        alloc: &'alloc Allocator<'_>,
        data: &[u8],
    ) -> Option<DecodedImage<'alloc>> {
        let (width, height, rgba) = decode(data)?;
        let mut bytes = Bytes::new_with_alloc(alloc, rgba.len()).ok()?;
        bytes.copy_from_slice(&rgba);
        Some(DecodedImage {
            width,
            height,
            data: bytes,
        })
    }
}

pub(crate) fn decode(data: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut decoder = Decoder::new(Cursor::new(data));
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.bit_depth != BitDepth::Eight {
        return None;
    }
    let pixels = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        ColorType::Rgba => pixels.to_vec(),
        ColorType::Rgb => pixels
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[r, g, b]| [r, g, b, 0xff])
            .collect(),
        ColorType::GrayscaleAlpha => pixels
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[g, a]| [g, g, g, a])
            .collect(),
        ColorType::Grayscale => pixels.iter().flat_map(|&g| [g, g, g, 0xff]).collect(),
        ColorType::Indexed => return None,
    };
    Some((info.width, info.height, rgba))
}
