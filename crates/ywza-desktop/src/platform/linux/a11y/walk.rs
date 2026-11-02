use super::bus::{proxy_of, step};
use super::node::{handle_of, raw_of, Handle};
use crate::snapshot::Raw;
use crate::Rect;
use atspi::proxy::accessible::AccessibleProxy;
use atspi::zbus;
use atspi::ObjectRefOwned;

/// Nodes visited per window and tree depth; beyond that the rest is not listed.
const MAX_NODES: usize = 4000;
const MAX_DEPTH: usize = 48;

/// Depth-first walk below `root` (document order), not listing `root` itself.
/// Hidden nodes are listed but not descended into.
pub(super) async fn walk(
    conn: &zbus::Connection,
    root: &AccessibleProxy<'_>,
    bounds: Option<Rect>,
    out: &mut Vec<(Raw, Option<Handle>)>,
) {
    let mut stack: Vec<(ObjectRefOwned, usize)> = step(root.get_children())
        .await
        .unwrap_or_default()
        .into_iter()
        .rev()
        .map(|c| (c, 1))
        .collect();
    let mut visited = 0;
    while let Some((node, depth)) = stack.pop() {
        visited += 1;
        if visited > MAX_NODES {
            break;
        }
        let Some(acc) = step(proxy_of(conn, &node)).await else {
            continue;
        };
        let Some((mut raw, showing)) = step(raw_of(&acc)).await else {
            continue;
        };
        raw.bounds = bounds;
        if showing && depth < MAX_DEPTH {
            let kids = step(acc.get_children()).await.unwrap_or_default();
            stack.extend(kids.into_iter().rev().map(|c| (c, depth + 1)));
        }
        out.push((raw, Some(handle_of(&acc))));
    }
}
