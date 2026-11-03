use crate::helper::{Request, Response};
use crate::{Attachable, Result};
use std::process::Command;

pub(crate) fn list_attachable() -> Result<Vec<Attachable>> {
    Ok(Vec::new())
}

pub(crate) fn run_request(_: Request) -> Response {
    Response {
        error: Some("attached consoles are only supported on Windows".into()),
        ..Default::default()
    }
}

pub(crate) fn hide_window(_: &mut Command) {}
