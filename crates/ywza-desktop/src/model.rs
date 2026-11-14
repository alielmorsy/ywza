use crate::{Image, Rect};
use std::fmt;

/// Native window handle (HWND on Windows). Rendered as `w<id>`.
pub type WindowId = u64;

/// The cursor's look, from `CursorWatch::poll`. `image` None: the cursor is hidden.
#[derive(Debug, Clone, Default)]
pub struct CursorShape {
    pub image: Option<Image>,
    /// Hotspot (the clicking point) in `image` pixels.
    pub hot_x: u32,
    pub hot_y: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Monitor {
    /// Index into `list_monitors` (enumeration order).
    pub index: usize,
    pub name: String,
    pub rect: Rect,
    pub work: Rect,
    pub scale: f32,
    pub primary: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    pub id: WindowId,
    pub title: String,
    pub process: String,
    pub pid: u32,
    /// Visible bounds (DWM extended frame bounds), not the invisible resize border.
    pub rect: Rect,
    pub monitor: usize,
    pub minimized: bool,
    /// Hidden by its app (closed to the tray); `focus_window` shows it again.
    pub hidden: bool,
    /// Has the keyboard focus (the active window).
    pub focused: bool,
    pub maximized: bool,
}

impl fmt::Display for WindowInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[w{}] {:?} ({}) mon {}",
            self.id, self.title, self.process, self.monitor
        )?;
        if self.minimized {
            write!(f, " [minimized]")?;
        }
        if self.hidden {
            write!(f, " [hidden]")?;
        }
        if self.maximized {
            write!(f, " [maximized]")?;
        }
        if self.focused {
            write!(f, " [focused]")?;
        }
        Ok(())
    }
}

/// A process started by `launch`; `Launched::wait_for_window` finds its window.
#[derive(Debug, Clone)]
pub struct Launched {
    pub pid: u32,
    /// Windows that existed before the launch, so new ones can be told apart.
    pub(crate) before: Vec<WindowId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
}
