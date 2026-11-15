//! No lens overlay on Linux yet: every call is a no-op.

use crate::{MarkId, MarkKind, Phase, Rect, WindowId};

pub fn overlay_mark(
    _window: WindowId,
    _rect: Rect,
    _caption: &str,
    _kind: MarkKind,
) -> Option<MarkId> {
    None
}

pub fn overlay_update(_id: MarkId, _phase: Phase) {}

pub fn overlay_hide() -> bool {
    false
}

pub fn overlay_restore() {}
