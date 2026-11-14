use crate::platform::windows::util::{com, from_wide, rect};
use crate::{Rect, Result};
use ::windows::Win32::Foundation::POINT;
use ::windows::Win32::Graphics::Gdi::ClientToScreen;
use ::windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use ::windows::Win32::UI::Accessibility::*;
use ::windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GUITHREADINFO,
};

/// Terminals draw their own text and expose no editable pattern, but always take keys.
const TERMINALS: &[&str] = &[
    "CASCADIA_HOSTING_WINDOW_CLASS", // Windows Terminal
    "ConsoleWindowClass",
    "mintty",
    "PuTTY",
    "VirtualConsoleClass", // ConEmu
    "org.wezfurlong.wezterm",
    "Alacritty",
];

/// Where typed text would land, for the live view: no API says "this takes text", so it
/// combines the system caret (Win32 and most native editors), the focused UIA element's
/// type and patterns (browsers, WinUI, Electron) and known terminal windows.
pub struct FocusWatch {
    uia: IUIAutomation,
}

impl FocusWatch {
    pub fn new() -> Result<Self> {
        com();
        let uia: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
        Ok(Self { uia })
    }

    /// Screen rect of the caret, else of the focused text field. None: nothing takes text.
    pub fn now(&self) -> Result<Option<Rect>> {
        let caret = caret();
        let terminal = TERMINALS.contains(&foreground_class().as_str());
        let field = unsafe { self.uia.GetFocusedElement() }
            .ok()
            .filter(|e| terminal || takes_text(e))
            .and_then(|e| unsafe { e.CurrentBoundingRectangle() }.ok().map(rect))
            .filter(|r| r.w > 0 && r.h > 0);
        Ok(match (field, caret) {
            // Chromium leaves a stale caret around: trust it only inside the field.
            (Some(f), Some(c)) if !f.intersect(&c).is_empty() => Some(c),
            (Some(f), _) => Some(f),
            (None, c) => c,
        })
    }
}

fn takes_text(e: &IUIAutomationElement) -> bool {
    unsafe {
        if !e.CurrentIsEnabled().is_ok_and(|b| b.as_bool()) {
            return false;
        }
        let read_only = e
            .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
            .ok()
            .map(|v| v.CurrentIsReadOnly().map_or(true, |b| b.as_bool()));
        let text = e
            .GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
            .is_ok();
        let Ok(kind) = e.CurrentControlType() else {
            return false;
        };
        if kind == UIA_EditControlTypeId {
            read_only != Some(true)
        } else if kind == UIA_DocumentControlTypeId {
            // A browser page is a read-only Document; Word and rich editors are writable ones.
            read_only == Some(false) || (read_only.is_none() && text)
        } else {
            read_only == Some(false)
        }
    }
}

/// The system caret of the foreground thread, in screen pixels.
fn caret() -> Option<Rect> {
    let mut gi = GUITHREADINFO {
        cbSize: size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetGUIThreadInfo(0, &mut gi) }.ok()?;
    let r = gi.rcCaret;
    if gi.hwndCaret.0.is_null() || r.bottom <= r.top {
        return None;
    }
    let mut p = POINT {
        x: r.left,
        y: r.top,
    };
    if !unsafe { ClientToScreen(gi.hwndCaret, &mut p) }.as_bool() {
        return None;
    }
    Some(Rect::new(p.x, p.y, (r.right - r.left).max(2), r.bottom - r.top))
}

fn foreground_class() -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(GetForegroundWindow(), &mut buf) };
    from_wide(&buf[..n.max(0) as usize])
}
