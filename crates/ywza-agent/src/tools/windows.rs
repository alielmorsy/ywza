use std::{path::Path, time::Duration};

use schemars::JsonSchema;
use serde::Deserialize;
use ywza_desktop as desk;

use super::{
    find::{frame, visibly_changed},
    output::{desk_err, flat},
    util::blocking,
    Ctx, Tool, ToolOutput, WindowArg,
};

/// List monitors: index, name, rect in physical pixels, scale, primary.
#[derive(Deserialize, JsonSchema)]
pub struct ListMonitors {}

impl Tool for ListMonitors {
    const NAME: &'static str = "list_monitors";

    async fn run(self, _: &Ctx) -> ToolOutput {
        ToolOutput::ok(super::monitors_text().await)
    }
}

/// List top-level app windows: [w<id>] "title" (process) mon <index> [minimized] [hidden] [maximized] [focused]. [hidden] = closed to the tray; focus_window shows it. [focused] = gets keys pressed without a window.
#[derive(Deserialize, JsonSchema)]
pub struct ListWindows {}

impl Tool for ListWindows {
    const NAME: &'static str = "list_windows";

    async fn run(self, _: &Ctx) -> ToolOutput {
        flat(async {
            let list = blocking(desk::list_windows).await.map_err(desk_err)?;
            let lines: Vec<String> = list.iter().map(|w| w.to_string()).collect();
            Ok(ToolOutput::ok(lines.join("\n")))
        })
        .await
    }
}

/// Bring a window to the front (restores it if minimized, shows it if hidden).
#[derive(Deserialize, JsonSchema)]
pub struct FocusWindow {
    /// Window id from list_windows/launch, e.g. "w3412106".
    window: WindowArg,
}

impl Tool for FocusWindow {
    const NAME: &'static str = "focus_window";

    async fn run(self, _: &Ctx) -> ToolOutput {
        flat(async {
            let w = self.window.0;
            blocking(move || desk::focus_window(w))
                .await
                .map_err(desk_err)?;
            Ok(ToolOutput::ok(format!("w{w} is in front")))
        })
        .await
    }
}

/// Move/resize a window by its visible bounds. x/y are relative to the target monitor's work area; omitted values keep the current ones.
#[derive(Deserialize, JsonSchema)]
pub struct MoveWindow {
    /// Window id from list_windows/launch, e.g. "w3412106".
    window: WindowArg,
    /// Target monitor index (list_monitors).
    monitor: Option<usize>,
    x: Option<i32>,
    y: Option<i32>,
    /// Width in physical pixels.
    w: Option<i32>,
    /// Height in physical pixels.
    h: Option<i32>,
}

impl Tool for MoveWindow {
    const NAME: &'static str = "move_window";

    async fn run(self, _: &Ctx) -> ToolOutput {
        flat(async {
            let w = self.window.0;
            let Self {
                monitor,
                x,
                y,
                w: ww,
                h: hh,
                ..
            } = self;
            let info = blocking(move || {
                desk::move_window(w, monitor, x, y, ww, hh)?;
                desk::list_windows().map(|l| l.into_iter().find(|i| i.id == w))
            })
            .await
            .map_err(desk_err)?;
            Ok(match info {
                Some(i) => ToolOutput::ok(format!(
                    "moved: {i} at {},{} {}x{}",
                    i.rect.x, i.rect.y, i.rect.w, i.rect.h
                )),
                None => ToolOutput::ok("moved"),
            })
        })
        .await
    }
}

/// Ask a window to close (like clicking X). If the app ignores the request (nothing changes, as with some frameless Qt windows), Alt+F4 is sent to the window. If it is still open afterwards (e.g. a save prompt), its fresh element list is returned so you can answer it.
#[derive(Deserialize, JsonSchema)]
pub struct CloseWindow {
    /// Window id from list_windows/launch, e.g. "w3412106".
    window: WindowArg,
}

impl Tool for CloseWindow {
    const NAME: &'static str = "close_window";
    const RISKY: bool = true;

    async fn run(self, cx: &Ctx) -> ToolOutput {
        flat(async {
            let w = self.window.0;
            let looks = || async move {
                let wins = blocking(desk::list_windows).await.unwrap_or_default();
                (frame(w).await, wins.len())
            };
            let before = looks().await;
            blocking(move || desk::close_window(w))
                .await
                .map_err(desk_err)?;
            tokio::time::sleep(Duration::from_millis(700)).await;
            let after = looks().await;
            // Still there, looking the same, no new window (prompt): the request was ignored.
            let ignored = match (&before.0, &after.0) {
                (Some(a), Some(b)) => before.1 == after.1 && !visibly_changed(a, b),
                _ => false,
            };
            if ignored {
                tracing::info!(w, "close request ignored; sending alt+f4");
                blocking(move || desk::press_keys(Some(w), "alt+f4"))
                    .await
                    .map_err(desk_err)?;
                tokio::time::sleep(Duration::from_millis(700)).await;
            }
            let prev = cx.snap(w);
            Ok(
                match blocking(move || desk::snapshot(w, prev.as_ref())).await {
                    Err(_) => {
                        cx.forget(w);
                        ToolOutput::ok(format!("w{w} closed"))
                    }
                    Ok(snap) => {
                        let text = format!(
                            "w{w} is still open; it may be asking something:\n{}",
                            snap.render()
                        );
                        cx.store(w, snap);
                        ToolOutput::ok(text).elements(w)
                    }
                },
            )
        })
        .await
    }
}

/// Start a program (path or name on PATH) and return its pid and new window ids.
#[derive(Deserialize, JsonSchema)]
pub struct Launch {
    /// Executable, e.g. "notepad.exe" or a full path.
    program: String,
    #[serde(default)]
    args: Vec<String>,
    cwd: Option<String>,
}

impl Tool for Launch {
    const NAME: &'static str = "launch";

    async fn run(self, _: &Ctx) -> ToolOutput {
        flat(async {
            let Self { program, args, cwd } = self;
            let res = blocking(move || {
                let refs: Vec<&str> = args.iter().map(String::as_str).collect();
                let l = desk::launch(&program, &refs, cwd.as_deref().map(Path::new))?;
                let wins = l.wait_for_window(Duration::from_secs(10));
                // A fresh window often has no content in its UI tree yet.
                if wins.is_ok() {
                    std::thread::sleep(Duration::from_millis(700));
                }
                Ok::<_, desk::Error>((l.pid, wins))
            })
            .await
            .map_err(desk_err)?;
            Ok(ToolOutput::ok(match res {
                (pid, Ok(wins)) => {
                    let lines: Vec<String> = wins.iter().map(|w| w.to_string()).collect();
                    format!("started pid {pid}; new windows:\n{}", lines.join("\n"))
                }
                (pid, Err(_)) => {
                    format!("started pid {pid}; no new window within 10 s (check list_windows)")
                }
            }))
        })
        .await
    }
}
