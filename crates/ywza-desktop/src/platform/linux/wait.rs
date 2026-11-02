use super::x;
use crate::{Error, Launched, Result, WindowInfo};
use std::path::Path;
use std::time::{Duration, Instant};

impl Launched {
    /// Waits for new windows owned by the launched process or its descendants
    /// (launcher stubs, wrapper scripts). If the process exits first, any new window counts.
    pub fn wait_for_window(&self, timeout: Duration) -> Result<Vec<WindowInfo>> {
        let deadline = Instant::now() + timeout;
        loop {
            let new: Vec<WindowInfo> = x::list_windows()?
                .into_iter()
                .filter(|w| !self.before.contains(&w.id))
                .collect();
            let family = family(self.pid);
            let own: Vec<WindowInfo> = new
                .iter()
                .filter(|w| family.contains(&w.pid))
                .cloned()
                .collect();
            if !own.is_empty() {
                return Ok(own);
            }
            if !new.is_empty() && !alive(self.pid) {
                return Ok(new);
            }
            if Instant::now() > deadline {
                return Err(Error::Timeout(format!("window of pid {}", self.pid)));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

/// `(pid, parent pid)` of every process, from `/proc/*/stat`.
fn processes() -> Vec<(u32, u32)> {
    let Ok(dir) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    dir.flatten()
        .filter_map(|e| {
            let pid: u32 = e.file_name().to_str()?.parse().ok()?;
            let stat = std::fs::read_to_string(e.path().join("stat")).ok()?;
            Some((pid, parent_pid(&stat)?))
        })
        .collect()
}

/// PPid field of a `/proc/<pid>/stat` line (the command name may contain spaces and `)`).
fn parent_pid(stat: &str) -> Option<u32> {
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace().nth(1)?.parse().ok()
}

/// `pid` and all its descendant processes.
fn family(pid: u32) -> Vec<u32> {
    descendants(pid, &processes())
}

fn descendants(pid: u32, procs: &[(u32, u32)]) -> Vec<u32> {
    let mut family = vec![pid];
    let mut i = 0;
    while i < family.len() {
        let parent = family[i];
        let kids: Vec<u32> = procs
            .iter()
            .filter(|&&(c, p)| p == parent && c != parent && !family.contains(&c))
            .map(|&(c, _)| c)
            .collect();
        family.extend(kids);
        i += 1;
    }
    family
}

fn alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_tree() {
        assert_eq!(parent_pid("42 (my (weird) app) S 7 42 42 0"), Some(7));
        let procs = [(2, 1), (3, 2), (4, 3), (5, 1), (6, 6)];
        assert_eq!(descendants(2, &procs), vec![2, 3, 4]);
        assert_eq!(descendants(6, &procs), vec![6]);
    }
}
