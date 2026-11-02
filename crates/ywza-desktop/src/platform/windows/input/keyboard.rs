use super::foreground::with_foreground;
use super::lock::InputLock;
use super::raw::{key, send};
use crate::{parse_keys, Error, Key, Result, WindowId};
use ::windows::Win32::UI::Input::KeyboardAndMouse::*;

/// Presses key combos like `"ctrl+shift+s"` or `"ctrl+k ctrl+s"`, into `window` if given
/// (brought to the front first), else into whatever has focus.
pub fn press_keys(window: Option<WindowId>, keys: &str) -> Result<()> {
    let combos = parse_keys(keys)?;
    let mut inputs = Vec::new();
    for combo in &combos {
        let mut vks = Vec::new();
        for &k in combo {
            let (vk, shift) = vk(k)?;
            if shift && !combo.contains(&Key::Shift) {
                vks.push(VK_SHIFT);
            }
            vks.push(vk);
        }
        inputs.extend(chord(&vks));
    }
    let _lock = InputLock::foreground(window)?;
    with_foreground(window, || send(&inputs))
}

/// Virtual key for a parsed key, plus whether the character needs shift.
fn vk(k: Key) -> Result<(VIRTUAL_KEY, bool)> {
    let vk = match k {
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
            return char_key(c)
                .ok_or_else(|| Error::NotFound(format!("no key for {c:?} on this layout")));
        }
    };
    Ok((vk, false))
}

/// Key and shift state that type `c` on the current layout; None if it needs
/// Ctrl/AltGr or has no key.
pub(super) fn char_key(c: char) -> Option<(VIRTUAL_KEY, bool)> {
    let scan = unsafe { VkKeyScanW(u16::try_from(c as u32).ok()?) };
    let [vk, mods] = scan.to_le_bytes();
    (scan != -1 && mods & 0b110 == 0).then_some((VIRTUAL_KEY(vk as u16), mods & 1 != 0))
}

/// Presses keys in order and releases them in reverse.
pub(super) fn chord(vks: &[VIRTUAL_KEY]) -> Vec<INPUT> {
    let downs = vks.iter().map(|&v| key(v, false));
    downs
        .chain(vks.iter().rev().map(|&v| key(v, true)))
        .collect()
}
