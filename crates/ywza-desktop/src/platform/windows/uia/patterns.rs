use super::handle::Uia;
use super::raw::automation;
use crate::Patterns;
use ::windows::core::{Interface, BSTR};
use ::windows::Win32::Foundation::HWND;
use ::windows::Win32::UI::Accessibility::*;

/// Click through a pattern. Returns false when no pattern applies or it failed.
pub(crate) fn pattern_click(el: &Uia, p: &Patterns) -> bool {
    unsafe {
        let done = if p.invoke {
            pattern::<IUIAutomationInvokePattern>(el, UIA_InvokePatternId).and_then(|x| x.Invoke())
        } else if p.toggle {
            pattern::<IUIAutomationTogglePattern>(el, UIA_TogglePatternId).and_then(|x| x.Toggle())
        } else if p.select {
            pattern::<IUIAutomationSelectionItemPattern>(el, UIA_SelectionItemPatternId)
                .and_then(|x| x.Select())
        } else if p.expand {
            pattern::<IUIAutomationExpandCollapsePattern>(el, UIA_ExpandCollapsePatternId).and_then(
                |x| {
                    if x.CurrentExpandCollapseState()? == ExpandCollapseState_Collapsed {
                        x.Expand()
                    } else {
                        x.Collapse()
                    }
                },
            )
        } else {
            return false;
        };
        done.is_ok()
    }
}

/// Opens the element's dropdown or menu through ExpandCollapse, skipping its main
/// action (split buttons: Invoke runs the action, Expand shows the menu).
pub(crate) fn expand(el: &Uia) -> bool {
    unsafe {
        pattern::<IUIAutomationExpandCollapsePattern>(el, UIA_ExpandCollapsePatternId)
            .and_then(|x| x.Expand())
            .is_ok()
    }
}

pub(crate) fn set_value(el: &Uia, text: &str) -> bool {
    unsafe {
        pattern::<IUIAutomationValuePattern>(el, UIA_ValuePatternId)
            .and_then(|x| x.SetValue(&BSTR::from(text)))
            .is_ok()
    }
}

/// The element's text: ValuePattern value, else the TextPattern document text.
/// None when unreadable (password fields refuse it).
pub(crate) fn text(el: &Uia) -> Option<String> {
    unsafe {
        if let Ok(v) = pattern::<IUIAutomationValuePattern>(el, UIA_ValuePatternId)
            .and_then(|x| x.CurrentValue())
        {
            return Some(v.to_string());
        }
        let t = pattern::<IUIAutomationTextPattern>(el, UIA_TextPatternId).ok()?;
        t.DocumentRange()
            .and_then(|r| r.GetText(-1))
            .ok()
            .map(|s| s.to_string())
    }
}

/// Line endings unified and trailing whitespace dropped, for comparing typed text.
pub(crate) fn norm(s: &str) -> String {
    s.replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim_end()
        .to_string()
}

/// Whether the element's value is `text`, also a moment later: some editors revert
/// a SetValue asynchronously.
pub(crate) fn value_is(el: &Uia, text: &str) -> bool {
    let now = || unsafe {
        pattern::<IUIAutomationValuePattern>(el, UIA_ValuePatternId)
            .and_then(|x| x.CurrentValue())
            .is_ok_and(|v| norm(&v.to_string()) == norm(text))
    };
    now() && {
        std::thread::sleep(std::time::Duration::from_millis(150));
        now()
    }
}

/// The Chromium render widget HWND hosting a web element: its nearest ancestor with
/// a window handle, if that is a `Chrome_RenderWidgetHostHWND`.
pub(crate) fn render_widget(el: &Uia) -> Option<HWND> {
    let walker = unsafe { automation().ok()?.RawViewWalker().ok()? };
    let mut cur = el.clone();
    for _ in 0..200 {
        cur = unsafe { walker.GetParentElement(&cur) }.ok()?;
        let h = unsafe { cur.CurrentNativeWindowHandle() }.ok()?;
        if !h.0.is_null() {
            let class = unsafe { cur.CurrentClassName() }.ok()?;
            return (class == "Chrome_RenderWidgetHostHWND").then_some(h);
        }
    }
    None
}

/// Web content (Chromium, Firefox): its rich editors (contenteditable: Lexical,
/// ProseMirror, Slate) return S_OK from SetValue and drop the value.
pub(crate) fn is_web(el: &Uia) -> bool {
    unsafe { el.CurrentFrameworkId() }.is_ok_and(|f| f == "Chrome" || f == "Gecko")
}

/// Scrolls by small increments; positive = down/right.
pub(crate) fn scroll_pattern(el: &Uia, dx: i32, dy: i32) -> bool {
    let step = |d: i32, i: u32| match d {
        _ if i >= d.unsigned_abs() => ScrollAmount_NoAmount,
        1.. => ScrollAmount_SmallIncrement,
        _ => ScrollAmount_SmallDecrement,
    };
    unsafe {
        let Ok(s) = pattern::<IUIAutomationScrollPattern>(el, UIA_ScrollPatternId) else {
            return false;
        };
        for i in 0..dx.unsigned_abs().max(dy.unsigned_abs()) {
            let (h, v) = (step(dx, i), step(dy, i));
            if s.Scroll(h, v).is_err() {
                return false;
            }
        }
        true
    }
}

pub(crate) fn set_focus(el: &Uia) {
    let _ = unsafe { el.SetFocus() };
}

pub(crate) fn has_focus(el: &Uia) -> bool {
    unsafe { el.CurrentHasKeyboardFocus() }.is_ok_and(|b| b.as_bool())
}

unsafe fn pattern<T: Interface>(el: &Uia, id: UIA_PATTERN_ID) -> ::windows::core::Result<T> {
    unsafe { el.GetCurrentPatternAs::<T>(id) }
}
