//! Builders for SendInput records.

use crate::{Error, Result};
use ::windows::Win32::UI::Input::KeyboardAndMouse::*;
use ::windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

/// Keys that need the extended flag or they turn into numpad keys.
const EXTENDED: &[VIRTUAL_KEY] = &[
    VK_INSERT,
    VK_DELETE,
    VK_HOME,
    VK_END,
    VK_PRIOR,
    VK_NEXT,
    VK_UP,
    VK_DOWN,
    VK_LEFT,
    VK_RIGHT,
    VK_LWIN,
    VK_APPS,
    VK_SNAPSHOT,
];

/// Virtual-desktop pixels to the 0..=65535 range of absolute mouse events.
pub(crate) fn absolute((x, y): (i32, i32)) -> (i32, i32) {
    let norm =
        |v: i32, origin, span: i32| ((v - origin) as i64 * 65535 / (span - 1).max(1) as i64) as i32;
    unsafe {
        (
            norm(
                x,
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
            ),
            norm(
                y,
                GetSystemMetrics(SM_YVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
            ),
        )
    }
}

/// Absolute mouse event in virtual-desktop coordinates.
pub(super) fn mouse(flags: MOUSE_EVENT_FLAGS, at: (i32, i32), data: i32) -> INPUT {
    let (dx, dy) = absolute(at);
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: data as u32,
                dwFlags: flags | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                ..Default::default()
            },
        },
    }
}

pub(crate) fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    let mut flags = if up {
        KEYEVENTF_KEYUP
    } else {
        KEYBD_EVENT_FLAGS(0)
    };
    if EXTENDED.contains(&vk) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: unsafe { MapVirtualKeyW(vk.0 as u32, MAPVK_VK_TO_VSC) } as u16,
                dwFlags: flags,
                ..Default::default()
            },
        },
    }
}

pub(super) fn unicode(unit: u16, up: bool) -> INPUT {
    let up = if up {
        KEYEVENTF_KEYUP
    } else {
        KEYBD_EVENT_FLAGS(0)
    };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wScan: unit,
                dwFlags: KEYEVENTF_UNICODE | up,
                ..Default::default()
            },
        },
    }
}

pub(super) fn send(inputs: &[INPUT]) -> Result<()> {
    let sent = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        // UIPI drops input into elevated windows when we are not elevated.
        return Err(Error::Os("input was blocked (elevated window?)".into()));
    }
    Ok(())
}
