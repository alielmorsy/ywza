use super::pixels::bgra_to_rgba;
use crate::platform::windows::util::{hwnd, rect};
use crate::{Error, Image, Rect, Result, WindowId};
use ::windows::Win32::Foundation::RECT;
use ::windows::Win32::Graphics::Gdi::*;
use ::windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
use ::windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

const PW_RENDERFULLCONTENT: u32 = 2;

/// GDI fallback for windows WGC cannot capture. Covers the whole window rect,
/// including the invisible resize border; `origin` accounts for that.
pub(super) fn print_window(id: WindowId) -> Result<Image> {
    let h = hwnd(id);
    unsafe {
        let mut r = RECT::default();
        GetWindowRect(h, &mut r)?;
        let Rect { x, y, w, h: ht } = rect(r);
        if w <= 0 || ht <= 0 {
            return Err(Error::Os(format!("w{id} has no area")));
        }
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, ht);
        let old = SelectObject(mem, bmp.into());
        let printed = PrintWindow(h, mem, PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT)).as_bool();
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -ht, // top-down rows
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut rgba = vec![0u8; (w * ht * 4) as usize];
        let rows = GetDIBits(
            mem,
            bmp,
            0,
            ht as u32,
            Some(rgba.as_mut_ptr() as *mut _),
            &mut info,
            DIB_RGB_COLORS,
        );
        SelectObject(mem, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        if !printed || rows == 0 {
            return Err(Error::Os(format!("could not capture w{id}")));
        }
        bgra_to_rgba(&mut rgba);
        for px in rgba.as_chunks_mut::<4>().0 {
            px[3] = 255; // GDI leaves alpha undefined
        }
        Ok(Image {
            width: w as u32,
            height: ht as u32,
            rgba,
            origin: (x, y),
        })
    }
}
