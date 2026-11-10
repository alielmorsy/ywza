//! ywza-agent: the background service. `main.rs` is a thin CLI over [`Agent::start`].

mod agent;
pub mod approval;
mod atomic;
pub mod config;
mod handle;
mod inspect;
pub mod live;
pub mod llm;
pub mod local;
pub mod mcp;
pub mod net;
pub mod platform;
pub mod remote;
pub mod sessions;
pub mod tools;

pub use agent::Agent;
