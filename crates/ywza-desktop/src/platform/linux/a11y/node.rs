use crate::platform::linux::roles;
use crate::platform::linux::x::os;
use crate::snapshot::Raw;
use crate::{Error, Patterns, Rect, Result};
use atspi::proxy::accessible::AccessibleProxy;
use atspi::proxy::proxy_ext::ProxyExt;
use atspi::{CoordType, Role, State};
use std::hash::{Hash, Hasher};

/// Where to find an element again: its D-Bus name and object path on the AT-SPI bus.
#[derive(Clone, Debug)]
pub struct Handle {
    pub(super) bus: String,
    pub(super) path: String,
}

/// Stable identity of a node: a hash of its bus name and object path (the matching
/// rule of `Snapshot::build` compares `key`s).
pub(super) fn key_of(bus: &str, path: &str) -> Vec<i32> {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (bus, path).hash(&mut h);
    let v = h.finish();
    vec![(v >> 32) as i32, v as i32]
}

pub(super) fn handle_of(acc: &AccessibleProxy<'_>) -> Handle {
    let inner = acc.inner();
    Handle {
        bus: inner.destination().to_string(),
        path: inner.path().to_string(),
    }
}

/// Reads one node. `Err` for defunct or unreadable nodes; the bool is `Showing`.
pub(super) async fn raw_of(acc: &AccessibleProxy<'_>) -> Result<(Raw, bool)> {
    let (role, name, state, desc, aid, proxies) = tokio::join!(
        acc.get_role(),
        acc.name(),
        acc.get_state(),
        acc.description(),
        acc.accessible_id(),
        acc.proxies()
    );
    let (role, state) = (role.map_err(os)?, state.map_err(os)?);
    if state.contains(State::Defunct) {
        return Err(Error::Os("defunct".into()));
    }
    let proxies = proxies.map_err(os)?;
    let showing = state.contains(State::Showing);

    let rect = match proxies.component().await {
        Ok(c) => c
            .get_extents(CoordType::Screen)
            .await
            .map(|(x, y, w, h)| Rect::new(x, y, w, h))
            .unwrap_or_default(),
        Err(_) => Rect::default(),
    };
    let actions = match proxies.action().await {
        Ok(a) => a.n_actions().await.unwrap_or(0),
        Err(_) => 0,
    };
    let editable = state.contains(State::Editable) && proxies.editable_text().await.is_ok();
    let toggle = roles::is_toggle(role).then(|| {
        if state.contains(State::Indeterminate) {
            2
        } else if state.contains(State::Checked) || state.contains(State::Pressed) {
            1
        } else {
            0
        }
    });
    let mut value = None;
    if roles::has_text_value(role) && role != Role::PasswordText {
        if let Ok(t) = proxies.text().await {
            let n = t.character_count().await.unwrap_or(0).clamp(0, 2000);
            value = t.get_text(0, n).await.ok();
        }
    } else if roles::has_number_value(role) {
        if let Ok(v) = proxies.value().await {
            value = v.current_value().await.ok().map(|v| format!("{v}"));
        }
    }
    let patterns = Patterns {
        invoke: actions > 0,
        value: editable || value.is_some(),
        value_readonly: !editable && !roles::has_number_value(role),
        toggle: toggle.is_some(),
        expand: state.contains(State::Expandable),
        select: state.contains(State::Selectable),
        scroll: false,
    };
    let inner = acc.inner();
    let key = key_of(inner.destination().as_str(), inner.path().as_str());
    let raw = Raw {
        role: roles::kind(role),
        name: name.unwrap_or_default(),
        help: desc.unwrap_or_default(),
        automation_id: aid.unwrap_or_default(),
        rect,
        offscreen: !showing,
        enabled: state.contains(State::Enabled) || state.contains(State::Sensitive),
        focusable: state.contains(State::Focusable),
        patterns,
        value,
        toggle,
        key,
        bounds: None,
    };
    Ok((raw, showing))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_stable_and_distinct() {
        assert_eq!(key_of(":1.5", "/a/1"), key_of(":1.5", "/a/1"));
        assert_ne!(key_of(":1.5", "/a/1"), key_of(":1.5", "/a/2"));
        assert_ne!(key_of(":1.5", "/a/1"), key_of(":1.6", "/a/1"));
    }
}
