//! The current clipboard as dock context: copied files, else text, else an image saved as a PNG.

use super::context::ContextItem;
use std::path::Path;

const MAX_TEXT: usize = 20000;
const MAX_FILES: usize = 50;
const CF_DIB: u32 = 8;
const CF_UNICODETEXT: u32 = 13;
const CF_HDROP: u32 = 15;
const CF_DIBV5: u32 = 17;

/// Empty when the clipboard is empty, busy or holds nothing usable.
pub fn read() -> Vec<ContextItem> {
    // Files and text first: Office and browsers put a picture next to copied text.
    let Some((files, text, dib)) = win::open(|| unsafe {
        let files = win::bytes(CF_HDROP);
        let text = if files.is_none() { win::bytes(CF_UNICODETEXT) } else { None };
        let dib = if files.is_none() && text.is_none() {
            win::bytes(CF_DIB).or_else(|| win::bytes(CF_DIBV5))
        } else {
            None
        };
        (files, text, dib)
    }) else {
        return Vec::new();
    };
    if let Some(b) = files {
        return drop_files(&b)
            .into_iter()
            .take(MAX_FILES)
            .map(|path| ContextItem::Path { dir: Path::new(&path).is_dir(), path })
            .collect();
    }
    if let Some(t) = text.as_deref().and_then(utf16_text) {
        return vec![ContextItem::Text { text: t }];
    }
    dib.as_deref()
        .and_then(dib_rgba)
        .and_then(|(w, h, px)| save_png(w, h, &px))
        .map(|path| vec![ContextItem::Image { path }])
        .unwrap_or_default()
}

fn utf16_text(b: &[u8]) -> Option<String> {
    let units: Vec<u16> = b
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    let s = String::from_utf16_lossy(&units);
    (!s.trim().is_empty()).then(|| s.chars().take(MAX_TEXT).collect())
}

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

/// Paths in a DROPFILES block.
fn drop_files(b: &[u8]) -> Vec<String> {
    let (Some(start), Some(wide)) = (u32_at(b, 0), u32_at(b, 16)) else {
        return Vec::new();
    };
    let Some(list) = b.get(start as usize..) else {
        return Vec::new();
    };
    let names: Vec<String> = if wide != 0 {
        let units: Vec<u16> = list.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        units.split(|&u| u == 0).map(String::from_utf16_lossy).collect()
    } else {
        list.split(|&c| c == 0).map(|s| String::from_utf8_lossy(s).into_owned()).collect()
    };
    names.into_iter().take_while(|s| !s.is_empty()).collect()
}

/// `(width, height, RGBA)` of a 24 or 32 bit packed DIB.
// shortcut: assumes the standard BGR(A) masks and skips palette and compressed DIBs, which screen snips never use.
fn dib_rgba(b: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let size = u32_at(b, 0)? as usize;
    let w = u32_at(b, 4)? as i32;
    let h = u32_at(b, 8)? as i32;
    let bpp = u16::from_le_bytes(b.get(14..16)?.try_into().ok()?) as usize;
    let comp = u32_at(b, 16)?;
    let used = u32_at(b, 32)? as usize;
    if size < 40 || w <= 0 || h == 0 || !(bpp == 24 || bpp == 32) || !(comp == 0 || comp == 3) {
        return None;
    }
    let (w, rows) = (w as usize, h.unsigned_abs() as usize);
    let masks = if comp == 3 && size == 40 { 12 } else { 0 };
    let start = size + masks + used.checked_mul(4)?;
    let stride = (w.checked_mul(bpp / 8)? + 3) & !3;
    let px = b.get(start..start.checked_add(stride.checked_mul(rows)?)?)?;
    let mut out = Vec::with_capacity(w * rows * 4);
    for y in 0..rows {
        let row = if h > 0 { rows - 1 - y } else { y };
        for p in px[row * stride..].chunks_exact(bpp / 8).take(w) {
            out.extend_from_slice(&[p[2], p[1], p[0], if bpp == 32 { p[3] } else { 255 }]);
        }
    }
    // Most apps leave the alpha byte of 32 bit DIBs at zero.
    if bpp == 32 && out.chunks_exact(4).all(|p| p[3] == 0) {
        out.chunks_exact_mut(4).for_each(|p| p[3] = 255);
    }
    Some((w as u32, rows as u32, out))
}

