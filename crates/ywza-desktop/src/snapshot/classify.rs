use super::model::{Element, Raw};
use crate::Rect;

/// Control types that are actionable on their own; they keep their name as kind.
const INTERACTIVE: &[&str] = &[
    "button",
    "splitbutton",
    "edit",
    "document",
    "checkbox",
    "radio",
    "combobox",
    "link",
    "menuitem",
    "tabitem",
    "listitem",
    "treeitem",
    "dataitem",
    "slider",
    "spinner",
    "headeritem",
];
/// Kept only when they carry a name: they inform rather than act.
pub(super) const INFORMATIVE: &[&str] = &["text", "image", "progressbar", "dialog"];
const MAX_VALUE: usize = 80;

/// Applies the filter, kind and naming rules. `None` = not listed.
pub(crate) fn classify(raw: &Raw, window: &Rect) -> Option<Element> {
    let window = raw.bounds.as_ref().unwrap_or(window);
    if raw.offscreen || raw.rect.intersect(window).is_empty() {
        return None;
    }
    let p = &raw.patterns;
    let kind = if INTERACTIVE.contains(&raw.role) {
        raw.role
    } else if p.value && !p.value_readonly {
        "editable"
    } else if p.toggle {
        "toggle"
    } else if p.select {
        "selectable"
    } else if p.expand {
        "expandable"
    } else if p.invoke {
        "clickable"
    } else if raw.focusable && matches!(raw.role, "group" | "text") {
        "maybe-clickable"
    } else if INFORMATIVE.contains(&raw.role) && !raw.name.trim().is_empty() {
        raw.role
    } else {
        return None;
    };
    let name = if raw.name.trim().is_empty() {
        &raw.help
    } else {
        &raw.name
    };
    let value = match raw.toggle {
        Some(0) => Some("off".to_string()),
        Some(1) => Some("on".to_string()),
        Some(_) => Some("mixed".to_string()),
        None => raw
            .value
            .as_deref()
            .map(|v| v.replace("\r\n", "\n").replace('\r', "\n"))
            .filter(|v| !v.trim().is_empty())
            .map(|v| truncate(v.trim_end(), MAX_VALUE)),
    };
    Some(Element {
        id: 0,
        kind,
        name: truncate(name.trim(), MAX_VALUE * 2),
        automation_id: raw.automation_id.clone(),
        value,
        enabled: raw.enabled,
        selected: raw.selected,
        busy: raw.busy,
        group: if kind == "radio" {
            truncate(raw.group.trim(), MAX_VALUE / 2)
        } else {
            String::new()
        },
        labeled: false,
        rect: raw.rect,
        patterns: raw.patterns,
        key: raw.key.clone(),
        popup: raw.bounds.is_some(),
        handle: None,
    })
}

fn truncate(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    }
}

/// Re-checks a live element against its snapshot entry (validation before acting).
pub(crate) fn recheck(old: &Element, raw: &Raw, old_win: &Rect, win: &Rect) -> Result<(), String> {
    if raw.offscreen || raw.rect.intersect(win).is_empty() {
        return Err("off screen".into());
    }
    if !raw.enabled {
        return Err("disabled".into());
    }
    let now = classify(raw, win).ok_or("no longer listed")?;
    if now.kind != old.kind {
        return Err(format!("now a {}", now.kind));
    }
    if now.name != old.name && !(old.labeled && now.name.is_empty()) {
        return Err(format!("name now {:?}", now.name));
    }
    // Compare relative to the window so a moved window does not invalidate everything.
    const TOLERANCE: i32 = 10;
    let (a, b) = (old.rect, now.rect);
    let moved = ((b.x - win.x) - (a.x - old_win.x)).abs() > TOLERANCE
        || ((b.y - win.y) - (a.y - old_win.y)).abs() > TOLERANCE
        || (b.w - a.w).abs() > TOLERANCE
        || (b.h - a.h).abs() > TOLERANCE;
    if moved {
        return Err(format!("moved to {},{} {}x{}", b.x, b.y, b.w, b.h));
    }
    Ok(())
}
