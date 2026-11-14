//! Ranked element search over a window's element list, so a model can ask for "the Send
//! button near the compose box" instead of reading the whole list.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use ywza_desktop::{self as desk, Element, Rect, Snapshot};

use super::{
    args::parse_element,
    find::label_norm,
    ocr::snapshot_with_ocr,
    output::{desk_err, flat},
    Ctx, Tool, ToolOutput, WindowArg,
};

const DEFAULT_LIMIT: usize = 10;
const MAX_LIMIT: usize = 100;

/// Find elements of a window by role, text, position or distance to another element, best match first, in the format of get_elements (same ids, usable with click and type_text). Use it to locate one control ("Send button near the compose box") without listing the whole window; it uses the window's latest element list, or takes one if there is none.
#[derive(Deserialize, JsonSchema)]
pub struct FindElements {
    /// Window id from list_windows/launch, e.g. "w3412106".
    window: WindowArg,
    /// Element kind as get_elements prints it: button, edit, checkbox, text, ocr, ...
    #[serde(default)]
    role: Option<String>,
    /// Text of the element: its name, value, automation id or label. Ignores case, spaces and mnemonics; tolerates OCR noise in `ocr` lines.
    #[serde(default)]
    text: Option<String>,
    /// Rank by distance to this element: an element id, or text of an element (its best match).
    #[serde(default, deserialize_with = "string_or_number")]
    near: Option<String>,
    /// Keep only elements in this part of the window: top, bottom, left, right or center.
    #[serde(default)]
    region: Option<String>,
    /// Most matches to return (default 10).
    #[serde(default)]
    limit: Option<usize>,
}

fn string_or_number<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::String(s)) => Some(s),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Region {
    Top,
    Bottom,
    Left,
    Right,
    Center,
}

impl Region {
    fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_lowercase().as_str() {
            "top" => Self::Top,
            "bottom" => Self::Bottom,
            "left" => Self::Left,
            "right" => Self::Right,
            "center" | "centre" | "middle" => Self::Center,
            _ => return None,
        })
    }

    /// Whether the center of `r` lies in this part of `win`: a half, or the middle third.
    fn holds(self, win: &Rect, r: &Rect) -> bool {
        let (cx, cy) = r.center();
        let (x, y) = (cx - win.x, cy - win.y);
        let third = |p: i32, len: i32| p * 3 >= len && p * 3 < len * 2;
        match self {
            Self::Top => y * 2 < win.h,
            Self::Bottom => y * 2 >= win.h,
            Self::Left => x * 2 < win.w,
            Self::Right => x * 2 >= win.w,
            Self::Center => third(x, win.w) && third(y, win.h),
        }
    }
}

/// 0 = same text, 1 = starts with it, 2 = contains it, 3 = OCR reading close to it.
fn text_score(e: &Element, want: &str) -> Option<u8> {
    let want = label_norm(want);
    if want.is_empty() {
        return None;
    }
    [
        e.name.as_str(),
        e.value.as_deref().unwrap_or_default(),
        e.automation_id.as_str(),
    ]
    .into_iter()
    .filter(|f| !f.is_empty())
    .filter_map(|f| {
        let got = label_norm(f);
        if got == want {
            Some(0)
        } else if got.starts_with(&want) {
            Some(1)
        } else if got.contains(&want) {
            Some(2)
        } else if e.kind == "ocr" && desk::ocr_matches(&got, &want) {
            Some(3)
        } else {
            None
        }
    })
    .min()
}

/// Squared gap between two rects (0 when they touch or overlap), then the squared distance
/// of their centers as the tie-break.
fn distance(a: &Rect, b: &Rect) -> (i64, i64) {
    let gap = |a0: i32, al: i32, b0: i32, bl: i32| {
        (a0 - (b0 + bl)).max(b0 - (a0 + al)).max(0) as i64
    };
    let (dx, dy) = (gap(a.x, a.w, b.x, b.w), gap(a.y, a.h, b.y, b.h));
    let ((ax, ay), (bx, by)) = (a.center(), b.center());
    (
        dx * dx + dy * dy,
        ((ax - bx) as i64).pow(2) + ((ay - by) as i64).pow(2),
    )
}

