//! OpenAI-compatible `/chat/completions` client and the tool loop.

mod client;
mod exec;
mod history;
mod multi;
mod prompt;
mod run;
mod usage;

pub use exec::{Executor, Local};
pub use history::{push_tool_result, Msg, SUPERSEDED};
pub use multi::Multi;
pub use prompt::{os_description, remote_system_prompt, system_prompt};
pub use run::{run, Opts};
