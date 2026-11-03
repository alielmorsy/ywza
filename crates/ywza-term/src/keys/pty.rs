use super::{Code, Key};

impl Key {
    /// xterm modifier parameter: 1 + shift + 2*alt + 4*ctrl.
    fn modifier(&self) -> u8 {
        1 + self.shift as u8 + 2 * self.alt as u8 + 4 * self.ctrl as u8
    }

    /// Bytes to write to a PTY. `app_cursor` selects `ESC O A` over `ESC [ A` for arrows.
    pub fn to_pty_bytes(&self, app_cursor: bool) -> Vec<u8> {
        let m = self.modifier();
        let csi_letter = |l: char| -> Vec<u8> {
            if m > 1 {
                format!("\x1b[1;{m}{l}").into_bytes()
            } else if app_cursor {
                format!("\x1bO{l}").into_bytes()
            } else {
                format!("\x1b[{l}").into_bytes()
            }
        };
        let csi_tilde = |n: u8| -> Vec<u8> {
            if m > 1 {
                format!("\x1b[{n};{m}~").into_bytes()
            } else {
                format!("\x1b[{n}~").into_bytes()
            }
        };
        let mut out = Vec::new();
        let mut alt_prefix = self.alt;
        match self.code {
            Code::Char(c) => {
                let c = if self.shift && !self.ctrl {
                    c.to_ascii_uppercase()
                } else {
                    c
                };
                let ctrl_byte = match c.to_ascii_lowercase() {
                    l @ 'a'..='z' => Some(l as u8 - b'a' + 1),
                    '[' => Some(0x1b),
                    '\\' => Some(0x1c),
                    ']' => Some(0x1d),
                    '^' => Some(0x1e),
                    '_' => Some(0x1f),
                    ' ' | '@' => Some(0),
                    '?' => Some(0x7f),
                    _ => None,
                };
                match (self.ctrl, ctrl_byte) {
                    (true, Some(b)) => out.push(b),
                    _ => out.extend(c.to_string().as_bytes()),
                }
            }
            Code::Enter => out.push(b'\r'),
            Code::Esc => out.push(0x1b),
            Code::Tab if self.shift => out.extend(b"\x1b[Z"),
            Code::Tab => out.push(b'\t'),
            Code::Backspace => out.push(if self.ctrl { 0x08 } else { 0x7f }),
            Code::Up => out = csi_letter('A'),
            Code::Down => out = csi_letter('B'),
            Code::Right => out = csi_letter('C'),
            Code::Left => out = csi_letter('D'),
            Code::Home => out = csi_letter('H'),
            Code::End => out = csi_letter('F'),
            Code::Insert => out = csi_tilde(2),
            Code::Delete => out = csi_tilde(3),
            Code::PageUp => out = csi_tilde(5),
            Code::PageDown => out = csi_tilde(6),
            Code::F(n @ 1..=4) => {
                let l = (b'P' + n - 1) as char;
                out = if m > 1 {
                    format!("\x1b[1;{m}{l}").into_bytes()
                } else {
                    format!("\x1bO{l}").into_bytes()
                };
            }
            Code::F(n) => {
                let code = match n {
                    5 => 15,
                    6 => 17,
                    7 => 18,
                    8 => 19,
                    9 => 20,
                    10 => 21,
                    11 => 23,
                    _ => 24,
                };
                out = csi_tilde(code);
            }
        }
        // Modified CSI sequences already carry alt; plain bytes get an ESC prefix.
        if matches!(
            self.code,
            Code::Up
                | Code::Down
                | Code::Left
                | Code::Right
                | Code::Home
                | Code::End
                | Code::Insert
                | Code::Delete
                | Code::PageUp
                | Code::PageDown
                | Code::F(_)
        ) {
            alt_prefix = false;
        }
        if alt_prefix {
            out.insert(0, 0x1b);
        }
        out
    }
}