/// An element id, else the best text match.
fn anchor<'a>(snap: &'a Snapshot, near: &str) -> Option<&'a Element> {
    if let Some(e) = parse_element(&Value::String(near.into())).and_then(|id| snap.get(id)) {
        return Some(e);
    }
    snap.elements
        .iter()
        .filter_map(|e| Some((text_score(e, near)?, e.rect.y, e.rect.x, e)))
        .min_by_key(|h| (h.0, h.1, h.2))
        .map(|h| h.3)
}

#[derive(Default)]
struct Query<'a> {
    role: Option<&'a str>,
    text: Option<&'a str>,
    near: Option<&'a str>,
    region: Option<Region>,
}

/// Matches best first: by text quality, then distance to `near`, then reading order.
fn search<'a>(snap: &'a Snapshot, q: &Query) -> Result<Vec<&'a Element>, String> {
    let anchor = match q.near {
        Some(n) => Some(anchor(snap, n).ok_or_else(|| format!("no element matches near={n:?}"))?),
        None => None,
    };
    let role = q.role.map(|r| r.trim().to_lowercase());
    let mut hits: Vec<_> = snap
        .elements
        .iter()
        .filter(|e| anchor.is_none_or(|a| a.id != e.id))
        .filter(|e| role.as_deref().is_none_or(|r| e.kind == r))
        .filter(|e| q.region.is_none_or(|g| g.holds(&snap.window.rect, &e.rect)))
        .filter_map(|e| {
            let score = match q.text {
                Some(t) => text_score(e, t)?,
                None => 0,
            };
            let dist = anchor.map_or((0, 0), |a| distance(&a.rect, &e.rect));
            Some(((score, dist, e.rect.y, e.rect.x), e))
        })
        .collect();
    hits.sort_by_key(|h| h.0);
    Ok(hits.into_iter().map(|h| h.1).collect())
}

fn render(snap: &Snapshot, hits: &[&Element], q: &Query, limit: usize) -> String {
    let w = snap.window.id;
    if hits.is_empty() {
        let mut kinds: Vec<&str> = snap.elements.iter().map(|e| e.kind).collect();
        kinds.sort_unstable();
        kinds.dedup();
        let hint = match q.role {
            Some(r) if !kinds.contains(&r.trim().to_lowercase().as_str()) => {
                format!(
                    "w{w} has no {r:?} elements; kinds present: {}",
                    kinds.join(", ")
                )
            }
            _ => format!("loosen text, role or region, or use get_elements to read w{w}"),
        };
        return format!("0 matches in w{w}; {hint}");
    }
    let shown = hits.len().min(limit);
    let mut out = if shown < hits.len() {
        format!("{} matches in w{w}, best {shown}:\n", hits.len())
    } else {
        format!("{} matches in w{w}:\n", hits.len())
    };
    for e in &hits[..shown] {
        out += &e.line();
        out.push('\n');
    }
    out
}

impl Tool for FindElements {
    const NAME: &'static str = "find_elements";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        flat(async {
            let w = self.window.0;
            let region = match self.region.as_deref() {
                Some(r) => Some(Region::parse(r).ok_or_else(|| {
                    ToolOutput::err(format!(
                        "bad `region` {r:?}: use top, bottom, left, right or center"
                    ))
                })?),
                None => None,
            };
            let q = Query {
                role: self.role.as_deref(),
                text: self.text.as_deref(),
                near: self.near.as_deref(),
                region,
            };
            if q.role.is_none() && q.text.is_none() && q.near.is_none() && region.is_none() {
                return Err(ToolOutput::err(
                    "give at least one of role, text, near, region (get_elements lists everything)",
                ));
            }
            let snap = match cx.snap(w) {
                Some(s) => s,
                None => {
                    let (s, _) = snapshot_with_ocr(w, None, false, cx.ocr)
                        .await
                        .map_err(desk_err)?;
                    cx.store(w, s.clone());
                    s
                }
            };
            let hits = search(&snap, &q).map_err(ToolOutput::err)?;
            let limit = self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
            Ok(ToolOutput::ok(render(&snap, &hits, &q, limit)))
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ywza_desktop::WindowInfo;

