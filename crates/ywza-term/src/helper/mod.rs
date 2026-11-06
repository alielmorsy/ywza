//! Helper-process protocol: the agent binary re-invokes itself with a hidden subcommand,
//! sends one JSON request on stdin and reads one JSON response from stdout.

mod client;
mod protocol;
mod server;

pub(crate) use client::call;
pub use client::set_helper_path;
// Info not currently used
#[allow(unused_imports)]
pub(crate) use protocol::{Info, Request, Response};
pub use server::helper_main;
