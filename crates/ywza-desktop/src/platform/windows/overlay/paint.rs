//! Draws a mark frame (outline or ripple, plus the caption pill) into a premultiplied BGRA DIB.

use super::window::Surface;
use crate::{MarkKind, Phase, Rect};
use ::windows::Win32::Foundation::{COLORREF, RECT};
use ::windows::Win32::Graphics::Gdi::{
    DrawTextW, GdiFlush, SetTextColor, DT_CENTER, DT_END_ELLIPSIS, DT_NOPREFIX, DT_SINGLELINE,
    DT_VCENTER,
};

const ACCENT: [u8; 3] = [0x87, 0x84, 0x9a];
const OK: [u8; 3] = [0xc0, 0xc1, 0xb2];
const BAD: [u8; 3] = [0xd6, 0x66, 0x7e];
const PILL: [u8; 3] = [0x13, 0x13, 0x0f];
const PILL_ALPHA: u32 = 235; // 92%
const TEXT: COLORREF = COLORREF(0x00ea_eeee);

pub(super) struct Look<'a> {
    pub kind: MarkKind,
    pub phase: Phase,
    /// Target in surface pixels (for `Point`, the ripple's full square).
    pub target: Rect,
    pub pill: Rect,
    pub caption: &'a mut [u16],
    pub scale: f32,
    /// Ripple growth, 0..=1.
    pub ripple: f32,
}

pub(super) fn draw(s: &mut Surface, look: &mut Look) {
    let (w, h) = (s.bounds.w, s.bounds.h);
    let color = match look.phase {
        Phase::Acting => ACCENT,
        Phase::Done => OK,
        Phase::Failed => BAD,
    };
    let stroke = (2.0 * look.scale).max(1.0);
    let t = look.target;
    let pill = look.pill;
    let saved = {
        let buf = s.pixels();
        buf.fill(0);
        let mut c = Canvas { buf, w, h };
        match look.kind {
            MarkKind::Element => {
                let gap = 4.0 * look.scale;
                c.outline(
                    t.x as f32 - gap,
                    t.y as f32 - gap,
                    (t.x + t.w) as f32 + gap,
                    (t.y + t.h) as f32 + gap,
                    6.0 * look.scale,
                    stroke,
                    color,
                );
            }
            MarkKind::Point => {
                let (cx, cy) = (t.x as f32 + t.w as f32 / 2.0, t.y as f32 + t.h as f32 / 2.0);
                c.circle(cx, cy, t.w as f32 / 2.0 * look.ripple, Some(stroke), color);
                c.circle(cx, cy, 3.0 * look.scale, None, color);
            }
        }
        let r = pill.h as f32 / 2.0;
        c.fill(pill, r, PILL);
        c.alphas(pill)
    };
    if !look.caption.is_empty() {
        let pad = crate::lens::px(10.0, look.scale);
        let mut rc = RECT {
            left: pill.x + pad,
            top: pill.y,
            right: pill.x + pill.w - pad,
            bottom: pill.y + pill.h,
        };
        unsafe {
            SetTextColor(s.dc, TEXT);
            DrawTextW(
                s.dc,
                &mut *look.caption,
                &mut rc,
                DT_SINGLELINE | DT_VCENTER | DT_CENTER | DT_NOPREFIX | DT_END_ELLIPSIS,
            );
            let _ = GdiFlush();
        }
    }
    let buf = s.pixels();
    Canvas { buf, w, h }.finish_pill(pill, &saved);
}

struct Canvas<'a> {
    buf: &'a mut [u8],
    w: i32,
    h: i32,
}

