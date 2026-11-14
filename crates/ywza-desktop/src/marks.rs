//! Numbered element marks drawn onto captured images.

use crate::{Image, Rect, Snapshot};

const MARK: [u8; 4] = [230, 30, 90, 255];
const INK: [u8; 4] = [255, 255, 255, 255];
const SCALE: i32 = 2;

/// 3x5 digit glyphs, one row per nibble (bit 2 = left column).
const DIGITS: [[u8; 5]; 10] = [
    [7, 5, 5, 5, 7],
    [2, 6, 2, 2, 7],
    [7, 1, 7, 4, 7],
    [7, 1, 7, 1, 7],
    [5, 5, 7, 1, 1],
    [7, 4, 7, 1, 7],
    [7, 4, 7, 5, 7],
    [7, 1, 2, 2, 2],
    [7, 5, 7, 5, 7],
    [7, 5, 7, 1, 7],
];

/// Draws each element's box and ID onto an image captured from the same screen area.
pub fn draw_marks(img: &mut Image, snapshot: &Snapshot) {
    for e in &snapshot.elements {
        let r = Rect {
            x: e.rect.x - img.origin.0,
            y: e.rect.y - img.origin.1,
            ..e.rect
        };
        for t in 0..2 {
            fill(img, Rect::new(r.x, r.y + t, r.w, 1), MARK);
            fill(img, Rect::new(r.x, r.y + r.h - 1 - t, r.w, 1), MARK);
            fill(img, Rect::new(r.x + t, r.y, 1, r.h), MARK);
            fill(img, Rect::new(r.x + r.w - 1 - t, r.y, 1, r.h), MARK);
        }
        let digits = e.id.to_string();
        let tag = Rect::new(r.x, r.y, digits.len() as i32 * 4 * SCALE + SCALE, 7 * SCALE);
        fill(img, tag, MARK);
        for (i, d) in digits.bytes().enumerate() {
            let glyph = DIGITS[(d - b'0') as usize];
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..3 {
                    if bits & (4 >> col) != 0 {
                        let x = tag.x + SCALE + (i as i32 * 4 + col) * SCALE;
                        let y = tag.y + SCALE + row as i32 * SCALE;
                        fill(img, Rect::new(x, y, SCALE, SCALE), INK);
                    }
                }
            }
        }
    }
}

fn fill(img: &mut Image, r: Rect, color: [u8; 4]) {
    let r = r.intersect(&Rect::new(0, 0, img.width as i32, img.height as i32));
    if r.is_empty() {
        return;
    }
    for y in r.y..r.y + r.h {
        let row = (y as usize * img.width as usize + r.x as usize) * 4;
        for px in img.rgba[row..row + r.w as usize * 4].as_chunks_mut::<4>().0 {
            *px = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{encode_png, merge_ocr, WindowInfo};

    #[test]
    fn marks_stay_in_bounds_and_png_encodes() {
        let mut img = Image {
            width: 40,
            height: 30,
            rgba: vec![0; 40 * 30 * 4],
            origin: (100, 100),
        };
        let mut snap = Snapshot {
            window: WindowInfo {
                id: 1,
                title: String::new(),
                process: String::new(),
                pid: 0,
                rect: Rect::new(100, 100, 40, 30),
                monitor: 0,
                minimized: false,
                hidden: false,
                focused: false,
                maximized: false,
            },
            seq: 1,
            elements: Vec::new(),
        };
        // Box runs past the image edge.
        merge_ocr(&mut snap, vec![("x".into(), Rect::new(105, 105, 100, 100))]);
        draw_marks(&mut img, &snap);
        assert_eq!(&img.rgba[(5 * 40 + 5) * 4..][..4], &MARK); // top-left corner marked
        let png = encode_png(&img).unwrap();
        assert_eq!(&png[1..4], b"PNG");
    }
}
