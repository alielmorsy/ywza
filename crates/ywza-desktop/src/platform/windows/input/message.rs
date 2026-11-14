//! Window-message tier: classic Win32 controls (Button, Edit, RichEdit) driven through
//! their own HWND, so the cursor, focus and foreground stay where they are. Every
//! function returns true only when the effect was read back or the message is reliable.

use crate::platform::windows::uia::{self, Uia};
use ::windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use ::windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
use ::windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetClientRect, GetWindowLongPtrW, PostMessageW, SendMessageTimeoutW,
    BM_GETCHECK, GWL_STYLE, SMTO_ABORTIFHUNG, SMTO_BLOCK, WM_GETTEXT, WM_GETTEXTLENGTH,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_NULL, WM_SETTEXT,
};
use std::thread::sleep;
use std::time::{Duration, Instant};

/// A hung app costs at most this per message.
const SEND_MS: u32 = 500;
const ES_MULTILINE: u32 = 0x4;
const ES_READONLY: u32 = 0x800;

#[derive(Debug, PartialEq)]
enum Kind {
    Push,
    Check,
    Radio,
    Edit,
}

/// Maps a window class (plain, or WinForms' `WindowsForms10.<CLASS>.app...` wrapper)
/// and its style to a control we can drive.
fn kind(class: &str, style: u32) -> Option<Kind> {
    let class = class.to_ascii_lowercase();
    let base = match class.strip_prefix("windowsforms10.") {
        Some(rest) => rest.split('.').next().unwrap_or_default(),
        None => &class,
    };
    match base {
        "button" => match style & 0xF {
            0 | 1 => Some(Kind::Push),
            2 | 3 | 5 | 6 => Some(Kind::Check),
            4 | 9 => Some(Kind::Radio),
            _ => None, // group box, owner-drawn, command link, split button
        },
        b if b == "edit" || b.starts_with("richedit") => Some(Kind::Edit),
        _ => None,
    }
}

/// The element's own window, when it is a classic control.
fn native(el: &Uia) -> Option<(HWND, Kind, u32)> {
    let h = unsafe { el.CurrentNativeWindowHandle() }.ok()?;
    if h.0.is_null() {
        return None;
    }
    let mut buf = [0u16; 64];
    let n = unsafe { GetClassNameW(h, &mut buf) };
    let class = String::from_utf16_lossy(&buf[..usize::try_from(n).ok()?]);
    let style = unsafe { GetWindowLongPtrW(h, GWL_STYLE) } as u32;
    Some((h, kind(&class, style)?, style))
}

fn send(h: HWND, msg: u32, w: usize, l: isize) -> Option<usize> {
    let mut out = 0usize;
    let r = unsafe {
        SendMessageTimeoutW(
            h,
            msg,
            WPARAM(w),
            LPARAM(l),
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            SEND_MS,
            Some(&mut out as *mut usize),
        )
    };
    (r.0 != 0).then_some(out)
}

fn usable(h: HWND) -> bool {
    unsafe { IsWindowEnabled(h) }.as_bool()
}

/// Clicks a push button, checkbox or radio without activating its window. BM_CLICK may
/// fail in an inactive dialog, so the button gets the mouse messages it would get from
/// a real click (posted: a click that opens a modal dialog must not block us). A
/// checkbox must change state and a radio must end up selected; a push button has
/// nothing to read back, so it needs to be enabled and its app to answer a ping.
/// None: not applicable, nothing sent. Some(false): sent but no state change was seen;
/// the caller must not click again, the messages may still land late.
pub(super) fn click(el: &Uia) -> Option<bool> {
    let (h, kind, _) = native(el)?;
    if kind == Kind::Edit || !usable(h) || send(h, WM_NULL, 0, 0).is_none() {
        return None;
    }
    let mut r = RECT::default();
    if unsafe { GetClientRect(h, &mut r) }.is_err() {
        return None;
    }
    let at = LPARAM((((r.bottom / 2) as u32) << 16 | ((r.right / 2) as u32 & 0xFFFF)) as isize);
    let before = send(h, BM_GETCHECK, 0, 0);
    let posted = unsafe {
        PostMessageW(Some(h), WM_LBUTTONDOWN, WPARAM(1), at).is_ok()
            && PostMessageW(Some(h), WM_LBUTTONUP, WPARAM(0), at).is_ok()
    };
    if !posted {
        return None;
    }
    if kind == Kind::Push {
        return Some(true);
    }
    let done = |now: Option<usize>| match kind {
        Kind::Radio => now == Some(1),
        _ => now.is_some() && now != before,
    };
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        if done(send(h, BM_GETCHECK, 0, 0)) {
            return Some(true);
        }
        sleep(Duration::from_millis(25));
    }
    Some(false)
}

/// Sets (or appends to) an edit control's text with WM_SETTEXT and reads it back.
/// Declines read-only or disabled edits, and control characters other than line
/// breaks and tabs in a multi-line edit.
pub(super) fn set_text(el: &Uia, text: &str, append: bool) -> bool {
    let Some((h, Kind::Edit, style)) = native(el) else {
        return false;
    };
    let multiline = style & ES_MULTILINE != 0;
    let plain = |c: char| !c.is_control() || (multiline && matches!(c, '\n' | '\r' | '\t'));
    if style & ES_READONLY != 0 || !usable(h) || !text.chars().all(plain) {
        return false;
    }
    let want = if append {
        get_text(h).map(|cur| cur + text)
    } else {
        Some(text.to_string())
    };
    let Some(want) = want else {
        return false;
    };
    let wide: Vec<u16> = crlf(&want).encode_utf16().chain([0]).collect();
    send(h, WM_SETTEXT, 0, wide.as_ptr() as isize) == Some(1)
        && get_text(h).is_some_and(|now| uia::norm(&now) == uia::norm(&want))
}

fn get_text(h: HWND) -> Option<String> {
    let n = send(h, WM_GETTEXTLENGTH, 0, 0)?;
    let mut buf = vec![0u16; n + 1];
    let got = send(h, WM_GETTEXT, buf.len(), buf.as_mut_ptr() as isize)?;
    Some(String::from_utf16_lossy(&buf[..got.min(n)]))
}

/// Edit controls break lines with CR LF.
fn crlf(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\n', "\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_controls_are_recognised() {
        assert_eq!(kind("Button", 0), Some(Kind::Push));
        assert_eq!(kind("Button", 1), Some(Kind::Push));
        assert_eq!(kind("Button", 3), Some(Kind::Check));
        assert_eq!(kind("Button", 0x5000_0009), Some(Kind::Radio));
        assert_eq!(kind("Button", 7), None); // group box
        assert_eq!(kind("Edit", 0), Some(Kind::Edit));
        assert_eq!(kind("RICHEDIT50W", 0), Some(Kind::Edit));
        assert_eq!(
            kind("WindowsForms10.BUTTON.app.0.141b42a_r6_ad1", 0),
            Some(Kind::Push)
        );
        assert_eq!(
            kind("WindowsForms10.EDIT.app.0.141b42a_r6_ad1", 0),
            Some(Kind::Edit)
        );
        assert_eq!(kind("Chrome_RenderWidgetHostHWND", 0), None);
        assert_eq!(kind("ComboBox", 0), None);
    }

    #[test]
    fn edits_get_crlf_line_breaks() {
        assert_eq!(crlf("a\nb\r\nc"), "a\r\nb\r\nc");
    }
}
