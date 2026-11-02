use super::atoms::Atoms;
use crate::{Error, Result};
use std::fmt::Display;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, Window,
};
use x11rb::rust_connection::RustConnection;

pub fn os(e: impl Display) -> Error {
    Error::Os(e.to_string())
}

macro_rules! from_x {
    ($($t:ty),*) => {$(impl From<$t> for Error { fn from(e: $t) -> Self { os(e) } })*};
}
from_x!(
    x11rb::errors::ConnectionError,
    x11rb::errors::ReplyError,
    x11rb::errors::ReplyOrIdError,
    x11rb::errors::ParseError
);

fn connect_error(e: impl Display) -> Error {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    if wayland && std::env::var_os("DISPLAY").is_none() {
        Error::Os(
            "unsupported: Wayland session without XWayland (no $DISPLAY); ywza needs an X11 display".into(),
        )
    } else {
        Error::Os(format!("cannot connect to the X11 display: {e}"))
    }
}

pub struct X {
    pub conn: RustConnection,
    pub root: Window,
    pub a: Atoms,
}

impl X {
    pub fn connect() -> Result<X> {
        let (conn, n) = RustConnection::connect(None).map_err(connect_error)?;
        let root = conn.setup().roots[n].root;
        let a = Atoms::new(&conn)?.reply()?;
        Ok(X { conn, root, a })
    }

    pub(super) fn prop32(&self, w: Window, prop: Atom, ty: impl Into<Atom>) -> Vec<u32> {
        self.conn
            .get_property(false, w, prop, ty, 0, 4096)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().map(|i| i.collect()))
            .unwrap_or_default()
    }

    pub(super) fn text_prop(&self, w: Window, prop: Atom, ty: impl Into<Atom>) -> Option<String> {
        let r = self
            .conn
            .get_property(false, w, prop, ty, 0, 2048)
            .ok()?
            .reply()
            .ok()?;
        (!r.value.is_empty()).then(|| String::from_utf8_lossy(&r.value).into_owned())
    }

    pub(super) fn title(&self, w: Window) -> String {
        self.text_prop(w, self.a._NET_WM_NAME, self.a.UTF8_STRING)
            .or_else(|| self.text_prop(w, AtomEnum::WM_NAME.into(), AtomEnum::ANY))
            .unwrap_or_default()
    }

    pub(super) fn state(&self, w: Window) -> Vec<u32> {
        self.prop32(w, self.a._NET_WM_STATE, AtomEnum::ATOM)
    }

    pub(super) fn supported(&self, atom: Atom) -> bool {
        self.prop32(self.root, self.a._NET_SUPPORTED, AtomEnum::ATOM)
            .contains(&atom)
    }

    pub(super) fn client_message(&self, win: Window, ty: Atom, data: [u32; 5]) -> Result<()> {
        let ev = ClientMessageEvent::new(32, win, ty, data);
        let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
        self.conn.send_event(false, self.root, mask, ev)?;
        self.conn.flush()?;
        Ok(())
    }

    /// Round trip: errors of earlier requests surface and the server has processed them.
    pub(super) fn sync(&self) -> Result<()> {
        self.conn.get_input_focus()?.reply()?;
        Ok(())
    }
}
