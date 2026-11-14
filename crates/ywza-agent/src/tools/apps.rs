use schemars::JsonSchema;
use serde::Deserialize;
use ywza_desktop as desk;

use super::{util::blocking, Ctx, Tool, ToolOutput};

/// Whether this computer can launch a program or app by name, and where it resolves to. Use it (with fanout) to find which device has an app.
#[derive(Deserialize, JsonSchema)]
pub struct FindApp {
    /// Program or app name, e.g. "virtuoso" or "chrome".
    name: String,
}

impl Tool for FindApp {
    const NAME: &'static str = "find_app";

    async fn run(self, _: &Ctx) -> ToolOutput {
        let name = self.name.clone();
        match blocking(move || desk::find_program(&name)).await {
            Some(path) => ToolOutput::ok(format!("`{}` found: {path}", self.name)),
            None => ToolOutput::err(format!("`{}` not found", self.name)),
        }
    }
}
