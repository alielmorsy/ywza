use super::role::role;
use super::variant::Var;
use crate::platform::windows::util::{com, rect};
use crate::snapshot::Raw;
use crate::{Patterns, Result};
use ::windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use ::windows::Win32::UI::Accessibility::*;

pub(super) fn automation() -> Result<IUIAutomation> {
    com();
    Ok(unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? })
}

const PROPS: &[UIA_PROPERTY_ID] = &[
    UIA_ControlTypePropertyId,
    UIA_NamePropertyId,
    UIA_HelpTextPropertyId,
    UIA_AutomationIdPropertyId,
    UIA_BoundingRectanglePropertyId,
    UIA_IsOffscreenPropertyId,
    UIA_IsEnabledPropertyId,
    UIA_IsKeyboardFocusablePropertyId,
    UIA_RuntimeIdPropertyId,
    UIA_IsInvokePatternAvailablePropertyId,
    UIA_IsValuePatternAvailablePropertyId,
    UIA_ValueIsReadOnlyPropertyId,
    UIA_ValueValuePropertyId,
    UIA_IsTogglePatternAvailablePropertyId,
    UIA_ToggleToggleStatePropertyId,
    UIA_IsExpandCollapsePatternAvailablePropertyId,
    UIA_IsSelectionItemPatternAvailablePropertyId,
    UIA_SelectionItemIsSelectedPropertyId,
    UIA_IsScrollPatternAvailablePropertyId,
    UIA_IsRangeValuePatternAvailablePropertyId,
    UIA_RangeValueValuePropertyId,
    UIA_AriaPropertiesPropertyId,
    UIA_NativeWindowHandlePropertyId,
];

/// Whether `prop=true` is in a UIA AriaProperties string ("checked=true;busy=false").
fn aria(props: &str, prop: &str) -> bool {
    props
        .split(';')
        .any(|kv| kv.trim().eq_ignore_ascii_case(&format!("{prop}=true")))
}

pub(super) fn cache_request(uia: &IUIAutomation) -> Result<IUIAutomationCacheRequest> {
    let c = unsafe { uia.CreateCacheRequest()? };
    for &p in PROPS {
        unsafe { c.AddProperty(p)? };
    }
    Ok(c)
}

/// Reads the cached properties of one element.
pub(super) fn raw_from(el: &IUIAutomationElement) -> Result<Raw> {
    unsafe {
        let r = el.CachedBoundingRectangle()?;
        let flag = |p| Var::get(el, p).bool();
        let patterns = Patterns {
            invoke: flag(UIA_IsInvokePatternAvailablePropertyId),
            value: flag(UIA_IsValuePatternAvailablePropertyId),
            value_readonly: flag(UIA_ValueIsReadOnlyPropertyId),
            toggle: flag(UIA_IsTogglePatternAvailablePropertyId),
            expand: flag(UIA_IsExpandCollapsePatternAvailablePropertyId),
            select: flag(UIA_IsSelectionItemPatternAvailablePropertyId),
            scroll: flag(UIA_IsScrollPatternAvailablePropertyId),
        };
        let props = Var::get(el, UIA_AriaPropertiesPropertyId)
            .string()
            .unwrap_or_default();
        let range = if flag(UIA_IsRangeValuePatternAvailablePropertyId) {
            Var::get(el, UIA_RangeValueValuePropertyId)
                .float()
                .map(|v| format!("{v}"))
        } else {
            None
        };
        Ok(Raw {
            role: role(el.CachedControlType()?),
            name: el.CachedName().map(|s| s.to_string()).unwrap_or_default(),
            help: el
                .CachedHelpText()
                .map(|s| s.to_string())
                .unwrap_or_default(),
            automation_id: el
                .CachedAutomationId()
                .map(|s| s.to_string())
                .unwrap_or_default(),
            rect: rect(r),
            offscreen: el.CachedIsOffscreen()?.as_bool(),
            enabled: el.CachedIsEnabled()?.as_bool(),
            focusable: el
                .CachedIsKeyboardFocusable()
                .map(|b| b.as_bool())
                .unwrap_or(false),
            value: if patterns.value {
                Var::get(el, UIA_ValueValuePropertyId)
                    .string()
                    .filter(|v| !v.is_empty())
                    .or(range)
            } else {
                range
            },
            selected: (patterns.select && flag(UIA_SelectionItemIsSelectedPropertyId))
                || (!patterns.toggle && aria(&props, "checked"))
                || aria(&props, "selected"),
            busy: aria(&props, "busy"),
            group: String::new(),
            toggle: if patterns.toggle {
                Var::get(el, UIA_ToggleToggleStatePropertyId).int()
            } else {
                None
            },
            key: Var::get(el, UIA_RuntimeIdPropertyId).ints(),
            bounds: None,
            parent: None,
            patterns,
        })
    }
}
