use super::foreground::with_foreground;
use crate::{Button, Result, WindowId};
use std::thread::sleep;
use std::time::Duration;

pub(crate) fn real_click(
    window: WindowId,
    at: (i32, i32),
    button: Button,
    count: u32,
    hold_ms: u64,
) -> Result<()> {
    let b = match button {
        Button::Left => 1,
        Button::Middle => 2,
        Button::Right => 3,
    };
    with_foreground(Some(window), |x| {
        x.motion(at)?;
        x.sync()?;
        sleep(Duration::from_millis(30));
        for _ in 0..count.max(1) {
            x.button(b, true)?;
            x.sync()?;
            if hold_ms > 0 {
                sleep(Duration::from_millis(hold_ms));
            }
            x.button(b, false)?;
            x.sync()?;
            sleep(Duration::from_millis(30));
        }
        Ok(())
    })
}

/// Scrolls `dy` wheel notches down (negative = up) and `dx` right (negative = left).
pub(crate) fn wheel(window: WindowId, at: (i32, i32), dx: i32, dy: i32) -> Result<()> {
    with_foreground(Some(window), |x| {
        x.motion(at)?;
        x.sync()?;
        for (delta, neg, pos) in [(dy, 4u8, 5u8), (dx, 6, 7)] {
            for _ in 0..delta.unsigned_abs() {
                let b = if delta < 0 { neg } else { pos };
                x.button(b, true)?;
                x.button(b, false)?;
            }
        }
        Ok(())
    })
}
