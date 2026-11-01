//! RGBA images and PNG encoding.

use crate::{Error, Result};

#[derive(Debug, Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// Tightly packed RGBA rows.
    pub rgba: Vec<u8>,
    /// Screen position of pixel (0, 0), so element rects can be mapped onto it.
    pub origin: (i32, i32),
}

pub fn encode_png(img: &Image) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, img.width, img.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .and_then(|mut w| w.write_image_data(&img.rgba))
        .map_err(|e| Error::Os(format!("png: {e}")))?;
    Ok(out)
}
