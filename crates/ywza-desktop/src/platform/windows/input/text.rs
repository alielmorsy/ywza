use super::clipboard::Held;
use super::foreground::{wait_idle, with_foreground};
use super::keyboard::{char_key, chord};
use super::lock::InputLock;
use super::mouse::warp;
use super::raw::{mouse, send, unicode};
use crate::platform::windows::util::hwnd;
use crate::platform::windows::{uia, wm};
use crate::{Error, Result, Snapshot};
use ::windows::Win32::Foundation::{LPARAM, POINT, WPARAM};
use ::windows::Win32::UI::Input::KeyboardAndMouse::*;
use ::windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetForegroundWindow, PostMessageW, SetCursorPos, WM_CHAR,
};
use std::thread::sleep;
use std::time::{Duration, Instant};

/// Types into an element, checking each time that the text arrived. It focuses the
/// element itself, so callers must not click it first. `append` false replaces the
/// content, true types at its end.
/// - native controls: ValuePattern SetValue, read back;
/// - Chromium web content (rich editors drop SetValue, see `uia::is_web`): posted
///   characters to the render widget while the window stays in the background, when
///   the element already has focus in its page and is empty. When it is not focused
///   (or not empty) this declines, it is not an error, and the takeover below runs;
/// - otherwise a short takeover: wait for the user to be idle, bring the window
///   forward, focus the element (UIA, then a real click), select all (or go to the
///   end when appending), type, and give the foreground back. Empty text deletes the
///   selection; long or non-keyboard text is pasted (see `pastes`).
///
/// Before every batch of keys the element is re-validated (same RuntimeId, name and
/// kind), so keys never go to a control that changed under us (another chat opened).
/// With `id` None, types at the cursor of the window's focused control.
pub fn type_text(snap: &Snapshot, id: Option<u32>, text: &str, append: bool) -> Result<()> {
    let window = snap.window.id;
    let Some(id) = id else {
        let _lock = InputLock::foreground(Some(window))?;
        return with_foreground(Some(window), || send_text(text));
    };
    {
        let _lock = InputLock::window(window)?;
        let (e, el) = uia::live(snap, id)?;
        if let Some(el) = el {
            let settable = e.patterns.value && !e.patterns.value_readonly;
            if uia::is_web(&el) {
                if background_type(snap, id, &el, text)? {
                    return Ok(());
                }
            } else if settable {
                let value = if append {
                    uia::text(&el).map(|cur| cur + text)
                } else {
                    Some(text.to_string())
                };
                if value.is_some_and(|v| uia::set_value(&el, &v) && uia::value_is(&el, &v)) {
                    return Ok(());
                }
            }
        }
    }
    // Real keys: retake the locks in global-then-window order and revalidate.
    let _lock = InputLock::foreground(Some(window))?;
    let (e, el) = uia::live(snap, id)?;
    if el.is_none() {
        return with_foreground(Some(window), || send_text(text)); // OCR box: no UIA object
    }
    takeover_type(snap, id, e.rect.center(), append, text)
}

/// Path A: WM_CHAR posted to the Chromium render widget, no focus change, no
/// foreground change. Ok(false) when it does not apply (the caller takes over):
/// the element is not focused in its page (UIA SetFocus could steal the foreground),
/// it has text that would need select-all (posted Ctrl is not seen), or the text has
/// control characters (Enter can mean "send").
fn background_type(snap: &Snapshot, id: u32, el: &uia::Uia, text: &str) -> Result<bool> {
    if text.chars().any(char::is_control) || !uia::has_focus(el) {
        return Ok(false);
    }
    if !uia::text(el).is_some_and(|t| uia::norm(&t).is_empty()) {
        return Ok(false);
    }
    let Some(target) = uia::render_widget(el) else {
        return Ok(false);
    };
    uia::live(snap, id)?;
    for u in text.encode_utf16() {
        unsafe { PostMessageW(Some(target), WM_CHAR, WPARAM(u as usize), LPARAM(1))? };
    }
    verify(id, el, text, true)?;
    Ok(true)
}

/// Text above this many characters is pasted: key-by-key typing drops letters in
/// browsers on long text.
const PASTE_FROM: usize = 80;

/// Whether to paste instead of typing: long text, or characters with no key on the
/// layout (each would be a slow paced Unicode packet).
fn pastes(text: &str) -> bool {
    text.chars().count() > PASTE_FROM
        || text
            .chars()
            .any(|c| !c.is_control() && char_key(c).is_none())
}

