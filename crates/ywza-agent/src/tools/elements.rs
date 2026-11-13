use schemars::JsonSchema;
use serde::Deserialize;

use super::{
    ocr::snapshot_with_ocr,
    output::{desk_err, flat},
    Ctx, Tool, ToolOutput, WindowArg,
};

/// List a window's UI elements, one per line with numeric ids: `[3] edit "Name" = "value"`. Toggles show `= on/off`, sliders and spinners their value; flags: `selected` (chosen radio/option/tab), `busy` (loading), `in "Question"` (a radio's group), `+menu`, `disabled`. An open dialog of the window (file picker) is one line naming its own window id: get_elements that window to use it. `ocr` lines are text read from pixels. Required before element actions; replaces the previous list of that window.
#[derive(Deserialize, JsonSchema)]
pub struct GetElements {
    /// Window id from list_windows/launch, e.g. "w3412106".
    window: WindowArg,
    /// Also read on-screen text with the more accurate OCR engine (slower). Use when text you can see is missing.
    #[serde(default)]
    accurate_ocr: bool,
    /// Browsers: list only the web page and its popups, not the toolbar, tabs and bookmarks. Ids of skipped elements stay valid.
    #[serde(default)]
    page_only: bool,
}

impl Tool for GetElements {
    const NAME: &'static str = "get_elements";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        flat(async {
            let w = self.window.0;
            let (snap, note) = snapshot_with_ocr(w, cx.snap(w), self.accurate_ocr, cx.ocr)
                .await
                .map_err(|e| desk_err(e).elements(w))?;
            let list = if self.page_only {
                snap.render_page()
            } else {
                snap.render()
            };
            let text = format!("{list}{note}");
            cx.store(w, snap);
            Ok(ToolOutput::ok(text).elements(w))
        })
        .await
    }
}
