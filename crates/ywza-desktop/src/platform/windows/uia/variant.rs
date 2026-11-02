use ::windows::Win32::System::Com::CoTaskMemFree;
use ::windows::Win32::System::Variant::{
    VariantClear, VariantToBoolean, VariantToInt32, VariantToInt32ArrayAlloc, VariantToStringAlloc,
    VARIANT,
};
use ::windows::Win32::UI::Accessibility::{IUIAutomationElement, UIA_PROPERTY_ID};

/// An owned VARIANT, cleared on drop. Unsupported properties read as None.
pub(super) struct Var(VARIANT);

impl Var {
    pub(super) fn get(el: &IUIAutomationElement, p: UIA_PROPERTY_ID) -> Var {
        Var(unsafe { el.GetCachedPropertyValue(p) }.unwrap_or_default())
    }

    pub(super) fn bool(&self) -> bool {
        unsafe { VariantToBoolean(&self.0) }.is_ok_and(|b| b.as_bool())
    }

    pub(super) fn int(&self) -> Option<i32> {
        unsafe { VariantToInt32(&self.0) }.ok()
    }

    pub(super) fn string(&self) -> Option<String> {
        unsafe {
            let p = VariantToStringAlloc(&self.0).ok()?;
            let s = p.to_string().ok();
            CoTaskMemFree(Some(p.0 as _));
            s
        }
    }

    pub(super) fn ints(&self) -> Vec<i32> {
        unsafe {
            let (mut p, mut n) = (std::ptr::null_mut(), 0u32);
            if VariantToInt32ArrayAlloc(&self.0, &mut p, &mut n).is_err() || p.is_null() {
                return Vec::new();
            }
            let v = std::slice::from_raw_parts(p, n as usize).to_vec();
            CoTaskMemFree(Some(p as _));
            v
        }
    }
}

impl Drop for Var {
    fn drop(&mut self) {
        let _ = unsafe { VariantClear(&mut self.0) };
    }
}
