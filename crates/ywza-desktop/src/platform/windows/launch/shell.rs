use super::target::{ext, is_uri, Target};
use crate::platform::windows::util::com;
use crate::Result;
use ::windows::core::{HSTRING, PCWSTR, PWSTR};
use ::windows::Win32::Foundation::CloseHandle;
use ::windows::Win32::System::Threading::GetProcessId;
use ::windows::Win32::UI::Shell::{
    AssocQueryStringW, ShellExecuteExW, ASSOCF, ASSOCSTR_EXECUTABLE, SEE_MASK_FLAG_NO_UI,
    SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
};
use ::windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use std::path::Path;

/// Lowercase exe names whose windows belong to this launch when the launcher hands off to a
/// running instance: the program itself, or the shell handler of a document, folder or URI.
/// Empty when unknown (scripts, handlers that cannot be resolved).
pub(super) fn handler_exes(target: &Target) -> Vec<String> {
    let name = |p: &str| {
        Path::new(p)
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
    };
    match target {
        Target::Exe(p) => ext(p)
            .is_some_and(|e| e == "exe" || e == "com")
            .then(|| name(&p.to_string_lossy()))
            .flatten()
            .into_iter()
            .collect(),
        Target::Shell(s) if Path::new(s).is_dir() => vec!["explorer.exe".into()],
        Target::Shell(s) => {
            let key = if is_uri(s) {
                s.split(':').next().unwrap_or_default().to_string()
            } else {
                ext(Path::new(s))
                    .map(|e| format!(".{e}"))
                    .unwrap_or_default()
            };
            let mut v: Vec<String> = assoc_exe(&key).and_then(|p| name(&p)).into_iter().collect();
            if key.starts_with("ms-") {
                // Settings and other packaged apps show their window in the frame host.
                v.extend([
                    "applicationframehost.exe".into(),
                    "systemsettings.exe".into(),
                ]);
            }
            v
        }
    }
}

/// The program registered for a file extension (".txt") or URI scheme ("https").
fn assoc_exe(assoc: &str) -> Option<String> {
    if assoc.is_empty() {
        return None;
    }
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    let hr = unsafe {
        AssocQueryStringW(
            ASSOCF(0),
            ASSOCSTR_EXECUTABLE,
            &HSTRING::from(assoc),
            PCWSTR::null(),
            Some(PWSTR(buf.as_mut_ptr())),
            &mut len,
        )
    };
    if hr.is_err() {
        return None;
    }
    let s = String::from_utf16_lossy(&buf[..(len as usize).saturating_sub(1).min(buf.len())]);
    (!s.is_empty()).then_some(s)
}

pub(super) fn shell_execute(file: &str, args: &[&str], cwd: Option<&Path>) -> Result<u32> {
    com();
    let file = HSTRING::from(file);
    let params = HSTRING::from(args.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" "));
    let dir = cwd.map(|d| HSTRING::from(d.as_os_str()));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI | SEE_MASK_NOASYNC,
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: if args.is_empty() {
            PCWSTR::null()
        } else {
            PCWSTR(params.as_ptr())
        },
        lpDirectory: dir.as_ref().map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr())),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info)? };
    if info.hProcess.is_invalid() {
        return Ok(0);
    }
    let pid = unsafe { GetProcessId(info.hProcess) };
    let _ = unsafe { CloseHandle(info.hProcess) };
    Ok(pid)
}

pub(super) fn quote(arg: &str) -> String {
    if arg.is_empty() || arg.contains([' ', '\t', '"']) {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        arg.to_string()
    }
}
