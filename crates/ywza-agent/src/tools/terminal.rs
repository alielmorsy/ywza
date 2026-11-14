use std::path::Path;
use std::time::Duration;

use schemars::JsonSchema;
use serde::Deserialize;
use ywza_term::ShellInfo;

use super::{
    args::yes,
    output::{flat, term_err, truncate},
    util::blocking,
    Ctx, TermArg, Tool, ToolOutput,
};

fn lines_default() -> u32 {
    80
}

fn run_timeout_default() -> u64 {
    120_000
}

const RUN_TIMEOUT_MAX_MS: u64 = 600_000;

fn shell_line(i: &ShellInfo) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(c) = &i.cwd {
        parts.push(format!("cwd {c}"));
    }
    if i.busy {
        parts.push("command running".to_string());
    } else if let Some(e) = i.exit {
        parts.push(format!("last exit code {e}"));
    }
    (!parts.is_empty()).then(|| format!("[{}]", parts.join(", ")))
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

/// Start a command in a new interactive terminal you own (e.g. "pwsh", "claude"). Returns its terminal id. A bare pwsh, powershell, bash or zsh gets shell integration (cwd and exit codes shown, term_run works).
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
            let (text, info) = blocking(move || {
                let text = terms.read(id, lines)?;
                let info = terms.owned(id).ok().and_then(|t| t.shell_info());
                Ok::<_, ywza_term::Error>((text, info))
            })
            .await
            .map_err(term_err)?;
            let text = truncate(text);
            Ok(ToolOutput::ok(match info.as_ref().and_then(shell_line) {
                Some(head) => format!("{head}\n{text}"),
                None => text,
            }))
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

/// Run one command in a terminal you own that has shell integration (a pwsh, powershell, bash or zsh started with term_start) and wait for it to finish. Returns its output, exit code and cwd. Use it instead of term_write + term_read for commands that end; on timeout it returns the output so far and the command keeps running.
#[derive(Deserialize, JsonSchema)]
pub struct TermRun {
    /// Terminal id from term_list/term_start, e.g. "t2".
    term: TermArg,
    command: String,
    #[serde(default = "run_timeout_default")]
    timeout_ms: u64,
}

impl Tool for TermRun {
    const NAME: &'static str = "term_run";
    const RISKY: bool = true;

    async fn run(self, cx: &Ctx) -> ToolOutput {
        flat(async {
            let terms = cx.terms();
            let Self {
                term,
                command,
                timeout_ms,
            } = self;
            let id = term.0;
            let limit = Duration::from_millis(timeout_ms.clamp(1_000, RUN_TIMEOUT_MAX_MS));
            let r = blocking(move || {
                let t = terms.owned(id)?;
                if !t.wait_integration() {
                    return Ok(None);
                }
                t.run(&command, limit).map(Some)
            })
            .await
            .map_err(term_err)?;
            let Some(r) = r else {
                return Err(ToolOutput::err(format!(
                    "no shell integration in {id}; use term_write and term_read"
                )));
            };
            let mut text = truncate(r.output);
            if !text.is_empty() {
                text.push('\n');
            }
            if r.finished {
                match r.exit {
                    Some(c) => text.push_str(&format!("exit code {c}")),
                    None => text.push_str("exit code unknown"),
                }
            } else if r.closed {
                text.push_str("terminal closed before the command finished");
            } else {
                text.push_str(&format!(
                    "still running after {} s; term_read to follow it, term_keys ctrl+c to stop it",
                    limit.as_secs()
                ));
            }
            if let Some(c) = r.cwd {
                text.push_str(&format!("\ncwd {c}"));
            }
            Ok(ToolOutput {
                ok: r.finished && r.exit == Some(0),
                text,
                ..Default::default()
            })
        })
        .await
    }
}
