//! Opening the dock with context (clipboard, Explorer selection) attached to the prompt.

use tauri::{AppHandle, Emitter};

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContextItem {
    Path { path: String, dir: bool },
    Text { text: String },
    Image { path: String },
}

#[derive(Clone, serde::Serialize)]
pub struct DockOpen {
    pub source: &'static str,
    pub items: Vec<ContextItem>,
}

/// Remembers the foreground window (like the hotkey does), then emits `dock-open` to the dock.
pub fn open(app: &AppHandle, source: &'static str, items: Vec<ContextItem>) {
    #[cfg(windows)]
    super::target::remember();
    let _ = app.emit_to(super::LABEL, "dock-open", DockOpen { source, items });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_shape() {
        let o = DockOpen {
            source: "clipboard",
            items: vec![
                ContextItem::Path { path: "C:\\a.pdf".into(), dir: false },
                ContextItem::Text { text: "hi".into() },
                ContextItem::Image { path: "C:\\c.png".into() },
            ],
        };
        assert_eq!(
            serde_json::to_string(&o).unwrap(),
            r#"{"source":"clipboard","items":[{"kind":"path","path":"C:\\a.pdf","dir":false},{"kind":"text","text":"hi"},{"kind":"image","path":"C:\\c.png"}]}"#
        );
    }
}
