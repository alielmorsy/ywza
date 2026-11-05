use schemars::JsonSchema;
use serde::Deserialize;

use super::{
    ocr::snapshot_with_ocr,
    output::{desk_err, flat},
    Ctx, Tool, ToolOutput, WindowArg,
};

/// List a window's UI elements, one per line with numeric ids: `[3] edit "Name" = "value"`. `ocr` lines are text read from pixels. Required before element actions; replaces the previous list of that window.
#[derive(Deserialize, JsonSchema)]
pub struct GetElements {
    /// Window id from list_windows/launch, e.g. "w3412106".
    window: WindowArg,
    /// Also read on-screen text with the more accurate OCR engine (slower). Use when text you can see is missing.
    #[serde(default)]
    accurate_ocr: bool,
}

impl Tool for GetElements {
    const NAME: &'static str = "get_elements";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        flat(async {
            let w = self.window.0;
            let (snap, note) = snapshot_with_ocr(w, cx.snap(w), self.accurate_ocr, cx.ocr)
                .await
                .map_err(|e| desk_err(e).elements(w))?;
            let text = format!("{}{note}", snap.render());
            cx.store(w, snap);
            Ok(ToolOutput::ok(text).elements(w))
        })
        .await
    }
}
