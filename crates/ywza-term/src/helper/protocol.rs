use serde::{Deserialize, Serialize};

/// First argument (after the program name) that marks a helper invocation.
pub(crate) const HELPER_ARG: &str = "__ywza_term_helper";

#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(crate) enum Request {
    Read { pid: u32, lines: usize },
    Write { pid: u32, text: String, enter: bool },
    Keys { pid: u32, keys: String },
    Info { pids: Vec<u32> },
}

#[derive(Serialize, Deserialize, Default)]
pub(crate) struct Info {
    pub pid: u32,
    pub error: Option<String>,
    pub hwnd: i64,
    pub title: String,
    /// All processes attached to the same console.
    pub procs: Vec<u32>,
}

#[derive(Serialize, Deserialize, Default)]
pub(crate) struct Response {
    pub error: Option<String>,
    pub text: Option<String>,
    pub infos: Vec<Info>,
}
