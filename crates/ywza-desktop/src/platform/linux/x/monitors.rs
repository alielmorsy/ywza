use super::conn::X;
use crate::{Monitor, Rect, Result};
use x11rb::protocol::randr;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};

pub fn set_dpi_awareness() -> Result<()> {
    Ok(()) // X11 reports physical pixels already
}

pub fn list_monitors() -> Result<Vec<Monitor>> {
    X::connect()?.monitors()
}

/// Index of the monitor the window overlaps most (nearest centre if none), 0 if no monitors.
pub(super) fn monitor_of(window: &Rect, monitors: &[Rect]) -> usize {
    let (cx, cy) = window.center();
    monitors
        .iter()
        .enumerate()
        .max_by_key(|(_, m)| {
            let (mx, my) = m.center();
            (
                window.intersect(m).area(),
                -((mx - cx) as i64).pow(2) - ((my - cy) as i64).pow(2),
            )
        })
        .map_or(0, |(i, _)| i)
}

/// `Xft.dpi` from an X resource string.
fn xft_dpi(resources: &str) -> Option<f32> {
    resources
        .lines()
        .find_map(|l| l.strip_prefix("Xft.dpi:"))
        .and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|d| *d > 0.0)
}

impl X {
    pub fn monitors(&self) -> Result<Vec<Monitor>> {
        let workarea = {
            let desk = self.prop32(self.root, self.a._NET_CURRENT_DESKTOP, AtomEnum::CARDINAL);
            let wa = self.prop32(self.root, self.a._NET_WORKAREA, AtomEnum::CARDINAL);
            let i = desk.first().copied().unwrap_or(0) as usize * 4;
            wa.get(i..i + 4)
                .map(|v| Rect::new(v[0] as i32, v[1] as i32, v[2] as i32, v[3] as i32))
        };
        let scale = self
            .text_prop(
                self.root,
                AtomEnum::RESOURCE_MANAGER.into(),
                AtomEnum::STRING,
            )
            .and_then(|r| xft_dpi(&r))
            .map_or(1.0, |d| d / 96.0);
        let rects: Vec<(String, Rect, bool)> = match self.randr_monitors() {
            Some(m) if !m.is_empty() => m,
            _ => {
                let g = self.conn.get_geometry(self.root)?.reply()?;
                vec![(
                    "screen".into(),
                    Rect::new(0, 0, g.width as i32, g.height as i32),
                    true,
                )]
            }
        };
        let any_primary = rects.iter().any(|m| m.2);
        Ok(rects
            .into_iter()
            .enumerate()
            .map(|(index, (name, rect, primary))| {
                let work = workarea
                    .map(|w| rect.intersect(&w))
                    .filter(|w| !w.is_empty())
                    .unwrap_or(rect);
                Monitor {
                    index,
                    name,
                    rect,
                    work,
                    scale,
                    primary: primary || (!any_primary && index == 0),
                }
            })
            .collect())
    }

    /// RandR 1.5 monitors (the server merges outputs into logical monitors for us).
    fn randr_monitors(&self) -> Option<Vec<(String, Rect, bool)>> {
        let v = randr::query_version(&self.conn, 1, 5).ok()?.reply().ok()?;
        if (v.major_version, v.minor_version) < (1, 5) {
            return None;
        }
        let list = randr::get_monitors(&self.conn, self.root, true)
            .ok()?
            .reply()
            .ok()?;
        Some(
            list.monitors
                .iter()
                .map(|m| {
                    let name = self
                        .conn
                        .get_atom_name(m.name)
                        .ok()
                        .and_then(|c| c.reply().ok())
                        .map(|r| String::from_utf8_lossy(&r.name).into_owned())
                        .unwrap_or_default();
                    (
                        name,
                        Rect::new(m.x as i32, m.y as i32, m.width as i32, m.height as i32),
                        m.primary,
                    )
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_choice() {
        let mons = [Rect::new(0, 0, 1920, 1080), Rect::new(1920, 0, 1920, 1080)];
        assert_eq!(monitor_of(&Rect::new(100, 100, 400, 300), &mons), 0);
        assert_eq!(monitor_of(&Rect::new(1800, 0, 600, 300), &mons), 1); // 120 px vs 480 px
        assert_eq!(monitor_of(&Rect::new(5000, 0, 100, 100), &mons), 1); // nearest
        assert_eq!(monitor_of(&Rect::new(0, 0, 1, 1), &[]), 0);
    }

    #[test]
    fn dpi_from_resources() {
        assert_eq!(xft_dpi("Xft.antialias:\t1\nXft.dpi:\t144\n"), Some(144.0));
        assert_eq!(xft_dpi("Xft.antialias: 1\n"), None);
    }
}
