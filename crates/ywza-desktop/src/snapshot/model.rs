use super::anchor::Anchor;
use crate::platform::Handle;
use crate::{Rect, WindowInfo};

/// Control patterns an element supports (UIA names; AT-SPI maps onto them).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Patterns {
    pub invoke: bool,
    pub value: bool,
    pub value_readonly: bool,
    pub toggle: bool,
    pub expand: bool,
    pub select: bool,
    pub scroll: bool,
}

/// Raw properties of one accessibility node, as fetched by the platform layer.
#[derive(Debug, Clone, Default)]
pub(crate) struct Raw {
    /// Lowercase control type name, e.g. "button", "pane", "window".
    pub role: &'static str,
    pub name: String,
    pub help: String,
    pub automation_id: String,
    pub rect: Rect,
    pub offscreen: bool,
    pub enabled: bool,
    pub focusable: bool,
    pub patterns: Patterns,
    pub value: Option<String>,
    /// Toggle state: Some(0) off, Some(1) on, Some(2) mixed.
    pub toggle: Option<i32>,
    /// Selected (SelectionItem, checked radio, aria-checked/selected).
    pub selected: bool,
    /// Loading (aria-busy, AT-SPI Busy).
    pub busy: bool,
    /// Name of the nearest named group around it (radio questions).
    pub group: String,
    /// Stable identity (UIA RuntimeId); empty for OCR boxes.
    pub key: Vec<i32>,
    /// Set for nodes of a popup window (menu, dropdown) that belongs to the snapshot
    /// window: the popup's own bounds, which may stick out of the window.
    pub bounds: Option<Rect>,
    /// Index of the parent in the walker's output (always earlier); `None` for direct
    /// children of the window or popup root.
    pub parent: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct Element {
    pub id: u32,
    /// "button", "edit", "clickable", "maybe-clickable", "ocr", ...
    pub kind: &'static str,
    /// Name, falling back to help text. Empty if neither exists.
    pub name: String,
    /// Shown as `(id=...)` when `name` is empty.
    pub automation_id: String,
    pub value: Option<String>,
    pub enabled: bool,
    pub selected: bool,
    pub busy: bool,
    /// Radios only: the question they answer.
    pub group: String,
    /// `name` is borrowed from the named node it wraps or sits next to (see `build`).
    pub labeled: bool,
    /// `name` was given by the model and stored by anchor (see `learned`).
    pub learned: bool,
    pub rect: Rect,
    pub patterns: Patterns,
    pub key: Vec<i32>,
    /// Lives in a separate popup window (menu, dropdown) rather than the window itself.
    pub popup: bool,
    /// Id of the nearest ancestor that is itself listed.
    pub parent: Option<u32>,
    pub anchor: Anchor,
    pub(crate) handle: Option<Handle>,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub window: WindowInfo,
    /// Increments each time a window is re-snapshotted with its previous snapshot.
    pub seq: u32,
    pub elements: Vec<Element>,
}
