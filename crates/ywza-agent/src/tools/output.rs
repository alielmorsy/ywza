use std::future::Future;

use ywza_desktop::{self as desk, Image, Rect, Snapshot, WindowId};

pub const MAX_OUTPUT: usize = 20 * 1024;
/// Long side of screenshots sent to the model (clients get full resolution).
pub const MODEL_IMAGE_MAX: u32 = 1568;

#[derive(Debug, Default, Clone)]
pub struct ToolOutput {
    pub text: String,
    pub ok: bool,
    pub image_png: Option<Vec<u8>>,
    /// Older results with the same key are replaced by "(element list superseded)" in the LLM history.
    pub supersedes_key: Option<String>,
    /// Element actions that diff: whether the UI visibly changed. None: not measured.
    pub changed: Option<bool>,
}

impl ToolOutput {
    pub fn ok(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ok: true,
            ..Default::default()
        }
    }

    pub fn err(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Default::default()
        }
    }

    pub(crate) fn elements(mut self, w: WindowId) -> Self {
        self.supersedes_key = Some(format!("elements:w{w}"));
        self
    }
}

/// Tool bodies use `?` on the error side, which is itself a `ToolOutput`.
pub type R = Result<ToolOutput, ToolOutput>;

pub async fn flat(f: impl Future<Output = R>) -> ToolOutput {
    match f.await {
        Ok(o) | Err(o) => o,
    }
}

pub fn desk_err(e: desk::Error) -> ToolOutput {
    ToolOutput::err(match e {
        desk::Error::UserBusy => "user is using the mouse; retry shortly".to_string(),
        e => e.to_string(),
    })
}

pub fn term_err(e: ywza_term::Error) -> ToolOutput {
    ToolOutput::err(e.0)
}

/// Keeps the last `MAX_OUTPUT` bytes (the end of a log is usually what matters).
pub fn truncate(text: String) -> String {
    if text.len() <= MAX_OUTPUT {
        return text;
    }
    let mut cut = text.len() - MAX_OUTPUT;
    while !text.is_char_boundary(cut) {
        cut += 1;
    }
    format!(
        "(output truncated: first {cut} of {} bytes dropped)\n{}",
        text.len(),
        &text[cut..]
    )
}

/// `r` in image pixels, clipped to the image. None when nothing is left.
pub fn crop(img: &Image, r: Rect) -> Option<ywza_ocr::Image> {
    let r = r.intersect(&Rect::new(0, 0, img.width as i32, img.height as i32));
    if r.is_empty() {
        return None;
    }
    let mut rgba = Vec::with_capacity(r.w as usize * r.h as usize * 4);
    for y in r.y..r.y + r.h {
        let start = (y as usize * img.width as usize + r.x as usize) * 4;
        rgba.extend_from_slice(&img.rgba[start..start + r.w as usize * 4]);
    }
    Some(ywza_ocr::Image {
        width: r.w as u32,
        height: r.h as u32,
        rgba,
    })
}

/// Box-filter downscale so the long side is at most `max`. Smaller images pass through.
pub fn downscale(img: Image, max: u32) -> Image {
    let long = img.width.max(img.height);
    if long <= max {
        return img;
    }
    let scale = |v: u32| ((v as u64 * max as u64 / long as u64) as u32).max(1);
    let (w, h) = (scale(img.width), scale(img.height));
    let (sw, sh) = (img.width as usize, img.height as usize);
    let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
    for dy in 0..h as usize {
        let y0 = dy * sh / h as usize;
        let y1 = ((dy + 1) * sh / h as usize).max(y0 + 1);
        for dx in 0..w as usize {
            let x0 = dx * sw / w as usize;
            let x1 = ((dx + 1) * sw / w as usize).max(x0 + 1);
            let mut sum = [0u32; 4];
            for y in y0..y1 {
                let row = &img.rgba[(y * sw + x0) * 4..(y * sw + x1) * 4];
                for px in row.as_chunks::<4>().0 {
                    for c in 0..4 {
                        sum[c] += px[c] as u32;
                    }
                }
            }
            let count = ((y1 - y0) * (x1 - x0)) as u32;
            rgba.extend(sum.iter().map(|s| (s / count) as u8));
        }
    }
    Image {
        width: w,
        height: h,
        rgba,
        origin: img.origin,
    }
}

/// `stale: element N (reason)` plus the fresh snapshot.
pub fn stale(w: WindowId, id: u32, reason: &str, fresh: desk::Result<Snapshot>) -> ToolOutput {
    tracing::warn!(id, reason, "stale element");
    let tail = match fresh {
        Ok(s) => format!("Fresh element list:\n{}", s.render()),
        Err(e) => format!("Could not re-read the window: {e}"),
    };
    ToolOutput::err(format!(
        "stale: element {id} ({reason}); nothing was done. {tail}"
    ))
    .elements(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_long_output() {
        let t = truncate("é".repeat(MAX_OUTPUT));
        assert!(t.starts_with("(output truncated"));
        assert!(t.len() < MAX_OUTPUT + 100);
    }

    #[test]
    fn downscale_dims() {
        let img = |w: u32, h: u32| Image {
            width: w,
            height: h,
            rgba: vec![200; (w * h * 4) as usize],
            origin: (5, 6),
        };
        let d = downscale(img(3840, 2160), MODEL_IMAGE_MAX);
        assert_eq!((d.width, d.height), (1568, 882));
        assert_eq!(d.rgba.len(), 1568 * 882 * 4);
        assert_eq!(d.origin, (5, 6));
        assert!(d.rgba.iter().all(|&b| b == 200));
        let d = downscale(img(1000, 4000), MODEL_IMAGE_MAX);
        assert_eq!((d.width, d.height), (392, 1568));
        let d = downscale(img(800, 600), MODEL_IMAGE_MAX);
        assert_eq!((d.width, d.height), (800, 600));
        let d = downscale(img(5000, 1), 1568);
        assert_eq!((d.width, d.height), (1568, 1));
    }

    #[test]
    fn crop_clips() {
        let img = Image {
            width: 10,
            height: 10,
            rgba: (0..400).map(|i| i as u8).collect(),
            origin: (0, 0),
        };
        let c = crop(&img, Rect::new(8, 8, 5, 5)).unwrap();
        assert_eq!((c.width, c.height), (2, 2));
        assert_eq!(&c.rgba[..4], &img.rgba[(8 * 10 + 8) * 4..][..4]);
        assert!(crop(&img, Rect::new(20, 20, 5, 5)).is_none());
    }

    #[test]
    fn errors_are_not_ok() {
        assert!(ToolOutput::ok("x").ok && !ToolOutput::err("x").ok);
        assert_eq!(
            ToolOutput::ok("x").elements(7).supersedes_key.as_deref(),
            Some("elements:w7")
        );
    }
}
