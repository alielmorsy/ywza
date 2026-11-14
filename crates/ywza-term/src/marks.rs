//! Stateful scanner for the shell-integration OSC sequences (133 prompt marks, 7 cwd).

const MAX_OSC: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mark {
    PromptStart,
    CommandStart,
    OutputStart,
    Finished(Option<i32>),
    Cwd(String),
}

/// A mark with the absolute stream offsets of its first and one-past-last byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub mark: Mark,
    pub start: u64,
    pub end: u64,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum St {
    #[default]
    Ground,
    Esc,
    Osc,
    OscEsc,
}

#[derive(Default)]
pub struct Scanner {
    st: St,
    buf: Vec<u8>,
    start: u64,
    pos: u64,
}

impl Scanner {
    /// Total bytes fed so far.
    pub fn pos(&self) -> u64 {
        self.pos
    }

    pub fn feed(&mut self, chunk: &[u8]) -> Vec<Found> {
        let mut out = Vec::new();
        for &b in chunk {
            self.pos += 1;
            self.st = match (self.st, b) {
                (St::Ground | St::Esc | St::OscEsc, 0x1b) => {
                    self.start = self.pos - 1;
                    St::Esc
                }
                (St::Esc, b']') => {
                    self.buf.clear();
                    St::Osc
                }
                (St::Osc, 0x07) | (St::OscEsc, b'\\') => {
                    if let Some(mark) = parse(&self.buf) {
                        out.push(Found {
                            mark,
                            start: self.start,
                            end: self.pos,
                        });
                    }
                    St::Ground
                }
                (St::Osc, 0x1b) => St::OscEsc,
                (St::Osc, _) if self.buf.len() < MAX_OSC => {
                    self.buf.push(b);
                    St::Osc
                }
                _ => St::Ground,
            };
        }
        out
    }
}

fn parse(osc: &[u8]) -> Option<Mark> {
    if let Some(rest) = osc.strip_prefix(b"133;") {
        let mut fields = rest.split(|&b| b == b';');
        return match fields.next()? {
            b"A" => Some(Mark::PromptStart),
            b"B" => Some(Mark::CommandStart),
            b"C" => Some(Mark::OutputStart),
            b"D" => {
                let code = fields
                    .next()
                    .and_then(|f| std::str::from_utf8(f).ok())
                    .and_then(|f| f.trim().parse().ok());
                Some(Mark::Finished(code))
            }
            _ => None,
        };
    }
    let url = osc.strip_prefix(b"7;")?;
    cwd_from_url(&String::from_utf8_lossy(url)).map(Mark::Cwd)
}

/// `file://host/path` to a path, percent-decoded; `/C:/x` becomes `C:/x`.
fn cwd_from_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("file://")?;
    let path = percent_decode(&rest[rest.find('/')?..]);
    let b = path.as_bytes();
    if b.len() >= 3 && b[1].is_ascii_alphabetic() && b[2] == b':' {
        return Some(path[1..].to_string());
    }
    Some(path)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marks(chunks: &[&[u8]]) -> Vec<Found> {
        let mut s = Scanner::default();
        chunks.iter().flat_map(|c| s.feed(c)).collect()
    }

    #[test]
    fn bel_and_st_terminators() {
        let f = marks(&[b"x\x1b]133;A\x07y\x1b]133;C\x1b\\z"]);
        assert_eq!(f[0].mark, Mark::PromptStart);
        assert_eq!((f[0].start, f[0].end), (1, 9));
        assert_eq!(f[1].mark, Mark::OutputStart);
        assert_eq!((f[1].start, f[1].end), (10, 19));
    }

    #[test]
    fn split_across_chunks() {
        let f = marks(&[b"a\x1b", b"]13", b"3;D;", b"42", b"\x1b", b"\\b"]);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].mark, Mark::Finished(Some(42)));
        assert_eq!((f[0].start, f[0].end), (1, 13));
    }

    #[test]
    fn finished_with_and_without_code() {
        let f = marks(&[b"\x1b]133;D\x07\x1b]133;D;0\x07\x1b]133;D;-1;aid=3\x07"]);
        let m: Vec<_> = f.into_iter().map(|f| f.mark).collect();
        assert_eq!(
            m,
            [
                Mark::Finished(None),
                Mark::Finished(Some(0)),
                Mark::Finished(Some(-1))
            ]
        );
    }

    #[test]
    fn cwd_decoding() {
        let f = marks(&[
            b"\x1b]7;file://host/home/a%20b/c%25\x07",
            b"\x1b]7;file://PC/C:/Users/Me%20X\x07",
            b"\x1b]7;file:///tmp\x07",
        ]);
        let m: Vec<_> = f.into_iter().map(|f| f.mark).collect();
        assert_eq!(
            m,
            [
                Mark::Cwd("/home/a b/c%".into()),
                Mark::Cwd("C:/Users/Me X".into()),
                Mark::Cwd("/tmp".into())
            ]
        );
    }

    #[test]
    fn ignores_other_sequences() {
        assert!(marks(&[b"\x1b]0;title\x07\x1b[31m\x1b]133;Z\x07\x1b]7;nope\x07"]).is_empty());
    }

    #[test]
    fn esc_inside_osc_restarts() {
        let f = marks(&[b"\x1b]13\x1b\x1b]133;A\x07"]);
        assert_eq!(f.len(), 1);
        assert_eq!((f[0].start, f[0].end), (5, 13));
    }
}