fn save_png(w: u32, h: u32, rgba: &[u8]) -> Option<String> {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();
    let path = std::env::temp_dir().join(format!("ywza-clip-{ms}.png"));
    let file = std::io::BufWriter::new(std::fs::File::create(&path).ok()?);
    let mut enc = png::Encoder::new(file, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().ok()?;
    writer.write_image_data(rgba).ok()?;
    writer.finish().ok()?;
    Some(path.to_string_lossy().into_owned())
}

mod win {
    use std::thread::sleep;
    use std::time::Duration;
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    pub fn open<T>(f: impl FnOnce() -> T) -> Option<T> {
        // Another app may hold it for a moment.
        for _ in 0..20 {
            if unsafe { OpenClipboard(None) }.is_ok() {
                let r = f();
                let _ = unsafe { CloseClipboard() };
                return Some(r);
            }
            sleep(Duration::from_millis(25));
        }
        None
    }

    /// Call with the clipboard open.
    pub unsafe fn bytes(format: u32) -> Option<Vec<u8>> {
        unsafe {
            let g = HGLOBAL(GetClipboardData(format).ok()?.0);
            let n = GlobalSize(g);
            let p = GlobalLock(g).cast::<u8>();
            if p.is_null() || n == 0 {
                return None;
            }
            let v = std::slice::from_raw_parts(p, n).to_vec();
            let _ = GlobalUnlock(g);
            Some(v)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn text_is_capped_and_blank_is_none() {
        assert_eq!(utf16_text(&wide("hi\0junk")), Some("hi".into()));
        assert_eq!(utf16_text(&wide("  \r\n\0")), None);
        assert_eq!(utf16_text(&wide(&"x".repeat(30000))).unwrap().len(), MAX_TEXT);
    }

    #[test]
    fn dropfiles() {
        let mut b = vec![0u8; 20];
        b[0] = 20;
        b[16] = 1;
        b.extend(wide("C:\\a.txt\0C:\\dir\0\0"));
        assert_eq!(drop_files(&b), vec!["C:\\a.txt", "C:\\dir"]);
        assert!(drop_files(&[1, 2]).is_empty());
    }

    fn header(w: i32, h: i32, bpp: u16) -> Vec<u8> {
        let mut b = vec![0u8; 40];
        b[0] = 40;
        b[4..8].copy_from_slice(&w.to_le_bytes());
        b[8..12].copy_from_slice(&h.to_le_bytes());
        b[14..16].copy_from_slice(&bpp.to_le_bytes());
        b
    }

    #[test]
    fn bottom_up_24_bit_rows_are_flipped_and_padded() {
        let mut b = header(1, 2, 24);
        b.extend([1, 2, 3, 0, 4, 5, 6, 0]);
        let (w, h, px) = dib_rgba(&b).unwrap();
        assert_eq!((w, h), (1, 2));
        assert_eq!(px, vec![6, 5, 4, 255, 3, 2, 1, 255]);
    }

    #[test]
    fn zero_alpha_becomes_opaque() {
        let mut b = header(2, -1, 32);
        b.extend([1, 2, 3, 0, 4, 5, 6, 0]);
        assert_eq!(dib_rgba(&b).unwrap().2, vec![3, 2, 1, 255, 6, 5, 4, 255]);
    }

    #[test]
    fn truncated_dib_is_rejected() {
        let mut b = header(100, 100, 32);
        b.extend([0; 16]);
        assert!(dib_rgba(&b).is_none());
        assert!(dib_rgba(&header(1, 1, 8)).is_none());
    }
}
