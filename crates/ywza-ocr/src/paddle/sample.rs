use crate::{Image, Rect};

/// Bilinear-resize `region` of `img` to `dw`x`dh` and return normalised planar
/// BGR floats (PaddleOCR normalises the BGR array channel by channel).
pub fn sample_chw(
    img: &Image,
    region: Rect,
    dw: usize,
    dh: usize,
    mean: [f32; 3],
    std: [f32; 3],
) -> Vec<f32> {
    let (iw, ih) = (img.width as usize, img.height as usize);
    let (rx, ry, rw, rh) = (
        region.x as f32,
        region.y as f32,
        region.w as f32,
        region.h as f32,
    );
    let (sx, sy) = (rw / dw as f32, rh / dh as f32);
    let px = |x: usize, y: usize, ch: usize| img.rgba[(y * iw + x) * 4 + ch] as f32;
    let mut out = vec![0.0; 3 * dw * dh];
    for y in 0..dh {
        let fy = (ry + (y as f32 + 0.5) * sy - 0.5).clamp(0.0, (ih - 1) as f32);
        let (y0, ty) = (fy.floor() as usize, fy.fract());
        let y1 = (y0 + 1).min(ih - 1);
        for x in 0..dw {
            let fx = (rx + (x as f32 + 0.5) * sx - 0.5).clamp(0.0, (iw - 1) as f32);
            let (x0, tx) = (fx.floor() as usize, fx.fract());
            let x1 = (x0 + 1).min(iw - 1);
            for (plane, src_ch) in [2usize, 1, 0].into_iter().enumerate() {
                let top = px(x0, y0, src_ch) * (1.0 - tx) + px(x1, y0, src_ch) * tx;
                let bot = px(x0, y1, src_ch) * (1.0 - tx) + px(x1, y1, src_ch) * tx;
                let v = top * (1.0 - ty) + bot * ty;
                out[plane * dw * dh + y * dw + x] = (v / 255.0 - mean[plane]) / std[plane];
            }
        }
    }
    out
}
