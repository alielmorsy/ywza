use super::conn::X;
use crate::platform::linux::keysym::Keymap;
use crate::Result;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ConnectionExt as _, BUTTON_PRESS_EVENT, BUTTON_RELEASE_EVENT, KEY_PRESS_EVENT,
    KEY_RELEASE_EVENT, MOTION_NOTIFY_EVENT,
};
use x11rb::protocol::{screensaver, xtest};

/// XTest primitives on one connection.
impl X {
    pub fn fake(&self, kind: u8, detail: u8, x: i16, y: i16) -> Result<()> {
        xtest::fake_input(&self.conn, kind, detail, 0, self.root, x, y, 0)?;
        Ok(())
    }

    pub fn motion(&self, (x, y): (i32, i32)) -> Result<()> {
        let c = |v: i32| v.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        self.fake(MOTION_NOTIFY_EVENT, 0, c(x), c(y)) // detail 0 = absolute
    }

    pub fn button(&self, button: u8, down: bool) -> Result<()> {
        self.fake(
            if down {
                BUTTON_PRESS_EVENT
            } else {
                BUTTON_RELEASE_EVENT
            },
            button,
            0,
            0,
        )
    }

    pub fn key(&self, code: u8, down: bool) -> Result<()> {
        self.fake(
            if down {
                KEY_PRESS_EVENT
            } else {
                KEY_RELEASE_EVENT
            },
            code,
            0,
            0,
        )
    }

    /// Presses keys in order and releases them in reverse.
    pub(super) fn chord(&self, codes: &[u8]) -> Result<()> {
        for &c in codes {
            self.key(c, true)?;
        }
        for &c in codes.iter().rev() {
            self.key(c, false)?;
        }
        Ok(())
    }

    pub fn keymap(&self) -> Result<Keymap> {
        let setup = self.conn.setup();
        let (min, max) = (setup.min_keycode, setup.max_keycode);
        let r = self
            .conn
            .get_keyboard_mapping(min, max - min + 1)?
            .reply()?;
        Ok(Keymap {
            min_keycode: min,
            per: r.keysyms_per_keycode as usize,
            syms: r.keysyms,
        })
    }

    pub(super) fn pointer(&self) -> Result<(i32, i32)> {
        let p = self.conn.query_pointer(self.root)?.reply()?;
        Ok((p.root_x as i32, p.root_y as i32))
    }

    /// Milliseconds since the user last touched mouse or keyboard (0 if unknown).
    pub(super) fn idle_ms(&self) -> u64 {
        screensaver::query_info(&self.conn, self.root)
            .ok()
            .and_then(|c| c.reply().ok())
            .map_or(u64::MAX, |r| r.ms_since_user_input as u64)
    }
}
