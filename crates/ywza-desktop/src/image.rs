//! RGBA images and PNG encoding.

use crate::{Error, Rect, Result};

/// Block size of `pixelate`: coarse enough that text and password dots are unreadable.
const BLOCK: usize = 12;

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

/// Replaces `r` (image pixels, clipped to the image) with blocks of their average color.
pub fn pixelate(img: &mut Image, r: Rect) {
    let r = r.intersect(&Rect::new(0, 0, img.width as i32, img.height as i32));
    if r.is_empty() {
        return;
    }
    let stride = img.width as usize;
    let at = |x: i32, y: i32| (y as usize * stride + x as usize) * 4;
    for by in (r.y..r.y + r.h).step_by(BLOCK) {
        for bx in (r.x..r.x + r.w).step_by(BLOCK) {
            let (x1, y1) = ((bx + BLOCK as i32).min(r.x + r.w), (by + BLOCK as i32).min(r.y + r.h));
            let n = ((x1 - bx) * (y1 - by)) as u32;
            let mut sum = [0u32; 3];
            for y in by..y1 {
                for x in bx..x1 {
                    for (c, s) in sum.iter_mut().enumerate() {
                        *s += img.rgba[at(x, y) + c] as u32;
                    }
                }
            }
            let avg = [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8, 255];
            for y in by..y1 {
                for x in bx..x1 {
                    img.rgba[at(x, y)..at(x, y) + 4].copy_from_slice(&avg);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: u32, h: u32) -> Image {
        let rgba = (0..w * h)
            .flat_map(|i| [(i % 251) as u8, (i % 13) as u8 * 20, 7, 255])
            .collect();
        Image {
            width: w,
            height: h,
            rgba,
            origin: (0, 0),
        }
    }

    #[test]
    fn pixelate_flattens_blocks_and_leaves_the_rest() {
        let (orig, mut img) = (gradient(40, 30), gradient(40, 30));
        pixelate(&mut img, Rect::new(4, 4, 12, 12));
        let px = |i: &Image, x: usize, y: usize| i.rgba[(y * 40 + x) * 4..][..4].to_vec();
        assert_eq!(px(&img, 4, 4), px(&img, 15, 15));
        assert_ne!(px(&img, 4, 4), px(&orig, 4, 4));
        assert_eq!(px(&img, 3, 4), px(&orig, 3, 4));
        assert_eq!(px(&img, 16, 4), px(&orig, 16, 4));
    }

    #[test]
    fn pixelate_clips_to_the_image() {
        let mut img = gradient(20, 10);
        pixelate(&mut img, Rect::new(-50, -50, 500, 500));
        pixelate(&mut img, Rect::new(15, 5, 100, 100));
        pixelate(&mut img, Rect::new(100, 100, 5, 5));
        pixelate(&mut img, Rect::new(0, 0, 0, 9));
        assert_eq!(img.rgba.len(), 20 * 10 * 4);
        assert!(img.rgba.chunks(4).all(|p| p[3] == 255));
    }
}
