//! Key-name parsing (`"ctrl+c"`, `"esc"`, `"shift+tab"`) and the PTY byte encoding.

mod parse;
mod pty;
mod types;

pub use parse::parse_keys;
pub use types::{Code, Key};

#[cfg(test)]
mod tests {
    use super::*;

    fn b(s: &str) -> Vec<u8> {
        parse_keys(s)
            .unwrap()
            .iter()
            .flat_map(|k| k.to_pty_bytes(false))
            .collect()
    }

    #[test]
    fn basic() {
        assert_eq!(b("ctrl+c"), [3]);
        assert_eq!(b("ctrl+z"), [26]);
        assert_eq!(b("esc"), [0x1b]);
        assert_eq!(b("enter"), b"\r");
        assert_eq!(b("tab"), b"\t");
        assert_eq!(b("up"), b"\x1b[A");
        assert_eq!(b("alt+x"), b"\x1bx");
        assert_eq!(b("shift+tab"), b"\x1b[Z");
        assert_eq!(b("ctrl+up"), b"\x1b[1;5A");
        assert_eq!(b("f5"), b"\x1b[15~");
        assert_eq!(b("f1"), b"\x1bOP");
        assert_eq!(b("delete"), b"\x1b[3~");
        assert_eq!(b("ctrl++"), b"+");
        assert_eq!(b("y enter"), b"y\r");
        assert_eq!(b("Ctrl+Space"), [0]);
    }

    #[test]
    fn app_cursor_and_errors() {
        let k = parse_keys("up").unwrap()[0];
        assert_eq!(k.to_pty_bytes(true), b"\x1bOA");
        assert!(parse_keys("ctrl+").is_err());
        assert!(parse_keys("bogus+a").is_err());
        assert!(parse_keys("  ").is_err());
    }
}
