//! Platform-neutral element model: filter/kind/naming rules, ID assignment, compact
//! rendering, diffs and the OCR merge rule.

mod build;
mod classify;
mod diff;
mod model;
mod ocr;
mod render;
#[cfg(test)]
mod tests;

pub(crate) use classify::recheck;
pub use diff::diff;
pub(crate) use model::Raw;
pub use model::{Element, Patterns, Snapshot};
pub use ocr::{merge_ocr, needs_ocr};
