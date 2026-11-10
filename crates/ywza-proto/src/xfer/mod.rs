//! File and folder copy between devices.
//!
//! The control stream only coordinates (`Pull`/`Push` and the `Xfer*` replies). Every entry
//! travels on its own QUIC uni stream: a [`crate::FileHeader`] frame, then the file's raw bytes.

mod send;
mod sink;
mod walk;

use std::path::{Path, PathBuf};

pub use send::{send_all, send_entry};
pub use sink::Sink;
pub use walk::{walk, Entry, Walk};

/// A path from a remote client: `~` and relative paths are under this user's home, like scp.
pub fn expand(path: &str) -> PathBuf {
    let rest = if path == "~" {
        ""
    } else {
        path.strip_prefix("~/")
            .or_else(|| path.strip_prefix("~\\"))
            .unwrap_or(path)
    };
    let p = Path::new(rest);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        dirs::home_dir().unwrap_or_default().join(p)
    }
}

/// `rel` from a header as a relative path, or None if it could leave the destination.
pub fn safe_rel(rel: &str) -> Option<PathBuf> {
    if rel.is_empty() {
        return Some(PathBuf::new());
    }
    rel.split('/').try_fold(PathBuf::new(), |mut p, c| {
        let bad =
            c.is_empty() || c == "." || c == ".." || (cfg!(windows) && c.contains(['\\', ':']));
        (!bad).then(|| {
            p.push(c);
            p
        })
    })
}

#[cfg(test)]
mod tests;
