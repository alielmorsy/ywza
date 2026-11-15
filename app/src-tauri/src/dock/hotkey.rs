//! Global shortcut for the dock: `RegisterHotKey` on its own message-loop thread (Windows only).

pub const DEFAULT: &str = "alt+backquote";
pub const TAKEN: &str = "That shortcut is taken by another app";

const MOD_ALT: u32 = 0x1;
const MOD_CONTROL: u32 = 0x2;
const MOD_NOREPEAT: u32 = 0x4000;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Key {
    ScanBackquote,
    Space,
}

pub fn parse_shortcut(key: &str) -> Option<(u32, Key)> {
    match key {
        "alt+backquote" => Some((MOD_ALT | MOD_NOREPEAT, Key::ScanBackquote)),
        "ctrl+alt+space" => Some((MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, Key::Space)),
        "alt+space" => Some((MOD_ALT | MOD_NOREPEAT, Key::Space)),
        _ => None,
    }
}

#[cfg(windows)]
pub use win::Hotkey;

#[cfg(not(windows))]
pub use other::Hotkey;

#[cfg(windows)]
mod win {
    use super::{parse_shortcut, Key, DEFAULT, TAKEN};
    use crate::dock::target;
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::time::Duration;
    use tauri::{AppHandle, Emitter};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        MapVirtualKeyW, RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MAPVK_VSC_TO_VK_EX,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetMessageW, PeekMessageW, PostThreadMessageW, MSG, PM_NOREMOVE, WM_APP, WM_HOTKEY,
        WM_USER,
    };

    const ID: i32 = 1;
    const VK_SPACE: u32 = 0x20;
    const SC_BACKQUOTE: u32 = 0x29;

    enum Cmd {
        Set(String, Sender<Result<(), String>>),
        Refresh,
    }

    pub struct Hotkey {
        tx: Sender<Cmd>,
        thread: u32,
    }

    impl Hotkey {
        pub fn start(app: AppHandle) -> Hotkey {
            let (tx, rx) = channel();
            let (ready_tx, ready_rx) = channel();
            std::thread::spawn(move || run(app, rx, ready_tx));
            let thread = ready_rx.recv().unwrap_or(0);
            Hotkey { tx, thread }
        }

        pub fn set(&self, key: &str) -> Result<(), String> {
            parse_shortcut(key).ok_or_else(|| "Unknown shortcut".to_string())?;
            let (reply, done) = channel();
            self.send(Cmd::Set(key.to_string(), reply))?;
            done.recv_timeout(Duration::from_secs(2))
                .map_err(|e| e.to_string())?
        }

        /// The layout may have changed since the key was resolved; resolve it again.
        pub fn refresh(&self) {
            let _ = self.send(Cmd::Refresh);
        }

        fn send(&self, cmd: Cmd) -> Result<(), String> {
            self.tx.send(cmd).map_err(|e| e.to_string())?;
            unsafe { PostThreadMessageW(self.thread, WM_APP, Default::default(), Default::default()) }
                .map_err(|e| e.to_string())
        }
    }

    fn register(key: &str) -> Result<(), String> {
        let (mods, k) = parse_shortcut(key).ok_or_else(|| "Unknown shortcut".to_string())?;
        let vk = match k {
            Key::Space => VK_SPACE,
            Key::ScanBackquote => unsafe { MapVirtualKeyW(SC_BACKQUOTE, MAPVK_VSC_TO_VK_EX) },
        };
        if vk == 0 {
            return Err("This keyboard layout has no key for that shortcut".to_string());
        }
        unsafe {
            let _ = UnregisterHotKey(None, ID);
            RegisterHotKey(None, ID, HOT_KEY_MODIFIERS(mods), vk)
        }
        .map_err(|_| TAKEN.to_string())
    }

    fn run(app: AppHandle, rx: Receiver<Cmd>, ready: Sender<u32>) {
        let mut msg = MSG::default();
        // Creates this thread's message queue so posted messages are not lost.
        unsafe {
            let _ = PeekMessageW(&mut msg, None, WM_USER, WM_USER, PM_NOREMOVE);
            let _ = ready.send(GetCurrentThreadId());
        }
        let mut current = DEFAULT.to_string();
        let _ = register(&current);
        while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
            match msg.message {
                WM_HOTKEY => {
                    target::remember();
                    let _ = app.emit_to("dock", "dock-hotkey", ());
                }
                WM_APP => {
                    while let Ok(cmd) = rx.try_recv() {
                        match cmd {
                            Cmd::Set(key, reply) => {
                                let r = register(&key);
                                if r.is_ok() {
                                    current = key;
                                } else {
                                    let _ = register(&current);
                                }
                                let _ = reply.send(r);
                            }
                            Cmd::Refresh => {
                                let _ = register(&current);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(not(windows))]
mod other {
    use super::parse_shortcut;
    use tauri::AppHandle;

    pub struct Hotkey;

    impl Hotkey {
        pub fn start(_app: AppHandle) -> Hotkey {
            Hotkey
        }

        pub fn set(&self, key: &str) -> Result<(), String> {
            parse_shortcut(key)
                .map(|_| ())
                .ok_or_else(|| "Unknown shortcut".to_string())
        }

        pub fn refresh(&self) {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts() {
        assert_eq!(
            parse_shortcut("alt+backquote"),
            Some((MOD_ALT | MOD_NOREPEAT, Key::ScanBackquote))
        );
        assert_eq!(
            parse_shortcut("ctrl+alt+space"),
            Some((MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, Key::Space))
        );
        assert_eq!(
            parse_shortcut("alt+space"),
            Some((MOD_ALT | MOD_NOREPEAT, Key::Space))
        );
        assert_eq!(parse_shortcut("nope"), None);
    }
}
