use super::conn::X;
use crate::{Error, Result, WindowId};
use std::thread::sleep;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    self, Atom, AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt as _, EventMask,
    StackMode, Window,
};
use x11rb::CURRENT_TIME;

pub fn focus_window(id: WindowId) -> Result<()> {
    let x = X::connect()?;
    x.window_info(id)?;
    x.bring_to_front(id as Window)
}

pub fn close_window(id: WindowId) -> Result<()> {
    X::connect()?.close_window(id)
}

impl X {
    /// Makes `w` the active window, restoring it if minimized.
    pub fn bring_to_front(&self, w: Window) -> Result<()> {
        if self.active() == Some(w) && !self.state(w).contains(&self.a._NET_WM_STATE_HIDDEN) {
            return Ok(());
        }
        if !self.supported(self.a._NET_ACTIVE_WINDOW) {
            // No EWMH window manager: raise and focus directly.
            self.conn.map_window(w)?;
            self.conn
                .configure_window(w, &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE))?;
            self.conn
                .set_input_focus(xproto::InputFocus::POINTER_ROOT, w, CURRENT_TIME)?;
            return self.sync();
        }
        // Source 2 = pager: window managers do not apply focus-stealing prevention to it.
        self.client_message(w, self.a._NET_ACTIVE_WINDOW, [2, 0, 0, 0, 0])?;
        let deadline = Instant::now() + Duration::from_millis(500);
        while self.active() != Some(w) {
            if Instant::now() > deadline {
                return Err(Error::Os(format!("could not bring w{w} to the front")));
            }
            sleep(Duration::from_millis(20));
        }
        Ok(())
    }

    pub(super) fn remove_state(&self, w: Window, state: Atom) -> Result<()> {
        self.client_message(w, self.a._NET_WM_STATE, [0, state, 0, 2, 0])
    }

    fn close_window(&self, id: WindowId) -> Result<()> {
        self.window_info(id)?;
        let w = id as Window;
        let deletable = self
            .prop32(w, self.a.WM_PROTOCOLS, AtomEnum::ATOM)
            .contains(&self.a.WM_DELETE_WINDOW);
        if !deletable && self.supported(self.a._NET_CLOSE_WINDOW) {
            return self.client_message(w, self.a._NET_CLOSE_WINDOW, [CURRENT_TIME, 2, 0, 0, 0]);
        }
        // Deliver the ICCCM close request straight to the client; window managers may drop the EWMH one.
        let ev = ClientMessageEvent::new(
            32,
            w,
            self.a.WM_PROTOCOLS,
            [self.a.WM_DELETE_WINDOW, CURRENT_TIME, 0, 0, 0],
        );
        self.conn.send_event(false, w, EventMask::NO_EVENT, ev)?;
        self.conn.flush()?;
        Ok(())
    }
}
