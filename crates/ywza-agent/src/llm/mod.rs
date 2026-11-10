//! OpenAI-compatible `/chat/completions` client and the tool loop.

mod client;
mod exec;
mod history;
mod prompt;
mod run;

pub use exec::{Executor, Local};
pub use history::{push_tool_result, Msg, SUPERSEDED};
pub use prompt::{os_description, remote_system_prompt, system_prompt};
pub use run::{run, Opts};
