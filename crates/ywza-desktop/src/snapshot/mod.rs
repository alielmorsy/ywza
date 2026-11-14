//! Platform-neutral element model: filter/kind/naming rules, ID assignment, compact
//! rendering, diffs and the OCR merge rule.

mod anchor;
mod build;
mod classify;
mod diff;
mod learned;
mod merge;
mod model;
mod ocr;
mod render;
#[cfg(test)]
mod tests;

pub use anchor::{resolve, Anchor, Candidate, Resolve, Step};
pub(crate) use classify::recheck;
pub use diff::{diff, Delta};
pub use learned::set_learned;
pub(crate) use merge::merge_popup;
pub(crate) use model::Raw;
pub use model::{Element, Patterns, Snapshot};
pub use ocr::{merge_ocr, needs_ocr, ocr_matches, ocr_norm, ocr_similar};
pub use render::render_diff;
