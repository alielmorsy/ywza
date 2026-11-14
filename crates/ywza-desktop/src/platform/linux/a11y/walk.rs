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
/// Hidden nodes are listed but not descended into. `Raw::parent` indexes `out`.
pub(super) async fn walk(
    conn: &zbus::Connection,
    root: &AccessibleProxy<'_>,
    bounds: Option<Rect>,
    out: &mut Vec<(Raw, Option<Handle>)>,
) {
    // Each node with its depth, the name of its nearest named group (radio questions) and
    // its parent's index.
    let mut stack: Vec<(ObjectRefOwned, usize, String, Option<usize>)> = step(root.get_children())
        .await
        .unwrap_or_default()
        .into_iter()
        .rev()
        .map(|c| (c, 1, String::new(), None))
        .collect();
    let mut visited = 0;
    while let Some((node, depth, group, parent)) = stack.pop() {
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
        raw.parent = parent;
        let at = out.len();
        if showing && depth < MAX_DEPTH {
            let inner = if raw.role == "group" && !raw.name.trim().is_empty() {
                raw.name.clone()
            } else {
                group.clone()
            };
            let kids = step(acc.get_children()).await.unwrap_or_default();
            stack.extend(
                kids.into_iter()
                    .rev()
                    .map(|c| (c, depth + 1, inner.clone(), Some(at))),
            );
        }
        raw.group = group;
        out.push((raw, Some(handle_of(&acc))));
    }
}
