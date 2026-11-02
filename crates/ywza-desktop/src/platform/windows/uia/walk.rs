use super::handle::Handle;
use super::raw::{automation, cache_request, raw_from};
use crate::platform::windows::util::hwnd;
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
    let mut raws = collect(&uia, &cache, window, None)?;
    // Menus and dropdowns are separate top-level windows of the same process.
    for popup in wm::popup_windows(&info) {
        let bounds = wm::visible_rect(hwnd(popup));
        for (raw, handle) in collect(&uia, &cache, popup, Some(bounds)).unwrap_or_default() {
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

/// All control-view descendants of a window in document order, each once, tagged
/// with `bounds` for popups.
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
        let mut stack = children(&root);
        let mut visits = 0;
        while let Some(el) = stack.pop() {
            visits += 1;
            if visits > MAX_NODES {
                break;
            }
            let Ok(mut raw) = raw_from(&el) else { continue };
            if !raw.key.is_empty() && !seen.insert(raw.key.clone()) {
                continue;
            }
            stack.extend(children(&el));
            raw.bounds = bounds;
            raws.push((raw, Some(Handle(el))));
        }
        Ok(raws)
    }
}
