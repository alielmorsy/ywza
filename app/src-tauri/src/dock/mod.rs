//! The dock window (command box and working tab) and its global shortcut.

#[cfg(windows)]
mod clipboard;
pub mod context;
mod hotkey;
mod target;

use serde::Serialize;
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use hotkey::Hotkey;

const LABEL: &str = "dock";

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let dock = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?dock".into()))
        .title("ywza dock")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .resizable(false)
        .focused(false)
        .visible(false)
        .build()?;
    #[cfg(windows)]
    {
        let main = app.get_webview_window("main");
        let own = [Some(&dock), main.as_ref()]
            .into_iter()
            .flatten()
            .filter_map(|w| w.hwnd().ok())
            .map(|h| h.0 as isize)
            .collect();
        target::ignore(own);
    }
    #[cfg(not(windows))]
    let _ = dock;
    app.manage(Hotkey::start(app.clone()));
    Ok(())
}

fn dock(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(LABEL)
        .ok_or_else(|| "The dock window is missing".to_string())
}

#[tauri::command]
pub fn dock_place(app: AppHandle, x: i32, y: i32, w: u32, h: u32) -> Result<(), String> {
    let d = dock(&app)?;
    d.set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    d.set_size(PhysicalSize::new(w, h))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn dock_show(app: AppHandle, focus: bool) -> Result<(), String> {
    let d = dock(&app)?;
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            ShowWindow, SW_SHOWNOACTIVATE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        };
        let h = d.hwnd().map_err(|e| e.to_string())?;
        // Without it, clicking the dock would pull the foreground away from the app being driven.
        set_ex_style(h, WS_EX_NOACTIVATE.0, !focus);
        if focus {
            d.show().map_err(|e| e.to_string())?;
            d.set_focus().map_err(|e| e.to_string())?;
        } else {
            let _ = unsafe { ShowWindow(h, SW_SHOWNOACTIVATE) };
        }
        // Keeps the dock out of the agent's window list; tao rewrites the ex style on show/hide.
        set_ex_style(h, WS_EX_TOOLWINDOW.0, true);
        Ok(())
    }
    #[cfg(not(windows))]
    {
        d.show().map_err(|e| e.to_string())?;
        if focus {
            d.set_focus().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[cfg(windows)]
fn set_ex_style(h: windows::Win32::Foundation::HWND, bits: u32, on: bool) {
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongW, SetWindowLongW, GWL_EXSTYLE};
    unsafe {
        let ex = GetWindowLongW(h, GWL_EXSTYLE) as u32;
        let ex = if on { ex | bits } else { ex & !bits };
        SetWindowLongW(h, GWL_EXSTYLE, ex as i32);
    }
}

#[tauri::command]
pub fn dock_hide(app: AppHandle, hotkey: State<Hotkey>) -> Result<(), String> {
    let d = dock(&app)?;
    d.hide().map_err(|e| e.to_string())?;
    // A no-activate show bypasses tao, which then thinks the window is already hidden.
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
        let h = d.hwnd().map_err(|e| e.to_string())?;
        let _ = unsafe { ShowWindow(h, SW_HIDE) };
    }
    hotkey.refresh();
    Ok(())
}

#[derive(Serialize)]
pub struct MonitorInfo {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    scale: f64,
}

#[tauri::command]
pub fn dock_monitor(app: AppHandle) -> Result<MonitorInfo, String> {
    let at_cursor = match app.cursor_position() {
        Ok(p) => app.monitor_from_point(p.x, p.y).map_err(|e| e.to_string())?,
        Err(_) => None,
    };
    let m = match at_cursor {
        Some(m) => m,
        None => app
            .primary_monitor()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "No monitor found".to_string())?,
    };
    let a = m.work_area();
    Ok(MonitorInfo {
        x: a.position.x,
        y: a.position.y,
        w: a.size.width,
        h: a.size.height,
        scale: m.scale_factor(),
    })
}

#[derive(Serialize)]
pub struct Target {
    window: String,
    title: String,
    process: String,
}

#[tauri::command]
pub fn dock_target() -> Option<Target> {
    target::current().map(|(window, title, process)| Target {
        window,
        title,
        process,
    })
}

#[tauri::command]
pub fn dock_restore_focus() -> Result<(), String> {
    target::restore()
}

#[tauri::command]
pub async fn dock_shortcut(key: String, hotkey: State<'_, Hotkey>) -> Result<(), String> {
    hotkey.set(&key)
}
