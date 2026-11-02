//! Pure key mapping: parsed keys and characters to X11 keysyms, and lookups in the
//! server's keyboard mapping. No X connection here, so it is unit-testable anywhere.

use crate::Key;

pub const SHIFT_L: u32 = 0xffe1;
pub const CONTROL_L: u32 = 0xffe3;
pub const ALT_L: u32 = 0xffe9;
pub const SUPER_L: u32 = 0xffeb;
pub const RETURN: u32 = 0xff0d;
pub const TAB: u32 = 0xff09;

/// Keysym of a named key.
pub fn key_keysym(k: Key) -> u32 {
    match k {
        Key::Ctrl => CONTROL_L,
        Key::Shift => SHIFT_L,
        Key::Alt => ALT_L,
        Key::Win => SUPER_L,
        Key::Enter => RETURN,
        Key::Esc => 0xff1b,
        Key::Tab => TAB,
        Key::Space => 0x20,
        Key::Backspace => 0xff08,
        Key::Delete => 0xffff,
        Key::Insert => 0xff63,
        Key::Home => 0xff50,
        Key::End => 0xff57,
        Key::PageUp => 0xff55,
        Key::PageDown => 0xff56,
        Key::Up => 0xff52,
        Key::Down => 0xff54,
        Key::Left => 0xff51,
        Key::Right => 0xff53,
        Key::CapsLock => 0xffe5,
        Key::PrintScreen => 0xff61,
        Key::Menu => 0xff67,
        Key::F(n) => 0xffbe + u32::from(n.max(1)) - 1,
        Key::Char(c) => char_keysym(c),
    }
}

/// Keysym that types `c`: Latin-1 keysyms equal their code point, everything else
/// uses the Unicode keysym range.
pub fn char_keysym(c: char) -> u32 {
    match c as u32 {
        cp @ (0x20..=0x7e | 0xa0..=0xff) => cp,
        cp => 0x0100_0000 | cp,
    }
}

/// The server's keycode -> keysym table (`GetKeyboardMapping`).
#[derive(Debug, Clone)]
pub struct Keymap {
    pub min_keycode: u8,
    /// Keysyms per keycode (column 0 = unshifted, 1 = shifted, then other groups).
    pub per: usize,
    pub syms: Vec<u32>,
}

impl Keymap {
    fn rows(&self) -> impl Iterator<Item = (u8, &[u32])> {
        self.syms
            .chunks(self.per.max(1))
            .enumerate()
            .map(|(i, r)| (self.min_keycode.wrapping_add(i as u8), r))
    }

    /// Keycode and shift state producing `sym`. Unshifted entries win over shifted ones.
    /// AltGr levels are not used.
    pub fn lookup(&self, sym: u32) -> Option<(u8, bool)> {
        let hit = |col: usize| {
            self.rows()
                .find(|(_, r)| r.get(col) == Some(&sym))
                .map(|(k, _)| k)
        };
        hit(0)
            .map(|k| (k, false))
            .or_else(|| hit(1).map(|k| (k, true)))
    }

    /// A keycode with no keysyms at all, free to be rebound temporarily.
    pub fn spare(&self) -> Option<u8> {
        self.rows()
            .filter(|(_, r)| r.iter().all(|&s| s == 0))
            .map(|(k, _)| k)
            .last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> Keymap {
        // keycodes 8.. : a/A, 1/!, Shift_L, (empty), Return, (empty)
        Keymap {
            min_keycode: 8,
            per: 2,
            syms: vec![0x61, 0x41, 0x31, 0x21, SHIFT_L, 0, 0, 0, RETURN, 0, 0, 0],
        }
    }

    #[test]
    fn chars_and_keys() {
        assert_eq!(char_keysym('a'), 0x61);
        assert_eq!(char_keysym('é'), 0xe9);
        assert_eq!(char_keysym('€'), 0x0100_20ac);
        assert_eq!(char_keysym('✓'), 0x0100_2713);
        assert_eq!(key_keysym(Key::Enter), 0xff0d);
        assert_eq!(key_keysym(Key::F(1)), 0xffbe);
        assert_eq!(key_keysym(Key::F(12)), 0xffc9);
        assert_eq!(key_keysym(Key::Char('/')), 0x2f);
        assert_eq!(key_keysym(Key::Win), SUPER_L);
    }

    #[test]
    fn lookup_unshifted_shifted_missing() {
        let m = map();
        assert_eq!(m.lookup(0x61), Some((8, false)));
        assert_eq!(m.lookup(0x41), Some((8, true)));
        assert_eq!(m.lookup(0x21), Some((9, true)));
        assert_eq!(m.lookup(SHIFT_L), Some((10, false)));
        assert_eq!(m.lookup(RETURN), Some((12, false)));
        assert_eq!(m.lookup(0x0100_20ac), None);
    }

    #[test]
    fn spare_is_highest_empty_keycode() {
        assert_eq!(map().spare(), Some(13));
        let full = Keymap {
            min_keycode: 8,
            per: 1,
            syms: vec![1, 2],
        };
        assert_eq!(full.spare(), None);
    }
}
