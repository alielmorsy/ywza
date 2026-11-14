use std::{
    io,
    path::{Path, PathBuf},
    time::{Duration, UNIX_EPOCH},
};

use tokio::io::{AsyncRead, AsyncWriteExt, BufWriter};

use super::safe_rel;
use crate::FileHeader;

/// Where one transfer's entries land.
#[derive(Debug, Clone)]
pub struct Sink {
    root: PathBuf,
}

impl Sink {
    /// cp semantics: into `dest/name` if `dest` is an existing folder, else `dest` itself.
    /// A trailing separator makes `dest` a folder, created when missing.
    pub fn new(dest: &Path, name: &str) -> io::Result<Self> {
        if name.is_empty() || safe_rel(name).is_none_or(|p| p.components().count() != 1) {
            return Err(io::Error::other(format!("bad source name `{name}`")));
        }
        if dest.to_string_lossy().ends_with(['/', '\\']) {
            std::fs::create_dir_all(dest)?;
        }
        let root = if dest.is_dir() {
            dest.join(name)
        } else {
            dest.to_path_buf()
        };
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Writes one entry whose header was already read from `body`. Files go to a `.part`
    /// name first, so an interrupted copy never looks complete. Returns the bytes written.
    pub async fn write<R: AsyncRead + Unpin>(
        &self,
        h: &FileHeader,
        body: &mut R,
    ) -> io::Result<u64> {
        let rel = safe_rel(&h.rel)
            .ok_or_else(|| io::Error::other(format!("refused path `{}`", h.rel)))?;
        let path = self.root.join(rel);
        if h.dir {
            tokio::fs::create_dir_all(&path).await?;
            return Ok(0);
        }
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut part = path.clone().into_os_string();
        part.push(".ywza-part");
        let part = PathBuf::from(part);
        let r = async {
            let mut f = BufWriter::with_capacity(256 * 1024, tokio::fs::File::create(&part).await?);
            let n = tokio::io::copy(body, &mut f).await?;
            f.flush().await?;
            let f = f.into_inner().into_std().await;
            finish_file(&f, h)?;
            drop(f);
            tokio::fs::rename(&part, &path).await?;
            Ok(n)
        }
        .await;
        if r.is_err() {
            let _ = tokio::fs::remove_file(&part).await;
        }
        r
    }
}

fn finish_file(f: &std::fs::File, h: &FileHeader) -> io::Result<()> {
    if let Some(ms) = h.mtime_ms {
        f.set_modified(UNIX_EPOCH + Duration::from_millis(ms))?;
    }
    #[cfg(unix)]
    if let Some(mode) = h.mode {
        use std::os::unix::fs::PermissionsExt;
        f.set_permissions(std::fs::Permissions::from_mode(mode))?;
    }
    Ok(())
}
