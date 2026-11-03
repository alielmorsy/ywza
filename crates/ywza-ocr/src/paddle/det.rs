use super::model::run;
use super::sample::sample_chw;
use crate::{Image, Rect};
use anyhow::{bail, Result};
use ort::session::Session;

const DET_MAX_SIDE: f32 = 1920.0; // screenshots have small text; PaddleOCR's default is 960
const DET_THRESH: f32 = 0.3;
const BOX_THRESH: f32 = 0.6;
const UNCLIP_RATIO: f32 = 1.5;
const MIN_BOX_SIDE: usize = 3;

type MapBox = (f32, f32, f32, f32);

pub fn detect(det: &mut Session, img: &Image) -> Result<Vec<Rect>> {
    let (w, h) = (img.width as f32, img.height as f32);
    let r = (DET_MAX_SIDE / w.max(h)).min(1.0);
    let round32 = |v: f32| (((v * r / 32.0).round() as usize).max(1)) * 32;
    let (dw, dh) = (round32(w), round32(h));
    let full = Rect {
        x: 0,
        y: 0,
        w: img.width as i32,
        h: img.height as i32,
    };
    let data = sample_chw(
        img,
        full,
        dw,
        dh,
        [0.485, 0.456, 0.406],
        [0.229, 0.224, 0.225],
    );
    let (_, prob) = run(det, [1, 3, dh, dw], data)?;
    if prob.len() != dw * dh {
        bail!(
            "unexpected detection output size {} for {dw}x{dh}",
            prob.len()
        );
    }
    let (sx, sy) = (w / dw as f32, h / dh as f32);
    Ok(boxes_from_map(&prob, dw, dh)
        .into_iter()
        .filter_map(|(x0, y0, x1, y1)| {
            let x = ((x0 * sx).floor() as i32).max(0);
            let y = ((y0 * sy).floor() as i32).max(0);
            let r = Rect {
                x,
                y,
                w: ((x1 * sx).ceil() as i32).min(img.width as i32) - x,
                h: ((y1 * sy).ceil() as i32).min(img.height as i32) - y,
            };
            (r.w > 0 && r.h > 0).then_some(r)
        })
        .collect())
}

/// DB post-processing on a `w`x`h` probability map: threshold, 8-connected
/// components, drop small / low-score ones, unclip the bounding box.
/// Boxes are (x0, y0, x1, y1) in map coordinates, in reading order.
fn boxes_from_map(prob: &[f32], w: usize, h: usize) -> Vec<MapBox> {
    let mut seen = vec![false; w * h];
    let mut boxes: Vec<MapBox> = Vec::new();
    let mut stack = Vec::new();
    for start in 0..w * h {
        if seen[start] || prob[start] <= DET_THRESH {
            continue;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        seen[start] = true;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            for ny in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                for nx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                    let j = ny * w + nx;
                    if !seen[j] && prob[j] > DET_THRESH {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
        let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
        if bw.min(bh) < MIN_BOX_SIDE {
            continue;
        }
        let mut sum = 0.0;
        for y in y0..=y1 {
            sum += prob[y * w + x0..=y * w + x1].iter().sum::<f32>();
        }
        if sum / ((bw * bh) as f32) < BOX_THRESH {
            continue;
        }
        // DB unclip: offset = area * ratio / perimeter.
        let d = (bw * bh) as f32 * UNCLIP_RATIO / (2 * (bw + bh)) as f32;
        boxes.push((
            (x0 as f32 - d).max(0.0),
            (y0 as f32 - d).max(0.0),
            (x1 as f32 + 1.0 + d).min(w as f32),
            (y1 as f32 + 1.0 + d).min(h as f32),
        ));
    }
    reading_order(boxes)
}

/// Rows by vertical position, left to right within a row.
fn reading_order(mut boxes: Vec<MapBox>) -> Vec<MapBox> {
    boxes.sort_by(|a, b| (a.1 + a.3).total_cmp(&(b.1 + b.3)));
    let mut rows: Vec<Vec<MapBox>> = Vec::new();
    for b in boxes {
        let cy = (b.1 + b.3) / 2.0;
        match rows.last_mut() {
            Some(r) if cy >= r[0].1 && cy <= r[0].3 => r.push(b),
            _ => rows.push(vec![b]),
        }
    }
    rows.into_iter()
        .flat_map(|mut r| {
            r.sort_by(|a, b| a.0.total_cmp(&b.0));
            r
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_boxes() {
        // 64x32 map with one 20x6 blob at (10,10) and one tiny speck.
        let (w, h) = (64, 32);
        let mut p = vec![0.0f32; w * h];
        for y in 10..16 {
            for x in 10..30 {
                p[y * w + x] = 0.9;
            }
        }
        p[2 * w + 50] = 0.9;
        let b = boxes_from_map(&p, w, h);
        assert_eq!(b.len(), 1);
        let (x0, y0, x1, y1) = b[0];
        assert!(x0 < 10.0 && y0 < 10.0 && x1 > 30.0 && y1 > 16.0, "{b:?}");
        // d = 120*1.5/52 ~ 3.46
        assert!((x0 - (10.0 - 3.46)).abs() < 0.1);
    }
}
