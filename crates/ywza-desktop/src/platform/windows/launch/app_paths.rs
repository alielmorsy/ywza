use ::windows::core::{HSTRING, PCWSTR};
use ::windows::Win32::System::Registry::{
    RegGetValueW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ,
};
use std::path::PathBuf;

/// Default value of `App Paths\<exe>` in HKCU then HKLM.
pub(super) fn app_path(exe: &str) -> Option<PathBuf> {
    let key = HSTRING::from(format!(
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{exe}"
    ));
    [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE]
        .into_iter()
        .find_map(|root: HKEY| {
            let mut buf = [0u16; 1024];
            let mut bytes = size_of_val(&buf) as u32;
            let err = unsafe {
                RegGetValueW(
                    root,
                    &key,
                    PCWSTR::null(),
                    RRF_RT_REG_SZ,
                    None,
                    Some(buf.as_mut_ptr().cast()),
                    Some(&mut bytes),
                )
            };
            if err.0 != 0 {
                return None;
            }
            let s = String::from_utf16_lossy(&buf[..(bytes as usize / 2).saturating_sub(1)]);
            let s = s.trim().trim_matches('"');
            (!s.is_empty()).then(|| PathBuf::from(s))
        })
}
