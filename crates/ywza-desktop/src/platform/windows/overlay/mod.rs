//! Native lens overlay: one thread owns every mark window and animates them on a timer.
//! Callers queue commands over a channel and wake the thread with a thread message.

mod paint;
mod window;

use crate::lens::{alpha_at, mark_bounds, px, visible, Owner, FADE_IN_MS};
use crate::platform::windows::util::{from_wide, hwnd, window_id};
use crate::{MarkId, MarkKind, Phase, Rect, WindowId};
use ::windows::Win32::Foundation::{HANDLE, HWND, LPARAM, POINT, WPARAM};
use ::windows::Win32::Graphics::Dwm::DwmFlush;
use ::windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use ::windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, GetProcessWindowStation, GetUserObjectInformationW, OpenInputDesktop,
    DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS, UOI_NAME,
};
use ::windows::Win32::System::Threading::GetCurrentThreadId;
use ::windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use ::windows::Win32::UI::WindowsAndMessaging::*;
use paint::Look;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use window::{Font, Surface};

const WAKE: u32 = WM_APP + 1;
const RIPPLE_MS: u64 = 350;
const HIDE_WAIT: Duration = Duration::from_millis(200);

struct Spec {
    id: u64,
    /// Screen rect; for `Point`, the ripple's full square.
    target: Rect,
    caption: String,
    kind: MarkKind,
    scale: f32,
    top_room: bool,
}

enum Cmd {
    Show(Spec),
    Update(u64, Phase),
    Hide(Sender<()>),
    Restore,
}

struct Overlay {
    tx: Sender<Cmd>,
    thread: u32,
}

impl Overlay {
    /// True once queued; a failed wake only delays the command to the next one.
    fn send(&self, cmd: Cmd) -> bool {
        if self.tx.send(cmd).is_err() {
            return false;
        }
        let _ = unsafe { PostThreadMessageW(self.thread, WAKE, WPARAM(0), LPARAM(0)) };
        true
    }
}

static OVERLAY: OnceLock<Option<Overlay>> = OnceLock::new();
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn running() -> Option<&'static Overlay> {
    OVERLAY.get()?.as_ref()
}

pub fn overlay_mark(window: WindowId, rect: Rect, caption: &str, kind: MarkKind) -> Option<MarkId> {
    if !interactive_desktop() {
        return None;
    }
    let (cx, cy) = rect.center();
    let pt = POINT { x: cx, y: cy };
    let (scale, top) = unsafe {
        if !visible(owner(hwnd(window)), owner(WindowFromPoint(pt))) {
            return None;
        }
        let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        let (mut dx, mut dy) = (96, 96);
        let _ = GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let top = GetMonitorInfoW(mon, &mut info)
            .as_bool()
            .then_some(info.rcMonitor.top);
        (dx as f32 / 96.0, top)
    };
    let target = match kind {
        MarkKind::Element => rect,
        MarkKind::Point => {
            let r = px(28.0, scale);
            Rect::new(cx - r, cy - r, 2 * r, 2 * r)
        }
    };
    let top_room = top.is_none_or(|t| target.y - t >= px(36.0, scale));
    let overlay = OVERLAY.get_or_init(start).as_ref()?;
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let spec = Spec {
        id,
        target,
        caption: caption.to_string(),
        kind,
        scale,
        top_room,
    };
    overlay.send(Cmd::Show(spec)).then_some(MarkId(id))
}

pub fn overlay_update(id: MarkId, phase: Phase) {
    if let Some(o) = running() {
        o.send(Cmd::Update(id.0, phase));
    }
}

/// Hides every mark and waits (up to 200 ms) until it is off screen. True when a
/// matching `overlay_restore` is owed.
pub fn overlay_hide() -> bool {
    let Some(o) = running() else {
        return false;
    };
    let (tx, rx) = mpsc::channel();
    if !o.send(Cmd::Hide(tx)) {
        return false;
    }
    let _ = rx.recv_timeout(HIDE_WAIT);
    true
}

pub fn overlay_restore() {
    if let Some(o) = running() {
        o.send(Cmd::Restore);
    }
}

fn owner(h: HWND) -> Owner {
    unsafe {
        let root = GetAncestor(h, GA_ROOT);
        let mut pid = 0u32;
        GetWindowThreadProcessId(root, Some(&mut pid as *mut u32));
        Owner {
            root: window_id(root),
            root_owner: window_id(GetAncestor(h, GA_ROOTOWNER)),
            pid,
        }
    }
}

fn object_name(h: HANDLE) -> Option<String> {
    let mut name = [0u16; 64];
    unsafe {
        GetUserObjectInformationW(
            h,
            UOI_NAME,
            Some(name.as_mut_ptr() as *mut _),
            (name.len() * 2) as u32,
            None,
        )
        .ok()?;
    }
    Some(from_wide(&name))
}

/// Marks only make sense on the interactive user desktop (not Winlogon, not a service).
fn interactive_desktop() -> bool {
    unsafe {
        let Ok(desk) = OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS)
        else {
            return false;
        };
        let name = object_name(HANDLE(desk.0));
        let _ = CloseDesktop(desk);
        name.is_some_and(|n| n.eq_ignore_ascii_case("Default"))
    }
}

