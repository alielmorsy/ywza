use super::conn::X;
use crate::{Error, Rect, Result, WindowId};
use std::thread::sleep;
use std::time::Duration;
use x11rb::protocol::xproto::{ConfigureWindowAux, ConnectionExt as _, Window};

pub fn move_window(
    id: WindowId,
    monitor: Option<usize>,
    x: Option<i32>,
    y: Option<i32>,
    w: Option<i32>,
    h: Option<i32>,
) -> Result<()> {
    X::connect()?.move_window(id, monitor, x, y, w, h)
}

impl X {
    fn move_window(
        &self,
        id: WindowId,
        monitor: Option<usize>,
        x: Option<i32>,
        y: Option<i32>,
        w: Option<i32>,
        h: Option<i32>,
    ) -> Result<()> {
        let info = self.window_info(id)?;
        let win = id as Window;
        let state = self.state(win);
        let a = &self.a;
        if info.minimized {
            self.bring_to_front(win)?;
        }
        let mut changed = false;
        for s in [
            a._NET_WM_STATE_MAXIMIZED_VERT,
            a._NET_WM_STATE_MAXIMIZED_HORZ,
            a._NET_WM_STATE_FULLSCREEN,
        ] {
            if state.contains(&s) {
                self.remove_state(win, s)?;
                changed = true;
            }
        }
        if changed {
            sleep(Duration::from_millis(150));
        }
        let monitors = self.monitors()?;
        let cur = monitors
            .get(info.monitor)
            .ok_or_else(|| Error::NotFound("current monitor".into()))?;
        let target = match monitor {
            Some(i) => monitors
                .get(i)
                .ok_or_else(|| Error::NotFound(format!("monitor {i}")))?,
            None => cur,
        };
        let vis = self.visible_rect(win)?;
        let want = Rect::new(
            target.work.x + x.unwrap_or(vis.x - cur.work.x),
            target.work.y + y.unwrap_or(vis.y - cur.work.y),
            w.unwrap_or(vis.w).max(1),
            h.unwrap_or(vis.h).max(1),
        );
        // Configure the client window, then correct by the measured offset: window
        // managers disagree on whether the requested position is the frame's or the client's.
        let ext = self.frame_extents(win);
        let mut cfg = Rect::new(
            want.x + ext[0],
            want.y + ext[2],
            want.w - ext[0] - ext[1],
            want.h - ext[2] - ext[3],
        );
        for _ in 0..3 {
            let aux = ConfigureWindowAux::new()
                .x(cfg.x)
                .y(cfg.y)
                .width(cfg.w.max(1) as u32)
                .height(cfg.h.max(1) as u32);
            self.conn.configure_window(win, &aux)?;
            self.sync()?;
            sleep(Duration::from_millis(80));
            let now = self.visible_rect(win)?;
            if now == want {
                break;
            }
            cfg = Rect::new(
                cfg.x + want.x - now.x,
                cfg.y + want.y - now.y,
                cfg.w + want.w - now.w,
                cfg.h + want.h - now.h,
            );
        }
        Ok(())
    }
}
