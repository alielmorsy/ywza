use super::conn::X;
use crate::{Error, Image, Rect, Result, WindowId};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{self, AtomEnum, ConnectionExt as _, ImageFormat, MapState, Window};

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
    let g = x.conn.get_geometry(x.root)?.reply()?;
    let screen = Rect::new(0, 0, g.width as i32, g.height as i32);
    // Open menus are separate windows that may hang outside the window (a menu below a
    // short window): the capture grows to hold them, so OCR sees all their items.
    let popups = x.popups(w);
    let area = popups
        .iter()
        .fold(client, |a, (_, r)| union(a, *r))
        .intersect(&screen);
    // Uncovered: the screen itself, which is what the user sees and stays current where a
    // compositor's copy of the window lags (seen with Qt chat views). Covered: the window's
    // own pixels (right under a compositor), else the screen area where it sits.
    let own = match x.uncovered(w, &client) {
        true => None,
        false => x.grab(w, Rect::new(0, 0, client.w, client.h)).ok(),
    };
    match own {
        Some(own) => {
            let mut img = Image {
                width: area.w.max(0) as u32,
                height: area.h.max(0) as u32,
                rgba: vec![0; area.area().max(0) as usize * 4],
                origin: (area.x, area.y),
            };
            paste(&mut img, &own, client.x - area.x, client.y - area.y);
            for (p, r) in popups {
                if let Ok(pimg) = x.grab(p, Rect::new(0, 0, r.w, r.h)) {
                    paste(&mut img, &pimg, r.x - area.x, r.y - area.y);
                }
            }
            Ok(img)
        }
        None => Ok(Image {
            origin: (area.x, area.y),
            ..x.grab(x.root, area)?
        }),
    }
}

fn union(a: Rect, b: Rect) -> Rect {
    let (x, y) = (a.x.min(b.x), a.y.min(b.y));
    let right = (a.x + a.w).max(b.x + b.w);
    let bottom = (a.y + a.h).max(b.y + b.h);
    Rect::new(x, y, right - x, bottom - y)
}

impl X {
    /// No other viewable app window above `w` in the stacking order overlaps `r`.
    fn uncovered(&self, w: Window, r: &Rect) -> bool {
        let stack = self.prop32(
            self.root,
            self.a._NET_CLIENT_LIST_STACKING,
            AtomEnum::WINDOW,
        );
        let Some(i) = stack.iter().position(|&s| s == w) else {
            return false;
        };
        stack[i + 1..].iter().all(|&o| {
            let viewable = self
                .conn
                .get_window_attributes(o)
                .ok()
                .and_then(|c| c.reply().ok())
                .is_some_and(|a| a.map_state == MapState::VIEWABLE);
            !viewable
                || self
                    .visible_rect(o)
                    .map_or(true, |v| v.intersect(r).is_empty())
        })
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
