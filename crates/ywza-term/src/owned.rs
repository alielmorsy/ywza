use crate::keys::parse_keys;
use crate::marks::Scanner;
use crate::track::{slice, Phase, RunOutcome, ShellInfo, ShellState};
use crate::{err, render, shell, Result};
use portable_pty::{native_pty_system, Child, ChildKiller, CommandBuilder, MasterPty, PtySize};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub const RING_CAP: usize = 4 * 1024 * 1024;
const SCROLLBACK: usize = 10_000;
const ROWS: u16 = 30;
const COLS: u16 = 120;

#[derive(Default)]
struct Callbacks {
    title: String,
}

impl vt100::Callbacks for Callbacks {
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        self.title = String::from_utf8_lossy(title).into_owned();
    }
}

struct Shared {
    parser: vt100::Parser<Callbacks>,
    ring: VecDeque<u8>,
    eof: bool,
    scanner: Scanner,
    sh: ShellState,
}

type Writer = Arc<Mutex<Box<dyn Write + Send>>>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

pub struct OwnedTerm {
    pub command: String,
    pub cwd: Option<PathBuf>,
    shared: Arc<Mutex<Shared>>,
    writer: Writer,
    child: Mutex<Box<dyn Child + Send + Sync>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    _master: Mutex<Box<dyn MasterPty + Send>>,
    exit: Mutex<Option<u32>>,
    /// Shell integration was injected at start, so marks are expected soon.
    integrated: bool,
}

