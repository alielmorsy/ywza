use crate::Rect;

/// Split `w`x`h` into a balanced grid of tiles no larger than `max` per side.
#[cfg_attr(not(all(windows, feature = "ocr-windows")), allow(dead_code))]
pub(crate) fn tiles(w: u32, h: u32, max: u32) -> Vec<Rect> {
    let (cols, rows) = (w.div_ceil(max).max(1), h.div_ceil(max).max(1));
    let (tw, th) = (w.div_ceil(cols), h.div_ceil(rows));
    let mut v = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let (x, y) = (c * tw, r * th);
            if x < w && y < h {
                v.push(Rect {
                    x: x as i32,
                    y: y as i32,
                    w: tw.min(w - x) as i32,
                    h: th.min(h - y) as i32,
                });
            }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiling_covers_image_without_overlap() {
        assert_eq!(
            tiles(100, 50, 100),
            vec![Rect {
                x: 0,
                y: 0,
                w: 100,
                h: 50
            }]
        );
        let t = tiles(250, 90, 100);
        assert_eq!(t.len(), 3);
        assert_eq!(t.iter().map(|r| r.x).collect::<Vec<_>>(), vec![0, 84, 168]);
        assert!(t.iter().all(|r| r.w <= 100 && r.h == 90));
        let area: i64 = tiles(250, 230, 100)
            .iter()
            .map(|r| (r.w * r.h) as i64)
            .sum();
        assert_eq!(area, 250 * 230);
        // offsets: a box at (5,5) in tile 2 maps to tile.x + 5
        assert_eq!(t[1].x + 5, 89);
    }
}
