use super::model::run;
use super::sample::sample_chw;
use crate::{Image, Rect, TextLine, Word};
use anyhow::{bail, Result};
use ort::session::Session;

const REC_H: usize = 48;
const REC_MAX_W: usize = 3200;
const REC_DROP_SCORE: f32 = 0.5;

pub fn recognize_box(
    rec: &mut Session,
    dict: &[String],
    img: &Image,
    b: Rect,
) -> Result<Option<TextLine>> {
    let dw = ((REC_H as f32 * b.w as f32 / b.h as f32).round() as usize).clamp(16, REC_MAX_W);
    let data = sample_chw(img, b, dw, REC_H, [0.5; 3], [0.5; 3]);
    let (shape, p) = run(rec, [1, 3, REC_H, dw], data)?;
    let (t, c) = match shape[..] {
        [1, t, c] if t > 0 && c > 0 => (t as usize, c as usize),
        _ => bail!("unexpected recognition output shape {shape:?}"),
    };
    let chars = ctc_greedy(&p, t, c, dict);
    if chars.is_empty() {
        return Ok(None);
    }
    let score = chars.iter().map(|c| c.conf).sum::<f32>() / chars.len() as f32;
    if score < REC_DROP_SCORE {
        return Ok(None);
    }
    let text: String = chars
        .iter()
        .map(|c| c.text.as_str())
        .collect::<String>()
        .trim()
        .to_string();
    Ok((!text.is_empty()).then(|| TextLine {
        text,
        rect: b,
        words: words_of(&chars, b, t),
    }))
}

/// Group decoded characters into words split at spaces; `t` is the step count of the
/// CTC output, which maps linearly onto the box width.
fn words_of(chars: &[Decoded], b: Rect, t: usize) -> Vec<Word> {
    let x_at = |step: usize| b.x + (step as f32 / t as f32 * b.w as f32).round() as i32;
    let mut words: Vec<Word> = Vec::new();
    let mut cur: Option<(String, usize, usize)> = None;
    let mut flush = |cur: &mut Option<(String, usize, usize)>| {
        if let Some((text, s, e)) = cur.take() {
            let (x0, x1) = (x_at(s), x_at(e).min(b.x + b.w));
            words.push(Word {
                text,
                rect: Rect {
                    x: x0,
                    y: b.y,
                    w: (x1 - x0).max(1),
                    h: b.h,
                },
            });
        }
    };
    for ch in chars {
        if ch.text.trim().is_empty() {
            flush(&mut cur);
        } else if let Some((text, _, e)) = cur.as_mut() {
            text.push_str(&ch.text);
            *e = ch.end;
        } else {
            cur = Some((ch.text.clone(), ch.start, ch.end));
        }
    }
    flush(&mut cur);
    words
}

struct Decoded {
    text: String,
    start: usize,
    /// exclusive
    end: usize,
    conf: f32,
}

/// CTC greedy decode of `t` steps x `c` classes (class 0 = blank, class i+1 = dict[i],
/// class dict.len()+1 = space).
fn ctc_greedy(p: &[f32], t: usize, c: usize, dict: &[String]) -> Vec<Decoded> {
    let mut out: Vec<Decoded> = Vec::new();
    let mut prev = 0;
    for step in 0..t {
        let row = &p[step * c..(step + 1) * c];
        let (idx, &conf) = row
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap();
        if idx != 0 {
            if idx == prev {
                if let Some(l) = out.last_mut() {
                    l.end = step + 1;
                }
            } else {
                let text = dict.get(idx - 1).map_or(" ", |s| s.as_str()).to_string();
                out.push(Decoded {
                    text,
                    start: step,
                    end: step + 1,
                    conf,
                });
            }
        }
        prev = idx;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctc_collapses_repeats_and_blanks() {
        let dict: Vec<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
        // classes: blank,a,b,space ; steps: a a blank a b b
        let hot = |i: usize| {
            (0..4)
                .map(|k| if k == i { 0.9 } else { 0.0 })
                .collect::<Vec<f32>>()
        };
        let p: Vec<f32> = [1, 1, 0, 1, 2, 2].iter().flat_map(|&i| hot(i)).collect();
        let d = ctc_greedy(&p, 6, 4, &dict);
        assert_eq!(d.iter().map(|c| c.text.as_str()).collect::<String>(), "aab");
        assert_eq!((d[0].start, d[0].end, d[2].start, d[2].end), (0, 2, 4, 6));
    }
}