impl OwnedTerm {
    /// Run `command` through the platform shell (`cmd /d /s /c` or `sh -c`). A bare pwsh,
    /// powershell, bash or zsh is started directly with shell integration injected.
    pub fn start(command: &str, cwd: Option<&Path>) -> Result<OwnedTerm> {
        let launch = shell::plan(command);
        let integrated = launch.is_some();
        let mut cmd = match launch {
            Some(l) => {
                let mut c = CommandBuilder::new(&l.program);
                c.args(&l.args);
                for (k, v) in &l.env {
                    c.env(k, v);
                }
                c
            }
            None if cfg!(windows) => {
                let mut c = CommandBuilder::new("cmd.exe");
                c.args(["/d", "/s", "/c", command]);
                c
            }
            None => {
                let mut c = CommandBuilder::new("sh");
                c.args(["-c", command]);
                c
            }
        };
        if let Some(c) = cwd {
            cmd.cwd(c);
        }
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: ROWS,
                cols: COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(err)?;
        let child = pair.slave.spawn_command(cmd).map_err(err)?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().map_err(err)?;
        let writer: Writer = Arc::new(Mutex::new(pair.master.take_writer().map_err(err)?));
        let shared = Arc::new(Mutex::new(Shared {
            parser: vt100::Parser::new_with_callbacks(ROWS, COLS, SCROLLBACK, Callbacks::default()),
            ring: VecDeque::new(),
            eof: false,
            scanner: Scanner::default(),
            sh: ShellState::default(),
        }));
        {
            let (shared, writer) = (shared.clone(), writer.clone());
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                while let Ok(n) = reader.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    let chunk = &buf[..n];
                    let mut s = lock(&shared);
                    s.parser.process(chunk);
                    s.ring.extend(chunk);
                    let over = s.ring.len().saturating_sub(RING_CAP);
                    s.ring.drain(..over);
                    for f in s.scanner.feed(chunk) {
                        s.sh.apply(&f);
                    }
                    // ConPTY asks for the cursor position at startup and waits for the answer.
                    if chunk.windows(4).any(|w| w == b"\x1b[6n") {
                        let (r, c) = s.parser.screen().cursor_position();
                        drop(s);
                        let _ = lock(&writer)
                            .write_all(format!("\x1b[{};{}R", r + 1, c + 1).as_bytes());
                    }
                }
                lock(&shared).eof = true;
            });
        }
        Ok(OwnedTerm {
            command: command.to_string(),
            cwd: cwd.map(Path::to_path_buf),
            shared,
            writer,
            killer: Mutex::new(child.clone_killer()),
            child: Mutex::new(child),
            _master: Mutex::new(pair.master),
            exit: Mutex::new(None),
            integrated,
        })
    }

    pub fn pid(&self) -> Option<u32> {
        lock(&self.child).process_id()
    }

    /// Rendered text (scrollback + screen, no escapes), last `lines` lines, trailing blanks trimmed.
    pub fn read(&self, lines: usize) -> String {
        render::lines(lock(&self.shared).parser.screen_mut(), lines)
    }

    /// Cwd, last exit code and busy flag; `None` until the shell has emitted prompt marks.
    pub fn shell_info(&self) -> Option<ShellInfo> {
        let s = lock(&self.shared);
        s.sh.seen.then(|| ShellInfo {
            cwd: s.sh.cwd.clone(),
            exit: s.sh.exit,
            busy: s.sh.phase == Phase::Running,
        })
    }

    /// Cwd reported by the shell (OSC 7), if any.
    pub fn shell_cwd(&self) -> Option<String> {
        lock(&self.shared).sh.cwd.clone()
    }

    /// True once the shell has shown integration marks. An injected shell gets a few seconds
    /// to print its first prompt.
    pub fn wait_integration(&self) -> bool {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            {
                let s = lock(&self.shared);
                if s.sh.seen {
                    return true;
                }
                if !self.integrated || s.eof {
                    return false;
                }
            }
            if Instant::now() >= end {
                return false;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    /// Type `command` plus Enter and wait for its prompt mark. On timeout returns what has been
    /// printed so far (`finished` false). Needs [`Self::wait_integration`] to be true.
    pub fn run(&self, command: &str, timeout: Duration) -> Result<RunOutcome> {
        let runs = {
            let s = lock(&self.shared);
            if !s.sh.seen {
                return Err(err("no shell integration"));
            }
            if s.sh.phase == Phase::Running {
                return Err(err(
                    "a command is still running in this terminal; read it or send keys",
                ));
            }
            s.sh.runs
        };
        self.write(command, true)?;
        let end = Instant::now() + timeout;
        loop {
            let (finished, closed) = {
                let s = lock(&self.shared);
                (s.sh.runs > runs && s.sh.out_end.is_some(), s.eof)
            };
            if finished || closed || Instant::now() >= end {
                return Ok(self.outcome(runs, finished, closed));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn outcome(&self, runs_before: u64, finished: bool, closed: bool) -> RunOutcome {
        let s = lock(&self.shared);
        let total = s.scanner.pos();
        let raw = if s.sh.runs > runs_before {
            slice(&s.ring, total, s.sh.out_start, s.sh.out_end.unwrap_or(total))
        } else {
            Vec::new()
        };
        let (exit, cwd) = (s.sh.exit.filter(|_| finished), s.sh.cwd.clone());
        drop(s);
        RunOutcome {
            output: render::plain(&raw),
            exit,
            cwd,
            finished,
            closed,
        }
    }

    /// Last `max` bytes of the raw output ring buffer (4 MB cap).
    pub fn raw_tail(&self, max: usize) -> Vec<u8> {
        let s = lock(&self.shared);
        let skip = s.ring.len().saturating_sub(max);
        s.ring.iter().skip(skip).copied().collect()
    }

    pub fn title(&self) -> String {
        lock(&self.shared).parser.callbacks().title.clone()
    }

    pub fn write(&self, text: &str, enter: bool) -> Result<()> {
        let mut w = lock(&self.writer);
        w.write_all(text.as_bytes()).map_err(err)?;
        if enter {
            w.write_all(b"\r").map_err(err)?;
        }
        w.flush().map_err(err)
    }

    pub fn keys(&self, keys: &str) -> Result<()> {
        let app = lock(&self.shared).parser.screen().application_cursor();
        let bytes: Vec<u8> = parse_keys(keys)?
            .iter()
            .flat_map(|k| k.to_pty_bytes(app))
            .collect();
        let mut w = lock(&self.writer);
        w.write_all(&bytes).map_err(err)?;
        w.flush().map_err(err)
    }

    pub fn kill(&self) -> Result<()> {
        let r = lock(&self.killer).kill();
        // portable-pty can report a spurious "os error 0" on Windows; trust the process state.
        match r {
            Err(e) if self.exit_status().is_none() && !self.wait_exit() => Err(err(e)),
            _ => Ok(()),
        }
    }

    /// `None` while running, else the exit code.
    pub fn exit_status(&self) -> Option<u32> {
        let mut e = lock(&self.exit);
        if e.is_none() {
            if let Ok(Some(st)) = lock(&self.child).try_wait() {
                *e = Some(st.exit_code());
            }
        }
        *e
    }

    fn wait_exit(&self) -> bool {
        for _ in 0..20 {
            if self.exit_status().is_some() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        false
    }

    pub fn is_running(&self) -> bool {
        self.exit_status().is_none()
    }

    /// True once the PTY output stream has closed.
    pub fn output_closed(&self) -> bool {
        lock(&self.shared).eof
    }
}

impl Drop for OwnedTerm {
    fn drop(&mut self) {
        let _ = lock(&self.killer).kill();
    }
}
