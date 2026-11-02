use super::bus::{block, connect};
use super::live::live;
use crate::platform::linux::x;
use crate::{Button, Error, Patterns, Result, Snapshot};
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

/// Clicks an element: the Action interface for a single left click, else a real
/// click at the element's centre.
pub fn click(snap: &Snapshot, id: u32, button: Button, count: u32) -> Result<()> {
    let at = block(async {
        let (e, acc) = live(&connect().await?, snap, id).await?;
        if let (Button::Left, 1, Some(acc)) = (button, count, &acc) {
            if pattern_click(acc, &e.patterns).await {
                return Ok(None);
            }
        }
        Ok(Some(e.rect.center()))
    })?;
    match at {
        None => Ok(()),
        Some(at) => x::real_click(snap.window.id, at, button, count, 0),
    }
}

/// Opens an expandable element's dropdown/menu. AT-SPI has no separate expand
/// action, so this is the default action, which opens it for expandable widgets.
pub fn expand(snap: &Snapshot, id: u32) -> Result<()> {
    match snap.get(id) {
        Some(e) if !e.patterns.expand => Err(Error::NotFound(format!(
            "element {id} has no dropdown to expand; click it normally"
        ))),
        _ => click(snap, id, Button::Left, 1),
    }
}

pub fn long_press(snap: &Snapshot, id: u32, ms: u64) -> Result<()> {
    let at = center(snap, Some(id))?;
    x::real_click(snap.window.id, at, Button::Left, 1, ms)
}

/// Sets the element's text through EditableText when possible; otherwise focuses the
/// element (or just the window when `id` is None) and types the text.
pub fn type_text(snap: &Snapshot, id: Option<u32>, text: &str, append: bool) -> Result<()> {
    let done = block(async {
        let Some(id) = id else { return Ok(false) };
        let (e, acc) = live(&connect().await?, snap, id).await?;
        let Some(acc) = acc else { return Ok(false) };
        let Ok(proxies) = acc.proxies().await else {
            return Ok(false);
        };
        if !append && e.patterns.value && !e.patterns.value_readonly {
            if let Ok(t) = proxies.editable_text().await {
                if matches!(t.set_text_contents(text).await, Ok(true)) {
                    return Ok(true);
                }
            }
        }
        if let Ok(c) = proxies.component().await {
            let _ = c.grab_focus().await;
        }
        Ok(false)
    })?;
    if done {
        return Ok(());
    }
    x::type_text(Some(snap.window.id), text)
}

/// Scrolls `dy` wheel notches down (negative = up) and `dx` right over the element or
/// window centre. AT-SPI has no scroll action, so this is always the wheel.
pub fn scroll(snap: &Snapshot, id: Option<u32>, dx: i32, dy: i32) -> Result<()> {
    let at = center(snap, id)?;
    x::wheel(snap.window.id, at, dx, dy)
}

/// Centre of a validated element, or of the window.
fn center(snap: &Snapshot, id: Option<u32>) -> Result<(i32, i32)> {
    match id {
        Some(id) => block(async { Ok(live(&connect().await?, snap, id).await?.0.rect.center()) }),
        None => Ok(x::window_info(snap.window.id)?.rect.center()),
    }
}
