use crate::keys::{Code, Key};
use windows::core::BOOL;
use windows::Win32::System::Console::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

fn rec(vk: u16, ch: u16, state: u32, down: bool) -> INPUT_RECORD {
    INPUT_RECORD {
        EventType: KEY_EVENT as u16,
        Event: INPUT_RECORD_0 {
            KeyEvent: KEY_EVENT_RECORD {
                bKeyDown: BOOL(down as i32),
                wRepeatCount: 1,
                wVirtualKeyCode: vk,
                wVirtualScanCode: unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as u16,
                uChar: KEY_EVENT_RECORD_0 { UnicodeChar: ch },
                dwControlKeyState: state,
            },
        },
    }
}

/// Key down + key up records with virtual key code, unicode char and control-key state.
pub(super) fn key_records(k: &Key) -> Vec<INPUT_RECORD> {
    let mut state = 0u32;
    if k.shift {
        state |= SHIFT_PRESSED;
    }
    if k.ctrl {
        state |= LEFT_CTRL_PRESSED;
    }
    if k.alt {
        state |= LEFT_ALT_PRESSED;
    }
    let enh = ENHANCED_KEY;
    let (vk, ch, state): (u16, u16, u32) = match k.code {
        Code::Enter => (VK_RETURN.0, 0x0d, state),
        Code::Esc => (VK_ESCAPE.0, 0x1b, state),
        Code::Tab => (VK_TAB.0, 0x09, state),
        Code::Backspace => (VK_BACK.0, 0x08, state),
        Code::Delete => (VK_DELETE.0, 0, state | enh),
        Code::Insert => (VK_INSERT.0, 0, state | enh),
        Code::Up => (VK_UP.0, 0, state | enh),
        Code::Down => (VK_DOWN.0, 0, state | enh),
        Code::Left => (VK_LEFT.0, 0, state | enh),
        Code::Right => (VK_RIGHT.0, 0, state | enh),
        Code::Home => (VK_HOME.0, 0, state | enh),
        Code::End => (VK_END.0, 0, state | enh),
        Code::PageUp => (VK_PRIOR.0, 0, state | enh),
        Code::PageDown => (VK_NEXT.0, 0, state | enh),
        Code::F(n) => (VK_F1.0 + n as u16 - 1, 0, state),
        Code::Char(c) => {
            let mut units = [0u16; 2];
            let units = c.encode_utf16(&mut units);
            if units.len() == 2 {
                // Outside the BMP: no virtual key, send the surrogate pair as characters.
                return units
                    .iter()
                    .flat_map(|&u| [rec(0, u, state, true), rec(0, u, state, false)])
                    .collect();
            }
            let scan = unsafe { VkKeyScanW(units[0]) };
            let (vk, sh) = if scan == -1 {
                (0, 0)
            } else {
                ((scan & 0xff) as u16, ((scan >> 8) & 0xff) as u32)
            };
            let mut st = state;
            if sh & 1 != 0 {
                st |= SHIFT_PRESSED;
            }
            if sh & 2 != 0 {
                st |= LEFT_CTRL_PRESSED;
            }
            if sh & 4 != 0 {
                st |= LEFT_ALT_PRESSED;
            }
            if k.ctrl && c.is_ascii_alphabetic() {
                // Ctrl+<letter>: control character, vk of the letter itself.
                (
                    c.to_ascii_uppercase() as u16,
                    c.to_ascii_lowercase() as u16 - 'a' as u16 + 1,
                    st,
                )
            } else {
                (vk, if k.ctrl { 0 } else { units[0] }, st)
            }
        }
    };
    vec![rec(vk, ch, state, true), rec(vk, ch, state, false)]
}
