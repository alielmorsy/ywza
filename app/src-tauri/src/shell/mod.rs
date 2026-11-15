//! "ywza" in Explorer's right-click menu, as per-user classic verbs.

pub mod layout;
#[cfg(windows)]
mod registry;

fn exe() -> Result<String, String> {
    let p = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(p.to_string_lossy().into_owned())
}

/// Turns the menu on at the first start of an installed build; after that the Settings toggle
/// decides, so turning it off sticks. Dev builds skip it so they do not point Explorer at target/.
pub fn first_run(app: &tauri::AppHandle) {
    #[cfg(windows)]
    if !cfg!(debug_assertions) {
        use tauri::Manager;
        let Ok(dir) = app.path().app_data_dir() else {
            return;
        };
        let mark = dir.join("explorer-menu-default");
        if mark.exists() {
            return;
        }
        // Not marked on failure, so the next start tries again.
        if let Err(e) = explorer_menu_set(true) {
            eprintln!("explorer menu: {e}");
            return;
        }
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(mark, "");
    }
    #[cfg(not(windows))]
    let _ = app;
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
