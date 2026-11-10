use std::path::Path;

use schemars::JsonSchema;
use serde::Deserialize;

use super::{
    args::yes,
    output::{flat, term_err, truncate},
    util::blocking,
    Ctx, TermArg, Tool, ToolOutput,
};

fn lines_default() -> u32 {
    80
}

/// List terminals: ones you started (owned) and existing consoles/Windows Terminal tabs you can attach to: [t2] name (kind) cwd "title".
#[derive(Deserialize, JsonSchema)]
pub struct TermList {}

impl Tool for TermList {
    const NAME: &'static str = "term_list";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        let terms = cx.terms();
        let text = blocking(move || terms.list_text()).await;
        ToolOutput::ok(if text.is_empty() {
            "no terminals".into()
        } else {
            text
        })
    }
}

/// Start a command in a new interactive terminal you own (e.g. "pwsh", "claude"). Returns its terminal id.
#[derive(Deserialize, JsonSchema)]
pub struct TermStart {
    command: String,
    cwd: Option<String>,
}

impl Tool for TermStart {
    const NAME: &'static str = "term_start";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        flat(async {
            let terms = cx.terms();
            let Self { command, cwd } = self;
            let id = blocking(move || terms.start(&command, cwd.as_deref().map(Path::new)))
                .await
                .map_err(term_err)?;
            Ok(ToolOutput::ok(format!(
                "started {id}; use term_read to see its output"
            )))
        })
        .await
    }
}

/// Read a terminal's recent text (owned: output log; attached: screen buffer).
#[derive(Deserialize, JsonSchema)]
pub struct TermRead {
    /// Terminal id from term_list/term_start, e.g. "t2".
    term: TermArg,
    #[serde(default = "lines_default")]
    lines: u32,
}

impl Tool for TermRead {
    const NAME: &'static str = "term_read";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        flat(async {
            let terms = cx.terms();
            let (id, lines) = (self.term.0, self.lines.clamp(1, 2000) as usize);
            let text = blocking(move || terms.read(id, lines))
                .await
                .map_err(term_err)?;
            Ok(ToolOutput::ok(truncate(text)))
        })
        .await
    }
}

/// Type text into a terminal, followed by Enter unless enter=false. Read the screen first.
#[derive(Deserialize, JsonSchema)]
pub struct TermWrite {
    /// Terminal id from term_list/term_start, e.g. "t2".
    term: TermArg,
    text: String,
    #[serde(default = "yes")]
    enter: bool,
}

impl Tool for TermWrite {
    const NAME: &'static str = "term_write";
    const RISKY: bool = true;

    async fn run(self, cx: &Ctx) -> ToolOutput {
        flat(async {
            let terms = cx.terms();
            let Self { term, text, enter } = self;
            let id = term.0;
            blocking(move || terms.write(id, &text, enter))
                .await
                .map_err(term_err)?;
            Ok(ToolOutput::ok(format!("sent to {id}")))
        })
        .await
    }
}

/// Send keys to a terminal, space-separated: "esc", "ctrl+c", "up up enter".
#[derive(Deserialize, JsonSchema)]
pub struct TermKeys {
    /// Terminal id from term_list/term_start, e.g. "t2".
    term: TermArg,
    keys: String,
}

impl Tool for TermKeys {
    const NAME: &'static str = "term_keys";
    const RISKY: bool = true;

    async fn run(self, cx: &Ctx) -> ToolOutput {
        flat(async {
            let terms = cx.terms();
            let Self { term, keys } = self;
            let id = term.0;
            blocking(move || terms.keys(id, &keys))
                .await
                .map_err(term_err)?;
            Ok(ToolOutput::ok(format!("sent to {id}")))
        })
        .await
    }
}
