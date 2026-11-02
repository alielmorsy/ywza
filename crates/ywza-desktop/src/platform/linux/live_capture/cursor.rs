use crate::platform::linux::x::X;
use crate::{CursorShape, Image, Rect, Result};
use x11rb::protocol::xfixes;

/// Blends a premultiplied ARGB cursor at `at` (frame coordinates) onto a `w`x`h` BGRA frame.
pub(super) fn blend_cursor(out: &mut [u8], w: i32, h: i32, at: Rect, img: &[u32]) {
    for y in at.y.max(0)..(at.y + at.h).min(h) {
        for x in at.x.max(0)..(at.x + at.w).min(w) {
            let p = img[((y - at.y) * at.w + (x - at.x)) as usize];
            let a = p >> 24;
            if a == 0 {
                continue;
            }
            let d = ((y * w + x) * 4) as usize;
            for (i, s) in [p & 0xff, (p >> 8) & 0xff, (p >> 16) & 0xff]
                .into_iter()
                .enumerate()
            {
                let o = &mut out[d + i];
                *o = (s + *o as u32 * (255 - a) / 255).min(255) as u8;
            }
        }
    }
}

/// Watches the cursor's shape for the live view (when it isn't drawn into the video).
pub struct CursorWatch {
    x: X,
    last: Option<u32>,
}

impl CursorWatch {
    pub fn new() -> Result<Self> {
        let x = X::connect()?;
        xfixes::query_version(&x.conn, 4, 0)?.reply()?;
        Ok(Self { x, last: None })
    }

    /// The cursor's shape if it changed since the last call (the first call always reports).
    /// ponytail: a cursor hidden by an app still reports its last shape (XFixes can't tell).
    pub fn poll(&mut self) -> Result<Option<CursorShape>> {
        let c = xfixes::get_cursor_image(&self.x.conn)?.reply()?;
        if self.last == Some(c.cursor_serial) {
            return Ok(None);
        }
        self.last = Some(c.cursor_serial);
        Ok(Some(CursorShape {
            image: Some(Image {
                width: c.width as u32,
                height: c.height as u32,
                rgba: argb_to_rgba(&c.cursor_image),
                origin: (0, 0),
            }),
            hot_x: c.xhot as u32,
            hot_y: c.yhot as u32,
        }))
    }
}

/// Premultiplied ARGB words to straight RGBA bytes.
fn argb_to_rgba(img: &[u32]) -> Vec<u8> {
    img.iter()
        .flat_map(|&p| {
            let a = p >> 24;
            let un = |v: u32| if a == 0 { 0 } else { ((v & 255) * 255 / a).min(255) as u8 };
            [un(p >> 16), un(p >> 8), un(p), a as u8]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_argb_unpremultiplies() {
        assert_eq!(argb_to_rgba(&[0x80_40_20_00, 0]), vec![127, 63, 0, 128, 0, 0, 0, 0]);
    }

    #[test]
    fn cursor_blend_clips_and_mixes() {
        let mut f = vec![100u8; 2 * 2 * 4];
        // 2x2 cursor hanging off the top-left: only its bottom-right pixel lands at (0,0).
        let img = [0, 0, 0, 0x80_40_20_10]; // half-alpha premultiplied
        blend_cursor(&mut f, 2, 2, Rect::new(-1, -1, 2, 2), &img);
        assert_eq!(&f[..4], [0x10 + 49, 0x20 + 49, 0x40 + 49, 100]);
        assert!(f[4..].iter().all(|&v| v == 100));
        let opaque = [0xff_00_00_ff];
        blend_cursor(&mut f, 2, 2, Rect::new(1, 1, 1, 1), &opaque);
        assert_eq!(&f[12..16], [0xff, 0, 0, 100]);
        blend_cursor(&mut f, 2, 2, Rect::new(5, 5, 1, 1), &opaque); // outside: no panic
    }
}
