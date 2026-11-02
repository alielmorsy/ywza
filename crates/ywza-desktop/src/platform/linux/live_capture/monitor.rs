use super::cursor::blend_cursor;
use super::shm::{shm_attach, Shm};
use crate::platform::linux::x::X;
use crate::{Error, Rect, Result};
use std::thread::sleep;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt as _, ImageFormat, ImageOrder};
use x11rb::protocol::{shm, xfixes};

/// How often the monitor layout is re-checked.
const LAYOUT_EVERY: Duration = Duration::from_secs(1);

/// Continuous capture of one monitor. Not `Send`.
pub struct MonitorCapture {
    x: X,
    index: usize,
    rect: Rect,
    msb: bool,
    shm: Option<Shm>,
    cursor: bool,
    frame: Duration,
    prev: Vec<u8>,
    next_at: Instant,
    layout_at: Instant,
}

impl MonitorCapture {
    /// Starts capturing monitor `index` (as in `list_monitors`) at up to `fps`, with the
    /// cursor composited in when `cursor` is set and XFixes is available.
    pub fn new(index: usize, fps: u32, cursor: bool) -> Result<Self> {
        let x = X::connect()?;
        let rect = monitor_rect(&x, index)?;
        if rect.is_empty() || rect.w > i16::MAX as i32 || rect.h > i16::MAX as i32 {
            return Err(Error::Os(format!(
                "cannot capture a {}x{} monitor",
                rect.w, rect.h
            )));
        }
        let msb = x.conn.setup().image_byte_order == ImageOrder::MSB_FIRST;
        let shm = shm_attach(&x, (rect.w * rect.h * 4) as usize);
        let cursor = cursor
            && xfixes::query_version(&x.conn, 4, 0)
                .ok()
                .and_then(|c| c.reply().ok())
                .is_some();
        let now = Instant::now();
        Ok(Self {
            x,
            index,
            rect,
            msb,
            shm,
            cursor,
            frame: Duration::from_secs(1) / fps.max(1),
            prev: Vec::new(),
            next_at: now,
            layout_at: now + LAYOUT_EVERY,
        })
    }

    /// Waits up to `timeout` for a changed frame and writes it into `out` as tightly
    /// packed BGRA. Returns its size, or None on timeout.
    pub fn next(&mut self, timeout: Duration, out: &mut Vec<u8>) -> Result<Option<(u32, u32)>> {
        let deadline = Instant::now() + timeout;
        loop {
            let now = Instant::now();
            if self.next_at > now {
                sleep(self.next_at - now);
            }
            let now = Instant::now();
            self.next_at = now + self.frame;
            if now >= self.layout_at {
                self.check_layout()?;
            }
            self.grab(out)?;
            if self.cursor {
                self.draw_cursor(out);
            }
            if *out != self.prev {
                self.prev.clear();
                self.prev.extend_from_slice(out);
                return Ok(Some((self.rect.w as u32, self.rect.h as u32)));
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
        }
    }

    fn check_layout(&mut self) -> Result<()> {
        self.layout_at = Instant::now() + LAYOUT_EVERY;
        if monitor_rect(&self.x, self.index)? != self.rect {
            return Err(Error::Os(format!("monitor {} changed", self.index)));
        }
        Ok(())
    }

    fn grab(&self, out: &mut Vec<u8>) -> Result<()> {
        let r = self.rect;
        let (x, y, w, h) = (r.x as i16, r.y as i16, r.w as u16, r.h as u16);
        let (conn, root) = (&self.x.conn, self.x.root);
        match &self.shm {
            Some(s) => {
                let z = ImageFormat::Z_PIXMAP.into();
                let rep = shm::get_image(conn, root, x, y, w, h, !0, z, s.seg, 0)?.reply()?;
                self.check_depth(rep.depth)?;
                // The server has written the segment once the reply arrived.
                let data = unsafe { std::slice::from_raw_parts(s.ptr as *const u8, s.len) };
                bgrx_to_bgra(data, self.msb, out);
            }
            None => {
                let rep = conn
                    .get_image(ImageFormat::Z_PIXMAP, root, x, y, w, h, !0)?
                    .reply()?;
                self.check_depth(rep.depth)?;
                bgrx_to_bgra(&rep.data, self.msb, out);
            }
        }
        if out.len() != (r.w * r.h * 4) as usize {
            return Err(Error::Os("short image from the X server".into()));
        }
        Ok(())
    }

    fn check_depth(&self, depth: u8) -> Result<()> {
        let bpp = self
            .x
            .conn
            .setup()
            .pixmap_formats
            .iter()
            .find(|f| f.depth == depth)
            .map(|f| f.bits_per_pixel);
        if bpp != Some(32) {
            return Err(Error::Os(format!(
                "unsupported pixel format (depth {depth}, {bpp:?} bpp)"
            )));
        }
        Ok(())
    }

    fn draw_cursor(&self, out: &mut [u8]) {
        let Some(c) = xfixes::get_cursor_image(&self.x.conn)
            .ok()
            .and_then(|c| c.reply().ok())
        else {
            return;
        };
        let r = self.rect;
        let at = Rect::new(
            c.x as i32 - c.xhot as i32 - r.x,
            c.y as i32 - c.yhot as i32 - r.y,
            c.width as i32,
            c.height as i32,
        );
        blend_cursor(out, r.w, r.h, at, &c.cursor_image);
    }
}

impl Drop for MonitorCapture {
    fn drop(&mut self) {
        if let Some(s) = self.shm.take() {
            let _ = shm::detach(&self.x.conn, s.seg).map(|c| c.check());
            unsafe { libc::shmdt(s.ptr) };
        }
    }
}

fn monitor_rect(x: &X, index: usize) -> Result<Rect> {
    x.monitors()?
        .get(index)
        .map(|m| m.rect)
        .ok_or_else(|| Error::NotFound(format!("monitor {index}")))
}

/// 32-bit ZPixmap pixels (B,G,R,X on little-endian servers, X,R,G,B on big-endian) to opaque BGRA.
fn bgrx_to_bgra(src: &[u8], msb: bool, out: &mut Vec<u8>) {
    out.clear();
    out.extend(src.chunks_exact(4).flat_map(|p| {
        if msb {
            [p[3], p[2], p[1], 255]
        } else {
            [p[0], p[1], p[2], 255]
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_order() {
        let mut out = Vec::new();
        bgrx_to_bgra(&[1, 2, 3, 0], false, &mut out);
        assert_eq!(out, [1, 2, 3, 255]);
        bgrx_to_bgra(&[0, 3, 2, 1], true, &mut out);
        assert_eq!(out, [1, 2, 3, 255]);
    }
}
