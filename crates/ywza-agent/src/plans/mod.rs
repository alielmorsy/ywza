//! Show once, run free: plans compiled from finished runs (or written by the model), replayed
//! without the model, each step re-found by its anchor and checked by its guards.

pub mod cli;
pub mod compile;
mod convert;
pub mod guard;
mod ir;
mod params;
pub mod record;
pub mod replay;
pub mod store;

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub use convert::{plan_from, step_from};
pub use ir::{Param, Plan, PlanStep, WindowRef};
pub use params::{bind, describe};

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Sets the agent dir; plans live in `<dir>/plans`. The first call wins.
pub fn init(dir: &Path) {
    let _ = DIR.set(dir.to_path_buf());
}

fn dir() -> anyhow::Result<PathBuf> {
    DIR.get()
        .map(|d| d.join("plans"))
        .ok_or_else(|| anyhow::anyhow!("plans are not set up (no agent dir)"))
}

/// Where recorded steps live: next to the session logs.
fn logs_dir() -> anyhow::Result<PathBuf> {
    DIR.get()
        .map(|d| d.join("logs"))
        .ok_or_else(|| anyhow::anyhow!("plans are not set up (no agent dir)"))
}

/// An element id as a model writes it: a number or `"[4]"`.
fn element_id(v: &serde_json::Value) -> Option<u32> {
    serde_json::from_value::<crate::tools::ElementArg>(v.clone())
        .ok()
        .map(|e| e.0)
}
