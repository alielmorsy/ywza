//! Live input from a human viewer: XTest events on one cached connection, sent as they
//! arrive (no idle wait, no focus changes).

use super::keysym::{self, Keymap};
use super::x::X;
use crate::{parse_keys, Button, Error, Result};
use std::sync::Mutex;
use std::thread::sleep;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::protocol::xtest;

struct State {
    x: Option<X>,
    /// Spare keycodes bound to a keysym while held: keysym -> keycode.
    remapped: Vec<(u32, u8)>,
    wheel: (f32, f32),
}

static STATE: Mutex<State> = Mutex::new(State {
    x: None,
    remapped: Vec::new(),
    wheel: (0.0, 0.0),
});

/// Runs `f` on the cached connection, reconnecting once if it fails with an OS error.
fn with_x<T>(mut f: impl FnMut(&X, &mut State) -> Result<T>) -> Result<T> {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    for attempt in 0..2 {
        if s.x.is_none() {
            let x = X::connect()?;
            xtest::get_version(&x.conn, 2, 2)?
                .reply()
                .map_err(|e| Error::Os(format!("XTEST extension unavailable: {e}")))?;
            s.x = Some(x);
        }
        let x = s.x.take().unwrap();
        let r = f(&x, &mut *s).and_then(|v| {
            x.conn.flush()?;
            Ok(v)
        });
        match r {
            Err(Error::Os(_)) if attempt == 0 => continue,
            r => {
                s.x = Some(x);
                return r;
            }
        }
    }
    unreachable!()
}

/// Keycode with `sym` in the first group, unshifted level first. Later columns belong to
/// other layout groups, which may not be active.
fn code_for(map: &Keymap, sym: u32) -> Option<u8> {
    let per = map.per.max(1);
    (0..per.min(2)).find_map(|col| {
        map.syms
            .chunks(per)
            .position(|r| r[col] == sym)
            .map(|i| map.min_keycode.wrapping_add(i as u8))
    })
}

/// Whole notches in `*acc + v`; the fraction stays in `acc`.
fn notches(acc: &mut f32, v: f32) -> i32 {
    *acc += v;
    let n = acc.trunc();
    *acc -= n;
    n as i32
}

/// Moves the cursor to root-window pixel (x, y).
pub fn live_mouse_move(x: i32, y: i32) -> Result<()> {
    with_x(|xc, _| xc.motion((x, y)))
}

/// Presses or releases a mouse button at the current cursor position.
pub fn live_mouse_button(button: Button, down: bool) -> Result<()> {
    let b = match button {
        Button::Left => 1,
        Button::Middle => 2,
        Button::Right => 3,
    };
    with_x(|x, _| x.button(b, down))
}

/// Wheel in notches: `dy` > 0 scrolls down, `dx` > 0 scrolls right. Fractions add up
/// across calls; each whole notch is one button 4-7 click.
pub fn live_wheel(dx: f32, dy: f32) -> Result<()> {
    with_x(|x, s| {
        let ny = notches(&mut s.wheel.1, dy);
        let nx = notches(&mut s.wheel.0, dx);
        for (n, neg, pos) in [(ny, 4u8, 5u8), (nx, 6, 7)] {
            let b = if n < 0 { neg } else { pos };
            for _ in 0..n.unsigned_abs() {
                x.button(b, true)?;
                x.button(b, false)?;
            }
        }
        Ok(())
    })
}

/// Presses or releases one key, named as in `press_keys` ("a", "enter", "ctrl").
/// Characters map to their key on the current layout without adding shift: the human's
/// own shift key arrives as a separate event.
pub fn live_key(name: &str, down: bool) -> Result<()> {
    let combos = parse_keys(name)?;
    let [combo] = combos.as_slice() else {
        return Err(Error::NotFound(format!("key {name:?}")));
    };
    let [k] = combo.as_slice() else {
        return Err(Error::NotFound(format!("key {name:?}")));
    };
    let sym = keysym::key_keysym(*k);
    with_x(|x, s| {
        if let Some(i) = s.remapped.iter().position(|&(k, _)| k == sym) {
            let code = s.remapped[i].1;
            if down {
                return x.key(code, true);
            }
            x.key(code, false)?;
            s.remapped.swap_remove(i);
            let map = x.keymap()?;
            let none = vec![0u32; map.per.max(1)];
            x.conn
                .change_keyboard_mapping(1, code, none.len() as u8, &none)?;
            return Ok(());
        }
        let map = x.keymap()?;
        if let Some(code) = code_for(&map, sym) {
            return x.key(code, down);
        }
        if !down {
            return Ok(());
        }
        let spare = map
            .spare()
            .ok_or_else(|| Error::NotFound(format!("no key for {name:?} on this layout")))?;
        let mut syms = vec![0u32; map.per.max(1)];
        syms[0] = sym;
        x.conn
            .change_keyboard_mapping(1, spare, syms.len() as u8, &syms)?;
        x.conn.flush()?;
        // Give clients time to apply MappingNotify before the press arrives.
        sleep(Duration::from_millis(30));
        s.remapped.push((sym, spare));
        x.key(spare, true)
    })
}

/// Types `text` at whatever has focus, as key presses.
// shortcut: newlines are typed, not pasted, so editors may auto-indent them; paste through the X selection if Linux coding needs it.
pub fn live_text(text: &str) -> Result<()> {
    with_x(|x, _| super::x::live_type(x, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_accumulates_fractions() {
        let mut acc = 0.0;
        assert_eq!(notches(&mut acc, 0.4), 0);
        assert_eq!(notches(&mut acc, 0.4), 0);
        assert_eq!(notches(&mut acc, 0.4), 1);
        assert_eq!(notches(&mut acc, -2.5), -2);
        assert!((acc - -0.3).abs() < 1e-5);
        assert_eq!(notches(&mut acc, 3.0), 2);
    }

    #[test]
    fn keysym_in_first_group() {
        // keycodes 8.. : a/A, 1/!, (empty)
        let map = Keymap {
            min_keycode: 8,
            per: 2,
            syms: vec![0x61, 0x41, 0x31, 0x21, 0, 0],
        };
        assert_eq!(code_for(&map, 0x61), Some(8));
        assert_eq!(code_for(&map, 0x41), Some(8));
        assert_eq!(code_for(&map, 0x21), Some(9));
        assert_eq!(code_for(&map, 0x40), None);
    }
}
