//! `wait`: a plain pause, or a watch that returns as soon as an element changes.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use schemars::JsonSchema;
use serde::Deserialize;
use ywza_desktop::{self as desk, Snapshot, WindowId};

use super::{
    act::settle_diff, ocr::check_ocr, util::blocking, Ctx, ElementArg, Tool, ToolOutput, WindowArg,
};

const SLEEP_MAX: u64 = 60_000;
const WATCH_MAX: u64 = 600_000;
const POLL: Duration = Duration::from_millis(300);
/// OCR re-reads cost more and are noisy: slower polls, two misses in a row count as a change.
const OCR_POLL: Duration = Duration::from_secs(1);
const OCR_MISSES: u32 = 2;
/// Consecutive failed re-reads (other than the window being gone) before giving up.
const READ_FAILS: u32 = 5;

/// Pause for `ms` milliseconds (max 60000). With `window` and `element`: instead return as soon as that element changes (name, value, toggle state, selected, busy, enabled) or disappears, or its window closes, waiting up to `ms` (max 600000), and report the changes. Use it to wait out long tasks in one call, e.g. until a "Stop" button turns back into "Start". With `window` and `text`: return as soon as an element whose name or value contains `text` (any case) appears, with its line, or with `gone` once none is left (a spinner, "Loading"); no element id needed.
#[derive(Deserialize, JsonSchema)]
pub struct Wait {
    ms: u64,
    /// Window id from list_windows/launch, e.g. "w3412106".
    window: Option<WindowArg>,
    /// Element id from this window's latest get_elements.
    element: Option<ElementArg>,
    /// Wait for an element with this text in its name or value, e.g. "Application submitted".
    text: Option<String>,
    /// With `text`: wait until no element has it any more.
    #[serde(default)]
    gone: bool,
}

impl Tool for Wait {
    const NAME: &'static str = "wait";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        match (self.window, self.element, self.text) {
            (Some(w), Some(e), _) => watch(cx, w.0, e.0, self.ms.min(WATCH_MAX)).await,
            (Some(w), None, Some(t)) => {
                wait_text(cx, w.0, &t, self.gone, self.ms.min(WATCH_MAX)).await
            }
            (None, Some(_), _) | (None, _, Some(_)) => {
                ToolOutput::err("`element` and `text` need `window`")
            }
            _ => {
                let ms = self.ms.min(SLEEP_MAX);
                tokio::time::sleep(Duration::from_millis(ms)).await;
                ToolOutput::ok(format!("waited {ms} ms"))
            }
        }
    }
}

/// Polls `id` against the stored snapshot until its line differs or it is gone, then settles
/// and reports the window's changes since the call started.
// ponytail: re-walks the whole window each poll; watch the single UIA node if big windows get slow.
async fn watch(cx: &Ctx, w: WindowId, id: u32, ms: u64) -> ToolOutput {
    let Some(base) = cx.snap(w) else {
        return ToolOutput::err(format!("no element list for w{w}; call get_elements first"));
    };
    let Some(el) = base.get(id) else {
        return ToolOutput::err(format!("no element {id} in w{w}'s latest get_elements"));
    };
    let (line, ocr, rect, name) = (el.line(), el.kind == "ocr", el.rect, el.name.clone());
    let base = Arc::new(base);
    let start = Instant::now();
    let deadline = start + Duration::from_millis(ms);
    let (mut misses, mut fails) = (0, 0);
    loop {
        if Instant::now() >= deadline {
            return ToolOutput::ok(format!("no change to element {id} after {ms} ms"));
        }
        tokio::time::sleep(if ocr { OCR_POLL } else { POLL }).await;
        let changed = if ocr {
            misses = match check_ocr(w, rect, name.clone(), cx.ocr).await {
                Ok(()) => 0,
                Err(_) => misses + 1,
            };
            misses >= OCR_MISSES
        } else {
            let prev = base.clone();
            match blocking(move || desk::snapshot(w, Some(&prev))).await {
                Ok(s) => {
                    fails = 0;
                    s.get(id).map(|e| e.line()).as_ref() != Some(&line)
                }
                Err(desk::Error::NotFound(_)) => true,
                Err(e) => {
                    fails += 1;
                    if fails >= READ_FAILS {
                        return ToolOutput::err(format!("could not re-read w{w}: {e}"));
                    }
                    false
                }
            }
        };
        if changed {
            break;
        }
    }
    let secs = start.elapsed().as_secs_f32();
    let d = settle_diff(cx, w, &base).await.unwrap_or_default();
    ToolOutput::ok(format!(
        "element {id} changed after {secs:.1} s; changes:\n{}",
        d.trim_end()
    ))
}

