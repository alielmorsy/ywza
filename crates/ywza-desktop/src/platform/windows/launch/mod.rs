//! Launching: resolve the program ourselves (never the exe's own folder, which
//! `Command` would search first), run executables directly and everything else
//! (documents, URLs, `ms-settings:`) through the shell.

mod app_paths;
mod ctx;
mod select;
mod shell;
mod start;
mod target;
mod wait;

pub use start::launch;
