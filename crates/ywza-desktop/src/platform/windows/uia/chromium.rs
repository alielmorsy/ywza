//! Chromium and Electron build their web accessibility tree only once an assistive
//! client asks (WM_GETOBJECT on the render widget); until then a window shows just
//! its frame. This asks once per window, as a screen reader does.

use super::handle::Handle;
use super::raw::automation;
use crate::platform::windows::util::hwnd;
use crate::snapshot::Raw;
use crate::WindowId;
use ::windows::core::BOOL;
use ::windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use ::windows::Win32::UI::Accessibility::UiaRootObjectId;
use ::windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, GetClassNameW, SendMessageTimeoutW, OBJID_CLIENT, SMTO_ABORTIFHUNG,
    WM_GETOBJECT,
};
use std::sync::Mutex;
use std::time::Duration;

const RENDER_CLASS: &str = "Chrome_RenderWidgetHostHWND";

static NUDGED: Mutex<Vec<WindowId>> = Mutex::new(Vec::new());

fn lacks_document(raws: &[(Raw, Option<Handle>)]) -> bool {
    raws.iter().all(|(r, _)| r.role != "document")
}

/// A walk with no web document inside a window that hosts a Chromium render widget,
/// not nudged before.
pub(super) fn needs_wake(raws: &[(Raw, Option<Handle>)], window: WindowId) -> bool {
    lacks_document(raws)
        && !NUDGED.lock().is_ok_and(|n| n.contains(&window))
        && !render_widgets(hwnd(window)).is_empty()
}

/// Nudges every render widget of the window and waits up to ~1 s for one to expose
/// children. True when a nudge was sent, so the caller walks again.
pub(super) fn wake(window: WindowId) -> bool {
    if let Ok(mut n) = NUDGED.lock() {
        n.push(window);
    }
    let widgets = render_widgets(hwnd(window));
    for &h in &widgets {
        for objid in [OBJID_CLIENT.0, UiaRootObjectId] {
            unsafe {
                SendMessageTimeoutW(
                    h,
                    WM_GETOBJECT,
                    WPARAM(0),
                    LPARAM(objid as isize),
                    SMTO_ABORTIFHUNG,
                    500,
                    None,
                );
            }
        }
    }
    for _ in 0..10 {
        if widgets.iter().any(|&h| has_children(h)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    !widgets.is_empty()
}

fn has_children(h: HWND) -> bool {
    let Ok(uia) = automation() else { return false };
    unsafe {
        let Ok(walker) = uia.ControlViewWalker() else {
            return false;
        };
        uia.ElementFromHandle(h)
            .and_then(|el| walker.GetFirstChildElement(&el))
            .is_ok()
    }
}

fn render_widgets(top: HWND) -> Vec<HWND> {
    unsafe extern "system" fn visit(h: HWND, out: LPARAM) -> BOOL {
        let mut buf = [0u16; 64];
        let n = unsafe { GetClassNameW(h, &mut buf) }.max(0) as usize;
        if String::from_utf16_lossy(&buf[..n]) == RENDER_CLASS {
            unsafe { (*(out.0 as *mut Vec<HWND>)).push(h) };
        }
        true.into()
    }
    let mut found: Vec<HWND> = Vec::new();
    let _ = unsafe {
        EnumChildWindows(
            Some(top),
            Some(visit),
            LPARAM(&mut found as *mut _ as isize),
        )
    };
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(role: &'static str) -> (Raw, Option<Handle>) {
        (
            Raw {
                role,
                ..Raw::default()
            },
            None,
        )
    }

    #[test]
    fn document_means_web_content_is_there() {
        assert!(lacks_document(&[]));
        assert!(lacks_document(&[node("button"), node("pane")]));
        assert!(!lacks_document(&[node("pane"), node("document")]));
    }
}
