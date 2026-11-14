//! Per-window UIA event mirror: counts StructureChanged and PropertyChanged events so
//! settle and watch can wait on them instead of re-walking the window.
//!
//! Delivery is lossy and provider dependent, so a window is only trusted once it has
//! delivered an event and has not been marked silent; everything else must poll.

use super::raw::automation;
use crate::platform::windows::util::hwnd;
use crate::settle::Activity;
use crate::WindowId;
use ::windows::core::{implement, Ref};
use ::windows::Win32::System::Com::SAFEARRAY;
use ::windows::Win32::UI::Accessibility::*;
use ::windows::Win32::UI::WindowsAndMessaging::IsWindow;
use std::collections::HashMap;
use std::sync::{Arc, Condvar, LazyLock, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// A watch unused for this long is unregistered.
const IDLE_TTL: Duration = Duration::from_secs(60);

const PROPS: &[UIA_PROPERTY_ID] = &[
    UIA_NamePropertyId,
    UIA_ValueValuePropertyId,
    UIA_IsEnabledPropertyId,
    UIA_ToggleToggleStatePropertyId,
    UIA_IsOffscreenPropertyId,
];

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Default)]
struct State {
    count: u64,
    live: bool,
    silent: bool,
}

#[derive(Default)]
struct Shared {
    st: Mutex<State>,
    cv: Condvar,
}

impl Shared {
    fn bump(&self) {
        let mut s = lock(&self.st);
        s.count += 1;
        s.live = true;
        s.silent = false;
        drop(s);
        self.cv.notify_all();
    }
}

#[implement(IUIAutomationStructureChangedEventHandler)]
struct Structure(Arc<Shared>);

impl IUIAutomationStructureChangedEventHandler_Impl for Structure_Impl {
    fn HandleStructureChangedEvent(
        &self,
        _sender: Ref<IUIAutomationElement>,
        _change: StructureChangeType,
        _runtime_id: *const SAFEARRAY,
    ) -> ::windows::core::Result<()> {
        self.0.bump();
        Ok(())
    }
}

#[implement(IUIAutomationPropertyChangedEventHandler)]
struct Property(Arc<Shared>);

impl IUIAutomationPropertyChangedEventHandler_Impl for Property_Impl {
    fn HandlePropertyChangedEvent(
        &self,
        _sender: Ref<IUIAutomationElement>,
        _property: UIA_PROPERTY_ID,
        _value: &::windows::Win32::System::Variant::VARIANT,
    ) -> ::windows::core::Result<()> {
        self.0.bump();
        Ok(())
    }
}

/// Registered handlers; unregistered on drop, which never happens inside a callback
/// (handlers only touch `Shared`).
struct Reg {
    uia: IUIAutomation,
    root: IUIAutomationElement,
    structure: IUIAutomationStructureChangedEventHandler,
    property: IUIAutomationPropertyChangedEventHandler,
}

// SAFETY: created and used in the process MTA (see `com()`), where UIA client objects
// are free-threaded.
unsafe impl Send for Reg {}

impl Reg {
    fn new(window: WindowId, shared: &Arc<Shared>) -> Option<Reg> {
        let uia = automation().ok()?;
        let none = None::<&IUIAutomationCacheRequest>;
        unsafe {
            let root = uia.ElementFromHandle(hwnd(window)).ok()?;
            let structure: IUIAutomationStructureChangedEventHandler =
                Structure(shared.clone()).into();
            let property: IUIAutomationPropertyChangedEventHandler =
                Property(shared.clone()).into();
            uia.AddStructureChangedEventHandler(&root, TreeScope_Subtree, none, &structure)
                .ok()?;
            let reg = Reg {
                uia,
                root,
                structure,
                property,
            };
            // On failure `reg` drops here and removes the structure handler.
            reg.uia
                .AddPropertyChangedEventHandlerNativeArray(
                    &reg.root,
                    TreeScope_Subtree,
                    none,
                    &reg.property,
                    PROPS,
                )
                .ok()?;
            Some(reg)
        }
    }
}

impl Drop for Reg {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .uia
                .RemoveStructureChangedEventHandler(&self.root, &self.structure);
            let _ = self
                .uia
                .RemovePropertyChangedEventHandler(&self.root, &self.property);
        }
    }
}

struct Watch {
    shared: Arc<Shared>,
    used: Instant,
    // None when registration failed: the window then never goes live and callers poll.
    _reg: Option<Reg>,
}

static WATCHES: LazyLock<Mutex<HashMap<WindowId, Watch>>> = LazyLock::new(Default::default);

fn alive(id: WindowId) -> bool {
    unsafe { IsWindow(Some(hwnd(id))) }.as_bool()
}

/// The window's shared state, registering its handlers on first use. Also drops watches
/// of other windows that are gone or idle.
fn shared(window: WindowId) -> Arc<Shared> {
    let (stale, found) = {
        let mut m = lock(&WATCHES);
        let dead: Vec<WindowId> = m
            .iter()
            .filter(|(&id, w)| id != window && (w.used.elapsed() > IDLE_TTL || !alive(id)))
            .map(|(&id, _)| id)
            .collect();
        let stale: Vec<Watch> = dead.iter().filter_map(|id| m.remove(id)).collect();
        let found = m.get_mut(&window).map(|w| {
            w.used = Instant::now();
            w.shared.clone()
        });
        (stale, found)
    };
    drop(stale);
    if let Some(s) = found {
        return s;
    }
    let shared = Arc::new(Shared::default());
    let reg = Reg::new(window, &shared);
    let mut m = lock(&WATCHES);
    if let Some(w) = m.get(&window) {
        return w.shared.clone();
    }
    m.insert(
        window,
        Watch {
            shared: shared.clone(),
            used: Instant::now(),
            _reg: reg,
        },
    );
    shared
}

/// Blocks until `window` has delivered more than `since` events or `max` passes.
/// Does not block when events are unusable (`live` false); the caller then polls.
/// `since = u64::MAX` with a zero `max` just reads the current count.
pub fn wait_activity(window: WindowId, since: u64, max: Duration) -> Activity {
    let s = shared(window);
    let st = lock(&s.st);
    if !st.live || st.silent {
        return Activity {
            count: st.count,
            changed: st.count > since,
            live: false,
        };
    }
    let (st, _) = s
        .cv
        .wait_timeout_while(st, max, |st| st.count <= since)
        .unwrap_or_else(PoisonError::into_inner);
    Activity {
        count: st.count,
        changed: st.count > since,
        live: true,
    }
}

/// The window changed without delivering events; poll until an event arrives again.
pub fn mark_silent(window: WindowId) {
    if let Some(w) = lock(&WATCHES).get(&window) {
        lock(&w.shared.st).silent = true;
    }
}
