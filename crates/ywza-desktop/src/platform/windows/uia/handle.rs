use ::windows::Win32::UI::Accessibility::IUIAutomationElement;

pub(crate) type Uia = IUIAutomationElement;

/// A UIA element kept per snapshot ID.
#[derive(Clone)]
pub struct Handle(pub(super) IUIAutomationElement);

// SAFETY: elements are created and used in the process MTA (see `com()`), where UIA
// client objects are free-threaded.
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("Handle")
    }
}