fn start() -> Option<Overlay> {
    // The process window station handle is not owned; it must not be closed.
    let station = unsafe { GetProcessWindowStation() }.ok()?;
    if !object_name(HANDLE(station.0))?.eq_ignore_ascii_case("WinSta0") {
        return None;
    }
    let (tx, rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("ywza-lens".into())
        .spawn(move || run(rx, ready_tx))
        .ok()?;
    let thread = ready_rx.recv_timeout(Duration::from_secs(2)).ok()??;
    Some(Overlay { tx, thread })
}

fn run(rx: Receiver<Cmd>, ready: SyncSender<Option<u32>>) {
    unsafe {
        // Creates the thread's message queue before anyone posts to it.
        let mut msg = MSG::default();
        let _ = PeekMessageW(&mut msg, None, WM_USER, WM_USER, PM_NOREMOVE);
    }
    if !window::register() {
        let _ = ready.send(None);
        return;
    }
    let _ = ready.send(Some(unsafe { GetCurrentThreadId() }));
    let mut st = State::default();
    loop {
        let mut msg = MSG::default();
        if unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 <= 0 {
            break;
        }
        match msg.message {
            WAKE => {
                while let Ok(cmd) = rx.try_recv() {
                    st.apply(cmd);
                }
            }
            WM_TIMER if msg.hwnd.is_invalid() => st.tick(),
            _ => unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            },
        }
        st.sync_timer();
    }
}

#[derive(Default)]
struct State {
    marks: Vec<Mark>,
    timer: usize,
    hidden: u32,
}

impl State {
    fn apply(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Show(spec) => {
                let now = Instant::now();
                if let Some(mut m) = Mark::new(spec, now) {
                    m.step(now, self.hidden > 0);
                    m.surface.show();
                    self.marks.push(m);
                }
            }
            Cmd::Update(id, phase) => {
                if phase == Phase::Acting {
                    return;
                }
                if let Some(m) = self
                    .marks
                    .iter_mut()
                    .find(|m| m.id == id && m.end.is_none())
                {
                    m.end = Some((phase, Instant::now()));
                }
            }
            Cmd::Hide(reply) => {
                self.hidden += 1;
                let mut any = false;
                for m in self.marks.iter_mut().filter(|m| m.alpha > 0) {
                    m.surface.set_alpha(0);
                    m.alpha = 0;
                    any = true;
                }
                if any {
                    let _ = unsafe { DwmFlush() };
                }
                let _ = reply.send(());
            }
            Cmd::Restore => {
                self.hidden = self.hidden.saturating_sub(1);
                self.tick();
            }
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let hidden = self.hidden > 0;
        self.marks.retain_mut(|m| m.step(now, hidden));
    }

    fn sync_timer(&mut self) {
        unsafe {
            if !self.marks.is_empty() && self.timer == 0 {
                self.timer = SetTimer(None, 0, 16, None);
            } else if self.marks.is_empty() && self.timer != 0 {
                let _ = KillTimer(None, self.timer);
                self.timer = 0;
            }
        }
    }
}

struct Mark {
    id: u64,
    surface: Surface,
    kind: MarkKind,
    /// Target and caption pill in surface pixels.
    target: Rect,
    pill: Rect,
    caption: Vec<u16>,
    scale: f32,
    shown: Instant,
    end: Option<(Phase, Instant)>,
    alpha: u8,
    drawn: Option<(Phase, f32)>,
}

impl Mark {
    fn new(spec: Spec, now: Instant) -> Option<Mark> {
        let s = spec.scale;
        let caption: Vec<u16> = spec.caption.encode_utf16().collect();
        let font = Font::new(s)?;
        let pill_w = if caption.is_empty() {
            0
        } else {
            font.width(&caption).min(px(360.0, s)) + 2 * px(10.0, s)
        };
        let pill_h = px(22.0, s);
        let gap = px(4.0, s);
        let left = px(8.0, s) - gap;
        let mut bounds = mark_bounds(spec.target, s, spec.top_room);
        bounds.w = bounds.w.max(left + pill_w);
        let target = Rect::new(
            spec.target.x - bounds.x,
            spec.target.y - bounds.y,
            spec.target.w,
            spec.target.h,
        );
        let pill_y = if spec.top_room {
            target.y - gap - px(4.0, s) - pill_h
        } else {
            target.y + target.h + gap + px(4.0, s)
        };
        Some(Mark {
            id: spec.id,
            surface: Surface::new(bounds, font)?,
            kind: spec.kind,
            target,
            pill: Rect::new(left, pill_y, pill_w, pill_h),
            caption,
            scale: s,
            shown: now,
            end: None,
            alpha: 0,
            drawn: None,
        })
    }

    /// Advances the animation; false once the mark has faded out.
    fn step(&mut self, now: Instant, hidden: bool) -> bool {
        let since = ms(now - self.shown);
        let (phase, alpha) = match self.end {
            None => (Phase::Acting, alpha_at(Phase::Acting, since)),
            Some((p, t)) => {
                let fade_in = if since < FADE_IN_MS {
                    alpha_at(Phase::Acting, since)
                } else {
                    255
                };
                (p, alpha_at(p, ms(now - t)).min(fade_in))
            }
        };
        if alpha == 0 && since >= FADE_IN_MS {
            return false;
        }
        let ripple = match self.kind {
            MarkKind::Point => (since as f32 / RIPPLE_MS as f32).min(1.0),
            MarkKind::Element => 1.0,
        };
        let a = if hidden { 0 } else { alpha };
        if self.drawn != Some((phase, ripple)) {
            paint::draw(
                &mut self.surface,
                &mut Look {
                    kind: self.kind,
                    phase,
                    target: self.target,
                    pill: self.pill,
                    caption: &mut self.caption,
                    scale: self.scale,
                    ripple,
                },
            );
            self.drawn = Some((phase, ripple));
            self.surface.push(a);
            self.alpha = a;
        } else if a != self.alpha {
            self.surface.set_alpha(a);
            self.alpha = a;
        }
        true
    }
}

fn ms(d: Duration) -> u64 {
    d.as_millis() as u64
}
