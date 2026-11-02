//! AT-SPI: snapshots, validation and pattern actions. The async client is driven by a
//! small current-thread tokio runtime inside each call (on its own thread, so callers
//! already inside a runtime are fine).

mod actions;
mod bus;
mod frame;
mod live;
mod node;
mod tree;
mod walk;

pub use actions::{click, expand, long_press, scroll, type_text};
pub use live::validate;
pub use node::Handle;
pub use tree::snapshot;
