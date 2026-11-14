use super::model::Raw;
use std::collections::HashMap;

/// Appends a popup's nodes (indices relative to `sub`) to `raws`. A node whose key is
/// already present is dropped and its children are re-parented onto the kept node.
pub(crate) fn merge_popup<T>(raws: &mut Vec<(Raw, T)>, sub: Vec<(Raw, T)>) {
    let mut index: HashMap<Vec<i32>, usize> = raws
        .iter()
        .enumerate()
        .filter(|(_, (r, _))| !r.key.is_empty())
        .map(|(i, (r, _))| (r.key.clone(), i))
        .collect();
    let mut at: Vec<usize> = Vec::with_capacity(sub.len());
    for (mut raw, extra) in sub {
        raw.parent = raw.parent.and_then(|p| at.get(p).copied());
        if !raw.key.is_empty() {
            if let Some(&kept) = index.get(&raw.key) {
                at.push(kept);
                continue;
            }
            index.insert(raw.key.clone(), raws.len());
        }
        at.push(raws.len());
        raws.push((raw, extra));
    }
}
