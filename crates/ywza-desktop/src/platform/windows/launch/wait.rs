use super::ctx::{foreground_id, LaunchCtx};
use super::select::select;
use crate::platform::windows::wm;
use crate::{Error, Launched, Result, WindowInfo};
use ::windows::Win32::Foundation::{CloseHandle, WAIT_TIMEOUT};
use ::windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use ::windows::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};
use std::time::{Duration, Instant};

impl Launched {
    /// Waits for the window of the launched program: one owned by the launched process or its
    /// descendants (launcher stubs like Store apps) first. If the launcher exits and hands off
    /// to a running instance (Notepad, Explorer, Settings), only windows of the same program
    /// that are new, newly titled or newly foreground count, not whatever else appeared.
    pub fn wait_for_window(&self, timeout: Duration) -> Result<Vec<WindowInfo>> {
        let ctx = LaunchCtx::lookup(&self.before);
        let deadline = Instant::now() + timeout;
        let mut gone_since: Option<Instant> = None;
        loop {
            let alive = alive(self.pid);
            if !alive {
                gone_since.get_or_insert_with(Instant::now);
            }
            let settled = gone_since.is_some_and(|t| t.elapsed() >= Duration::from_millis(1500));
            let family = if self.pid == 0 {
                Vec::new()
            } else {
                family(self.pid)
            };
            let found = select(
                &wm::list_windows()?,
                &ctx,
                &family,
                alive,
                foreground_id(),
                settled,
            );
            if !found.is_empty() {
                return Ok(found);
            }
            if Instant::now() > deadline {
                return Err(Error::Timeout(format!("window of pid {}", self.pid)));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

/// `pid` and all its descendant processes.
fn family(pid: u32) -> Vec<u32> {
    let mut procs = Vec::new();
    unsafe {
        if let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut e = PROCESSENTRY32W {
                dwSize: size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            let mut ok = Process32FirstW(snap, &mut e).is_ok();
            while ok {
                procs.push((e.th32ProcessID, e.th32ParentProcessID));
                ok = Process32NextW(snap, &mut e).is_ok();
            }
            let _ = CloseHandle(snap);
        }
    }
    let mut family = vec![pid];
    let mut i = 0;
    while i < family.len() {
        let parent = family[i];
        family.extend(
            procs
                .iter()
                .filter(|&&(c, p)| p == parent && c != parent && !family.contains(&c))
                .map(|&(c, _)| c)
                .collect::<Vec<_>>(),
        );
        i += 1;
    }
    family
}

fn alive(pid: u32) -> bool {
    unsafe {
        let Ok(p) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) else {
            return false;
        };
        let running = WaitForSingleObject(p, 0) == WAIT_TIMEOUT;
        let _ = CloseHandle(p);
        running
    }
}
