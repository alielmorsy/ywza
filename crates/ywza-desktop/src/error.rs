use std::fmt;

#[derive(Debug)]
pub enum Error {
    Unsupported,
    NotFound(String),
    /// The element no longer matches its snapshot; no action was taken.
    Stale {
        id: u32,
        reason: String,
    },
    /// The user kept using mouse/keyboard; real input was not sent.
    UserBusy,
    Timeout(String),
    Os(String),
    /// Another process holds this window's input lock; nothing was sent.
    Busy,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Unsupported => write!(f, "unsupported on this platform"),
            Error::NotFound(what) => write!(f, "not found: {what}"),
            Error::Stale { id, reason } => write!(f, "stale: element {id} ({reason})"),
            Error::UserBusy => write!(f, "user is active; input not sent"),
            Error::Timeout(what) => write!(f, "timed out: {what}"),
            Error::Os(msg) => write!(f, "os error: {msg}"),
            Error::Busy => write!(f, "window is busy: another ywza process is acting on it"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
