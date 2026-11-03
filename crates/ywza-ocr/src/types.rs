use anyhow::{bail, Result};

/// RGBA8, row-major, `rgba.len() == width * height * 4`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Pixel rectangle in image coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    pub text: String,
    pub rect: Rect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextLine {
    pub text: String,
    pub rect: Rect,
    pub words: Vec<Word>,
}

impl Rect {
    pub(crate) fn union_all(it: impl IntoIterator<Item = Rect>) -> Option<Rect> {
        it.into_iter().reduce(|a, b| {
            let (x, y) = (a.x.min(b.x), a.y.min(b.y));
            let (r, bt) = ((a.x + a.w).max(b.x + b.w), (a.y + a.h).max(b.y + b.h));
            Rect {
                x,
                y,
                w: r - x,
                h: bt - y,
            }
        })
    }
}

impl Image {
    /// All pixels have the same colour (an empty result is then expected, not a failure).
    pub(crate) fn is_blank(&self) -> bool {
        let px = self.rgba.as_chunks::<4>().0;
        px.first().is_none_or(|f| px.iter().all(|p| p == f))
    }

    pub(crate) fn check(&self) -> Result<()> {
        if self.rgba.len() as u64 != self.width as u64 * self.height as u64 * 4 {
            bail!(
                "image buffer is {} bytes, expected {}x{}x4",
                self.rgba.len(),
                self.width,
                self.height
            );
        }
        Ok(())
    }
}

impl TextLine {
    /// The line split where words sit far apart (a gap over 3/4 of the text height),
    /// so menu bars and toolbars ("File   View") give one piece per item instead of
    /// one box whose centre lies between them. A line without word boxes stays whole.
    pub fn segments(&self) -> Vec<(String, Rect)> {
        let mut groups: Vec<Vec<&Word>> = Vec::new();
        for w in &self.words {
            match groups.last_mut() {
                Some(g)
                    if {
                        let prev = g.last().unwrap().rect;
                        let h = prev.h.max(w.rect.h);
                        (w.rect.x - (prev.x + prev.w)) * 4 <= h * 3
                    } =>
                {
                    g.push(w)
                }
                _ => groups.push(vec![w]),
            }
        }
        if groups.len() <= 1 {
            return vec![(self.text.clone(), self.rect)];
        }
        groups
            .into_iter()
            .map(|g| {
                let text = g
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                (text, Rect::union_all(g.iter().map(|w| w.rect)).unwrap())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(t: &str) -> TextLine {
        TextLine {
            text: t.into(),
            rect: Rect::default(),
            words: vec![],
        }
    }

    #[test]
    fn segments_split_on_wide_gaps() {
        let word = |t: &str, x: i32| Word {
            text: t.into(),
            rect: Rect {
                x,
                y: 0,
                w: 30,
                h: 12,
            },
        };
        let l = TextLine {
            text: "File View Load LUT".into(),
            rect: Rect {
                x: 0,
                y: 0,
                w: 194,
                h: 12,
            },
            // File..View: 20px gap (menu); Load LUT: 4px (a space).
            words: vec![
                word("File", 0),
                word("View", 50),
                word("Load", 130),
                word("LUT", 164),
            ],
        };
        let s: Vec<_> = l
            .segments()
            .into_iter()
            .map(|(t, r)| (t, r.x, r.w))
            .collect();
        assert_eq!(
            s,
            [
                ("File".into(), 0, 30),
                ("View".into(), 50, 30),
                ("Load LUT".into(), 130, 64)
            ]
        );
        // Normal sentence spacing stays one line, with the engine's own text.
        let l = TextLine {
            text: "Load  LUT".into(),
            words: vec![word("Load", 0), word("LUT", 34)],
            ..l
        };
        assert_eq!(l.segments(), [("Load  LUT".into(), l.rect)]);
        assert_eq!(line("x").segments(), [("x".into(), Rect::default())]);
    }

    #[test]
    fn blank_detection() {
        let blank = Image {
            width: 2,
            height: 1,
            rgba: vec![9; 8],
        };
        assert!(blank.is_blank());
        let mut px = blank.clone();
        px.rgba[4] = 0;
        assert!(!px.is_blank());
    }
}
