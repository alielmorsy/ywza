use ywza_proto::SessionEvent;

const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// Renders one session event as readable text: tool calls dimmed, results cut to 3 lines unless `verbose`.
pub fn format_event(ev: &SessionEvent, verbose: bool, color: bool) -> String {
    let dim = |s: String| {
        if color {
            format!("{DIM}{s}{RESET}")
        } else {
            s
        }
    };
    match ev {
        SessionEvent::User { text } => format!("you: {text}"),
        SessionEvent::Assistant { text } => text.clone(),
        SessionEvent::ToolCall { name, args, .. } => dim(format!("> {name} {args}")),
        SessionEvent::ToolResult {
            ok,
            text,
            has_image,
            ..
        } => {
            let lines: Vec<&str> = text.lines().collect();
            let mut out = if verbose || lines.len() <= 3 {
                text.trim_end().to_string()
            } else {
                format!(
                    "{}\n... ({} more lines)",
                    lines[..3].join("\n"),
                    lines.len() - 3
                )
            };
            if *has_image {
                out.push_str(" [image]");
            }
            let tag = if *ok { "<" } else { "< error:" };
            dim(format!("{tag} {}", out.replace('\n', "\n  ")))
        }
        SessionEvent::Done { summary } => format!("done: {summary}"),
        SessionEvent::Failed { error } => format!("failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_events() {
        let call = SessionEvent::ToolCall {
            id: "1".into(),
            name: "click".into(),
            args: serde_json::json!({"element": 3}),
            device: None,
        };
        assert_eq!(
            format_event(&call, false, true),
            "\x1b[2m> click {\"element\":3}\x1b[0m"
        );
        assert_eq!(format_event(&call, false, false), "> click {\"element\":3}");

        let res = |text: &str| SessionEvent::ToolResult {
            id: "1".into(),
            ok: true,
            text: text.into(),
            has_image: false,
            image: None,
        };
        assert_eq!(format_event(&res("a\nb"), false, false), "< a\n  b");
        let long = "1\n2\n3\n4\n5";
        assert_eq!(
            format_event(&res(long), false, false),
            "< 1\n  2\n  3\n  ... (2 more lines)"
        );
        assert_eq!(
            format_event(&res(long), true, false),
            "< 1\n  2\n  3\n  4\n  5"
        );

        let done = SessionEvent::Done {
            summary: "ok".into(),
        };
        assert_eq!(format_event(&done, false, true), "done: ok");
        let failed = SessionEvent::Failed { error: "x".into() };
        assert_eq!(format_event(&failed, false, true), "failed: x");
    }
}
