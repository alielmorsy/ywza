//! Spawns its OWN console window and drives it through the helper. Never touches other consoles.
#![cfg(windows)]

use std::time::{Duration, Instant};
use windows::core::PWSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Console::SetConsoleCtrlHandler;
use windows::Win32::System::Threading::{
    CreateProcessW, TerminateProcess, CREATE_NEW_CONSOLE, PROCESS_INFORMATION, STARTUPINFOW,
};
use ywza_term::{attached_keys, attached_read, attached_write, list_attachable, set_helper_path};

fn wait_for(pid: u32, needle: &str) -> String {
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        let text = attached_read(pid, 100).unwrap_or_default();
        if text.contains(needle) || Instant::now() > end {
            return text;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn count(text: &str, line: &str) -> usize {
    text.lines().filter(|l| l.trim() == line).count()
}

#[test]
#[ignore = "opens a visible console window"]
fn own_console_roundtrip() {
    set_helper_path(env!("CARGO_BIN_EXE_ywza-term-helper"));
    // No STARTF_USESTDHANDLES: the child must use its new console, not our pipes.
    let mut cmdline: Vec<u16> = "cmd.exe /k title ywza-attach-test"
        .encode_utf16()
        .chain([0])
        .collect();
    let si = STARTUPINFOW {
        cb: size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut pi = PROCESS_INFORMATION::default();
    unsafe {
        // A parent that ignores ctrl+c passes that on to its children; start from the default.
        let _ = SetConsoleCtrlHandler(None, false);
        CreateProcessW(
            None,
            Some(PWSTR(cmdline.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_NEW_CONSOLE,
            None,
            None,
            &si,
            &mut pi,
        )
        .unwrap();
    }
    let pid = pi.dwProcessId;
    let result = std::panic::catch_unwind(|| {
        wait_for(pid, ">");

        let list = list_attachable().unwrap();
        let me = list
            .iter()
            .find(|a| a.pid == pid)
            .expect("own console not listed");
        println!("listed: {me:?}");
        assert_eq!(me.title.trim(), "ywza-attach-test");
        assert!(me.hwnd.is_some(), "window not mapped");

        attached_write(pid, "echo ywza-attach-one", true).unwrap();
        let t = wait_for(pid, "ywza-attach-one");
        assert_eq!(count(&t, "ywza-attach-one"), 1, "got {t:?}");

        attached_keys(pid, "up enter").unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        let mut t = String::new();
        while count(&t, "ywza-attach-one") < 2 && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(200));
            t = attached_read(pid, 100).unwrap();
        }
        assert_eq!(count(&t, "ywza-attach-one"), 2, "up+enter failed: {t:?}");

        attached_write(pid, "ping -n 30 127.0.0.1", true).unwrap();
        wait_for(pid, "Reply from");
        attached_keys(pid, "ctrl+c").unwrap();
        let t = wait_for(pid, "Control-C");
        assert!(t.contains("Control-C"), "ctrl+c did not interrupt: {t:?}");

        attached_write(pid, "echo h\u{e9}llo-\u{4e16}", true).unwrap();
        let t = wait_for(pid, "h\u{e9}llo");
        assert!(t.contains("h\u{e9}llo"), "unicode lost: {t:?}");
    });
    let _ = attached_write(pid, "exit", true);
    std::thread::sleep(Duration::from_millis(500));
    unsafe {
        let _ = TerminateProcess(pi.hProcess, 0);
        let _ = CloseHandle(pi.hProcess);
        let _ = CloseHandle(pi.hThread);
    }
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

/// Forces a classic conhost window (not Windows Terminal) and checks mapping + roundtrip.
#[test]
#[ignore = "opens a visible console window"]
fn classic_conhost_roundtrip() {
    set_helper_path(env!("CARGO_BIN_EXE_ywza-term-helper"));
    let mut cmdline: Vec<u16> = "conhost.exe cmd.exe /k title ywza-classic-test"
        .encode_utf16()
        .chain([0])
        .collect();
    let si = STARTUPINFOW {
        cb: size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut pi = PROCESS_INFORMATION::default();
    unsafe {
        let _ = SetConsoleCtrlHandler(None, false);
        CreateProcessW(
            None,
            Some(PWSTR(cmdline.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_NEW_CONSOLE,
            None,
            None,
            &si,
            &mut pi,
        )
        .unwrap();
    }
    let mut pid = 0;
    let result = std::panic::catch_unwind(|| {
        let end = Instant::now() + Duration::from_secs(15);
        let me = loop {
            let list = list_attachable().unwrap();
            if let Some(a) = list
                .into_iter()
                .find(|a| a.title.trim() == "ywza-classic-test")
            {
                break a;
            }
            assert!(Instant::now() < end, "classic console not listed");
            std::thread::sleep(Duration::from_millis(300));
        };
        println!("listed: {me:?}");
        assert_eq!(me.host, ywza_term::Host::Conhost);
        attached_write(me.pid, "echo ywza-classic-one", true).unwrap();
        let t = wait_for(me.pid, "ywza-classic-one");
        assert_eq!(count(&t, "ywza-classic-one"), 1, "got {t:?}");
        me.pid
    });
    if let Ok(p) = &result {
        pid = *p;
    }
    if pid != 0 {
        let _ = attached_write(pid, "exit", true);
    }
    std::thread::sleep(Duration::from_millis(500));
    unsafe {
        let _ = TerminateProcess(pi.hProcess, 0);
        let _ = CloseHandle(pi.hProcess);
        let _ = CloseHandle(pi.hThread);
    }
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

/// Read-only: listing existing consoles must not fail and must use the unified line format.
#[test]
fn unified_listing() {
    set_helper_path(env!("CARGO_BIN_EXE_ywza-term-helper"));
    let m = ywza_term::TermManager::new();
    let id = m.start("cmd", None).unwrap();
    let text = m.list_text();
    println!("{text}");
    assert!(text
        .lines()
        .any(|l| l.starts_with(&format!("[{id}] cmd (owned, running)"))));
    assert!(text.lines().all(|l| l.starts_with("[t")));
}
