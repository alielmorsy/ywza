#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use self::windows::*;

#[cfg(not(windows))]
mod unsupported;
#[cfg(not(windows))]
pub(crate) use unsupported::*;
