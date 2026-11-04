//! OpenAI-compatible `/chat/completions` client and the tool loop.

mod client;
mod history;
mod prompt;
mod run;

pub use history::{push_tool_result, Msg, SUPERSEDED};
pub use prompt::system_prompt;
pub use run::run;
