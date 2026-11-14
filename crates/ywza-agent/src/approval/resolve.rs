//! What a local call targets: the window and element it names, as one readable line
//! (`click [14] button "Send" in OUTLOOK.EXE: Inbox - Outlook`) and a boxed crop of the window.
//! Any failure leaves the call unresolved: approvals then show the plain summary.

use std::{fmt::Write, time::Duration};

use serde_json::Value;
use tokio::time::timeout;
use ywza_desktop::{self as desk, Image, Rect, WindowId, WindowInfo};
use ywza_proto::Target;

use super::summary::steps;
use crate::tools::{blocking, capture, downscale, parse_window, ElementArg, Space, Tools};

/// An approval must not wait long on the desktop.
const LIMIT: Duration = Duration::from_millis(1500);
/// Context kept around the element in the crop, in window pixels.
const PAD: i32 = 160;
const SHOT_MAX: u32 = 800;
const TOOLS: &[&str] = &[
    "click",
    "long_press",
    "type_text",
    "scroll",
    "press_keys",
    "focus_window",
    "close_window",
    "move_window",
];

#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub line: String,
    pub window: WindowId,
    /// The element in screen pixels, when the call names one.
    pub element: Option<Rect>,
}

/// One entry per call of `summary::calls` (the call itself, then each action of a batch);
/// None where the call is not resolvable.
/// Element ids are looked up in `space`, the calling session's own element list.
pub async fn resolve(
    tools: &Tools,
    space: Space,
    tool: &str,
    args: &Value,
) -> Vec<Option<Resolved>> {
    let mut list = vec![(tool.to_string(), args.clone())];
    list.extend(steps(tool, args).into_iter().map(|s| (s.tool, s.args)));
    let none = vec![None; list.len()];
    if !list.iter().any(|(t, _)| TOOLS.contains(&t.as_str())) {
        return none;
    }
    let work = async {
        let wins = blocking(desk::list_windows).await.ok()?;
        let mut out = Vec::with_capacity(list.len());
        for (t, a) in &list {
            out.push(one(tools, space, &wins, t, a).await);
        }
        Some(out)
    };
    timeout(LIMIT, work).await.ok().flatten().unwrap_or(none)
}

async fn one(
    tools: &Tools,
    space: Space,
    wins: &[WindowInfo],
    tool: &str,
    args: &Value,
) -> Option<Resolved> {
    if !TOOLS.contains(&tool) {
        return None;
    }
    let w = parse_window(&args["window"])?;
    let info = wins.iter().find(|i| i.id == w)?;
    let id = args
        .get("element")
        .and_then(|v| serde_json::from_value::<ElementArg>(v.clone()).ok())
        .map(|e| e.0);
    let label = args
        .get("text")
        .and_then(Value::as_str)
        .filter(|_| tool == "click" && id.is_none());
    let mut found = None;
    if id.is_some() || label.is_some() {
        let snap = tools.latest_snapshot(Some(space), w, info.rect).await.ok()?;
        let e = match (id, label) {
            (Some(id), _) => Some(snap.get(id)?),
            (None, Some(t)) => {
                let want = desk::ocr_norm(t);
                let nth = args.get("nth").and_then(Value::as_u64).unwrap_or(1).max(1);
                let mut named = snap.elements.iter().filter(|e| desk::ocr_norm(&e.name) == want);
                named.nth(nth as usize - 1)
            }
            _ => None,
        };
        found = e.map(|e| (e.id, e.kind, e.name.clone(), e.rect));
    }
    let el = found.as_ref().map(|(id, kind, name, _)| (*id, *kind, name.as_str()));
    Some(Resolved {
        line: line(tool, args, el, label, &info.process, &info.title),
        window: w,
        element: found.map(|f| f.3),
    })
}

fn line(
    tool: &str,
    args: &Value,
    el: Option<(u32, &str, &str)>,
    label: Option<&str>,
    process: &str,
    title: &str,
) -> String {
    let mut s = tool.to_string();
    match (el, label) {
        (Some((id, kind, name)), _) => {
            let _ = write!(s, " [{id}] {kind}");
            if !name.is_empty() {
                let _ = write!(s, " {name:?}");
            }
        }
        (None, Some(t)) => {
            let _ = write!(s, " text {t:?}");
        }
        _ => {}
    }
    let _ = write!(s, " in {process}: {title}");
    let extra = match tool {
        "type_text" => args.get("text").and_then(Value::as_str).map(|t| ("typed", t)),
        "press_keys" => args.get("keys").and_then(Value::as_str).map(|k| ("keys", k)),
        _ => None,
    };
    if let Some((what, v)) = extra {
        let _ = write!(s, " | {what}: {v}");
    }
    s.replace(['\r', '\n'], " ")
}

