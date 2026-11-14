use super::bus::{block, connect};
use super::live::live;
use crate::platform::linux::x;
use crate::{Button, Error, Patterns, Result, Snapshot, Tier};
use atspi::proxy::accessible::AccessibleProxy;
use atspi::proxy::proxy_ext::ProxyExt;

/// Click through the Action interface (action 0). False when there is none or it failed.
async fn pattern_click(acc: &AccessibleProxy<'_>, p: &Patterns) -> bool {
    if !(p.invoke || p.toggle || p.select || p.expand) {
        return false;
    }
    let Ok(proxies) = acc.proxies().await else {
        return false;
    };
    let Ok(action) = proxies.action().await else {
        return false;
    };
    matches!(action.do_action(0).await, Ok(true))
}

/// Clicks an element: the Action interface for a single left click (unless `real`),
/// else a real XTest click at the element's centre, which puts the pointer back.
pub fn click(snap: &Snapshot, id: u32, button: Button, count: u32, real: bool) -> Result<Tier> {
    let at = block(async {
        let (e, acc) = live(&connect().await?, snap, id).await?;
        if let (false, Button::Left, 1, Some(acc)) = (real, button, count, &acc) {
            if pattern_click(acc, &e.patterns).await {
                return Ok(None);
            }
        }
        Ok(Some(e.rect.center()))
    })?;
    match at {
        None => Ok(Tier::Pattern),
        Some(at) => {
            x::real_click(snap.window.id, at, button, count, 0)?;
            Ok(Tier::Takeover)
        }
    }
}

/// Opens an expandable element's dropdown/menu. AT-SPI has no separate expand
/// action, so this is the default action, which opens it for expandable widgets.
pub fn expand(snap: &Snapshot, id: u32) -> Result<Tier> {
    match snap.get(id) {
        Some(e) if !e.patterns.expand => Err(Error::NotFound(format!(
            "element {id} has no dropdown to expand; click it normally"
        ))),
        _ => click(snap, id, Button::Left, 1, false),
    }
}

pub fn long_press(snap: &Snapshot, id: u32, ms: u64) -> Result<Tier> {
    let at = center(snap, Some(id))?;
    x::real_click(snap.window.id, at, Button::Left, 1, ms)?;
    Ok(Tier::Takeover)
}

/// Sets the element's text through EditableText when possible (and it reads back;
/// never with `real`); otherwise focuses the element (or just the window when `id` is None), selects all
/// when replacing (Delete for empty text) and types the text.
// shortcut: no paste mode on X11 (serving the CLIPBOARD selection needs an event loop), keys are synced per character instead; add it if long text proves slow.
pub fn type_text(
    snap: &Snapshot,
    id: Option<u32>,
    text: &str,
    append: bool,
    real: bool,
) -> Result<Tier> {
    let done = block(async {
        let Some(id) = id else { return Ok(false) };
        let (e, acc) = live(&connect().await?, snap, id).await?;
        let Some(acc) = acc else { return Ok(false) };
        let Ok(proxies) = acc.proxies().await else {
            return Ok(false);
        };
        if !real && !append && e.patterns.value && !e.patterns.value_readonly {
            if let Ok(t) = proxies.editable_text().await {
                if matches!(t.set_text_contents(text).await, Ok(true)) {
                    let now = match proxies.text().await {
                        Ok(t) => {
                            let n = t.character_count().await.unwrap_or(0);
                            t.get_text(0, n).await.ok()
                        }
                        Err(_) => None,
                    };
                    if now.is_none_or(|v| v.trim_end() == text.trim_end()) {
                        return Ok(true);
                    }
                }
            }
        }
        if let Ok(c) = proxies.component().await {
            let _ = c.grab_focus().await;
        }
        Ok(false)
    })?;
    if done {
        return Ok(Tier::Pattern);
    }
    if id.is_some() && !append {
        let keys = if text.is_empty() {
            "ctrl+a delete"
        } else {
            "ctrl+a"
        };
        x::press_keys(Some(snap.window.id), keys)?;
    }
    x::type_text(Some(snap.window.id), text)?;
    Ok(Tier::Takeover)
}

/// Scrolls `dy` wheel notches down (negative = up) and `dx` right over the element or
/// window centre. AT-SPI has no scroll action, so this is always the wheel (`real` is moot).
pub fn scroll(snap: &Snapshot, id: Option<u32>, dx: i32, dy: i32, _real: bool) -> Result<Tier> {
    let at = center(snap, id)?;
    x::wheel(snap.window.id, at, dx, dy)?;
    Ok(Tier::Takeover)
}

/// Centre of a validated element, or of the window.
fn center(snap: &Snapshot, id: Option<u32>) -> Result<(i32, i32)> {
    match id {
        Some(id) => block(async { Ok(live(&connect().await?, snap, id).await?.0.rect.center()) }),
        None => Ok(x::window_info(snap.window.id)?.rect.center()),
    }
}
