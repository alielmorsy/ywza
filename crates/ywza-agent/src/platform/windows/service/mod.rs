//! `ywza-agent service` (Windows): run by the SCM as LocalSystem; the installer registers it.
//!
//! Session 0 has no desktop, so the service keeps one `ywza-agent run` in the active console
//! session: as the signed-in user on `winsta0\default`, or as SYSTEM on `winsta0\winlogon` when
//! nobody is signed in, so a paired client can sign in remotely.
//!
//! All of them share one machine identity in `%ProgramData%\ywza` (`YWZA_CONFIG_DIR`). SYSTEM
//! and Administrators keep it; the served user gets access only while their agent runs.

mod acl;
mod session;

use std::{
    ffi::c_void,
    path::{Path, PathBuf},
    sync::{mpsc, Mutex, OnceLock},
    time::Duration,
};

use windows::{
    core::{w, PCWSTR, PWSTR},
    Win32::System::Services::*,
};

const NAME: PCWSTR = w!("ywza-agent");

struct Opts {
    verbosity: Option<String>,
    log_file: Option<PathBuf>,
}

static STOP: OnceLock<mpsc::Sender<()>> = OnceLock::new();
static PENDING: Mutex<Option<(Opts, mpsc::Receiver<()>)>> = Mutex::new(None);

/// `verbosity` and `log_file` are the service's own flags, forwarded to every agent.
pub fn main(verbosity: Option<&str>, log_file: Option<&Path>) -> anyhow::Result<()> {
    let opts = Opts {
        verbosity: verbosity.map(String::from),
        log_file: log_file.map(std::path::absolute).transpose()?,
    };
    let (tx, rx) = mpsc::channel();
    let _ = STOP.set(tx);
    *PENDING.lock().unwrap() = Some((opts, rx));
    let table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: PWSTR(NAME.as_ptr().cast_mut()),
            lpServiceProc: Some(service_main),
        },
        SERVICE_TABLE_ENTRYW::default(),
    ];
    // Fails when not started by the SCM: run in the foreground (an elevated prompt, for debugging).
    if unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) }.is_err() {
        let (opts, rx) = PENDING.lock().unwrap().take().expect("set above");
        return follow(&opts, rx);
    }
    Ok(())
}

unsafe extern "system" fn service_main(_argc: u32, _argv: *mut PWSTR) {
    let Ok(handle) = (unsafe { RegisterServiceCtrlHandlerExW(NAME, Some(control), None) }) else {
        return;
    };
    let report = |state, accept, failed: bool| {
        let s = SERVICE_STATUS {
            dwServiceType: SERVICE_WIN32_OWN_PROCESS,
            dwCurrentState: state,
            dwControlsAccepted: accept,
            // ERROR_SERVICE_SPECIFIC_ERROR, so the SCM's recovery actions restart us.
            dwWin32ExitCode: if failed { 1066 } else { 0 },
            dwServiceSpecificExitCode: failed as u32,
            ..Default::default()
        };
        let _ = unsafe { SetServiceStatus(handle, &s) };
    };
    report(
        SERVICE_RUNNING,
        SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN,
        false,
    );
    let pending = PENDING.lock().unwrap().take();
    let result = pending.map_or(Ok(()), |(opts, rx)| follow(&opts, rx));
    if let Err(e) = &result {
        tracing::error!(error = %format!("{e:#}"), "service failed");
    }
    report(SERVICE_STOPPED, 0, result.is_err());
}

unsafe extern "system" fn control(ctl: u32, _: u32, _: *mut c_void, _: *mut c_void) -> u32 {
    match ctl {
        SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN => {
            if let Some(tx) = STOP.get() {
                let _ = tx.send(());
            }
            0
        }
        SERVICE_CONTROL_INTERROGATE => 0,
        _ => 120, // ERROR_CALL_NOT_IMPLEMENTED
    }
}

/// Keeps one agent on the active console session until `stop` fires.
fn follow(opts: &Opts, stop: mpsc::Receiver<()>) -> anyhow::Result<()> {
    let dir = acl::config_dir();
    acl::lock(&dir)?;
    let exe = std::env::current_exe()?;
    tracing::info!(dir = %dir.display(), "service following the console session");

    let mut cur: Option<(session::Target, session::Agent)> = None;
    let mut last_note = String::new();
    loop {
        let want = session::active();
        if let Some((t, agent)) = &cur {
            let exited = agent.exited();
            if exited || want.as_ref() != Some(t) {
                tracing::info!(session = t.id, user = ?t.user, exited, "service stopping the agent");
                cur = None;
                acl::lock(&dir)?;
            }
        }
        if let (None, Some(t)) = (&cur, want) {
            let started = acl::grant(&dir, t.user.as_deref()).and_then(|()| {
                session::spawn(
                    &exe,
                    &t,
                    &dir,
                    opts.verbosity.as_deref(),
                    opts.log_file.as_deref(),
                )
            });
            match started {
                Ok(agent) => {
                    tracing::info!(session = t.id, user = ?t.user, "service started an agent");
                    last_note.clear();
                    cur = Some((t, agent));
                }
                Err(e) => {
                    acl::lock(&dir)?;
                    // A session that is still signing in fails every tick: say it once.
                    let note = format!("session {}: {e:#}", t.id);
                    if note != last_note {
                        tracing::warn!("service could not start an agent: {note}");
                        last_note = note;
                    }
                }
            }
        }
        if !matches!(
            stop.recv_timeout(Duration::from_secs(2)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            break;
        }
    }
    tracing::info!("service stopping");
    drop(cur);
    acl::lock(&dir)
}
