use crate::tile::tiles;
use crate::{Image, Recognizer, Rect, TextLine, Word};
use anyhow::{anyhow, Context, Result};
use windows::{
    Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap},
    Media::Ocr::OcrEngine,
    Storage::Streams::DataWriter,
    Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED},
};

pub struct WindowsOcr;

impl Recognizer for WindowsOcr {
    fn recognize(&self, img: &Image) -> Result<Vec<TextLine>> {
        recognize(img)
    }
}

fn recognize(img: &Image) -> Result<Vec<TextLine>> {
    // Ignore errors: S_FALSE (already initialised) and RPC_E_CHANGED_MODE (STA thread) are both fine.
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(|e| anyhow!("Windows OCR: no OCR language available for the user profile: {e}"))?;
    let max = OcrEngine::MaxImageDimension()?.max(64);
    let mut out = Vec::new();
    for t in tiles(img.width, img.height, max) {
        let bmp = tile_bitmap(img, t).context("build SoftwareBitmap")?;
        let res = engine.RecognizeAsync(&bmp)?.join()?;
        for line in res.Lines()? {
            let mut words = Vec::new();
            for w in line.Words()? {
                let r = w.BoundingRect()?;
                let x0 = r.X.floor() as i32;
                let y0 = r.Y.floor() as i32;
                words.push(Word {
                    text: w.Text()?.to_string(),
                    rect: Rect {
                        x: t.x + x0,
                        y: t.y + y0,
                        w: ((r.X + r.Width).ceil() as i32 - x0).max(1),
                        h: ((r.Y + r.Height).ceil() as i32 - y0).max(1),
                    },
                });
            }
            if let Some(rect) = Rect::union_all(words.iter().map(|w| w.rect)) {
                out.push(TextLine {
                    text: line.Text()?.to_string(),
                    rect,
                    words,
                });
            }
        }
    }
    Ok(out)
}

/// RGBA sub-rectangle -> opaque BGRA8 SoftwareBitmap.
fn tile_bitmap(img: &Image, t: Rect) -> Result<SoftwareBitmap> {
    let stride = img.width as usize * 4;
    let mut buf = Vec::with_capacity(t.w as usize * t.h as usize * 4);
    for y in t.y as usize..(t.y + t.h) as usize {
        let row = &img.rgba[y * stride + t.x as usize * 4..y * stride + (t.x + t.w) as usize * 4];
        for px in row.as_chunks::<4>().0 {
            buf.extend_from_slice(&[px[2], px[1], px[0], 255]);
        }
    }
    let w = DataWriter::new()?;
    w.WriteBytes(&buf)?;
    Ok(SoftwareBitmap::CreateCopyFromBuffer(
        &w.DetachBuffer()?,
        BitmapPixelFormat::Bgra8,
        t.w,
        t.h,
    )?)
}
