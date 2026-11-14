//! Records successful local tool calls as plan steps, one JSON line each, in
//! `<dir>/logs/<session>.steps.jsonl`. `compile` turns that file into a plan.

use std::{io::Write, path::PathBuf};

use serde_json::Value;

use super::{element_id, PlanStep, WindowRef};
use crate::{
    anchor::SavedAnchor,
    tools::{parse_batch, parse_window, Space, ToolOutput, Tools, BATCH_TOOLS},
};

pub(super) const EXT: &str = "steps.jsonl";

/// Calls a plan may hold.
pub(super) fn recordable(tool: &str) -> bool {
    tool == "launch" || BATCH_TOOLS.contains(&tool)
}

/// Steps of one call, written by `after` once the call succeeded.
pub struct Recording {
    path: PathBuf,
    steps: Vec<PlanStep>,
}

/// The steps a call would record, taken before it runs; `args` are the model's own, vault
/// placeholders included. Never touches the desktop. None: nothing to record, or a step that
/// could not be replayed (an element with no stored element list).
pub fn before(tools: &Tools, space: Space, name: &str, args: &Value) -> Option<Recording> {
    let calls: Vec<(String, Value)> = if name == "batch" {
        let list = args.get("actions")?.as_array()?;
        parse_batch(args.get("window"), list)
            .ok()?
            .into_iter()
            .map(|s| (s.tool, s.args))
            .collect()
    } else if recordable(name) {
        vec![(name.to_string(), args.clone())]
    } else {
        return None;
    };
    let steps = calls
        .iter()
        .map(|(t, a)| step(tools, space, t, a))
        .collect::<Option<Vec<_>>>()?;
    let path = super::logs_dir()
        .ok()?
        .join(format!("{}.{EXT}", space.session.0));
    Some(Recording { path, steps })
}

fn step(tools: &Tools, space: Space, tool: &str, args: &Value) -> Option<PlanStep> {
    let mut a = args.as_object().cloned().unwrap_or_default();
    let window = a.remove("window");
    let element = a.remove("element").filter(|v| !v.is_null());
    let expect = a.remove("expect").and_then(|v| v.as_str().map(String::from));
    let snap = window
        .as_ref()
        .and_then(parse_window)
        .and_then(|w| tools.snap(space, w));
    let anchor = match element {
        Some(v) => Some(SavedAnchor::from(&snap.as_ref()?.get(element_id(&v)?)?.anchor)),
        None => None,
    };
    Some(PlanStep {
        tool: tool.into(),
        args: a,
        window: snap.map(|s| WindowRef {
            process: s.window.process,
            title: s.window.title,
        }),
        anchor,
        expect,
        changes: None,
    })
}

/// Appends the steps when the call succeeded (for a batch: all of it).
pub fn after(rec: Recording, out: &ToolOutput) {
    if !out.ok {
        return;
    }
    let lines: String = rec
        .steps
        .iter()
        .filter_map(|s| serde_json::to_string(s).ok())
        .map(|l| l + "\n")
        .collect();
    let r = rec
        .path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|_| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&rec.path)?
                .write_all(lines.as_bytes())
        });
    if let Err(e) = r {
        tracing::warn!(path = %rec.path.display(), error = %e, "recording steps failed");
    }
}
