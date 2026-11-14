//! Clipboard swap for paste-based typing: the user's clipboard comes back afterwards.

use crate::{Error, Result};
use ::windows::core::w;
use ::windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use ::windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use ::windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};
use ::windows::Win32::System::Ole::CF_UNICODETEXT;
use std::thread::sleep;
use std::time::Duration;

/// Formats held as GDI handles, not memory blocks (bitmap, palette, metafiles, owner
/// display). Windows synthesizes CF_BITMAP from CF_DIB, so images still come back.
// shortcut: GDI-handle and delayed-only formats are not restored, copy them too if users lose clipboard content.
const GDI: &[u32] = &[2, 3, 9, 14, 0x80, 0x82, 0x83, 0x8E];

/// `text` on the clipboard while this lives; dropping it puts the previous content back.
pub(super) struct Held(Vec<(u32, Vec<u8>)>);

impl Held {
    pub(super) fn put(text: &str) -> Result<Held> {
        let saved = open(|| Ok(save()))?;
        let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
        let bytes =
            unsafe { std::slice::from_raw_parts(units.as_ptr().cast::<u8>(), units.len() * 2) };
        open(|| write(&[(CF_UNICODETEXT.0 as u32, bytes.to_vec())]))?;
        Ok(Held(saved))
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        // The app reads the clipboard when it handles Ctrl+V, which may be after we return.
        sleep(Duration::from_millis(200));
        let _ = open(|| write(&self.0));
    }
}

fn open<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    // Another app may hold it for a moment.
    for _ in 0..20 {
        if unsafe { OpenClipboard(None) }.is_ok() {
            let r = f();
            let _ = unsafe { CloseClipboard() };
            return r;
        }
        sleep(Duration::from_millis(25));
    }
    Err(Error::Os("the clipboard is busy; nothing typed".into()))
}

fn save() -> Vec<(u32, Vec<u8>)> {
    let mut out = Vec::new();
    let mut f = 0;
    loop {
        f = unsafe { EnumClipboardFormats(f) };
        if f == 0 {
            return out;
        }
        if !GDI.contains(&f) {
            if let Some(b) = unsafe { bytes(f) } {
                out.push((f, b));
            }
        }
    }
}

unsafe fn bytes(format: u32) -> Option<Vec<u8>> {
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

/// Replaces the clipboard, kept out of clipboard history and cloud sync (typed text
/// may be a password; restored content is already in the history).
fn write(items: &[(u32, Vec<u8>)]) -> Result<()> {
    unsafe {
        EmptyClipboard()?;
        for (f, b) in items {
            set(*f, b)?;
        }
        let private = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
        set(private, &[0; 4])
    }
}

unsafe fn set(format: u32, data: &[u8]) -> Result<()> {
    unsafe {
        let g = GlobalAlloc(GMEM_MOVEABLE, data.len().max(1))?;
        let p = GlobalLock(g).cast::<u8>();
        if p.is_null() {
            let _ = GlobalFree(Some(g));
            return Err(Error::Os("out of memory for the clipboard".into()));
        }
        std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
        let _ = GlobalUnlock(g);
        if let Err(e) = SetClipboardData(format, Some(HANDLE(g.0))) {
            let _ = GlobalFree(Some(g));
            return Err(e.into());
        }
        Ok(())
    }
}
