use schemars::JsonSchema;
use serde::Deserialize;
use ywza_desktop::{self as desk, Image, Snapshot, WindowId};
use ywza_proto::Target;

use super::{
    output::{desk_err, downscale, flat, MODEL_IMAGE_MAX},
    util::blocking,
    Ctx, Tool, ToolOutput, WindowArg,
};

/// Full-resolution capture of a monitor or window.
pub async fn capture(target: &Target) -> desk::Result<Image> {
    let t = *target;
    blocking(move || match t {
        Target::Monitor { id } => desk::capture_monitor(id as usize),
        Target::Window { id } => desk::capture_window(id),
    })
    .await
}

/// `[0] DISPLAY1 2560x1440 at 0,0 scale 1.5 primary`, one per line.
pub async fn monitors_text() -> String {
    match blocking(desk::list_monitors).await {
        Ok(list) => list
            .iter()
            .map(|m| {
                let r = m.rect;
                format!(
                    "[{}] {} {}x{} at {},{} scale {}{}",
                    m.index,
                    m.name,
                    r.w,
                    r.h,
                    r.x,
                    r.y,
                    m.scale,
                    if m.primary { " primary" } else { "" }
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Err(e) => format!("(monitors unavailable: {e})"),
    }
}

/// The window a user sees on top of `monitor`: `list_windows` is in z-order (top first).
fn topmost_on(wins: &[desk::WindowInfo], monitor: usize) -> Option<WindowId> {
    wins.iter()
        .find(|w| w.monitor == monitor && !w.minimized && !w.hidden)
        .map(|w| w.id)
}

/// PNG of a window or monitor. Costly: use only when get_elements cannot answer (visual layout, images, canvas). With `marks`, element boxes are drawn with their ids.
#[derive(Deserialize, JsonSchema)]
pub struct Screenshot {
    /// Window id from list_windows/launch, e.g. "w3412106".
    window: Option<WindowArg>,
    /// Monitor index, if no window is given.
    monitor: Option<u32>,
    #[serde(default)]
    marks: bool,
}

impl Tool for Screenshot {
    const NAME: &'static str = "screenshot";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        flat(async {
            let target = match self.window {
                Some(w) => Target::Window { id: w.0 },
                None => Target::Monitor {
                    id: self.monitor.unwrap_or(0),
                },
            };
            let mut img = capture(&target).await.map_err(desk_err)?;
            let mut marked = 0;
            if self.marks {
                for s in &marks_snapshots(cx, &target).await? {
                    desk::draw_marks(&mut img, s);
                    marked += s.elements.len();
                }
            }
            let (full_w, full_h) = (img.width, img.height);
            let small = downscale(img, MODEL_IMAGE_MAX);
            let png = desk::encode_png(&small).map_err(desk_err)?;
            let what = match target {
                Target::Window { id } => format!("w{id}"),
                Target::Monitor { id } => format!("monitor {id}"),
            };
            let mut text = format!("screenshot of {what}: {full_w}x{full_h}");
            if small.width != full_w {
                text += &format!(
                    ", scaled to {}x{} (1 image px = {:.2} screen px)",
                    small.width,
                    small.height,
                    full_w as f32 / small.width as f32
                );
            }
            if self.marks {
                text += &format!(", {marked} element boxes marked with their ids");
            }
            Ok(ToolOutput {
                text,
                ok: true,
                image_png: Some(png),
                supersedes_key: None,
            })
        })
        .await
    }
}

/// The element lists to draw on a capture of `target`.
async fn marks_snapshots(cx: &Ctx, target: &Target) -> Result<Vec<Snapshot>, ToolOutput> {
    match *target {
        Target::Window { id } => {
            let snap = match cx.snap(id) {
                Some(s) => s,
                None => {
                    let s = blocking(move || desk::snapshot(id, None))
                        .await
                        .map_err(desk_err)?;
                    cx.store(id, s.clone());
                    s
                }
            };
            Ok(vec![snap])
        }
        // Only the topmost window on the monitor is known to be uncovered.
        Target::Monitor { id } => {
            let wins = blocking(desk::list_windows).await.map_err(desk_err)?;
            Ok(topmost_on(&wins, id as usize)
                .and_then(|w| cx.snap(w))
                .into_iter()
                .collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use ywza_desktop::Rect;

    use super::*;

    #[test]
    fn marks_go_to_topmost_window_of_the_monitor() {
        let win = |id, monitor, minimized| desk::WindowInfo {
            id,
            title: String::new(),
            process: String::new(),
            pid: 1,
            rect: Rect::default(),
            monitor,
            minimized,
            hidden: false,
        };
        let wins = [
            win(1, 0, true),
            win(2, 1, false),
            win(3, 0, false),
            win(4, 0, false),
        ];
        assert_eq!(topmost_on(&wins, 0), Some(3));
        assert_eq!(topmost_on(&wins, 1), Some(2));
        assert_eq!(topmost_on(&wins, 2), None);
    }
}
