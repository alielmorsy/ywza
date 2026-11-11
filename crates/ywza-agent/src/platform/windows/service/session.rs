//! The active console session and the agent started into it.

use std::path::Path;

use anyhow::Context;
use windows::{
    core::{Owned, PCWSTR, PWSTR},
    Win32::{
        Foundation::{HANDLE, WAIT_OBJECT_0},
        Security::{
            DuplicateTokenEx, GetTokenInformation, SecurityImpersonation, SetTokenInformation,
            TokenElevationType, TokenElevationTypeLimited, TokenLinkedToken, TokenPrimary,
            TokenSessionId, TOKEN_ALL_ACCESS, TOKEN_ELEVATION_TYPE, TOKEN_LINKED_TOKEN,
        },
        System::{
            Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock},
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
                SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK,
            },
            RemoteDesktop::{
                WTSDomainName, WTSFreeMemory, WTSGetActiveConsoleSessionId,
                WTSQuerySessionInformationW, WTSQueryUserToken, WTSUserName, WTS_INFO_CLASS,
            },
            Threading::{
                CreateProcessAsUserW, GetCurrentProcess, OpenProcessToken, ResumeThread,
                TerminateProcess, WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED,
                CREATE_UNICODE_ENVIRONMENT, PROCESS_INFORMATION, STARTUPINFOW,
            },
        },
    },
};

/// What the console shows: a user's session (`DOMAIN\name`) or the sign-in screen (`None`).
#[derive(Debug, PartialEq)]
pub struct Target {
    pub id: u32,
    pub user: Option<String>,
}

pub fn active() -> Option<Target> {
    let id = unsafe { WTSGetActiveConsoleSessionId() };
    if id == u32::MAX {
        return None;
    }
    let user = query(id, WTSUserName)
        .filter(|u| !u.is_empty())
        .map(|u| match query(id, WTSDomainName) {
            Some(d) if !d.is_empty() => format!(r"{d}\{u}"),
            _ => u,
        });
    Some(Target { id, user })
}

fn query(id: u32, class: WTS_INFO_CLASS) -> Option<String> {
    let mut buf = PWSTR::null();
    let mut len = 0;
    unsafe { WTSQuerySessionInformationW(None, id, class, &mut buf, &mut len) }.ok()?;
    let s = unsafe { buf.to_string() }.ok();
    unsafe { WTSFreeMemory(buf.0.cast()) };
    s
}

/// A running `ywza-agent run`. Dropping it ends the agent; so does the service dying, through
/// the job. Apps the agent starts leave the job and live on.
pub struct Agent {
    process: Owned<HANDLE>,
    _job: Owned<HANDLE>,
}

impl Agent {
    pub fn exited(&self) -> bool {
        let state = unsafe { WaitForSingleObject(*self.process, 0) };
        state == WAIT_OBJECT_0
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        unsafe {
            let _ = TerminateProcess(*self.process, 1);
            WaitForSingleObject(*self.process, 5000);
        }
    }
}

