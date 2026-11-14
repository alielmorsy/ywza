use super::ctx::LaunchCtx;
use super::shell::{handler_exes, shell_execute};
use super::target::{is_self, resolve, Target};
use crate::platform::windows::wm;
use crate::{Error, Launched, Result};
use std::path::Path;

/// The executable `launch` would start for `program`, without starting it. None for
/// documents, URIs and folders (the shell picks their handler) and unknown names.
pub fn find_program(program: &str) -> Option<String> {
    match resolve(program, None)? {
        Target::Exe(p) => Some(p.to_string_lossy().into_owned()),
        Target::Shell(_) => None,
    }
}

/// Starts a program, document or URI. Executables are resolved through PATH and
/// App Paths (skipping our own folder) and spawned directly; anything else goes
/// through ShellExecuteEx. For shell targets `Launched::pid` is the started process
/// when the shell reports one, else 0 (an existing process handled it, so `wait_for_window`
/// accepts only new, newly titled or newly foreground windows of the shell handler).
pub fn launch(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<Launched> {
    let target =
        resolve(program, cwd).ok_or_else(|| Error::NotFound(format!("program {program}")))?;
    let mut ctx = LaunchCtx::capture()?;
    ctx.exes = handler_exes(&target);
    let pid = match target {
        Target::Exe(path) => {
            if is_self(&path) {
                return Err(Error::Os(format!(
                    "refusing to launch ourselves ({})",
                    path.display()
                )));
            }
            let mut cmd = std::process::Command::new(&path);
            cmd.args(args);
            if let Some(dir) = cwd {
                cmd.current_dir(dir);
            }
            cmd.spawn()
                .map_err(|e| Error::Os(format!("launch {}: {e}", path.display())))?
                .id()
        }
        Target::Shell(file) => shell_execute(&file, args, cwd)?,
    };
    if pid != 0 {
        // A stub (app execution alias) may differ in name from the app it starts.
        let name = wm::process_name(pid).to_lowercase();
        if name != "?" && !ctx.exes.is_empty() && !ctx.exes.contains(&name) {
            ctx.exes.push(name);
        }
    }
    Ok(Launched {
        pid,
        before: ctx.register(),
    })
}
