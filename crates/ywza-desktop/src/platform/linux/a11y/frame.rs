use super::bus::{proxy, proxy_of, stage, step};
use super::node::raw_of;
use crate::{Rect, Result, WindowInfo};
use atspi::proxy::accessible::AccessibleProxy;
use atspi::zbus::{self, names::BusName};

/// Frames count when the title is equal or the bounds are within this many pixels
/// (the X rect includes window-manager decorations, AT-SPI's may not).
const MAX_DISTANCE: i64 = 200;

/// How well a frame matches the X window: exact title first, then closest bounds.
fn match_rank(title: &str, rect: &Rect, win: &WindowInfo) -> (u8, i64) {
    let d = |a: i32, b: i32| (a - b).abs() as i64;
    let dist = d(rect.x, win.rect.x)
        + d(rect.y, win.rect.y)
        + d(rect.w, win.rect.w)
        + d(rect.h, win.rect.h);
    (u8::from(title != win.title), dist)
}

fn acceptable(rank: (u8, i64)) -> bool {
    rank.0 == 0 || rank.1 <= MAX_DISTANCE
}

pub(super) struct Found {
    pub(super) frame: AccessibleProxy<'static>,
    pub(super) app: AccessibleProxy<'static>,
}

/// The AT-SPI application of the window's process and its frame matching the window.
pub(super) async fn find_frame(conn: &zbus::Connection, win: &WindowInfo) -> Result<Option<Found>> {
    let registry = stage(
        "AT-SPI registry",
        proxy(
            conn,
            "org.a11y.atspi.Registry",
            "/org/a11y/atspi/accessible/root",
        ),
    )
    .await?;
    let dbus = stage("AT-SPI bus daemon", zbus::fdo::DBusProxy::new(conn)).await?;
    let mut best: Option<((u8, i64), Found)> = None;
    for app_ref in stage("AT-SPI application list", registry.get_children()).await? {
        let Some(bus) = app_ref.name_as_str() else {
            continue;
        };
        let Ok(name) = BusName::try_from(bus) else {
            continue;
        };
        if step(dbus.get_connection_unix_process_id(name)).await != Some(win.pid) {
            continue;
        }
        let Some(app) = step(proxy_of(conn, &app_ref)).await else {
            continue;
        };
        for child in step(app.get_children()).await.unwrap_or_default() {
            let Some(frame) = step(proxy_of(conn, &child)).await else {
                continue;
            };
            let Some((raw, showing)) = step(raw_of(&frame)).await else {
                continue;
            };
            if !showing {
                continue;
            }
            let rank = match_rank(&raw.name, &raw.rect, win);
            if acceptable(rank) && best.as_ref().is_none_or(|(b, _)| rank < *b) {
                best = Some((
                    rank,
                    Found {
                        frame,
                        app: app.clone(),
                    },
                ));
            }
        }
    }
    Ok(best.map(|(_, f)| f))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win() -> WindowInfo {
        WindowInfo {
            id: 1,
            title: "Doc".into(),
            process: "app".into(),
            pid: 7,
            rect: Rect::new(100, 100, 800, 600),
            monitor: 0,
            minimized: false,
            hidden: false,
            focused: false,
            maximized: false,
        }
    }

    #[test]
    fn frame_matching() {
        let w = win();
        // Exact title beats a closer rect; a close rect still matches a changed title.
        assert!(match_rank("Doc", &Rect::new(0, 0, 10, 10), &w) < match_rank("Other", &w.rect, &w));
        assert!(acceptable(match_rank("Doc", &Rect::new(0, 0, 10, 10), &w)));
        assert!(acceptable(match_rank(
            "Doc*",
            &Rect::new(100, 130, 800, 570),
            &w
        )));
        assert!(!acceptable(match_rank(
            "Other",
            &Rect::new(900, 100, 800, 600),
            &w
        )));
    }
}
