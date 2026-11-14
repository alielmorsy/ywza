use super::bus::{block, connect, proxy_of, step};
use super::frame::find_frame;
use super::node::{key_of, raw_of, Handle};
use super::walk::walk;
use crate::platform::linux::roles;
use crate::platform::linux::x;
use crate::snapshot::{merge_popup, Raw};
use crate::{Result, Snapshot, WindowId, WindowInfo};

/// Snapshots a window's elements. With `prev` (the last snapshot of the same
/// window), unchanged elements keep their IDs and `seq` increments. A window the
/// accessibility bus does not know (toolkit without AT-SPI) yields an empty snapshot,
/// so the caller falls back to OCR.
pub fn snapshot(window: WindowId, prev: Option<&Snapshot>) -> Result<Snapshot> {
    let info = x::window_info(window)?;
    let mut raws = walk_window(&info)?;
    if super::chromium::needs_wake(&raws, &info) && super::chromium::wake(info.pid) {
        raws = walk_window(&info)?;
    }
    Ok(Snapshot::build(info, raws, prev))
}

fn walk_window(info: &WindowInfo) -> Result<Vec<(Raw, Option<Handle>)>> {
    block(async {
        let conn = &connect().await?;
        let Some(found) = find_frame(conn, info).await? else {
            return Ok(Vec::new());
        };
        let mut raws = Vec::new();
        walk(conn, &found.frame, None, &mut raws).await;
        // Menus and dropdowns hang off the application, not the frame.
        let frame_key = key_of(
            found.frame.inner().destination().as_str(),
            found.frame.inner().path().as_str(),
        );
        for child in step(found.app.get_children()).await.unwrap_or_default() {
            let Some(popup) = step(proxy_of(conn, &child)).await else {
                continue;
            };
            let Some((raw, showing)) = step(raw_of(&popup)).await else {
                continue;
            };
            if raw.key == frame_key || !showing || raw.rect.intersect(&info.rect).is_empty() {
                continue;
            }
            if !step(popup.get_role())
                .await
                .is_some_and(roles::is_popup_root)
            {
                continue;
            }
            let mut sub = Vec::new();
            walk(conn, &popup, Some(raw.rect), &mut sub).await;
            merge_popup(&mut raws, sub);
        }
        Ok(raws)
    })
}
