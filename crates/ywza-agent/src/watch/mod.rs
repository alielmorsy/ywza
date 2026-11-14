//! Watchers: persistent triggers that run on the device, without the AI, until they fire.
//! The tools in `tools/watchers.rs` edit the store; the engine reads it every tick.

mod check;
mod engine;
mod model;
mod schedule;
mod shots;
mod store;
mod trigger;

pub(crate) use check::{term_line, text_finder, title_match};
pub use engine::spawn;
pub use model::{Action, Condition, Watcher, DEFAULT_MAX_PER_HOUR, DEFAULT_MIN_GAP_S};
pub use schedule::parse_at;
pub use store::{get as store, init};
