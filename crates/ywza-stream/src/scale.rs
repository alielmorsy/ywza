/// Frames wider than this are scaled down.
pub const MAX_WIDTH: u32 = 1920;

/// Encoded size: at most [`MAX_WIDTH`] wide, aspect kept, both even (I420 needs it).
pub fn target_size(w: u32, h: u32) -> (u32, u32) {
    let tw = w.min(MAX_WIDTH);
    let th = (h as u64 * tw as u64 / w.max(1) as u64) as u32;
    ((tw & !1).max(2), (th & !1).max(2))
}

/// Bilinear resize of tightly packed 4-byte pixels.
pub fn resize_bgra(src: &[u8], w: u32, h: u32, tw: u32, th: u32, out: &mut Vec<u8>) {
    out.clear();
    out.reserve((tw * th * 4) as usize);
    let (w, h) = (w as usize, h as usize);
    let sx = w as f32 / tw as f32;
    let sy = h as f32 / th as f32;
    let px = |x: usize, y: usize, c: usize| src[(y * w + x) * 4 + c] as f32;
    for ty in 0..th as usize {
        let fy = ((ty as f32 + 0.5) * sy - 0.5).clamp(0.0, (h - 1) as f32);
        let (y0, wy) = (fy as usize, fy.fract());
        let y1 = (y0 + 1).min(h - 1);
        for tx in 0..tw as usize {
            let fx = ((tx as f32 + 0.5) * sx - 0.5).clamp(0.0, (w - 1) as f32);
            let (x0, wx) = (fx as usize, fx.fract());
            let x1 = (x0 + 1).min(w - 1);
            for c in 0..4 {
                let top = px(x0, y0, c) * (1.0 - wx) + px(x1, y0, c) * wx;
                let bot = px(x0, y1, c) * (1.0 - wx) + px(x1, y1, c) * wx;
                out.push((top * (1.0 - wy) + bot * wy + 0.5) as u8);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_cap_width_and_stay_even() {
        assert_eq!(target_size(1920, 1080), (1920, 1080));
        assert_eq!(target_size(3840, 2160), (1920, 1080));
        assert_eq!(target_size(2560, 1440), (1920, 1080));
        assert_eq!(target_size(1366, 768), (1366, 768));
        assert_eq!(target_size(1025, 769), (1024, 768));
        let mut out = Vec::new();
        resize_bgra(&[10; 4 * 4 * 4], 4, 4, 2, 2, &mut out);
        assert_eq!(out, [10; 2 * 2 * 4]);
    }
}