/// PNG of the window around the element, the element boxed. None without an element or when
/// the window cannot be captured in time.
pub async fn shot(r: &Resolved) -> Option<Vec<u8>> {
    let rect = r.element?;
    let work = async {
        let img = capture(&Target::Window { id: r.window }).await.ok()?;
        blocking(move || {
            let mut img = img;
            desk::draw_box(&mut img, rect);
            let (x, y) = (rect.x - img.origin.0 - PAD, rect.y - img.origin.1 - PAD);
            let region = Rect::new(x, y, rect.w + 2 * PAD, rect.h + 2 * PAD);
            desk::encode_png(&downscale(crop(&img, region)?, SHOT_MAX)).ok()
        })
        .await
    };
    timeout(LIMIT, work).await.ok().flatten()
}

/// `region` in image pixels, clipped to the image. None when nothing is left.
fn crop(img: &Image, region: Rect) -> Option<Image> {
    let r = region.intersect(&Rect::new(0, 0, img.width as i32, img.height as i32));
    if r.is_empty() {
        return None;
    }
    let mut rgba = Vec::with_capacity(r.w as usize * r.h as usize * 4);
    for y in r.y..r.y + r.h {
        let start = (y as usize * img.width as usize + r.x as usize) * 4;
        rgba.extend_from_slice(&img.rgba[start..start + r.w as usize * 4]);
    }
    Some(Image {
        width: r.w as u32,
        height: r.h as u32,
        rgba,
        origin: (img.origin.0 + r.x, img.origin.1 + r.y),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn lines_name_the_element_and_the_window() {
        let click = json!({"window":"w3","element":14});
        assert_eq!(
            line(
                "click",
                &click,
                Some((14, "button", "Send")),
                None,
                "OUTLOOK.EXE",
                "Inbox - Outlook"
            ),
            r#"click [14] button "Send" in OUTLOOK.EXE: Inbox - Outlook"#
        );
        let typed = json!({"window":"w3","element":9,"text":"hi\nthere"});
        assert_eq!(
            line("type_text", &typed, Some((9, "edit", "")), None, "a.exe", "t"),
            "type_text [9] edit in a.exe: t | typed: hi there"
        );
        assert_eq!(
            line("click", &json!({"text":"File"}), None, Some("File"), "a.exe", "t"),
            r#"click text "File" in a.exe: t"#
        );
        assert_eq!(
            line("press_keys", &json!({"keys":"ctrl+s"}), None, None, "a.exe", "t"),
            "press_keys in a.exe: t | keys: ctrl+s"
        );
        assert_eq!(
            line("close_window", &json!({"window":"w3"}), None, None, "a.exe", "t"),
            "close_window in a.exe: t"
        );
    }

    #[test]
    fn crops_are_clipped_to_the_image_and_keep_the_origin() {
        let img = Image {
            width: 10,
            height: 8,
            rgba: (0..10 * 8 * 4).map(|i| i as u8).collect(),
            origin: (100, 200),
        };
        let c = crop(&img, Rect::new(-5, 2, 8, 100)).unwrap();
        assert_eq!((c.width, c.height, c.origin), (3, 6, (100, 202)));
        assert_eq!(&c.rgba[..4], &img.rgba[2 * 10 * 4..][..4]);
        assert!(crop(&img, Rect::new(20, 0, 5, 5)).is_none());
    }

    #[tokio::test]
    async fn calls_that_target_nothing_stay_unresolved() {
        let t = Tools::default();
        let sp = Space {
            session: ywza_proto::SessionId::new(),
            sub: None,
        };
        let r = resolve(&t, sp, "run_command", &json!({"command":"ls"})).await;
        assert_eq!(r, vec![None]);
        let batch = json!({"window":"w1","actions":[{"tool":"wait","ms":1}]});
        assert_eq!(resolve(&t, sp, "batch", &batch).await, vec![None, None]);
    }
}