    /// Elements (kind, name, x, y), 80x20 each, ids 1.. in order, in a 1000x800 window.
    fn snap(items: &[(&'static str, &str, i32, i32)]) -> Snapshot {
        let mut s = Snapshot {
            window: WindowInfo {
                id: 3,
                title: "Mail".into(),
                process: "mail.exe".into(),
                pid: 1,
                rect: Rect::new(0, 0, 1000, 800),
                monitor: 1,
                minimized: false,
                hidden: false,
                focused: false,
                maximized: false,
            },
            seq: 1,
            elements: Vec::new(),
        };
        let lines = items
            .iter()
            .map(|i| (i.1.to_string(), Rect::new(i.2, i.3, 80, 20)))
            .collect();
        desk::merge_ocr(&mut s, lines);
        for (e, i) in s.elements.iter_mut().zip(items) {
            e.kind = i.0;
        }
        s
    }

    fn names<'a>(hits: &[&'a Element]) -> Vec<&'a str> {
        hits.iter().map(|e| e.name.as_str()).collect()
    }

    fn ids(hits: &[&Element]) -> Vec<u32> {
        hits.iter().map(|e| e.id).collect()
    }

    #[test]
    fn text_matches_rank_exact_then_prefix_then_contains() {
        let s = snap(&[
            ("button", "Resend", 0, 0),
            ("button", "Send later", 0, 100),
            ("button", "_Send", 0, 200),
            ("button", "Cancel", 0, 300),
        ]);
        let q = Query {
            text: Some("send"),
            ..Query::default()
        };
        assert_eq!(
            names(&search(&s, &q).unwrap()),
            ["_Send", "Send later", "Resend"]
        );
    }

    #[test]
    fn near_breaks_ties_by_distance_and_skips_the_anchor() {
        let s = snap(&[
            ("button", "Send", 0, 700),
            ("edit", "Compose", 0, 100),
            ("button", "Send", 0, 130),
        ]);
        let by_text = Query {
            text: Some("send"),
            near: Some("compose"),
            ..Query::default()
        };
        assert_eq!(ids(&search(&s, &by_text).unwrap()), [3, 1]);
        let by_id = Query {
            near: Some("2"),
            ..Query::default()
        };
        assert_eq!(ids(&search(&s, &by_id).unwrap()), [3, 1]);
        let missing = Query {
            near: Some("nowhere"),
            ..Query::default()
        };
        assert!(search(&s, &missing).is_err());
    }

    #[test]
    fn role_and_region_filter() {
        let s = snap(&[
            ("button", "Top left", 10, 10),
            ("edit", "Body", 460, 380),
            ("button", "Bottom", 460, 770),
        ]);
        let in_region = |role, region| {
            let q = Query {
                role,
                region: Some(region),
                ..Query::default()
            };
            names(&search(&s, &q).unwrap()).join(",")
        };
        assert_eq!(in_region(Some("Button"), Region::Bottom), "Bottom");
        assert_eq!(in_region(None, Region::Center), "Body");
        assert_eq!(in_region(Some("button"), Region::Left), "Top left");
        assert_eq!(Region::parse(" Top "), Some(Region::Top));
        assert_eq!(Region::parse("diagonal"), None);
    }

    #[test]
    fn ocr_lines_match_through_noise_and_empty_results_hint() {
        let s = snap(&[
            ("ocr", "Import ADT Design Pont", 0, 0),
            ("button", "Save", 0, 50),
        ]);
        let q = Query {
            text: Some("Import ADT Design Point"),
            ..Query::default()
        };
        let hits = search(&s, &q).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(render(&s, &hits, &q, 10).starts_with("1 matches in w3:\n[1] ocr"));
        let none = Query {
            role: Some("slider"),
            ..Query::default()
        };
        let text = render(&s, &[], &none, 10);
        assert!(
            text.contains("no \"slider\"") && text.contains("button, ocr"),
            "{text}"
        );
    }
}
