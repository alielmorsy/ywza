use crate::{CursorShape, Error, Image, Result};
use ::windows::Win32::Graphics::Gdi::*;
use ::windows::Win32::UI::WindowsAndMessaging::{
    GetCursorInfo, GetIconInfo, CURSORINFO, CURSOR_SHOWING, HCURSOR, HICON, ICONINFO,
};

/// Watches the cursor's shape for the live view (when it isn't drawn into the video).
#[derive(Default)]
pub struct CursorWatch {
    last: Option<usize>,
}

impl CursorWatch {
    pub fn new() -> Result<Self> {
        Ok(Self::default())
    }

    /// The cursor's shape if it changed since the last call (the first call always reports).
    pub fn poll(&mut self) -> Result<Option<CursorShape>> {
        let mut ci = CURSORINFO {
            cbSize: size_of::<CURSORINFO>() as u32,
            ..Default::default()
        };
        unsafe { GetCursorInfo(&mut ci)? };
        let showing = ci.flags.0 & CURSOR_SHOWING.0 != 0 && !ci.hCursor.is_invalid();
        let id = if showing { ci.hCursor.0 as usize } else { 0 };
        if self.last == Some(id) {
            return Ok(None);
        }
        self.last = Some(id);
        if !showing {
            return Ok(Some(CursorShape::default()));
        }
        cursor_shape(ci.hCursor).map(Some)
    }
}

fn cursor_shape(c: HCURSOR) -> Result<CursorShape> {
    let mut ii = ICONINFO::default();
    unsafe { GetIconInfo(HICON(c.0), &mut ii)? };
    let mask = bitmap_bgra(ii.hbmMask);
    let color = (!ii.hbmColor.is_invalid()).then(|| bitmap_bgra(ii.hbmColor));
    unsafe {
        let _ = DeleteObject(ii.hbmMask.into());
        if !ii.hbmColor.is_invalid() {
            let _ = DeleteObject(ii.hbmColor.into());
        }
    }
    let (w, mh, mask) = mask?;
    let rgba = match color {
        Some(c) => {
            let (_, h, mut px) = c?;
            let no_alpha = px.chunks_exact(4).all(|p| p[3] == 0);
            for (i, p) in px.chunks_exact_mut(4).enumerate() {
                p.swap(0, 2); // BGRA -> RGBA
                if no_alpha {
                    // Old-style cursor: the AND mask says which pixels are transparent.
                    p[3] = if mask.get(i * 4) == Some(&0) { 255 } else { 0 };
                }
            }
            (h, px)
        }
        None => {
            // Monochrome: AND mask on top, XOR mask below.
            let h = mh / 2;
            let n = (w * h) as usize;
            let mut px = vec![0u8; n * 4];
            for i in 0..n {
                let and = mask[i * 4] != 0;
                let xor = mask[(n + i) * 4] != 0;
                // ponytail: "invert screen" pixels (and && xor) show black.
                let v = if !and && xor { 255 } else { 0 };
                px[i * 4..i * 4 + 4].copy_from_slice(&[v, v, v, if and && !xor { 0 } else { 255 }]);
            }
            (h, px)
        }
    };
    Ok(CursorShape {
        image: Some(Image {
            width: w as u32,
            height: rgba.0 as u32,
            rgba: rgba.1,
            origin: (0, 0),
        }),
        hot_x: ii.xHotspot,
        hot_y: ii.yHotspot,
    })
}

/// A bitmap as top-down 32-bit BGRA rows: (width, height, pixels).
fn bitmap_bgra(bmp: HBITMAP) -> Result<(i32, i32, Vec<u8>)> {
    unsafe {
        let mut bm = BITMAP::default();
        if GetObjectW(
            bmp.into(),
            size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut _ as *mut _),
        ) == 0
        {
            return Err(Error::Os("cursor bitmap unreadable".into()));
        }
        let (w, h) = (bm.bmWidth, bm.bmHeight);
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down rows
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut px = vec![0u8; (w * h * 4) as usize];
        let screen = GetDC(None);
        let rows = GetDIBits(
            screen,
            bmp,
            0,
            h as u32,
            Some(px.as_mut_ptr() as *mut _),
            &mut info,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, screen);
        if rows == 0 {
            return Err(Error::Os("cursor bitmap unreadable".into()));
        }
        Ok((w, h, px))
    }
}
