use super::conn::X;
use super::foreground::with_foreground;
use crate::platform::linux::keysym::{self, Keymap};
use crate::{parse_keys, Error, Key, Result, WindowId};
use std::thread::sleep;
use std::time::Duration;
use x11rb::protocol::xproto::ConnectionExt as _;

/// Types `text` into the focused window (after bringing `window` forward, if given).
pub(crate) fn type_text(window: Option<WindowId>, text: &str) -> Result<()> {
    with_foreground(window, |x| send_text(x, text))
}

/// Presses key combos like `"ctrl+shift+s"` or `"ctrl+k ctrl+s"`, into `window` if given
/// (brought to the front first), else into whatever has focus.
pub fn press_keys(window: Option<WindowId>, keys: &str) -> Result<()> {
    let combos = parse_keys(keys)?;
    with_foreground(window, |x| {
        let map = x.keymap()?;
        // Resolve everything first so a bad key sends nothing.
        let mut plans = Vec::new();
        for combo in &combos {
            let mut codes = Vec::new();
            for &k in combo {
                let (code, shift) = lookup(&map, k)?;
                if shift && !combo.contains(&Key::Shift) {
                    codes.push(lookup(&map, Key::Shift)?.0);
                }
                codes.push(code);
            }
            plans.push(codes);
        }
        for codes in plans {
            x.chord(&codes)?;
            x.sync()?;
        }
        Ok(())
    })
}

fn lookup(map: &Keymap, k: Key) -> Result<(u8, bool)> {
    map.lookup(keysym::key_keysym(k))
        .ok_or_else(|| Error::NotFound(format!("no key for {k:?} on this layout")))
}

/// Types text. Characters on the current layout are real key presses. Others
/// (no keysym in the map) are typed through a spare keycode that is bound to the
/// keysym for the duration of the press; with no spare keycode the call fails.
fn send_text(x: &X, text: &str) -> Result<()> {
    let map = x.keymap()?;
    let shift = map.lookup(keysym::SHIFT_L).map(|(k, _)| k);
    for c in text.chars() {
        let sym = match c {
            '\r' => continue,
            '\n' => keysym::RETURN,
            '\t' => keysym::TAB,
            _ => keysym::char_keysym(c),
        };
        match map.lookup(sym) {
            Some((code, false)) => x.chord(&[code])?,
            Some((code, true)) => {
                let s = shift.ok_or_else(|| Error::NotFound("Shift key".into()))?;
                x.chord(&[s, code])?
            }
            None => {
                let spare = map
                    .spare()
                    .ok_or_else(|| Error::NotFound(format!("no key for {c:?} on this layout")))?;
                let mut syms = vec![0u32; map.per.max(1)];
                syms[0] = sym;
                x.conn
                    .change_keyboard_mapping(1, spare, syms.len() as u8, &syms)?;
                x.sync()?;
                sleep(Duration::from_millis(30));
                let sent = x.chord(&[spare]).and_then(|_| x.sync());
                sleep(Duration::from_millis(30));
                let none = vec![0u32; syms.len()];
                x.conn
                    .change_keyboard_mapping(1, spare, none.len() as u8, &none)?;
                x.sync()?;
                sent?;
            }
        }
        x.sync()?;
    }
    Ok(())
}
