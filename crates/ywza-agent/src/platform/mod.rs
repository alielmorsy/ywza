//! OS-specific pieces: the shell for `run_command`, the OS name for the prompt and, on
//! Linux, the seat0 service.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(any(windows, target_os = "linux")))]
mod other;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::{os_description, service, shell};
#[cfg(not(any(windows, target_os = "linux")))]
pub use other::{os_description, service, shell};
#[cfg(windows)]
pub use windows::{os_description, service, shell};
