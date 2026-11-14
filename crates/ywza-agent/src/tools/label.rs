use schemars::JsonSchema;
use serde::Deserialize;

use super::{Ctx, ElementArg, Tool, ToolOutput, WindowArg};

/// Name an unnamed element (an icon-only button) so later element lists show it as `"label" (learned)` in this app, also after restarts. An empty label forgets it.
#[derive(Deserialize, JsonSchema)]
pub struct LabelElement {
    /// Window id from list_windows/launch, e.g. "w3412106".
    window: WindowArg,
    /// Element id from this window's latest get_elements.
    element: ElementArg,
    /// What the element does, e.g. "Send".
    label: String,
}

impl Tool for LabelElement {
    const NAME: &'static str = "label_element";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        let (w, id) = (self.window.0, self.element.0);
        let Some(mut snap) = cx.snap(w) else {
            return ToolOutput::err(format!("no element list for w{w}; call get_elements first"));
        };
        let Some(el) = snap.elements.iter_mut().find(|e| e.id == id) else {
            return ToolOutput::err(format!("no element {id} in w{w}'s latest get_elements"));
        };
        if !el.anchor.name.is_empty() {
            return ToolOutput::err(format!("element {id} already has a name: {}", el.line()));
        }
        let label = self.label.trim();
        if let Err(e) = crate::labels::learn(&el.anchor, label) {
            return ToolOutput::err(format!("could not save the label: {e:#}"));
        }
        if !label.is_empty() {
            el.name = label.to_string();
            el.learned = true;
        } else if el.learned {
            el.name.clear();
            el.learned = false;
        }
        let line = el.line();
        cx.store(w, snap);
        ToolOutput::ok(if label.is_empty() {
            format!("forgot the label: {line}")
        } else {
            format!("ok: {line}")
        })
    }
}
