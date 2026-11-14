//! ywza-agent: the background service. `main.rs` is a thin CLI over [`Agent::start`].

mod agent;
mod anchor;
pub mod approval;
mod atomic;
pub mod config;
mod handle;
mod inspect;
pub mod labels;
pub mod live;
pub mod llm;
pub mod local;
pub mod mcp;
pub mod net;
pub mod plans;
pub mod platform;
pub mod remote;
pub mod sessions;
pub mod sos;
pub mod tools;
pub mod vault;
mod watch;

pub use agent::Agent;
