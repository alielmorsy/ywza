//! The lens mark and the activity caption shown right before an action sends input.

use std::time::Duration;

use ywza_desktop::{self as desk, Button, MarkId, MarkKind, Phase, Rect, WindowId};

use super::Ctx;

const TARGET_MAX: usize = 40;

/// What an element action is called on screen: `Click "Save"`, or a bare `Press ctrl+s`.
pub enum Verb {
    On(&'static str),
    Keys(String),
}

/// `Click "Save"`: the target's text collapsed to one line and cut to 40 characters.
pub fn caption(verb: &str, target: Option<&str>) -> String {
    let target = target.map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "));
    match target.filter(|t| !t.is_empty()) {
        Some(t) if t.chars().count() > TARGET_MAX => {
            let cut: String = t.chars().take(TARGET_MAX).collect();
            format!("{verb} \"{}...\"", cut.trim_end())
        }
        Some(t) => format!("{verb} \"{t}\""),
        None => verb.into(),
    }
}

/// `Press ctrl+s`. A bare single character token ("p a s s") could spell a secret, so such
/// key lists are not shown.
pub fn keys_caption(keys: &str) -> String {
    let spelled = keys
        .split_whitespace()
        .any(|t| t.chars().count() == 1 && !t.contains('+') && !t.chars().all(char::is_control));
    if spelled {
        return "Press keys".into();
    }
    let keys = keys.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut = keys.chars().count() > TARGET_MAX;
    let keys: String = keys.chars().take(TARGET_MAX).collect();
    format!("Press {}{}", keys.trim_end(), if cut { "..." } else { "" })
}

pub fn click_verb(button: Button, count: u32, expand: bool) -> &'static str {
    match (expand, button, count) {
        (true, ..) => "Open",
        (_, Button::Right, _) => "Right-click",
        (_, _, 2..) => "Double-click",
        _ => "Click",
    }
}

/// Emits the activity, shows the mark and waits the dwell when one was shown. Blocks: call
/// it inside `blocking`.
pub(crate) fn begin(
    cx: &Ctx,
    w: WindowId,
    rect: Rect,
    verb: &str,
    target: Option<&str>,
    kind: MarkKind,
) -> Option<MarkId> {
    let text = caption(verb, target);
    let id = desk::lens_mark(w, rect, &text, kind);
    cx.activity(text);
    if id.is_some() {
        if let Some(d) = desk::lens_dwell().filter(|d| *d > Duration::ZERO) {
            std::thread::sleep(d);
        }
    }
    id
}

pub(crate) fn finish(id: Option<MarkId>, ok: bool) {
    if let Some(id) = id {
        desk::lens_update(id, if ok { Phase::Done } else { Phase::Failed });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caption_quotes_target() {
        assert_eq!(caption("Click", Some("Save")), "Click \"Save\"");
        assert_eq!(caption("Scroll", None), "Scroll");
        assert_eq!(caption("Scroll", Some("  ")), "Scroll");
        assert_eq!(caption("Click", Some(" Save\n all ")), "Click \"Save all\"");
    }

    #[test]
    fn caption_cuts_long_names() {
        let c = caption("Click", Some(&"x".repeat(80)));
        assert!(c.chars().count() <= 40 + 12 && c.ends_with("...\""));
    }

    #[test]
    fn typing_shows_the_field_not_the_text() {
        assert_eq!(caption("Type in", Some("Search")), "Type in \"Search\"");
    }

    #[test]
    fn keys_caption_hides_spelled_text() {
        assert_eq!(keys_caption("p a s s"), "Press keys");
        assert_eq!(keys_caption("ctrl+s"), "Press ctrl+s");
        assert_eq!(keys_caption("ctrl+k   ctrl+s"), "Press ctrl+k ctrl+s");
        assert!(keys_caption(&"enter ".repeat(20)).ends_with("..."));
    }

    #[test]
    fn click_verbs_follow_the_gesture() {
        assert_eq!(click_verb(Button::Left, 1, false), "Click");
        assert_eq!(click_verb(Button::Left, 2, false), "Double-click");
        assert_eq!(click_verb(Button::Right, 1, false), "Right-click");
        assert_eq!(click_verb(Button::Left, 1, true), "Open");
    }
}
