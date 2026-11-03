use super::{Code, Key};
use crate::{err, Result};

fn parse_one(tok: &str) -> Result<Key> {
    let (mods, name) = if tok == "+" {
        ("", "+")
    } else if let Some(p) = tok.strip_suffix("++") {
        (p, "+")
    } else {
        tok.rsplit_once('+').unwrap_or(("", tok))
    };
    let mut key = Key::plain(Code::Enter);
    for m in mods.split('+').filter(|m| !m.is_empty()) {
        match m.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "ctl" => key.ctrl = true,
            "alt" | "meta" | "option" => key.alt = true,
            "shift" => key.shift = true,
            other => return Err(err(format!("unknown modifier '{other}' in '{tok}'"))),
        }
    }
    let lower = name.to_ascii_lowercase();
    key.code = match lower.as_str() {
        "enter" | "return" => Code::Enter,
        "esc" | "escape" => Code::Esc,
        "tab" => Code::Tab,
        "backspace" | "bs" => Code::Backspace,
        "delete" | "del" => Code::Delete,
        "insert" | "ins" => Code::Insert,
        "up" => Code::Up,
        "down" => Code::Down,
        "left" => Code::Left,
        "right" => Code::Right,
        "home" => Code::Home,
        "end" => Code::End,
        "pageup" | "pgup" => Code::PageUp,
        "pagedown" | "pgdn" | "pgdown" => Code::PageDown,
        "space" => Code::Char(' '),
        f if f.len() > 1
            && f.starts_with('f')
            && f[1..].parse::<u8>().is_ok_and(|n| (1..=12).contains(&n)) =>
        {
            Code::F(f[1..].parse().unwrap())
        }
        _ => {
            let mut it = name.chars();
            match (it.next(), it.next()) {
                (Some(c), None) => Code::Char(c),
                _ => return Err(err(format!("unknown key '{name}'"))),
            }
        }
    };
    Ok(key)
}

/// Parse a whitespace-separated key sequence such as `"ctrl+c"` or `"esc : w q enter"`.
pub fn parse_keys(s: &str) -> Result<Vec<Key>> {
    let keys = s
        .split_whitespace()
        .map(parse_one)
        .collect::<Result<Vec<_>>>()?;
    if keys.is_empty() {
        return Err(err("empty key sequence"));
    }
    Ok(keys)
}
