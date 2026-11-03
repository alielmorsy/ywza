mod console;
mod discover;
mod records;
mod run;

pub(crate) use discover::list_attachable;
pub(crate) use run::run_request;

use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(crate) fn hide_window(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}