impl Canvas<'_> {
    fn put(&mut self, x: i32, y: i32, c: [u8; 3], cov: f32) {
        if cov <= 0.0 || x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let a = cov.min(1.0);
        let inv = 1.0 - a;
        let i = ((y * self.w + x) * 4) as usize;
        let px = &mut self.buf[i..i + 4];
        px[0] = (c[2] as f32 * a + px[0] as f32 * inv) as u8;
        px[1] = (c[1] as f32 * a + px[1] as f32 * inv) as u8;
        px[2] = (c[0] as f32 * a + px[2] as f32 * inv) as u8;
        px[3] = (255.0 * a + px[3] as f32 * inv) as u8;
    }

    /// Rounded-rect stroke centered on the edge; only rows and columns near it are visited.
    #[allow(clippy::too_many_arguments)]
    fn outline(&mut self, l: f32, t: f32, r: f32, b: f32, rad: f32, stroke: f32, c: [u8; 3]) {
        let m = (stroke / 2.0 + 2.0).ceil() as i32;
        let (li, ti, ri, bi, ra) = (l as i32, t as i32, r as i32, b as i32, rad.ceil() as i32);
        for y in (ti - m).max(0)..(bi + m).min(self.h) {
            let near_edge = y <= ti + ra + m || y >= bi - ra - m;
            let spans = if near_edge {
                [(li - m, ri + m), (0, -1)]
            } else {
                [(li - m, li + m), (ri - m, ri + m)]
            };
            for (x0, x1) in spans {
                for x in x0.max(0)..=x1.min(self.w - 1) {
                    let d = rrect_dist(x as f32 + 0.5, y as f32 + 0.5, l, t, r, b, rad);
                    self.put(x, y, c, stroke / 2.0 + 0.5 - d.abs());
                }
            }
        }
    }

    /// A ring of width `stroke`, or a filled disc when `stroke` is None.
    fn circle(&mut self, cx: f32, cy: f32, rad: f32, stroke: Option<f32>, c: [u8; 3]) {
        let ext = rad + stroke.unwrap_or(0.0) + 2.0;
        for y in (cy - ext) as i32..=(cy + ext) as i32 {
            for x in (cx - ext) as i32..=(cx + ext) as i32 {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let d = (dx * dx + dy * dy).sqrt() - rad;
                let cov = match stroke {
                    Some(s) => s / 2.0 + 0.5 - d.abs(),
                    None => 0.5 - d,
                };
                self.put(x, y, c, cov);
            }
        }
    }

    fn fill(&mut self, r: Rect, rad: f32, c: [u8; 3]) {
        let (l, t, rt, b) = (
            r.x as f32,
            r.y as f32,
            (r.x + r.w) as f32,
            (r.y + r.h) as f32,
        );
        for y in r.y..r.y + r.h {
            for x in r.x..r.x + r.w {
                let d = rrect_dist(x as f32 + 0.5, y as f32 + 0.5, l, t, rt, b, rad);
                self.put(x, y, c, 0.5 - d);
            }
        }
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.w && y < self.h).then(|| ((y * self.w + x) * 4 + 3) as usize)
    }

    fn alphas(&self, r: Rect) -> Vec<u8> {
        let mut out = Vec::with_capacity(r.area() as usize);
        for y in r.y..r.y + r.h {
            for x in r.x..r.x + r.w {
                out.push(self.index(x, y).map_or(0, |i| self.buf[i]));
            }
        }
        out
    }

    /// Restores the alpha GDI zeroed under the text, then fades the pill to 92%
    /// scaling all four channels so the pixels stay premultiplied.
    fn finish_pill(&mut self, r: Rect, saved: &[u8]) {
        let mut k = 0;
        for y in r.y..r.y + r.h {
            for x in r.x..r.x + r.w {
                if let Some(i) = self.index(x, y) {
                    self.buf[i] = saved[k];
                    for c in &mut self.buf[i - 3..=i] {
                        *c = (*c as u32 * PILL_ALPHA / 255) as u8;
                    }
                }
                k += 1;
            }
        }
    }
}

fn rrect_dist(px: f32, py: f32, l: f32, t: f32, r: f32, b: f32, rad: f32) -> f32 {
    let (hw, hh) = ((r - l) / 2.0, (b - t) / 2.0);
    let rad = rad.min(hw).min(hh);
    let qx = (px - (l + hw)).abs() - hw + rad;
    let qy = (py - (t + hh)).abs() - hh + rad;
    let (ox, oy) = (qx.max(0.0), qy.max(0.0));
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - rad
}
