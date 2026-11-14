//! Plain-text rendering of a vt100 screen with its scrollback.

const COLS: u16 = 200;
const ROWS: u16 = 50;
const SCROLLBACK: usize = 5_000;
const MAX_BYTES: usize = 1 << 20;

/// Scrollback + screen as text lines (no escapes), last `lines` lines, trailing blanks trimmed.
pub fn lines(screen: &mut vt100::Screen, lines: usize) -> String {
    let (rows, cols) = screen.size();
    screen.set_scrollback(usize::MAX);
    let mut off = screen.scrollback();
    let mut all: Vec<String> = Vec::new();
    while off > 0 {
        screen.set_scrollback(off);
        let take = off.min(rows as usize);
        all.extend(screen.rows(0, cols).take(take));
        off -= take;
    }
    screen.set_scrollback(0);
    all.extend(screen.rows(0, cols));
    while all.last().is_some_and(|l| l.trim().is_empty()) {
        all.pop();
    }
    let start = all.len().saturating_sub(lines);
    all[start..]
        .iter()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Raw terminal bytes (the tail of them when huge) rendered through a fresh wide parser.
pub fn plain(bytes: &[u8]) -> String {
    let bytes = &bytes[bytes.len().saturating_sub(MAX_BYTES)..];
    let mut p = vt100::Parser::new(ROWS, COLS, SCROLLBACK);
    p.process(bytes);
    lines(p.screen_mut(), usize::MAX)
        .trim_start_matches('\n')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_escapes_and_leading_blanks() {
        let out = plain(b"\r\n\x1b[31mred\x1b[0m line\r\nsecond\r\n");
        assert_eq!(out, "red line\nsecond");
    }

    #[test]
    fn keeps_more_lines_than_the_screen() {
        let text: String = (1..=120).map(|i| format!("l{i}\r\n")).collect();
        let out = plain(text.as_bytes());
        assert!(out.starts_with("l1\n") && out.ends_with("l120"), "{out:?}");
    }
}
