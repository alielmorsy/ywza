pub fn fit(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize, out: &mut Vec<u32>) {
    out.clear();
    out.resize(dw * dh, 0);
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 {
        return;
    }
    let (fw, fh) = if dw * sh <= dh * sw {
        (dw, (sh * dw / sw).max(1))
    } else {
        ((sw * dh / sh).max(1), dh)
    };
    let (ox, oy) = ((dw - fw) / 2, (dh - fh) / 2);
    for y in 0..fh {
        let row = &mut out[(oy + y) * dw + ox..][..fw];
        for (x, px) in row.iter_mut().enumerate() {
            let c = if fw == sw && fh == sh {
                let i = (y * sw + x) * 3;
                [src[i] as u32, src[i + 1] as u32, src[i + 2] as u32]
            } else if fw < sw || fh < sh {
                area(src, sw, sh, fw, fh, x, y)
            } else {
                bilinear(src, sw, sh, fw, fh, x, y)
            };
            *px = c[0] << 16 | c[1] << 8 | c[2];
        }
    }
}

fn area(src: &[u8], sw: usize, sh: usize, fw: usize, fh: usize, x: usize, y: usize) -> [u32; 3] {
    let (x0, y0) = (x * sw / fw, y * sh / fh);
    let x1 = ((x + 1) * sw / fw).clamp(x0 + 1, sw);
    let y1 = ((y + 1) * sh / fh).clamp(y0 + 1, sh);
    let mut sum = [0u32; 3];
    for sy in y0..y1 {
        for sx in x0..x1 {
            let i = (sy * sw + sx) * 3;
            for k in 0..3 {
                sum[k] += src[i + k] as u32;
            }
        }
    }
    let n = ((x1 - x0) * (y1 - y0)) as u32;
    sum.map(|s| (s + n / 2) / n)
}

fn bilinear(
    src: &[u8],
    sw: usize,
    sh: usize,
    fw: usize,
    fh: usize,
    x: usize,
    y: usize,
) -> [u32; 3] {
    let pos =
        |d: usize, s: usize, f: usize| ((d as f32 + 0.5) * s as f32 / f as f32 - 0.5).max(0.0);
    let (fx, fy) = (pos(x, sw, fw), pos(y, sh, fh));
    let (x0, y0) = (fx as usize, fy as usize);
    let (x1, y1) = ((x0 + 1).min(sw - 1), (y0 + 1).min(sh - 1));
    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
    let at = |sx: usize, sy: usize, k: usize| src[(sy * sw + sx) * 3 + k] as f32;
    let mut c = [0u32; 3];
    for k in 0..3 {
        let top = at(x0, y0, k) * (1.0 - tx) + at(x1, y0, k) * tx;
        let bot = at(x0, y1, k) * (1.0 - tx) + at(x1, y1, k) * tx;
        c[k] = (top * (1.0 - ty) + bot * ty + 0.5) as u32;
    }
    c
}

#[cfg(test)]
mod tests {
    use super::fit;

    fn src4() -> Vec<u8> {
        (0..16)
            .flat_map(|i| {
                let v = if (i % 4) < 2 { 0 } else { 100 } + if i < 8 { 0 } else { 40 };
                [v, v, v]
            })
            .collect()
    }

    #[test]
    fn downscale_averages_and_pads() {
        let mut out = Vec::new();
        fit(&src4(), 4, 4, 2, 2, &mut out);
        assert_eq!(out, [0x000000, 0x646464, 0x282828, 0x8c8c8c]);

        fit(&src4(), 4, 4, 4, 2, &mut out);
        assert_eq!(out, [0, 0x000000, 0x646464, 0, 0, 0x282828, 0x8c8c8c, 0]);
    }
}
