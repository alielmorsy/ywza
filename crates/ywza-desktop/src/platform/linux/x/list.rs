use super::conn::X;
use super::facts::is_app_window;
use super::monitors::monitor_of;
use crate::{Error, Rect, Result, WindowId, WindowInfo};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, MapState, Window};

pub fn list_windows() -> Result<Vec<WindowInfo>> {
    X::connect()?.list_windows()
}

pub fn window_info(id: WindowId) -> Result<WindowInfo> {
    X::connect()?.window_info(id)
}

/// Window rect grown by `_NET_FRAME_EXTENTS` (left, right, top, bottom): the visible frame.
fn with_frame(client: Rect, ext: [i32; 4]) -> Rect {
    Rect::new(
        client.x - ext[0],
        client.y - ext[2],
        client.w + ext[0] + ext[1],
        client.h + ext[2] + ext[3],
    )
}

fn process_name(pid: u32) -> String {
    if pid == 0 {
        return "?".into();
    }
    let exe = std::fs::read_link(format!("/proc/{pid}/exe"))
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
    let comm = || {
        std::fs::read_to_string(format!("/proc/{pid}/comm"))
            .ok()
            .map(|s| s.trim().to_string())
    };
    exe.or_else(comm)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "?".into())
}

impl X {
    pub(super) fn frame_extents(&self, w: Window) -> [i32; 4] {
        match self.prop32(w, self.a._NET_FRAME_EXTENTS, AtomEnum::CARDINAL)[..] {
            [l, r, t, b] => [l as i32, r as i32, t as i32, b as i32],
            _ => [0; 4],
        }
    }

    /// Client area in root coordinates.
    pub(super) fn client_rect(&self, w: Window) -> Result<Rect> {
        let g = self.conn.get_geometry(w)?.reply()?;
        let t = self
            .conn
            .translate_coordinates(w, self.root, 0, 0)?
            .reply()?;
        Ok(Rect::new(
            t.dst_x as i32,
            t.dst_y as i32,
            g.width as i32,
            g.height as i32,
        ))
    }

    /// Open popups (menus, dropdowns) of `w`'s app, bottom to top, with their screen
    /// rects: mapped override-redirect top-level windows created by the same X client
    /// (same resource-id base). They are not children of `w`, so its own pixels miss them.
    pub(super) fn popups(&self, w: Window) -> Vec<(Window, Rect)> {
        let mask = self.conn.setup().resource_id_mask;
        let Some(tree) = self
            .conn
            .query_tree(self.root)
            .ok()
            .and_then(|c| c.reply().ok())
        else {
            return Vec::new();
        };
        tree.children
            .into_iter()
            .filter(|&c| c != w && c & !mask == w & !mask)
            .filter(|&c| {
                self.conn
                    .get_window_attributes(c)
                    .ok()
                    .and_then(|r| r.reply().ok())
                    .is_some_and(|a| a.override_redirect && a.map_state == MapState::VIEWABLE)
            })
            .filter_map(|c| Some((c, self.client_rect(c).ok()?)))
            .collect()
    }

    /// Visible bounds: the client area plus the window manager's frame.
    pub(super) fn visible_rect(&self, w: Window) -> Result<Rect> {
        Ok(with_frame(self.client_rect(w)?, self.frame_extents(w)))
    }

    pub fn window_info(&self, id: WindowId) -> Result<WindowInfo> {
        let gone = || Error::NotFound(format!("window w{id}"));
        let w = u32::try_from(id).map_err(|_| gone())?;
        let (facts, hidden) = self.facts(w).ok_or_else(gone)?;
        let monitors: Vec<Rect> = self.monitors()?.iter().map(|m| m.rect).collect();
        let rect = self.visible_rect(w).map_err(|_| gone())?;
        let pid = self
            .prop32(w, self.a._NET_WM_PID, AtomEnum::CARDINAL)
            .first()
            .copied()
            .unwrap_or(0);
        let state = self.state(w);
        let maximized = [
            self.a._NET_WM_STATE_MAXIMIZED_VERT,
            self.a._NET_WM_STATE_MAXIMIZED_HORZ,
        ]
        .iter()
        .all(|s| state.contains(s))
            || state.contains(&self.a._NET_WM_STATE_FULLSCREEN);
        Ok(WindowInfo {
            id,
            title: facts.title,
            process: process_name(pid),
            pid,
            rect,
            monitor: monitor_of(&rect, &monitors),
            minimized: hidden,
            hidden: false,
            focused: self.active() == Some(w),
            maximized,
        })
    }

    fn top_level(&self) -> Vec<Window> {
        let list = self.prop32(self.root, self.a._NET_CLIENT_LIST, AtomEnum::WINDOW);
        if !list.is_empty() {
            return list;
        }
        // No EWMH window manager: the clients (windows with WM_STATE) are root children,
        // or children of the frame windows the manager reparented them into.
        let kids = |w: Window| {
            self.conn
                .query_tree(w)
                .ok()
                .and_then(|c| c.reply().ok())
                .map(|r| r.children)
                .unwrap_or_default()
        };
        let managed = |w: Window| !self.prop32(w, self.a.WM_STATE, self.a.WM_STATE).is_empty();
        kids(self.root)
            .into_iter()
            .flat_map(|w| {
                if managed(w) {
                    vec![w]
                } else {
                    kids(w).into_iter().filter(|&c| managed(c)).collect()
                }
            })
            .collect()
    }

    pub fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        Ok(self
            .top_level()
            .into_iter()
            .filter(|&w| self.facts(w).is_some_and(|(f, _)| is_app_window(&f)))
            .filter_map(|w| self.window_info(w as WindowId).ok())
            .collect())
    }

    pub(super) fn active(&self) -> Option<Window> {
        self.prop32(self.root, self.a._NET_ACTIVE_WINDOW, AtomEnum::WINDOW)
            .first()
            .copied()
            .filter(|&w| w != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_grows_client() {
        assert_eq!(
            with_frame(Rect::new(10, 30, 100, 50), [2, 2, 20, 2]),
            Rect::new(8, 10, 104, 72)
        );
    }
}