pub fn spawn(
    exe: &Path,
    t: &Target,
    config_dir: &Path,
    verbosity: Option<&str>,
    log_file: Option<&Path>,
) -> anyhow::Result<Agent> {
    let (token, desktop) = match t.user {
        Some(_) => (user_token(t.id)?, r"winsta0\default"),
        None => (system_token(t.id)?, r"winsta0\winlogon"),
    };
    let mut vars = environment(*token)?;
    vars.retain(|v| !v.to_ascii_uppercase().starts_with("YWZA_CONFIG_DIR="));
    vars.push(format!("YWZA_CONFIG_DIR={}", config_dir.display()));
    if let Ok(v) = std::env::var("RUST_LOG") {
        vars.retain(|v| !v.to_ascii_uppercase().starts_with("RUST_LOG="));
        vars.push(format!("RUST_LOG={v}"));
    }
    let cwd = vars
        .iter()
        .find_map(|v| v.strip_prefix("USERPROFILE="))
        .filter(|p| Path::new(p).is_dir())
        .map(wide);
    let env = env_block(&vars);
    let mut line = wide(&command_line(exe, verbosity, log_file));
    let mut desktop = wide(desktop);

    unsafe {
        let job = Owned::new(CreateJobObjectW(None, PCWSTR::null())?);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK;
        SetInformationJobObject(
            *job,
            JobObjectExtendedLimitInformation,
            (&raw const limits).cast(),
            size_of_val(&limits) as u32,
        )?;
        let mut si: STARTUPINFOW = std::mem::zeroed();
        si.cb = size_of::<STARTUPINFOW>() as u32;
        si.lpDesktop = PWSTR(desktop.as_mut_ptr());
        let mut pi = PROCESS_INFORMATION::default();
        CreateProcessAsUserW(
            Some(*token),
            PCWSTR::null(),
            Some(PWSTR(line.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_UNICODE_ENVIRONMENT | CREATE_NO_WINDOW | CREATE_SUSPENDED,
            Some(env.as_ptr().cast()),
            cwd.as_ref().map_or(PCWSTR::null(), |c| PCWSTR(c.as_ptr())),
            &si,
            &mut pi,
        )
        .context("starting ywza-agent run")?;
        let process = Owned::new(pi.hProcess);
        let thread = Owned::new(pi.hThread);
        // Suspended until it is in the job, so nothing it starts early escapes the kill.
        if let Err(e) = AssignProcessToJobObject(*job, *process) {
            let _ = TerminateProcess(*process, 1);
            return Err(e).context("assigning the agent to its job");
        }
        ResumeThread(*thread);
        Ok(Agent { process, _job: job })
    }
}

/// The session user's token, elevated when the user is an admin under UAC, so the agent can
/// drive elevated windows too. Other users get their normal token.
fn user_token(session: u32) -> anyhow::Result<Owned<HANDLE>> {
    unsafe {
        let mut h = HANDLE::default();
        WTSQueryUserToken(session, &mut h).context("no token for the session's user")?;
        let token = Owned::new(h);
        let mut kind = TOKEN_ELEVATION_TYPE::default();
        let mut len = 0;
        GetTokenInformation(
            *token,
            TokenElevationType,
            Some((&raw mut kind).cast()),
            size_of_val(&kind) as u32,
            &mut len,
        )?;
        if kind != TokenElevationTypeLimited {
            return Ok(token);
        }
        let mut linked = TOKEN_LINKED_TOKEN::default();
        GetTokenInformation(
            *token,
            TokenLinkedToken,
            Some((&raw mut linked).cast()),
            size_of_val(&linked) as u32,
            &mut len,
        )
        .context("getting the user's elevated token")?;
        Ok(Owned::new(linked.LinkedToken))
    }
}

/// The service's own SYSTEM token, moved to `session`: what runs on the sign-in screen.
fn system_token(session: u32) -> anyhow::Result<Owned<HANDLE>> {
    unsafe {
        let mut own = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_ALL_ACCESS, &mut own)?;
        let own = Owned::new(own);
        let mut dup = HANDLE::default();
        DuplicateTokenEx(
            *own,
            TOKEN_ALL_ACCESS,
            None,
            SecurityImpersonation,
            TokenPrimary,
            &mut dup,
        )?;
        let dup = Owned::new(dup);
        SetTokenInformation(
            *dup,
            TokenSessionId,
            (&raw const session).cast(),
            size_of::<u32>() as u32,
        )?;
        Ok(dup)
    }
}

/// The token user's default environment, as `NAME=value` entries.
fn environment(token: HANDLE) -> anyhow::Result<Vec<String>> {
    unsafe {
        let mut block = std::ptr::null_mut();
        CreateEnvironmentBlock(&mut block, Some(token), false)?;
        let mut vars = Vec::new();
        let mut p = block as *const u16;
        loop {
            let len = (0..).take_while(|&i| *p.add(i) != 0).count();
            if len == 0 {
                break;
            }
            vars.push(String::from_utf16_lossy(std::slice::from_raw_parts(p, len)));
            p = p.add(len + 1);
        }
        let _ = DestroyEnvironmentBlock(block);
        Ok(vars)
    }
}

fn env_block(vars: &[String]) -> Vec<u16> {
    let mut b: Vec<u16> = vars
        .iter()
        .flat_map(|v| v.encode_utf16().chain([0]))
        .collect();
    b.push(0);
    b
}

/// `verbosity` and `log_file` become the agent's own `-v`/`-vv`/`-q` and `--log-file`.
fn command_line(exe: &Path, verbosity: Option<&str>, log_file: Option<&Path>) -> String {
    let mut line = format!("\"{}\"", exe.display());
    match verbosity {
        Some("debug") => line += " -v",
        Some("trace") => line += " -vv",
        Some("warn") => line += " -q",
        _ => {}
    }
    if let Some(f) = log_file {
        line += &format!(" --log-file \"{}\"", f.display());
    }
    line + " run"
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_agent_command_line() {
        let exe = Path::new(r"C:\Program Files\ywza\ywza-agent.exe");
        assert_eq!(
            command_line(exe, None, None),
            r#""C:\Program Files\ywza\ywza-agent.exe" run"#
        );
        assert_eq!(
            command_line(exe, Some("trace"), Some(Path::new(r"C:\ProgramData\ywza\agent.log"))),
            r#""C:\Program Files\ywza\ywza-agent.exe" -vv --log-file "C:\ProgramData\ywza\agent.log" run"#
        );
    }

    #[test]
    fn env_block_is_double_nul_terminated() {
        let b = env_block(&["A=1".into(), "B=2".into()]);
        assert_eq!(String::from_utf16_lossy(&b), "A=1\0B=2\0\0");
    }
}
