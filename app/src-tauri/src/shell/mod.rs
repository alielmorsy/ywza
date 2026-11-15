//! "ywza" in Explorer's right-click menu, as per-user classic verbs.

pub mod layout;
#[cfg(windows)]
mod registry;

fn exe() -> Result<String, String> {
    let p = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(p.to_string_lossy().into_owned())
}

/// On only when every key is present and points at this exe.
#[tauri::command]
pub fn explorer_menu_status() -> bool {
    #[cfg(windows)]
    return exe().is_ok_and(|e| registry::installed(&layout::keys(&e)));
    #[cfg(not(windows))]
    false
}

#[tauri::command]
pub fn explorer_menu_set(on: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        registry::remove().map_err(|e| e.to_string())?;
        if on {
            let keys = layout::keys(&exe()?);
            if let Err(e) = registry::install(&keys) {
                let _ = registry::remove();
                return Err(e.to_string());
            }
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = on;
        Err("The Explorer menu is only on Windows".into())
    }
}
