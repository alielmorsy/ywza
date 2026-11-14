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
        SessionEvent::Help {
            kind,
            reason,
            window,
            title,
            ..
        } => {
            let mut out = format!("needs you ({kind}): {reason}");
            if let Some(w) = window {
                out.push_str(&format!(" in {w}"));
                if !title.is_empty() {
                    out.push_str(&format!(" {title:?}"));
                }
            }
            out
        }
        SessionEvent::Resumed { digest } => {
            format!("resumed: {}", digest.lines().next().unwrap_or_default())
        }
        SessionEvent::Usage {
            input,
            output,
            requests,
            screenshot_estimate,
            cost,
            screenshot_cost,
            ..
        } => {
            if *requests == 0 {
                return String::new();
            }
            let money = |c: &Option<f64>, pre: &str| match c {
                Some(c) if *c < 0.01 => format!(", {pre}${c:.3}"),
                Some(c) => format!(", {pre}${c:.2}"),
                None => String::new(),
            };
            dim(format!(
                "tokens {} in / {} out{} (a screenshot agent: ~{}{})",
                tokens(*input),
                tokens(*output),
                money(cost, ""),
                tokens(*screenshot_estimate),
                money(screenshot_cost, "~")
            ))
        }
    }
}

fn tokens(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1000..=99_999 => format!("{:.1}k", n as f64 / 1e3),
        100_000..=999_999 => format!("{}k", n / 1000),
        _ => format!("{:.1}M", n as f64 / 1e6),
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

        let help = SessionEvent::Help {
            kind: "otp".into(),
            reason: "a code is needed".into(),
            window: Some("w123".into()),
            title: "Sign in".into(),
            monitor: None,
            rect: None,
        };
        assert_eq!(
            format_event(&help, false, true),
            "needs you (otp): a code is needed in w123 \"Sign in\""
        );
        let resumed = SessionEvent::Resumed {
            digest: "first\nsecond".into(),
        };
        assert_eq!(format_event(&resumed, false, true), "resumed: first");

        let usage = |cost, screenshot_cost| SessionEvent::Usage {
            input: 18_200,
            output: 1_100,
            cached: 0,
            requests: 5,
            tool_calls: 4,
            screenshot_estimate: 142_000,
            cost,
            screenshot_cost,
        };
        assert_eq!(
            format_event(&usage(Some(0.04), Some(0.35)), false, false),
            "tokens 18.2k in / 1.1k out, $0.04 (a screenshot agent: ~142k, ~$0.35)"
        );
        assert_eq!(
            format_event(&usage(None, None), false, false),
            "tokens 18.2k in / 1.1k out (a screenshot agent: ~142k)"
        );
    }
}
