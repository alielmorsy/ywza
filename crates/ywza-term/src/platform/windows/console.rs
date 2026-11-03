//! Windows console access. Only ever runs inside the helper child process:
//! a process can be attached to one console at a time.

use super::records::key_records;
use crate::keys::{Code, Key};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Console::*;

const ACCESS_DENIED: i32 = 0x8007_0005_u32 as i32;
const INVALID_HANDLE: i32 = 0x8007_0006_u32 as i32;
/// Error text for a process that has no console (redirected stdio, services, ...).
pub const NO_CONSOLE: &str = "no console";

/// Attached console. Dropping detaches and restores the caller's stdout/stderr handles.
pub struct Console {
    out: HANDLE,
    err: HANDLE,
}

impl Console {
    pub fn attach(pid: u32) -> Result<Console, String> {
        unsafe {
            // Saved so our JSON reply still reaches the caller when stdout was a console.
            let out = GetStdHandle(STD_OUTPUT_HANDLE).unwrap_or_default();
            let err = GetStdHandle(STD_ERROR_HANDLE).unwrap_or_default();
            let me = Console { out, err };
            let _ = FreeConsole();
            if let Err(e) = AttachConsole(pid) {
                return Err(if e.code().0 == ACCESS_DENIED {
                    format!("terminal {pid} is elevated; run the agent elevated")
                } else if e.code().0 == INVALID_HANDLE {
                    NO_CONSOLE.to_string()
                } else {
                    format!("AttachConsole({pid}) failed: {e}")
                });
            }
            // Don't let a synthetic ctrl+c kill the helper itself.
            let _ = SetConsoleCtrlHandler(None, true);
            Ok(me)
        }
    }

    pub fn hwnd(&self) -> i64 {
        unsafe { GetConsoleWindow().0 as i64 }
    }

    pub fn title(&self) -> String {
        let mut buf = [0u16; 1024];
        let n = unsafe { GetConsoleTitleW(&mut buf) } as usize;
        String::from_utf16_lossy(&buf[..n.min(buf.len())])
    }

    pub fn process_list(&self) -> Vec<u32> {
        let mut buf = [0u32; 256];
        let n = unsafe { GetConsoleProcessList(&mut buf) } as usize;
        buf[..n.min(buf.len())].to_vec()
    }

    /// Last `lines` rows (trailing blank rows trimmed) of the screen buffer. Scrollback is
    /// included when the console keeps one; ConPTY hosts such as Windows Terminal only
    /// expose the viewport.
    pub fn read(&self, lines: usize) -> Result<String, String> {
        unsafe {
            let h = open(w!("CONOUT$"))?;
            let mut csbi = CONSOLE_SCREEN_BUFFER_INFO::default();
            let r = GetConsoleScreenBufferInfo(h, &mut csbi);
            let result = r
                .map_err(|e| format!("GetConsoleScreenBufferInfo: {e}"))
                .map(|_| {
                    let width = csbi.dwSize.X.max(1) as usize;
                    let end = csbi
                        .dwCursorPosition
                        .Y
                        .max(csbi.srWindow.Bottom)
                        .min(csbi.dwSize.Y - 1);
                    let mut buf = vec![0u16; width];
                    let mut out: Vec<String> = Vec::new();
                    for y in (0..=end).rev() {
                        let mut n = 0u32;
                        let _ =
                            ReadConsoleOutputCharacterW(h, &mut buf, COORD { X: 0, Y: y }, &mut n);
                        let line = String::from_utf16_lossy(&buf[..n as usize])
                            .trim_end()
                            .to_string();
                        if out.is_empty() && line.is_empty() {
                            continue;
                        }
                        out.push(line);
                        if out.len() >= lines {
                            break;
                        }
                    }
                    out.reverse();
                    out.join("\n")
                });
            let _ = CloseHandle(h);
            result
        }
    }

    pub fn write_text(&self, text: &str, enter: bool) -> Result<(), String> {
        let mut recs = Vec::new();
        let mut prev_cr = false;
        for c in text.chars() {
            if c == '\n' && prev_cr {
                prev_cr = false;
                continue;
            }
            prev_cr = c == '\r';
            let code = if c == '\r' || c == '\n' {
                Code::Enter
            } else {
                Code::Char(c)
            };
            recs.extend(key_records(&Key::plain(code)));
        }
        if enter {
            recs.extend(key_records(&Key::plain(Code::Enter)));
        }
        self.send(&recs)
    }

    /// Ctrl+C is delivered as a real CTRL_C_EVENT when the console processes it (a plain key
    /// event is ignored by cooked-mode programs); in raw mode it goes through as a key.
    pub fn send_keys(&self, keys: &[Key]) -> Result<(), String> {
        let mut recs = Vec::new();
        for k in keys {
            if k.code == Code::Char('c') && k.ctrl && !k.alt && !k.shift && self.processed_input() {
                self.send(&std::mem::take(&mut recs))?;
                unsafe { GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0) }
                    .map_err(|e| format!("GenerateConsoleCtrlEvent: {e}"))?;
            } else {
                recs.extend(key_records(k));
            }
        }
        self.send(&recs)
    }

    fn processed_input(&self) -> bool {
        unsafe {
            let Ok(h) = open(w!("CONIN$")) else {
                return false;
            };
            let mut mode = CONSOLE_MODE(0);
            let ok = GetConsoleMode(h, &mut mode).is_ok();
            let _ = CloseHandle(h);
            ok && mode.0 & ENABLE_PROCESSED_INPUT.0 != 0
        }
    }

    fn send(&self, recs: &[INPUT_RECORD]) -> Result<(), String> {
        if recs.is_empty() {
            return Ok(());
        }
        unsafe {
            let h = open(w!("CONIN$"))?;
            let mut n = 0u32;
            let r = WriteConsoleInputW(h, recs, &mut n);
            let _ = CloseHandle(h);
            r.map_err(|e| format!("WriteConsoleInputW: {e}"))?;
            if (n as usize) < recs.len() {
                return Err(format!("only {n} of {} input records written", recs.len()));
            }
            Ok(())
        }
    }
}

impl Drop for Console {
    fn drop(&mut self) {
        unsafe {
            let _ = FreeConsole();
            if !self.out.is_invalid() {
                let _ = SetStdHandle(STD_OUTPUT_HANDLE, self.out);
            }
            if !self.err.is_invalid() {
                let _ = SetStdHandle(STD_ERROR_HANDLE, self.err);
            }
        }
    }
}

unsafe fn open(name: PCWSTR) -> Result<HANDLE, String> {
    unsafe {
        CreateFileW(
            name,
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
        .map_err(|e| format!("open console handle: {e}"))
    }
}
