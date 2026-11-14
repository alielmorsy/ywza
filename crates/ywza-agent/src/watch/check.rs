//! Condition checks shared by the `wait` tool and the watcher engine.

use ywza_desktop::{self as desk, Element, Snapshot, WindowInfo};

/// `want`: the title substring, lowercase.
pub fn title_match(w: &WindowInfo, want: &str) -> bool {
    w.title.to_lowercase().contains(want)
}

/// The last line of `out` containing `want` (lowercase).
pub fn term_line<'a>(out: &'a str, want: &str) -> Option<&'a str> {
    out.lines().rev().find(|l| l.to_lowercase().contains(want))
}

/// Finds the first element whose name or value contains `text` (OCR lines up to noise) and
/// returns its line.
pub fn text_finder(text: &str) -> impl Fn(&Snapshot) -> Option<String> {
    let want = text.to_lowercase();
    let want_ocr = desk::ocr_norm(text);
    move |s: &Snapshot| {
        s.elements
            .iter()
            .find(|e| {
                if e.kind == "ocr" {
                    return desk::ocr_matches(&desk::ocr_norm(&e.name), &want_ocr);
                }
                e.name.to_lowercase().contains(&want)
                    || e.value
                        .as_ref()
                        .is_some_and(|v| v.to_lowercase().contains(&want))
            })
            .map(|e| e.line())
    }
}

/// The first element whose name contains `want` (lowercase).
pub fn find_named<'a>(s: &'a Snapshot, want: &str) -> Option<&'a Element> {
    s.elements
        .iter()
        .find(|e| e.name.to_lowercase().contains(want))
}

/// What counts as the element changing: everything but its id and position.
pub fn element_state(e: &Element) -> String {
    format!(
        "{}|{:?}|{:?}|{}|{}|{}",
        e.kind, e.name, e.value, e.selected, e.busy, e.enabled
    )
}

/// One reading of an owned terminal.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TermNow {
    pub busy: bool,
    /// The shell's last command exit code (needs shell integration).
    pub exit: Option<i32>,
    /// The terminal's process exit code once it has ended.
    pub proc_exit: Option<i32>,
}

/// The exit code of a command (or the process) that finished between two readings and
/// matches `want` (None: any non-zero). A command that starts and ends between two readings
/// shows only as a changed exit code, so repeating the same code back to back is missed.
// shortcut: one reading a second, no run counter from the terminal; upgrade if that miss matters.
pub fn term_finished(prev: &TermNow, now: &TermNow, want: Option<i32>) -> Option<i32> {
    let ended = (prev.busy && !now.busy) || (now.exit.is_some() && now.exit != prev.exit);
    let cmd = now.exit.filter(|_| ended);
    let code = cmd.or(now.proc_exit.filter(|_| prev.proc_exit.is_none()));
    code.filter(|c| want.map_or(*c != 0, |w| *c == w))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now(busy: bool, exit: Option<i32>, proc_exit: Option<i32>) -> TermNow {
        TermNow {
            busy,
            exit,
            proc_exit,
        }
    }

    #[test]
    fn command_end_matches_the_wanted_code() {
        let run = now(true, Some(0), None);
        let done = now(false, Some(1), None);
        assert_eq!(term_finished(&run, &done, None), Some(1));
        assert_eq!(term_finished(&run, &done, Some(1)), Some(1));
        assert_eq!(term_finished(&run, &done, Some(0)), None);
        let ok = now(false, Some(0), None);
        assert_eq!(term_finished(&run, &ok, None), None);
        assert_eq!(term_finished(&run, &ok, Some(0)), Some(0));
    }

    #[test]
    fn quick_commands_show_as_a_changed_exit_code_and_idle_is_quiet() {
        let a = now(false, None, None);
        let b = now(false, Some(2), None);
        assert_eq!(term_finished(&a, &b, None), Some(2));
        assert_eq!(term_finished(&b, &b, None), None);
        assert_eq!(term_finished(&a, &a, None), None);
    }

    #[test]
    fn process_exit_fires_once() {
        let alive = now(false, Some(0), None);
        let dead = now(false, Some(0), Some(3));
        assert_eq!(term_finished(&alive, &dead, None), Some(3));
        assert_eq!(term_finished(&dead, &dead, None), None);
    }

    #[test]
    fn terminal_lines_match_any_case() {
        assert_eq!(term_line("a\nBuild OK\nb", "build ok"), Some("Build OK"));
        assert_eq!(term_line("a\nb", "zzz"), None);
    }
}
