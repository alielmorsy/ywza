use super::ctx::LaunchCtx;
use crate::{WindowId, WindowInfo};

/// Which of `wins` belong to a launch (pure, see `Launched::wait_for_window`), best first:
/// 1. windows of the launched process or its descendants (`family`) that are new or newly titled;
/// 2. once the launcher is gone (it handed off to a running instance): new or newly titled
///    windows of the handler exe (any new window when the handler is unknown);
/// 3. an existing window of the handler exe that became the foreground window after the launch
///    (single-instance apps reuse their window);
/// 4. `settled` (launcher gone for a while): the handler's foreground window, which may already
///    have been in front before the launch.
pub(super) fn select(
    wins: &[WindowInfo],
    ctx: &LaunchCtx,
    family: &[u32],
    launcher_alive: bool,
    fg: Option<WindowId>,
    settled: bool,
) -> Vec<WindowInfo> {
    let pick = |f: &dyn Fn(&WindowInfo) -> bool| -> Vec<WindowInfo> {
        wins.iter().filter(|w| !w.hidden && f(w)).cloned().collect()
    };
    let own = pick(&|w| family.contains(&w.pid) && ctx.fresh(w));
    if !own.is_empty() || launcher_alive {
        return own;
    }
    if ctx.exes.is_empty() {
        return pick(&|w| !ctx.before.contains_key(&w.id));
    }
    let handed = pick(&|w| ctx.handler(w) && ctx.fresh(w));
    if !handed.is_empty() {
        return handed;
    }
    let reused = pick(&|w| ctx.handler(w) && fg == Some(w.id) && fg != ctx.fg);
    if !reused.is_empty() || !settled {
        return reused;
    }
    pick(&|w| ctx.handler(w) && fg == Some(w.id))
}

#[cfg(test)]
mod tests {
    use super::super::ctx::KEY_BIT;
    use super::*;
    use crate::Rect;

    fn win(id: WindowId, pid: u32, process: &str, title: &str) -> WindowInfo {
        WindowInfo {
            id,
            title: title.into(),
            process: process.into(),
            pid,
            rect: Rect::default(),
            monitor: 0,
            minimized: false,
            hidden: false,
            focused: false,
            maximized: false,
        }
    }

    fn ctx(before: &[(WindowId, &str)], fg: Option<WindowId>, exes: &[&str]) -> LaunchCtx {
        LaunchCtx {
            before: before
                .iter()
                .map(|&(i, t)| (i, Some(t.to_string())))
                .collect(),
            fg,
            hidden: Vec::new(),
            exes: exes.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn ids(v: Vec<WindowInfo>) -> Vec<WindowId> {
        v.into_iter().map(|w| w.id).collect()
    }

    #[test]
    fn own_process_family_beats_others() {
        let c = ctx(&[(1, "Old")], Some(1), &["calc.exe"]);
        let wins = [
            win(1, 5, "x.exe", "Old"),
            win(2, 100, "Other.exe", "Other"),
            win(3, 11, "CalculatorApp.exe", "Calc"),
        ];
        // Launcher pid 10 spawned 11: only its family counts, even while the launcher lives.
        assert_eq!(ids(select(&wins, &c, &[10, 11], true, Some(1), false)), [3]);
        assert_eq!(
            ids(select(&wins, &c, &[10, 11], false, Some(1), false)),
            [3]
        );
        // Alive and nothing of its own yet: keep waiting, never grab window 2.
        assert!(select(&wins[..2], &c, &[10], true, Some(1), false).is_empty());
    }

    #[test]
    fn handoff_accepts_only_the_handler_exe() {
        let c = ctx(&[(1, "Settings")], Some(1), &["notepad.exe"]);
        let wins = [
            win(1, 5, "Settings.exe", "Settings"),
            win(2, 6, "chrome.exe", "New tab"),
            win(3, 7, "Notepad.exe", "Untitled - Notepad"),
        ];
        assert_eq!(ids(select(&wins, &c, &[10], false, Some(1), false)), [3]);
        // Only an unrelated window appeared: nothing yet.
        assert!(select(&wins[..2], &c, &[10], false, Some(1), false).is_empty());
        // Existing Notepad window that got a new title (new document in the same window).
        let c = ctx(&[(3, "a.txt - Notepad")], Some(1), &["notepad.exe"]);
        assert_eq!(ids(select(&wins, &c, &[10], false, Some(1), false)), [3]);
    }

    #[test]
    fn handoff_with_unknown_handler_takes_new_windows() {
        let c = ctx(&[(1, "A")], Some(1), &[]);
        let wins = [win(1, 5, "a.exe", "A"), win(2, 6, "b.exe", "B")];
        assert_eq!(ids(select(&wins, &c, &[], false, Some(1), false)), [2]);
        assert!(select(&wins[..1], &c, &[], false, Some(1), true).is_empty());
    }

    #[test]
    fn reused_window_is_found_by_foreground() {
        let wins = [
            win(1, 5, "other.exe", "Editor"),
            win(3, 7, "Notepad.exe", "a.txt - Notepad"),
        ];
        let c = ctx(
            &[(1, "Editor"), (3, "a.txt - Notepad")],
            Some(1),
            &["notepad.exe"],
        );
        // Same title, but it came to the front after the launch.
        assert_eq!(ids(select(&wins, &c, &[10], false, Some(3), false)), [3]);
        // Foreground did not change: not yet, only after the launcher has been gone a while.
        let c = ctx(
            &[(1, "Editor"), (3, "a.txt - Notepad")],
            Some(3),
            &["notepad.exe"],
        );
        assert!(select(&wins, &c, &[10], false, Some(3), false).is_empty());
        assert_eq!(ids(select(&wins, &c, &[10], false, Some(3), true)), [3]);
        // A foreground window of another program never counts.
        assert!(select(&wins, &c, &[10], false, Some(1), true).is_empty());
    }

    #[test]
    fn reshown_hidden_window_is_the_launch_window() {
        // Relaunching a tray app re-shows its hidden window: that is the launch's window.
        let mut c = ctx(&[(3, "WhatsApp")], Some(1), &["whatsapp.exe"]);
        c.hidden = vec![3];
        let mut w = win(3, 7, "WhatsApp.exe", "WhatsApp");
        w.hidden = true;
        assert!(select(std::slice::from_ref(&w), &c, &[10], false, Some(1), true).is_empty());
        w.hidden = false;
        assert_eq!(ids(select(&[w], &c, &[10], false, Some(1), false)), [3]);
    }

    #[test]
    fn context_key_roundtrips() {
        let before = ctx(&[(1, "A")], Some(1), &["x.exe"]).register();
        let got = LaunchCtx::lookup(&before);
        assert_eq!(got.exes, ["x.exe"]);
        assert!(
            !got.fresh(&win(1, 1, "", "A"))
                && got.fresh(&win(1, 1, "", "B"))
                && got.fresh(&win(2, 1, "", "A"))
        );
        // Unknown key: ids only, nothing is "newly titled".
        let lost = LaunchCtx::lookup(&[1, KEY_BIT | 999_999]);
        assert!(!lost.fresh(&win(1, 1, "", "whatever")));
    }
}
