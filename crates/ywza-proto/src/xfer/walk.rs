use std::{
    collections::HashSet,
    fs::{self, Metadata},
    io,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use crate::FileHeader;

/// One file or folder to send; `header.xfer` is filled in when it is sent.
#[derive(Debug, Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub header: FileHeader,
}

#[derive(Debug, Default)]
pub struct Walk {
    /// The source's own file name.
    pub name: String,
    pub entries: Vec<Entry>,
    pub bytes: u64,
    /// Entries that could not be read, by relative path. The rest are still sent.
    pub errors: Vec<(String, io::Error)>,
}

/// Lists `root` and, if `recursive`, everything under it. Symlinks are followed, like scp.
pub fn walk(root: &Path, recursive: bool) -> io::Result<Walk> {
    let root = fs::canonicalize(root)?;
    let meta = fs::metadata(&root)?;
    if meta.is_dir() && !recursive {
        return Err(io::Error::other(format!(
            "{} is a folder (copy it recursively)",
            root.display()
        )));
    }
    let name = root
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| io::Error::other("cannot copy a filesystem root"))?
        .to_string();
    let mut w = Walk {
        name,
        ..Default::default()
    };
    let mut seen = HashSet::new();
    visit(&mut w, &mut seen, root, String::new(), meta);
    Ok(w)
}

fn visit(w: &mut Walk, seen: &mut HashSet<PathBuf>, path: PathBuf, rel: String, meta: Metadata) {
    if meta.is_file() {
        w.bytes += meta.len();
        w.entries.push(Entry {
            header: header(rel, &meta),
            path,
        });
        return;
    }
    if !meta.is_dir() {
        w.errors
            .push((rel, io::Error::other("not a regular file or folder")));
        return;
    }
    // A followed symlink can point back up the tree.
    match fs::canonicalize(&path) {
        Ok(real) => {
            if !seen.insert(real) {
                w.errors.push((rel, io::Error::other("symlink loop, skipped")));
                return;
            }
        }
        Err(e) => {
            w.errors.push((rel, e));
            return;
        }
    }
    w.entries.push(Entry {
        header: header(rel.clone(), &meta),
        path: path.clone(),
    });
    let dir = match fs::read_dir(&path) {
        Ok(d) => d,
        Err(e) => {
            w.errors.push((rel, e));
            return;
        }
    };
    for item in dir {
        let item = match item {
            Ok(i) => i,
            Err(e) => {
                w.errors.push((rel.clone(), e));
                continue;
            }
        };
        let child = item.path();
        let Some(name) = item.file_name().to_str().map(str::to_string) else {
            let shown = format!("{rel}/{}", item.file_name().to_string_lossy());
            w.errors
                .push((shown, io::Error::other("file name is not valid UTF-8")));
            continue;
        };
        let child_rel = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        match fs::metadata(&child) {
            Ok(m) => visit(w, seen, child, child_rel, m),
            Err(e) => w.errors.push((child_rel, e)),
        }
    }
}

fn header(rel: String, meta: &Metadata) -> FileHeader {
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        Some(meta.permissions().mode() & 0o7777)
    };
    #[cfg(not(unix))]
    let mode = None;
    FileHeader {
        xfer: 0,
        rel,
        dir: meta.is_dir(),
        size: if meta.is_dir() { 0 } else { meta.len() },
        mode,
        mtime_ms: meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64),
    }
}
