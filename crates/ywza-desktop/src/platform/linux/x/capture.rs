use super::conn::X;
use crate::{Error, Image, Rect, Result, WindowId};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{self, ConnectionExt as _, ImageFormat, Window};

impl X {
    /// RGBA pixels of a drawable region (32 bits per pixel ZPixmap only).
    fn grab(&self, drawable: u32, r: Rect) -> Result<Image> {
        if r.is_empty() || r.w > i16::MAX as i32 || r.h > i16::MAX as i32 {
            return Err(Error::Os(format!("cannot capture a {}x{} area", r.w, r.h)));
        }
        let reply = self
            .conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                drawable,
                r.x as i16,
                r.y as i16,
                r.w as u16,
                r.h as u16,
                !0,
            )?
            .reply()?;
        let setup = self.conn.setup();
        let bpp = setup
            .pixmap_formats
            .iter()
            .find(|f| f.depth == reply.depth)
            .map(|f| f.bits_per_pixel);
        if bpp != Some(32) {
            return Err(Error::Os(format!(
                "unsupported pixel format (depth {}, {bpp:?} bpp)",
                reply.depth
            )));
        }
        let msb = setup.image_byte_order == xproto::ImageOrder::MSB_FIRST;
        let rgba = bgrx_to_rgba(&reply.data, msb);
        Ok(Image {
            width: r.w as u32,
            height: r.h as u32,
            rgba,
            origin: (0, 0),
        })
    }
}

/// 32-bit ZPixmap pixels (B,G,R,X on little-endian servers, X,R,G,B on big-endian) to opaque RGBA.
fn bgrx_to_rgba(data: &[u8], msb: bool) -> Vec<u8> {
    data.as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            if msb {
                [p[1], p[2], p[3], 255]
            } else {
                [p[2], p[1], p[0], 255]
            }
        })
        .collect()
}

/// Captures a window's client area. Minimized windows have no pixels: focus them first.
pub fn capture_window(id: WindowId) -> Result<Image> {
    let x = X::connect()?;
    let info = x.window_info(id)?;
    if info.minimized {
        return Err(Error::Os(format!("w{id} is minimized; focus it first")));
    }
    let w = id as Window;
    let client = x.client_rect(w)?;
    // The window's own pixels (right under a compositor even when covered), else the
    // screen area where it sits.
    match x.grab(w, Rect::new(0, 0, client.w, client.h)) {
        Ok(mut img) => {
            // Open menus are separate windows: paint them in so OCR sees their items.
            for (p, r) in x.popups(w) {
                if let Ok(pimg) = x.grab(p, Rect::new(0, 0, r.w, r.h)) {
                    paste(&mut img, &pimg, r.x - client.x, r.y - client.y);
                }
            }
            Ok(Image {
                origin: (client.x, client.y),
                ..img
            })
        }
        Err(_) => {
            let g = x.conn.get_geometry(x.root)?.reply()?;
            let r = client.intersect(&Rect::new(0, 0, g.width as i32, g.height as i32));
            Ok(Image {
                origin: (r.x, r.y),
                ..x.grab(x.root, r)?
            })
        }
    }
}

/// Captures monitor `index` (as in `list_monitors`).
pub fn capture_monitor(index: usize) -> Result<Image> {
    let x = X::connect()?;
    let m = x
        .monitors()?
        .into_iter()
        .nth(index)
        .ok_or_else(|| Error::NotFound(format!("monitor {index}")))?;
    let img = x.grab(x.root, m.rect)?;
    Ok(Image {
        origin: (m.rect.x, m.rect.y),
        ..img
    })
}

/// Copies `src` onto `dst` with its top-left at (`dx`, `dy`), clipped to `dst`.
fn paste(dst: &mut Image, src: &Image, dx: i32, dy: i32) {
    let (dw, dh) = (dst.width as i32, dst.height as i32);
    let x0 = dx.max(0);
    let x1 = (dx + src.width as i32).min(dw);
    if x0 >= x1 {
        return;
    }
    for y in dy.max(0)..(dy + src.height as i32).min(dh) {
        let s = (((y - dy) * src.width as i32 + (x0 - dx)) * 4) as usize;
        let d = ((y * dw + x0) * 4) as usize;
        let n = ((x1 - x0) * 4) as usize;
        dst.rgba[d..d + n].copy_from_slice(&src.rgba[s..s + n]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paste_clips_popup_into_window() {
        let img = |w: u32, h: u32, v: u8| Image {
            width: w,
            height: h,
            rgba: vec![v; (w * h * 4) as usize],
            origin: (0, 0),
        };
        let mut dst = img(4, 3, 0);
        paste(&mut dst, &img(3, 2, 9), 2, 2); // hangs off the right and bottom
        let px = |d: &Image, x: u32, y: u32| d.rgba[((y * 4 + x) * 4) as usize];
        let d = &dst;
        assert_eq!(
            (px(d, 1, 2), px(d, 2, 2), px(d, 3, 2), px(d, 2, 1)),
            (0, 9, 9, 0)
        );
        paste(&mut dst, &img(2, 2, 7), -1, -1); // hangs off the top-left
        assert_eq!((px(&dst, 0, 0), px(&dst, 1, 0), px(&dst, 0, 1)), (7, 0, 0));
        paste(&mut dst, &img(2, 2, 5), 10, 0); // fully outside: no panic
    }

    #[test]
    fn pixel_order() {
        assert_eq!(bgrx_to_rgba(&[1, 2, 3, 0], false), [3, 2, 1, 255]);
        assert_eq!(bgrx_to_rgba(&[0, 3, 2, 1], true), [3, 2, 1, 255]);
    }
}
