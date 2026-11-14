//! Anchors as stored on disk (plans, learned labels): `ywza_desktop::Anchor` with owned roles.

use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};

use serde::{Deserialize, Serialize};
use ywza_desktop::{Anchor, Rect, Step};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SavedStep {
    pub role: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub automation_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SavedAnchor {
    pub process: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub automation_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<SavedStep>,
    /// x, y, w, h relative to the window.
    pub rel: [i32; 4],
}

/// Roles are a small fixed vocabulary, so interning them leaks a bounded amount.
fn intern(s: &str) -> &'static str {
    static ROLES: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut set = ROLES.get_or_init(Default::default).lock().unwrap();
    if let Some(&r) = set.get(s) {
        return r;
    }
    let r: &'static str = Box::leak(s.to_string().into_boxed_str());
    set.insert(r);
    r
}

impl From<&Anchor> for SavedAnchor {
    fn from(a: &Anchor) -> Self {
        Self {
            process: a.process.clone(),
            role: a.role.into(),
            automation_id: a.automation_id.clone(),
            name: a.name.clone(),
            path: a
                .path
                .iter()
                .map(|s| SavedStep {
                    role: s.role.into(),
                    automation_id: s.automation_id.clone(),
                    name: s.name.clone(),
                })
                .collect(),
            rel: [a.rel.x, a.rel.y, a.rel.w, a.rel.h],
        }
    }
}

impl SavedAnchor {
    pub fn to_anchor(&self) -> Anchor {
        let [x, y, w, h] = self.rel;
        Anchor {
            process: self.process.clone(),
            role: intern(&self.role),
            automation_id: self.automation_id.clone(),
            name: self.name.clone(),
            path: self
                .path
                .iter()
                .map(|s| Step {
                    role: intern(&s.role),
                    automation_id: s.automation_id.clone(),
                    name: s.name.clone(),
                })
                .collect(),
            rel: Rect::new(x, y, w, h),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let a = Anchor {
            process: "app.exe".into(),
            role: "button",
            automation_id: "save".into(),
            name: "save".into(),
            path: vec![Step {
                role: "pane",
                automation_id: String::new(),
                name: "toolbar".into(),
            }],
            rel: Rect::new(1, 2, 3, 4),
        };
        let json = serde_json::to_string(&SavedAnchor::from(&a)).unwrap();
        let back: SavedAnchor = serde_json::from_str(&json).unwrap();
        assert_eq!(back.to_anchor(), a);
    }
}
