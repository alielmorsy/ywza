use crate::attach::{Attachable, Host};
use crate::helper::{call, Request};
use crate::Result;
use std::collections::HashMap;
use std::ffi::c_void;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::System::Diagnostics::ToolHelp::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const SHELLS: &[&str] = &[
    "cmd",
    "powershell",
    "pwsh",
    "wsl",
    "bash",
    "sh",
    "zsh",
    "fish",
    "nu",
    "node",
    "python",
    "claude",
    "ssh",
];
/// Console hosts and terminals: never listed themselves; their direct children are.
const HOSTS: &[&str] = &["conhost", "openconsole", "windowsterminal", "wt"];

struct Proc {
    ppid: u32,
    name: String,
}

fn snapshot() -> HashMap<u32, Proc> {
    let mut map = HashMap::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return map;
        };
        let mut e = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut ok = Process32FirstW(snap, &mut e).is_ok();
        while ok {
            let n = e
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(e.szExeFile.len());
            let name = String::from_utf16_lossy(&e.szExeFile[..n]).to_ascii_lowercase();
            let name = name.strip_suffix(".exe").unwrap_or(&name).to_string();
            map.insert(
                e.th32ProcessID,
                Proc {
                    ppid: e.th32ParentProcessID,
                    name,
                },
            );
            ok = Process32NextW(snap, &mut e).is_ok();
        }
        let _ = windows::Win32::Foundation::CloseHandle(snap);
    }
    map
}

fn ancestors(procs: &HashMap<u32, Proc>, pid: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut cur = pid;
    while let Some(p) = procs.get(&cur) {
        if p.ppid == 0 || p.ppid == cur || out.len() > 32 || !procs.contains_key(&p.ppid) {
            break;
        }
        out.push(p.ppid);
        cur = p.ppid;
    }
    out
}

fn hwnd(h: isize) -> HWND {
    HWND(h as *mut c_void)
}

fn class_name(h: HWND) -> String {
    let mut b = [0u16; 256];
    let n = unsafe { GetClassNameW(h, &mut b) } as usize;
    String::from_utf16_lossy(&b[..n.min(b.len())])
}

fn windows_of(pid: u32, class: &str) -> Vec<isize> {
    struct Ctx<'a> {
        pid: u32,
        class: &'a str,
        out: Vec<isize>,
    }
    unsafe extern "system" fn cb(h: HWND, lp: LPARAM) -> BOOL {
        unsafe {
            let ctx = &mut *(lp.0 as *mut Ctx);
            let mut p = 0u32;
            GetWindowThreadProcessId(h, Some(&mut p));
            if p == ctx.pid && class_name(h) == ctx.class {
                ctx.out.push(h.0 as isize);
            }
        }
        BOOL(1)
    }
    let mut ctx = Ctx {
        pid,
        class,
        out: Vec::new(),
    };
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut ctx as *mut Ctx as isize));
    }
    ctx.out
}

const WT_CLASS: &str = "CASCADIA_HOSTING_WINDOW_CLASS";

/// Map a console window handle to its terminal window.
fn classify(console: i64, ancestors: &[u32], procs: &HashMap<u32, Proc>) -> (Host, Option<isize>) {
    if console == 0 {
        return (Host::Other, None);
    }
    let h = hwnd(console as isize);
    match class_name(h).as_str() {
        "ConsoleWindowClass" => (Host::Conhost, Some(console as isize)),
        _ => {
            // ConPTY: the pseudo console window is owned by the hosting terminal window.
            if let Ok(owner) = unsafe { GetWindow(h, GW_OWNER) } {
                if class_name(owner) == WT_CLASS {
                    return (Host::WindowsTerminal, Some(owner.0 as isize));
                }
            }
            let wt = ancestors
                .iter()
                .find(|a| procs.get(a).is_some_and(|p| p.name == "windowsterminal"));
            match wt.and_then(|&pid| windows_of(pid, WT_CLASS).first().copied()) {
                Some(w) => (Host::WindowsTerminal, Some(w)),
                None => (Host::Other, None),
            }
        }
    }
}

pub fn list_attachable() -> Result<Vec<Attachable>> {
    let procs = snapshot();
    let me = std::process::id();
    let mut cands: Vec<u32> = procs
        .iter()
        .filter(|(&pid, p)| {
            pid != me
                && !HOSTS.contains(&p.name.as_str())
                && (SHELLS.contains(&p.name.as_str())
                    || procs
                        .get(&p.ppid)
                        .is_some_and(|pp| HOSTS.contains(&pp.name.as_str())))
        })
        .map(|(&pid, _)| pid)
        .collect();
    cands.sort_unstable();
    if cands.is_empty() {
        return Ok(Vec::new());
    }
    let mut infos = call(&Request::Info {
        pids: cands.clone(),
    })?
    .infos;
    infos.retain(|i| i.error.as_deref() != Some(super::console::NO_CONSOLE));

    // One entry per console: group by the lowest pid attached to it, keep the root process.
    let mut groups: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, inf) in infos.iter().enumerate() {
        let key = inf.procs.iter().copied().min().unwrap_or(inf.pid);
        groups.entry(key).or_default().push(i);
    }
    let mut out = Vec::new();
    for members in groups.values() {
        let pids: Vec<u32> = members.iter().map(|&i| infos[i].pid).collect();
        let root = members
            .iter()
            .copied()
            .find(|&i| {
                !ancestors(&procs, infos[i].pid)
                    .iter()
                    .any(|a| pids.contains(a))
            })
            .unwrap_or(members[0]);
        let inf = &infos[root];
        let anc = ancestors(&procs, inf.pid);
        let (host, window) = classify(inf.hwnd, &anc, &procs);
        out.push(Attachable {
            pid: inf.pid,
            name: procs
                .get(&inf.pid)
                .map(|p| p.name.clone())
                .unwrap_or_default(),
            host,
            hwnd: window,
            title: inf.title.clone(),
            ancestors: anc,
            error: inf.error.clone(),
        });
    }
    out.sort_by_key(|a| a.pid);
    Ok(out)
}
