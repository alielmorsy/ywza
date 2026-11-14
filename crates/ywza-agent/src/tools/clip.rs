use schemars::JsonSchema;
use serde::Deserialize;
use ywza_desktop as desk;

use super::{output::desk_err, util::blocking, Ctx, Tool, ToolOutput};

const DEFAULT_MAX: usize = 100_000;

/// Read the text on this computer's clipboard.
#[derive(Deserialize, JsonSchema)]
pub struct ClipboardGet {
    /// Longest text to return in characters; longer text is cut with a note (default 100000).
    max_chars: Option<usize>,
}

impl Tool for ClipboardGet {
    const NAME: &'static str = "clipboard_get";

    async fn run(self, _: &Ctx) -> ToolOutput {
        match blocking(desk::clipboard_text).await {
            Err(e) => desk_err(e),
            Ok(None) => ToolOutput::err("clipboard holds no text"),
            Ok(Some(text)) => ToolOutput::ok(limit(&text, self.max_chars.unwrap_or(DEFAULT_MAX))),
        }
    }
}

/// Put text on this computer's clipboard, replacing what it holds.
#[derive(Deserialize, JsonSchema)]
pub struct ClipboardSet {
    text: String,
}

impl Tool for ClipboardSet {
    const NAME: &'static str = "clipboard_set";

    async fn run(self, _: &Ctx) -> ToolOutput {
        let n = self.text.chars().count();
        match blocking(move || desk::set_clipboard_text(&self.text)).await {
            Ok(()) => ToolOutput::ok(format!("clipboard set ({n} chars)")),
            Err(e) => desk_err(e),
        }
    }
}

fn limit(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        None => text.to_string(),
        Some((cut, _)) => format!(
            "{}\n(clipboard truncated: first {max} of {} chars shown)",
            &text[..cut],
            text.chars().count()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_text_is_cut_on_a_char_boundary_with_a_note() {
        assert_eq!(limit("abc", 3), "abc");
        let cut = limit("aéz", 2);
        assert!(cut.starts_with("aé\n(clipboard truncated"), "{cut}");
        assert!(cut.contains("2 of 3"));
    }
}
