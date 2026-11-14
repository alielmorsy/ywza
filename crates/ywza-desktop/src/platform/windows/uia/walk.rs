use super::handle::Handle;
use super::raw::{automation, cache_request, raw_from};
use crate::platform::windows::util::{hwnd, window_id};
use crate::platform::windows::wm;
use crate::snapshot::Raw;
use crate::{Rect, Result, Snapshot, WindowId};
use ::windows::Win32::UI::Accessibility::{
    IUIAutomation, IUIAutomationCacheRequest, IUIAutomationElement,
};

/// Upper bound on walked nodes, so a provider that loops cannot hang a snapshot.
const MAX_NODES: usize = 20_000;

/// Snapshots a window's elements. With `prev` (the last snapshot of the same
/// window), unchanged elements keep their IDs and `seq` increments.
pub fn snapshot(window: WindowId, prev: Option<&Snapshot>) -> Result<Snapshot> {
    let info = wm::window_info(window)?;
    let uia = automation()?;
    let cache = cache_request(&uia)?;
    // Menus and dropdowns are separate top-level windows of the same process.
    let popups = wm::popup_windows(&info);
    let dialogs: Vec<WindowId> = popups.iter().filter(|p| p.1).map(|p| p.0).collect();
    let mut raws = collect(&uia, &cache, window, None, &dialogs)?;
    for (popup, dialog) in popups {
        let bounds = wm::visible_rect(hwnd(popup));
        if dialog {
            raws.push((dialog_pointer(popup, bounds), None));
            continue;
        }
        for (raw, handle) in collect(&uia, &cache, popup, Some(bounds), &[]).unwrap_or_default() {
            if raws
                .iter()
                .all(|(r, _)| r.key.is_empty() || r.key != raw.key)
            {
                raws.push((raw, handle));
            }
        }
    }
    Ok(Snapshot::build(info, raws, prev))
}

/// One line standing for an owned dialog: it is driven as its own window, so its
/// elements always come from a fresh snapshot of it rather than a late merge here.
fn dialog_pointer(id: WindowId, rect: Rect) -> Raw {
    let title = wm::window_info(id).map(|i| i.title).unwrap_or_default();
    Raw {
        role: "dialog",
        name: format!("{title} (window w{id}, get_elements it)"),
        // Its title bar: a click on this line only activates the dialog.
        rect: Rect {
            h: rect.h.min(30),
            ..rect
        },
        bounds: Some(rect),
        enabled: true,
        key: vec![-1, id as i32, (id >> 32) as i32],
        ..Raw::default()
    }
}

/// All control-view descendants of a window in document order, each once, tagged
/// with `bounds` for popups. The `skip` windows (owned dialogs UIA nests under their
/// owner) are left out with their subtree.
///
/// A TreeWalker with a RuntimeId visited set, not `FindAll(Descendants)`: WinUI 3
/// hosts expose a WebView2 HWND twice (as a Win32 child and under the XAML WebView2
/// element), and FindAll over that tree returned each Chromium node dozens to
/// hundreds of times (WhatsApp: 12k elements for ~480 nodes). Revisited nodes are
/// skipped with their whole subtree.
fn collect(
    uia: &IUIAutomation,
    cache: &IUIAutomationCacheRequest,
    window: WindowId,
    bounds: Option<Rect>,
    skip: &[WindowId],
) -> Result<Vec<(Raw, Option<Handle>)>> {
    unsafe {
        let walker = uia.ControlViewWalker()?;
        let root = uia.ElementFromHandle(hwnd(window))?;
        let children = |el: &IUIAutomationElement| {
            let mut out = Vec::new();
            let mut c = walker.GetFirstChildElementBuildCache(el, cache).ok();
            while let Some(ch) = c {
                c = walker.GetNextSiblingElementBuildCache(&ch, cache).ok();
                out.push(ch);
            }
            out.reverse(); // popped from the stack in document order
            out
        };
        let mut seen = std::collections::HashSet::new();
        let mut raws = Vec::new();
        // Each node with the name of its nearest named group (radio questions).
        let mut stack: Vec<_> = children(&root)
            .into_iter()
            .map(|c| (c, String::new()))
            .collect();
        let mut visits = 0;
        while let Some((el, group)) = stack.pop() {
            visits += 1;
            if visits > MAX_NODES {
                break;
            }
            let Ok(mut raw) = raw_from(&el) else { continue };
            if !raw.key.is_empty() && !seen.insert(raw.key.clone()) {
                continue;
            }
            if raw.role == "dialog"
                && !skip.is_empty()
                && el
                    .CachedNativeWindowHandle()
                    .is_ok_and(|h| skip.contains(&window_id(h)))
            {
                continue;
            }
            let inner = if raw.role == "group" && !raw.name.trim().is_empty() {
                raw.name.clone()
            } else {
                group.clone()
            };
            stack.extend(children(&el).into_iter().map(|c| (c, inner.clone())));
            raw.bounds = bounds;
            raw.group = group;
            raws.push((raw, Some(Handle(el))));
        }
        Ok(raws)
    }
}
