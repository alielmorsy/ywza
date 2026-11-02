use crate::platform::windows::input::{absolute, key};
use crate::{parse_keys, Button, Error, Key, Result};
use ::windows::Win32::UI::Input::KeyboardAndMouse::*;
use ::windows::Win32::UI::WindowsAndMessaging::WHEEL_DELTA;

/// Moves the cursor to virtual-screen pixel (x, y).
pub fn live_mouse_move(x: i32, y: i32) -> Result<()> {
    send(&[mouse(
        MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
        x,
        y,
        0,
    )])
}

/// Presses or releases a mouse button at the current cursor position.
pub fn live_mouse_button(button: Button, down: bool) -> Result<()> {
    let flags = match (button, down) {
        (Button::Left, true) => MOUSEEVENTF_LEFTDOWN,
        (Button::Left, false) => MOUSEEVENTF_LEFTUP,
        (Button::Right, true) => MOUSEEVENTF_RIGHTDOWN,
        (Button::Right, false) => MOUSEEVENTF_RIGHTUP,
        (Button::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
        (Button::Middle, false) => MOUSEEVENTF_MIDDLEUP,
    };
    send(&[mouse(flags, 0, 0, 0)])
}

/// Wheel in notches: `dy` > 0 scrolls down, `dx` > 0 scrolls right. Fractions accumulate
/// in the receiving app (precision touchpads send them too).
pub fn live_wheel(dx: f32, dy: f32) -> Result<()> {
    let mut inputs = Vec::new();
    let delta = |v: f32| (v * WHEEL_DELTA as f32).round() as i32;
    if delta(dy) != 0 {
        inputs.push(mouse(MOUSEEVENTF_WHEEL, 0, 0, -delta(dy)));
    }
    if delta(dx) != 0 {
        inputs.push(mouse(MOUSEEVENTF_HWHEEL, 0, 0, delta(dx)));
    }
    if inputs.is_empty() {
        return Ok(());
    }
    send(&inputs)
}

/// Presses or releases one key, named as in `press_keys` ("a", "enter", "ctrl").
/// Characters map to their key on the current layout without adding shift: the human's
/// own shift key arrives as a separate event.
pub fn live_key(name: &str, down: bool) -> Result<()> {
    let combos = parse_keys(name)?;
    let [combo] = combos.as_slice() else {
        return Err(Error::NotFound(format!("key {name:?}")));
    };
    let [k] = combo.as_slice() else {
        return Err(Error::NotFound(format!("key {name:?}")));
    };
    let vk =
        vk(*k).ok_or_else(|| Error::NotFound(format!("no key for {name:?} on this layout")))?;
    send(&[key(vk, !down)])
}

fn mouse(flags: MOUSE_EVENT_FLAGS, x: i32, y: i32, data: i32) -> INPUT {
    let (dx, dy) = absolute((x, y));
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: data as u32,
                dwFlags: flags,
                ..Default::default()
            },
        },
    }
}

fn send(inputs: &[INPUT]) -> Result<()> {
    if unsafe { SendInput(inputs, size_of::<INPUT>() as i32) } as usize != inputs.len() {
        return Err(Error::Os(
            "input was blocked (elevated window or secure desktop?)".into(),
        ));
    }
    Ok(())
}

fn vk(k: Key) -> Option<VIRTUAL_KEY> {
    Some(match k {
        Key::Ctrl => VK_CONTROL,
        Key::Shift => VK_SHIFT,
        Key::Alt => VK_MENU,
        Key::Win => VK_LWIN,
        Key::Enter => VK_RETURN,
        Key::Esc => VK_ESCAPE,
        Key::Tab => VK_TAB,
        Key::Space => VK_SPACE,
        Key::Backspace => VK_BACK,
        Key::Delete => VK_DELETE,
        Key::Insert => VK_INSERT,
        Key::Home => VK_HOME,
        Key::End => VK_END,
        Key::PageUp => VK_PRIOR,
        Key::PageDown => VK_NEXT,
        Key::Up => VK_UP,
        Key::Down => VK_DOWN,
        Key::Left => VK_LEFT,
        Key::Right => VK_RIGHT,
        Key::CapsLock => VK_CAPITAL,
        Key::PrintScreen => VK_SNAPSHOT,
        Key::Menu => VK_APPS,
        Key::F(n) => VIRTUAL_KEY(VK_F1.0 + n as u16 - 1),
        Key::Char(c) => {
            let scan = unsafe { VkKeyScanW(u16::try_from(c as u32).ok()?) };
            if scan == -1 {
                return None;
            }
            VIRTUAL_KEY(scan.to_le_bytes()[0] as u16)
        }
    })
}
