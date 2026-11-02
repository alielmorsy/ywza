use crate::platform::windows::util::window_id;
use crate::platform::windows::wm;
use crate::{Result, WindowId, WindowInfo};
use ::windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// What `launch` knew before starting a program; decides which window is its own.
#[derive(Debug, Clone, Default)]
pub(super) struct LaunchCtx {
    /// Windows that existed before, with their titles (`None`: title unknown, never "newly titled").
    pub(super) before: HashMap<WindowId, Option<String>>,
    /// Foreground window before the launch.
    pub(super) fg: Option<WindowId>,
    /// Windows that were hidden (closed to the tray) before; showing one counts as new.
    pub(super) hidden: Vec<WindowId>,
    /// Lowercase exe names of the program, or of the shell handler of a document/URI.
    /// Empty: unknown, so a hand-off accepts any new window.
    pub exes: Vec<String>,
}

/// Registry of contexts, so `Launched` can stay `{ pid, before }`: the context key rides
/// in `before` as one extra id with the top bit set (never a real HWND).
static CTXS: Mutex<Vec<(WindowId, LaunchCtx)>> = Mutex::new(Vec::new());
pub(super) const KEY_BIT: WindowId = 1 << 63;

impl LaunchCtx {
    pub fn capture() -> Result<Self> {
        let wins = wm::list_windows()?;
        let hidden = wins.iter().filter(|w| w.hidden).map(|w| w.id).collect();
        let before = wins.into_iter().map(|w| (w.id, Some(w.title))).collect();
        Ok(LaunchCtx {
            before,
            fg: foreground_id(),
            hidden,
            exes: Vec::new(),
        })
    }

    /// Stores the context; the result is `Launched::before`.
    pub fn register(self) -> Vec<WindowId> {
        static SEQ: AtomicU64 = AtomicU64::new(1);
        let key = KEY_BIT | SEQ.fetch_add(1, Ordering::Relaxed);
        let mut ids: Vec<WindowId> = self.before.keys().copied().collect();
        ids.push(key);
        let mut all = CTXS.lock().unwrap();
        all.push((key, self));
        if all.len() > 32 {
            all.remove(0);
        }
        ids
    }

    pub(super) fn lookup(before: &[WindowId]) -> Self {
        let found = before.iter().find(|&&i| i & KEY_BIT != 0).and_then(|k| {
            CTXS.lock()
                .unwrap()
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, c)| c.clone())
        });
        // Evicted: ids only, nothing known about titles, handler or foreground.
        found.unwrap_or_else(|| LaunchCtx {
            before: before.iter().map(|&i| (i, None)).collect(),
            ..LaunchCtx::default()
        })
    }

    /// New, shown after being hidden, or existing with a different title than before the launch.
    pub(super) fn fresh(&self, w: &WindowInfo) -> bool {
        if self.hidden.contains(&w.id) && !w.hidden {
            return true;
        }
        match self.before.get(&w.id) {
            None => true,
            Some(None) => false,
            Some(Some(t)) => *t != w.title,
        }
    }

    pub(super) fn handler(&self, w: &WindowInfo) -> bool {
        let p = w.process.to_lowercase();
        self.exes.contains(&p)
    }
}

pub(super) fn foreground_id() -> Option<WindowId> {
    let h = unsafe { GetForegroundWindow() };
    (!h.0.is_null()).then(|| window_id(h))
}
