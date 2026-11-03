#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    Char(char),
    Enter,
    Esc,
    Tab,
    Backspace,
    Delete,
    Insert,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    F(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub code: Code,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Key {
    pub fn plain(code: Code) -> Key {
        Key {
            code,
            ctrl: false,
            alt: false,
            shift: false,
        }
    }
}
