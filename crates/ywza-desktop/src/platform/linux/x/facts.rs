use super::conn::X;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, MapState, Window};

/// What decides whether a top-level window is a real app window.
#[derive(Debug, Default)]
pub(super) struct Facts {
    /// Viewable, or iconified (minimized windows are listed).
    shown: bool,
    skip_taskbar: bool,
    dialog: bool,
    /// Dock, desktop, menu, tooltip, splash, toolbar, utility...: not an app window.
    non_app_type: bool,
    pub(super) title: String,
}

pub(super) fn is_app_window(f: &Facts) -> bool {
    f.shown && !f.non_app_type && (!f.skip_taskbar || f.dialog) && !f.title.trim().is_empty()
}

impl X {
    /// Window facts and whether it is hidden (minimized); None if the window is gone.
    pub(super) fn facts(&self, w: Window) -> Option<(Facts, bool)> {
        let attrs = self.conn.get_window_attributes(w).ok()?.reply().ok()?;
        let a = &self.a;
        let state = self.state(w);
        let hidden = state.contains(&a._NET_WM_STATE_HIDDEN);
        let types = self.prop32(w, a._NET_WM_WINDOW_TYPE, AtomEnum::ATOM);
        let non_app = [
            a._NET_WM_WINDOW_TYPE_DOCK,
            a._NET_WM_WINDOW_TYPE_DESKTOP,
            a._NET_WM_WINDOW_TYPE_TOOLBAR,
            a._NET_WM_WINDOW_TYPE_UTILITY,
            a._NET_WM_WINDOW_TYPE_SPLASH,
            a._NET_WM_WINDOW_TYPE_MENU,
            a._NET_WM_WINDOW_TYPE_DROPDOWN_MENU,
            a._NET_WM_WINDOW_TYPE_POPUP_MENU,
            a._NET_WM_WINDOW_TYPE_TOOLTIP,
            a._NET_WM_WINDOW_TYPE_NOTIFICATION,
            a._NET_WM_WINDOW_TYPE_COMBO,
            a._NET_WM_WINDOW_TYPE_DND,
        ];
        let facts = Facts {
            shown: attrs.map_state == MapState::VIEWABLE || hidden,
            skip_taskbar: state.contains(&a._NET_WM_STATE_SKIP_TASKBAR),
            dialog: types.contains(&a._NET_WM_WINDOW_TYPE_DIALOG),
            non_app_type: types.iter().any(|t| non_app.contains(t)),
            title: self.title(w),
        };
        Some((facts, hidden))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_window_filter() {
        let app = Facts {
            shown: true,
            title: "Terminal".into(),
            ..Facts::default()
        };
        assert!(is_app_window(&app));
        assert!(!is_app_window(&Facts {
            shown: false,
            ..Facts {
                title: "x".into(),
                ..Facts::default()
            }
        }));
        assert!(!is_app_window(&Facts {
            non_app_type: true,
            ..Facts {
                shown: true,
                title: "x".into(),
                ..Facts::default()
            }
        }));
        assert!(!is_app_window(&Facts {
            shown: true,
            title: "  ".into(),
            ..Facts::default()
        }));
        let skip = Facts {
            shown: true,
            skip_taskbar: true,
            title: "Panel".into(),
            ..Facts::default()
        };
        assert!(!is_app_window(&skip));
        assert!(is_app_window(&Facts {
            dialog: true,
            ..skip
        })); // dialogs often skip the taskbar
    }
}
