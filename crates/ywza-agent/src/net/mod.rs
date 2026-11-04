//! iroh endpoint, the permission gate, per-client connections and code pairing.

mod accept;
mod bind;
mod dispatch;
pub mod pair;
mod peer;
mod request;
mod writer;

pub use accept::accept_loop;
pub use bind::{bind, bind_rendezvous, local_addr, saved_port};
pub use request::watch_pairing_requests;
pub(crate) use writer::write_loop;