/// Path B: brief foreground takeover with real keys; the previous foreground window
/// and the cursor are restored afterwards.
fn takeover_type(snap: &Snapshot, id: u32, at: (i32, i32), append: bool, text: &str) -> Result<()> {
    wait_idle()?;
    let prev = unsafe { GetForegroundWindow() };
    let mut cursor = POINT::default();
    unsafe { GetCursorPos(&mut cursor)? };
    let target = hwnd(snap.window.id);
    wm::bring_to_front(target)?;
    let typed = (|| {
        let (_, el) = uia::live(snap, id)?;
        let el = el.ok_or_else(|| Error::Stale {
            id,
            reason: "gone".into(),
        })?;
        uia::set_focus(&el);
        if !uia::has_focus(&el) {
            warp(at)?;
            send(&[
                mouse(MOUSEEVENTF_LEFTDOWN, at, 0),
                mouse(MOUSEEVENTF_LEFTUP, at, 0),
            ])?;
            sleep(Duration::from_millis(100));
        }
        if !uia::has_focus(&el) {
            return Err(Error::Stale {
                id,
                reason: "could not focus it; nothing typed".into(),
            });
        }
        let go = if append {
            VK_END
        } else {
            VIRTUAL_KEY(b'A' as u16)
        };
        send(&chord(&[VK_CONTROL, go]))?; // Ctrl+End or Ctrl+A (replace)
        if !append && text.is_empty() {
            send(&chord(&[VK_DELETE]))?;
        }
        if pastes(text) {
            let held = Held::put(text)?;
            send(&chord(&[VK_CONTROL, VIRTUAL_KEY(b'V' as u16)]))?;
            return Ok((el, Some(held)));
        }
        send_text(text)?;
        Ok((el, None))
    })();
    // Let the app take the keys from its queue before focus moves away again.
    sleep(Duration::from_millis(100));
    unsafe {
        let _ = SetCursorPos(cursor.x, cursor.y);
    }
    if !prev.0.is_null() && prev != target {
        let _ = wm::bring_to_front(prev);
    }
    // `_held` restores the clipboard once the paste is verified (or not).
    let (el, _held) = typed?;
    verify(id, &el, text, append)
}

/// Polls the element's text (keys are processed asynchronously), longer for longer
/// text. Replacing with "" needs the field empty, anything else needs it to contain
/// the text. Unreadable text (password fields) is not checked.
fn verify(id: u32, el: &uia::Uia, text: &str, append: bool) -> Result<()> {
    let ms = (1000 + 5 * text.chars().count() as u64).min(10_000);
    let deadline = Instant::now() + Duration::from_millis(ms);
    loop {
        match uia::text(el) {
            None => return Ok(()),
            Some(t) if arrived(&t, text, append) => return Ok(()),
            Some(_) if Instant::now() >= deadline => {
                let what = if text.is_empty() && !append {
                    format!("cleared element {id}, but it is not empty")
                } else {
                    format!("typed into element {id}, but its text does not contain it")
                };
                return Err(Error::Os(format!("{what}; check it before retrying")));
            }
            Some(_) => sleep(Duration::from_millis(100)),
        }
    }
}

/// Whether the field's text shows the typing: replacing with "" left it empty,
/// anything else made it contain the text.
fn arrived(field: &str, text: &str, append: bool) -> bool {
    let (field, want) = (uia::norm(field), uia::norm(text));
    if want.is_empty() && !append {
        field.trim().is_empty()
    } else {
        field.contains(&want)
    }
}

/// Live view text at whatever has focus, with no lock, idle wait or foreground change:
/// the human is driving. A single keystroke is typed; anything longer is pasted: typed
/// bursts drop characters in lagging apps (seen in Windows 11 Notepad), and code editors
/// would auto-indent or auto-close pasted code again.
pub fn live_text(text: &str) -> Result<()> {
    if text.chars().count() > 1 || pastes(text) {
        let _held = Held::put(text)?;
        return send(&chord(&[VK_CONTROL, VIRTUAL_KEY(b'V' as u16)]));
    }
    send_text(text)
}

/// Types text. Characters on an unshifted key are real key presses. Everything else
/// goes as Unicode (VK_PACKET), paced, because both alternatives break in lagging
/// apps (seen in Windows 11 Notepad): an injected Shift is released before the app
/// reads it, and queued packets all read the latest character.
fn send_text(text: &str) -> Result<()> {
    let caps = unsafe { GetKeyState(VK_CAPITAL.0 as i32) } & 1 != 0;
    for c in text.chars() {
        let (inputs, slow) = match c {
            '\r' => continue,
            '\n' => (chord(&[VK_RETURN]), false),
            '\t' => (chord(&[VK_TAB]), false),
            _ => match char_key(c) {
                Some((vk, false)) if !(caps && c.is_alphabetic()) => (chord(&[vk]), false),
                _ => {
                    let mut buf = [0u16; 2];
                    let units = c.encode_utf16(&mut buf).iter();
                    (
                        units
                            .flat_map(|&u| [unicode(u, false), unicode(u, true)])
                            .collect(),
                        true,
                    )
                }
            },
        };
        send(&inputs)?;
        if slow {
            sleep(Duration::from_millis(20));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_replace_needs_an_empty_field() {
        assert!(arrived("", "", false));
        assert!(arrived("\r\n", "", false));
        assert!(!arrived("old cover letter", "", false)); // Ctrl+A with nothing typed
        assert!(arrived("old cover letter", "", true));
        assert!(arrived("Dear team,\r\nhi", "Dear team,\nhi", false));
        assert!(!arrived("Dear tam", "Dear team", false));
    }

    #[test]
    fn long_or_unkeyed_text_is_pasted() {
        assert!(!pastes("hello"));
        assert!(pastes(&"a".repeat(PASTE_FROM + 1)));
        assert!(pastes("ok 🙂")); // no key on any layout
    }
}
