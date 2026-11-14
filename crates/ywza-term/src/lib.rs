//! Owned PTY terminals and attach-to-existing Windows consoles.
//!
//! The agent's `main()` must call [`helper_main`] first: console reads and writes run in a
//! short-lived child process (the same binary with a hidden argument).

mod attach;
mod error;
mod helper;
mod id;
mod info;
pub mod keys;
mod manager;
mod marks;
mod owned;
mod platform;
mod render;
mod shell;
mod track;

pub use attach::{attached_keys, attached_read, attached_write, list_attachable, Attachable, Host};
pub use error::{Error, Result};
pub use helper::{helper_main, set_helper_path};
pub use id::TermId;
pub use info::{Kind, TermInfo};
pub use manager::TermManager;
pub use owned::{OwnedTerm, RING_CAP};
pub use track::{RunOutcome, ShellInfo};

pub(crate) use error::err;
