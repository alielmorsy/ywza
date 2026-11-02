use ::windows::Win32::Foundation::HWND;
use ::windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use ::windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetWindow, GetWindowLongW, GetWindowTextW, IsWindowVisible, GWL_EXSTYLE,
    GWL_STYLE, GW_CHILD, GW_OWNER, WS_CAPTION, WS_CHILD, WS_EX_APPWINDOW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW,
};

/// What decides whether a top-level window is a real app window.
#[derive(Debug, Default)]
pub(super) struct Facts {
    visible: bool,
    cloaked: bool,
    child: bool,
    tool: bool,
    no_activate: bool,
    /// Has a title bar (WS_CAPTION): real main windows do, helper windows rarely.
    caption: bool,
    /// WS_EX_APPWINDOW, or no owner window.
    app_or_unowned: bool,
    /// Has child windows. Hidden app UIs keep their content children; the many hidden
    /// helper windows of background processes (also captioned and titled) have none.
    has_children: bool,
    title: String,
    class: String,
}

/// Desktop and taskbar surfaces have titles but are not apps.
const SHELL_CLASSES: &[&str] = &[
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
];

pub(super) fn is_app_window(f: &Facts) -> bool {
    f.visible
        && !f.cloaked
        && !f.child
        && !f.tool
        && !f.no_activate
        && !f.title.trim().is_empty()
        && !SHELL_CLASSES.contains(&f.class.as_str())
}

/// A main window hidden by its app (closed to the tray: WhatsApp, Discord, Teams).
/// Listed only for processes without a visible app window, see `list_windows`.
pub(super) fn is_hidden_main(f: &Facts) -> bool {
    !f.visible
        && !f.child
        && !f.tool
        && !f.no_activate
        && f.caption
        && f.app_or_unowned
        && f.has_children
        && !f.title.trim().is_empty()
        && !SHELL_CLASSES.contains(&f.class.as_str())
}

/// What makes a visible top-level window of the snapshot window's process a popup
/// (menu, dropdown, flyout) of it rather than another app window.
pub(super) fn is_popup(f: &Facts, owned_by_root: bool) -> bool {
    f.visible
        && !f.cloaked
        && !f.child
        && (f.class == "#32768" || owned_by_root || !is_app_window(f))
}

pub(super) fn facts(h: HWND) -> Facts {
    unsafe {
        let style = GetWindowLongW(h, GWL_STYLE) as u32;
        let ex = GetWindowLongW(h, GWL_EXSTYLE) as u32;
        let mut cloaked = 0u32;
        let _ = DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut cloaked as *mut _ as *mut _, 4);
        let mut class = [0u16; 256];
        let n = GetClassNameW(h, &mut class) as usize;
        Facts {
            visible: IsWindowVisible(h).as_bool(),
            cloaked: cloaked != 0,
            child: style & WS_CHILD.0 != 0,
            tool: ex & WS_EX_TOOLWINDOW.0 != 0,
            no_activate: ex & WS_EX_NOACTIVATE.0 != 0,
            caption: style & WS_CAPTION.0 == WS_CAPTION.0,
            app_or_unowned: ex & WS_EX_APPWINDOW.0 != 0 || GetWindow(h, GW_OWNER).is_err(),
            has_children: GetWindow(h, GW_CHILD).is_ok(),
            title: title(h),
            class: String::from_utf16_lossy(&class[..n]),
        }
    }
}

pub(super) fn title(h: HWND) -> String {
    let mut buf = [0u16; 512];
    let n = unsafe { GetWindowTextW(h, &mut buf) } as usize;
    String::from_utf16_lossy(&buf[..n])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_window_filter() {
        let app = Facts {
            visible: true,
            title: "Untitled - Notepad".into(),
            class: "Notepad".into(),
            ..Facts::default()
        };
        assert!(is_app_window(&app));
        assert!(!is_app_window(&Facts {
            cloaked: true,
            ..Facts {
                title: "x".into(),
                visible: true,
                ..Facts::default()
            }
        }));
        assert!(!is_app_window(&Facts {
            tool: true,
            visible: true,
            title: "x".into(),
            ..Facts::default()
        }));
        assert!(!is_app_window(&Facts {
            visible: true,
            title: "  ".into(),
            ..Facts::default()
        }));
        assert!(!is_app_window(&Facts {
            visible: false,
            title: "x".into(),
            ..Facts::default()
        }));
        assert!(!is_app_window(&Facts {
            visible: true,
            title: "Program Manager".into(),
            class: "Progman".into(),
            ..Facts::default()
        }));
    }

    #[test]
    fn hidden_main_window_filter() {
        let main = || Facts {
            title: "WhatsApp".into(),
            caption: true,
            app_or_unowned: true,
            has_children: true,
            ..Facts::default()
        };
        assert!(is_hidden_main(&main()) && !is_app_window(&main()));
        assert!(!is_hidden_main(&Facts {
            caption: false,
            ..main()
        })); // helper window
        assert!(!is_hidden_main(&Facts {
            has_children: false,
            ..main()
        })); // message window
        assert!(!is_hidden_main(&Facts {
            app_or_unowned: false,
            ..main()
        })); // owned
        assert!(!is_hidden_main(&Facts {
            visible: true,
            ..main()
        }));
        assert!(!is_hidden_main(&Facts {
            tool: true,
            ..main()
        }));
    }

    #[test]
    fn popup_filter() {
        let menu = Facts {
            visible: true,
            class: "#32768".into(),
            ..Facts::default()
        };
        assert!(is_popup(&menu, false));
        let flyout = Facts {
            visible: true,
            tool: true,
            class: "Host".into(),
            ..Facts::default()
        };
        assert!(is_popup(&flyout, false)); // untitled tool window
        let dialog = Facts {
            visible: true,
            title: "Save as".into(),
            class: "#32770".into(),
            ..Facts::default()
        };
        assert!(!is_popup(&dialog, false)); // another app window...
        assert!(is_popup(&dialog, true)); // ...unless the snapshot window owns it
        assert!(!is_popup(
            &Facts {
                class: "#32768".into(),
                ..Facts::default()
            },
            false
        )); // hidden
    }
}
