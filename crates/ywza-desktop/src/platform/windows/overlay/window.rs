//! One layered, click-through, topmost window per mark, backed by a BGRA DIB.

use crate::Rect;
use ::windows::core::w;
use ::windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use ::windows::Win32::Graphics::Gdi::*;
use ::windows::Win32::System::LibraryLoader::GetModuleHandleW;
use ::windows::Win32::UI::WindowsAndMessaging::*;

const CLASS: ::windows::core::PCWSTR = w!("ywza-lens-mark");

unsafe extern "system" fn proc(h: HWND, m: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(h, m, wp, lp) }
}

pub(super) fn register() -> bool {
    unsafe {
        let Ok(module) = GetModuleHandleW(None) else {
            return false;
        };
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(proc),
            hInstance: module.into(),
            lpszClassName: CLASS,
            ..Default::default()
        };
        RegisterClassExW(&class) != 0
    }
}

pub(super) struct Font(pub(super) HFONT);

impl Font {
    pub(super) fn new(scale: f32) -> Option<Font> {
        let f = unsafe {
            CreateFontW(
                -crate::lens::px(12.0, scale),
                0,
                0,
                0,
                FW_SEMIBOLD.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                ANTIALIASED_QUALITY,
                0,
                w!("Segoe UI"),
            )
        };
        (!f.is_invalid()).then_some(Font(f))
    }

    pub(super) fn width(&self, text: &[u16]) -> i32 {
        unsafe {
            let dc = CreateCompatibleDC(None);
            let old = SelectObject(dc, self.0.into());
            let mut size = SIZE::default();
            let _ = GetTextExtentPoint32W(dc, text, &mut size);
            SelectObject(dc, old);
            let _ = DeleteDC(dc);
            size.cx
        }
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.0.into());
        }
    }
}

pub(super) struct Surface {
    hwnd: HWND,
    pub(super) dc: HDC,
    bmp: HBITMAP,
    old_bmp: HGDIOBJ,
    old_font: HGDIOBJ,
    _font: Font,
    bits: *mut u8,
    pub(super) bounds: Rect,
}

impl Surface {
    pub(super) fn new(bounds: Rect, font: Font) -> Option<Surface> {
        let Rect { x, y, w, h } = bounds;
        if w <= 0 || h <= 0 {
            return None;
        }
        unsafe {
            let info = BITMAPINFO {
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
            let mut bits = std::ptr::null_mut();
            let bmp = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
            let dc = CreateCompatibleDC(None);
            let old_bmp = SelectObject(dc, bmp.into());
            let old_font = SelectObject(dc, font.0.into());
            let _ = SetBkMode(dc, TRANSPARENT);
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOPMOST
                    | WS_EX_NOACTIVATE
                    | WS_EX_TOOLWINDOW,
                CLASS,
                w!(""),
                WS_POPUP,
                x,
                y,
                w,
                h,
                None,
                None,
                GetModuleHandleW(None).ok().map(Into::into),
                None,
            );
            let mut s = Surface {
                hwnd: HWND::default(),
                dc,
                bmp,
                old_bmp,
                old_font,
                _font: font,
                bits: bits as *mut u8,
                bounds,
            };
            s.hwnd = hwnd.ok()?;
            Some(s)
        }
    }

    pub(super) fn pixels(&mut self) -> &mut [u8] {
        let n = (self.bounds.w * self.bounds.h * 4) as usize;
        unsafe { std::slice::from_raw_parts_mut(self.bits, n) }
    }

    /// Pushes the DIB with `alpha` as the overall opacity.
    pub(super) fn push(&self, alpha: u8) {
        let pos = POINT {
            x: self.bounds.x,
            y: self.bounds.y,
        };
        let size = SIZE {
            cx: self.bounds.w,
            cy: self.bounds.h,
        };
        let src = POINT::default();
        let blend = blend(alpha);
        unsafe {
            let _ = UpdateLayeredWindow(
                self.hwnd,
                None,
                Some(&pos as *const _),
                Some(&size as *const _),
                Some(self.dc),
                Some(&src as *const _),
                COLORREF(0),
                Some(&blend as *const _),
                ULW_ALPHA,
            );
        }
    }

    /// Changes only the overall opacity, keeping the pushed pixels.
    pub(super) fn set_alpha(&self, alpha: u8) {
        let blend = blend(alpha);
        unsafe {
            let _ = UpdateLayeredWindow(
                self.hwnd,
                None,
                None,
                None,
                None,
                None,
                COLORREF(0),
                Some(&blend as *const _),
                ULW_ALPHA,
            );
        }
    }

    pub(super) fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
        }
    }
}

fn blend(alpha: u8) -> BLENDFUNCTION {
    BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: alpha,
        AlphaFormat: AC_SRC_ALPHA as u8,
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            if !self.hwnd.is_invalid() {
                let _ = DestroyWindow(self.hwnd);
            }
            SelectObject(self.dc, self.old_font);
            SelectObject(self.dc, self.old_bmp);
            let _ = DeleteObject(self.bmp.into());
            let _ = DeleteDC(self.dc);
        }
    }
}