/// Re-reads `w` until an element whose name or value contains `text` shows up (or, with
/// `gone`, until none is left). The last read is stored, so the returned ids are usable.
async fn wait_text(cx: &Ctx, w: WindowId, text: &str, gone: bool, ms: u64) -> ToolOutput {
    let want = text.to_lowercase();
    let hit = move |s: &Snapshot| {
        s.elements
            .iter()
            .find(|e| {
                e.name.to_lowercase().contains(&want)
                    || e.value
                        .as_ref()
                        .is_some_and(|v| v.to_lowercase().contains(&want))
            })
            .map(|e| e.line())
    };
    let start = Instant::now();
    let deadline = start + Duration::from_millis(ms);
    let mut prev = cx.snap(w);
    let mut fails = 0;
    loop {
        let p = prev.take();
        let (res, p) = blocking(move || (desk::snapshot(w, p.as_ref()), p)).await;
        prev = p;
        match res {
            Ok(s) => {
                fails = 0;
                let found = hit(&s);
                let secs = start.elapsed().as_secs_f32();
                let done = match (found, gone) {
                    (Some(line), false) => Some(format!("found after {secs:.1} s: {line}")),
                    (None, true) => Some(format!("{text:?} gone after {secs:.1} s")),
                    _ if Instant::now() >= deadline => {
                        let state = if gone { "still there" } else { "not found" };
                        Some(format!("{text:?} {state} after {ms} ms"))
                    }
                    _ => None,
                };
                if let Some(msg) = done {
                    cx.store(w, s);
                    return ToolOutput::ok(msg);
                }
                prev = Some(s);
            }
            Err(desk::Error::NotFound(_)) => {
                cx.forget(w);
                return ToolOutput::ok(format!("w{w} closed"));
            }
            Err(e) => {
                fails += 1;
                if fails >= READ_FAILS || Instant::now() >= deadline {
                    return ToolOutput::err(format!("could not re-read w{w}: {e}"));
                }
            }
        }
        tokio::time::sleep(POLL).await;
    }
}

#[cfg(all(test, windows))]
mod tests {
    use serde_json::{json, Value};
    use ywza_proto::OcrEngine;

    use crate::tools::Tools;

    /// A WinForms window: "Stop" turns into "Start" after 2 s, "Extra" is removed after
    /// 4 s, the window closes after 6 s.
    const FORM: &str = r#"
Add-Type -AssemblyName System.Windows.Forms
$f = New-Object Windows.Forms.Form; $f.Text = 'ywza-wait-test'
$b = New-Object Windows.Forms.Button; $b.Text = 'Stop'; $f.Controls.Add($b)
$x = New-Object Windows.Forms.Button; $x.Text = 'Extra'; $x.Top = 40; $f.Controls.Add($x)
$n = 0; $t = New-Object Windows.Forms.Timer; $t.Interval = 2000
$t.Add_Tick({ $script:n++; if ($n -eq 1) { $b.Text = 'Start' } elseif ($n -eq 2) { $f.Controls.Remove($x) } else { $f.Close() } })
$f.Add_Shown({ $t.Start() }); [Windows.Forms.Application]::Run($f)
"#;

    fn id_of(list: &str, name: &str) -> Value {
        let line = list.lines().find(|l| l.contains(name)).expect(name);
        let id = &line[1..line.find(']').unwrap()];
        json!(id.parse::<u32>().unwrap())
    }

    #[tokio::test]
    #[ignore = "opens a real window"]
    async fn watch_reports_change_removal_and_close() {
        let mut child = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", FORM])
            .spawn()
            .unwrap();
        let t = Tools::default();
        let call = |n: &'static str, a: Value| {
            let t = &t;
            async move { t.call(n, &a, OcrEngine::Auto).await }
        };
        let mut w = None;
        for _ in 0..50 {
            let out = call("list_windows", json!({})).await;
            w = out
                .text
                .lines()
                .find(|l| l.contains("ywza-wait-test"))
                .map(|l| l[1..l.find(']').unwrap()].to_string());
            if w.is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        let w = json!(w.expect("test window"));
        let list = call("get_elements", json!({"window": w})).await.text;
        let (stop, extra) = (id_of(&list, "\"Stop\""), id_of(&list, "\"Extra\""));

        let out = call("wait", json!({"window": w, "element": extra, "ms": 500})).await;
        assert!(out.ok && out.text.starts_with("no change"), "{}", out.text);

        let out = call("wait", json!({"window": w, "element": stop, "ms": 10_000})).await;
        assert!(out.ok && out.text.contains("\"Start\""), "{}", out.text);

        let out = call("wait", json!({"window": w, "element": extra, "ms": 10_000})).await;
        assert!(
            out.ok && out.text.contains(&format!("-[{extra}]")),
            "{}",
            out.text
        );

        let out = call("wait", json!({"window": w, "element": stop, "ms": 10_000})).await;
        assert!(out.ok && out.text.contains("closed"), "{}", out.text);

        let out = call("wait", json!({"window": w, "element": stop, "ms": 10})).await;
        assert!(!out.ok, "{}", out.text);
        let _ = child.wait();
    }
}
