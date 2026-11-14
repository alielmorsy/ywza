//! The text the model gets when the user hands the run back.

use ywza_desktop::{diff, render_diff, Delta, Element, Snapshot, WindowId};

use crate::plans::PlanStep;

/// The changes between `before` and `fresh` (None: the window is gone), without what the user
/// typed into editable fields.
pub fn changes(w: WindowId, before: &Snapshot, fresh: Option<&Snapshot>) -> String {
    let Some(fresh) = fresh else {
        return format!("window w{w} closed");
    };
    let mut deltas = diff(before, fresh);
    for d in &mut deltas {
        match d {
            Delta::Added(e) => hide(e),
            Delta::Changed { old, new } => {
                hide(old);
                hide(new);
            }
            Delta::Removed(_) => {}
        }
    }
    format!("Changes in w{w}:\n{}", render_diff(&deltas).trim_end())
}

fn hide(e: &mut Element) {
    if e.patterns.value && !e.patterns.value_readonly && e.value.as_deref().is_some_and(|v| !v.is_empty())
    {
        e.value = Some("(hidden)".into());
    }
}

/// `changes` is the `changes` text, if the window was tracked. `saved`: the plan the taps became.
pub fn text(
    kind: &str,
    note: &str,
    changes: Option<&str>,
    steps: &[PlanStep],
    saved: Option<&str>,
) -> String {
    let mut out = format!("The user took over ({kind}) and handed back.");
    if !note.trim().is_empty() {
        out.push_str(&format!("\nTheir note: {}", note.trim()));
    }
    if let Some(c) = changes {
        out.push_str(&format!("\n{c}"));
    }
    if !steps.is_empty() {
        let clicked: Vec<String> = steps.iter().map(clicked).collect();
        out.push_str(&format!("\nThey clicked: {}", clicked.join(", ")));
    }
    if let Some(name) = saved {
        out.push_str(&format!("\nTheir clicks were saved as plan {name}."));
    }
    out
}

fn clicked(s: &PlanStep) -> String {
    match &s.anchor {
        Some(a) if !a.name.is_empty() => format!("{} {:?}", a.role, a.name),
        Some(a) => a.role.clone(),
        None => s.tool.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{anchor::SavedAnchor, plans::PlanStep};

    #[test]
    fn digest_lists_note_clicks_and_plan() {
        let step = PlanStep {
            tool: "click".into(),
            anchor: Some(SavedAnchor {
                role: "button".into(),
                name: "verify".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let t = text("otp", " done ", Some("window w1 closed"), &[step], Some("fix-otp-a"));
        assert_eq!(
            t,
            "The user took over (otp) and handed back.\nTheir note: done\nwindow w1 closed\n\
             They clicked: button \"verify\"\nTheir clicks were saved as plan fix-otp-a."
        );
        assert_eq!(
            text("stuck", "", None, &[], None),
            "The user took over (stuck) and handed back."
        );
    }
}
