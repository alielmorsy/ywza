//! What the shell-integration marks say about a terminal, and command output extraction.

use crate::marks::{Found, Mark};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Unknown,
    Prompt,
    Input,
    Running,
}

#[derive(Default)]
pub struct ShellState {
    /// Any prompt mark seen: the shell has integration.
    pub seen: bool,
    pub phase: Phase,
    pub cwd: Option<String>,
    pub exit: Option<i32>,
    /// Number of command-output starts (C marks).
    pub runs: u64,
    /// Stream offset just after the last C mark.
    pub out_start: u64,
    /// Stream offset just before the D mark that ended the last C, once it arrived.
    pub out_end: Option<u64>,
}

impl ShellState {
    pub fn apply(&mut self, f: &Found) {
        if let Mark::Cwd(c) = &f.mark {
            self.cwd = Some(c.clone());
            return;
        }
        self.seen = true;
        match &f.mark {
            Mark::PromptStart | Mark::Cwd(_) => self.phase = Phase::Prompt,
            Mark::CommandStart => self.phase = Phase::Input,
            Mark::OutputStart => {
                self.phase = Phase::Running;
                self.runs += 1;
                self.out_start = f.end;
                self.out_end = None;
            }
            Mark::Finished(code) => {
                if self.phase == Phase::Running {
                    self.out_end = Some(f.start);
                }
                self.phase = Phase::Prompt;
                self.exit = *code;
            }
        }
    }
}

/// What term_read and term_run report about an integrated shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellInfo {
    pub cwd: Option<String>,
    pub exit: Option<i32>,
    pub busy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    pub output: String,
    pub exit: Option<i32>,
    pub cwd: Option<String>,
    /// False on timeout or when the terminal closed first.
    pub finished: bool,
    pub closed: bool,
}

/// Ring bytes between absolute stream offsets, clamped to what the ring still holds.
pub fn slice(ring: &VecDeque<u8>, total: u64, from: u64, to: u64) -> Vec<u8> {
    let base = total - ring.len() as u64;
    let from = from.clamp(base, total);
    let to = to.clamp(from, total);
    ring.iter()
        .skip((from - base) as usize)
        .take((to - from) as usize)
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marks::Scanner;

    fn drive(state: &mut ShellState, sc: &mut Scanner, bytes: &[u8]) {
        for f in sc.feed(bytes) {
            state.apply(&f);
        }
    }

    #[test]
    fn extracts_last_command_output() {
        let stream = b"\x1b]133;A\x07$ \x1b]133;B\x07ls\r\n\x1b]133;C\x07a\r\nb\r\n\x1b]133;D;3\x07\x1b]133;A\x07$ ";
        let (mut st, mut sc) = (ShellState::default(), Scanner::default());
        let mut ring: VecDeque<u8> = VecDeque::new();
        for chunk in stream.chunks(7) {
            drive(&mut st, &mut sc, chunk);
            ring.extend(chunk);
        }
        assert!(st.seen);
        assert_eq!((st.runs, st.exit, st.phase), (1, Some(3), Phase::Prompt));
        let out = slice(&ring, sc.pos(), st.out_start, st.out_end.unwrap());
        assert_eq!(out, b"a\r\nb\r\n");
    }

    #[test]
    fn running_has_no_end_and_empty_enter_is_not_a_run() {
        let (mut st, mut sc) = (ShellState::default(), Scanner::default());
        drive(&mut st, &mut sc, b"\x1b]133;D;0\x07");
        assert_eq!((st.runs, st.out_end), (0, None));
        drive(&mut st, &mut sc, b"\x1b]133;C\x07partial");
        assert_eq!((st.phase, st.out_end), (Phase::Running, None));
    }

    #[test]
    fn slice_clamps_to_trimmed_ring() {
        let ring: VecDeque<u8> = b"cdef".iter().copied().collect();
        assert_eq!(slice(&ring, 6, 0, 5), b"cde");
        assert_eq!(slice(&ring, 6, 3, 99), b"def");
        assert_eq!(slice(&ring, 6, 9, 3), b"");
    }
}
