//! Key combo parser: `"ctrl+shift+s"`, `"alt+f4"`, `"enter"`, `"ctrl+k ctrl+s"`.

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Ctrl,
    Shift,
    Alt,
    Win,
    Enter,
    Esc,
    Tab,
    Space,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    CapsLock,
    PrintScreen,
    Menu,
    F(u8),
    /// A printable character key, lowercased (`'a'`, `'1'`, `'/'`).
    Char(char),
}

/// Keys pressed together, in order; released in reverse.
pub type Combo = Vec<Key>;

/// Parses whitespace-separated combos of `+`-joined key names.
pub fn parse_keys(s: &str) -> Result<Vec<Combo>> {
    let combos: Vec<Combo> = s
        .split_whitespace()
        .map(|combo| {
            // "ctrl++" means ctrl and the plus key.
            let combo = combo.replace("++", "+plus");
            combo.split('+').map(parse_key).collect()
        })
        .collect::<Result<_>>()?;
    if combos.is_empty() {
        return Err(Error::NotFound("no keys given".into()));
    }
    Ok(combos)
}

fn parse_key(name: &str) -> Result<Key> {
    let lower = name.to_lowercase();
    let key = match lower.as_str() {
        "ctrl" | "control" => Key::Ctrl,
        "shift" => Key::Shift,
        "alt" | "option" => Key::Alt,
        "win" | "meta" | "super" | "cmd" => Key::Win,
        "enter" | "return" => Key::Enter,
        "esc" | "escape" => Key::Esc,
        "tab" => Key::Tab,
        "space" => Key::Space,
        "backspace" | "bs" => Key::Backspace,
        "delete" | "del" => Key::Delete,
        "insert" | "ins" => Key::Insert,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" | "pgup" => Key::PageUp,
        "pagedown" | "pgdn" => Key::PageDown,
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        "capslock" => Key::CapsLock,
        "printscreen" | "prtsc" => Key::PrintScreen,
        "menu" | "apps" => Key::Menu,
        "plus" => Key::Char('+'),
        "minus" => Key::Char('-'),
        _ => {
            let mut chars = lower.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if !c.is_whitespace() => Key::Char(c),
                (Some('f'), Some(_)) => match lower[1..].parse::<u8>() {
                    Ok(n @ 1..=24) => Key::F(n),
                    _ => return Err(Error::NotFound(format!("key {name:?}"))),
                },
                _ => return Err(Error::NotFound(format!("key {name:?}"))),
            }
        }
    };
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combos() {
        assert_eq!(
            parse_keys("ctrl+shift+s").unwrap(),
            vec![vec![Key::Ctrl, Key::Shift, Key::Char('s')]]
        );
        assert_eq!(
            parse_keys("Alt+F4").unwrap(),
            vec![vec![Key::Alt, Key::F(4)]]
        );
        assert_eq!(parse_keys("enter").unwrap(), vec![vec![Key::Enter]]);
        assert_eq!(
            parse_keys("ctrl+k ctrl+s").unwrap(),
            vec![
                vec![Key::Ctrl, Key::Char('k')],
                vec![Key::Ctrl, Key::Char('s')]
            ]
        );
        assert_eq!(
            parse_keys("ctrl++").unwrap(),
            vec![vec![Key::Ctrl, Key::Char('+')]]
        );
        assert_eq!(
            parse_keys("win+pgdn").unwrap(),
            vec![vec![Key::Win, Key::PageDown]]
        );
        assert_eq!(parse_keys("f24").unwrap(), vec![vec![Key::F(24)]]);
        assert_eq!(parse_keys("/").unwrap(), vec![vec![Key::Char('/')]]);
    }

    #[test]
    fn rejects_unknown() {
        assert!(parse_keys("ctrl+banana").is_err());
        assert!(parse_keys("f25").is_err());
        assert!(parse_keys("ctrl+").is_err());
        assert!(parse_keys("   ").is_err());
    }
}
