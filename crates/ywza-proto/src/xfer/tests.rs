use std::{fs, path::Path};

use super::*;
use crate::{read_frame, FileHeader};

#[test]
fn rel_paths_stay_inside() {
    assert_eq!(safe_rel("").unwrap(), PathBuf::new());
    assert_eq!(safe_rel("a/b").unwrap(), Path::new("a").join("b"));
    for bad in ["..", "a/../b", "/a", "a//b", "./a", "a/"] {
        assert!(safe_rel(bad).is_none(), "{bad}");
    }
    #[cfg(windows)]
    for bad in ["C:/x", "a\\..\\b"] {
        assert!(safe_rel(bad).is_none(), "{bad}");
    }
}

#[test]
fn remote_paths_resolve_under_home() {
    let home = dirs::home_dir().unwrap();
    assert_eq!(expand(""), home);
    assert_eq!(expand("~"), home);
    assert_eq!(expand("~/x"), home.join("x"));
    assert_eq!(expand("x/y"), home.join("x/y"));
    let abs = if cfg!(windows) { "C:\\x" } else { "/x" };
    assert_eq!(expand(abs), PathBuf::from(abs));
}

#[test]
fn folders_need_recursive() {
    let src = tempfile::tempdir().unwrap();
    assert!(walk(src.path(), false).is_err());
}

#[tokio::test]
async fn folder_round_trip() {
    let src = tempfile::tempdir().unwrap();
    let root = src.path().join("proj");
    fs::create_dir_all(root.join("sub/empty")).unwrap();
    fs::write(root.join("a.txt"), b"hello").unwrap();
    fs::write(root.join("sub/b.bin"), vec![7u8; 300_000]).unwrap();

    let w = walk(&root, true).unwrap();
    assert_eq!(w.name, "proj");
    assert_eq!(w.bytes, 300_005);
    assert!(w.errors.is_empty());
    let mut rels: Vec<_> = w.entries.iter().map(|e| e.header.rel.as_str()).collect();
    rels.sort();
    assert_eq!(rels, ["", "a.txt", "sub", "sub/b.bin", "sub/empty"]);

    // Into an existing folder: lands in dst/proj.
    let dst = tempfile::tempdir().unwrap();
    let sink = Sink::new(dst.path(), &w.name).unwrap();
    for e in &w.entries {
        let (mut a, mut b) = tokio::io::duplex(64 * 1024);
        let send = async {
            let n = send_entry(&mut a, e, 9).await.unwrap();
            drop(a);
            n
        };
        let recv = async {
            let h: FileHeader = read_frame(&mut b).await.unwrap();
            assert_eq!(h.xfer, 9);
            sink.write(&h, &mut b).await.unwrap()
        };
        let (sent, got) = tokio::join!(send, recv);
        assert_eq!(sent, got);
    }
    let out = dst.path().join("proj");
    assert_eq!(fs::read(out.join("a.txt")).unwrap(), b"hello");
    assert_eq!(fs::read(out.join("sub/b.bin")).unwrap().len(), 300_000);
    assert!(out.join("sub/empty").is_dir());
    let src_mtime = fs::metadata(root.join("a.txt")).unwrap().modified().unwrap();
    let dst_mtime = fs::metadata(out.join("a.txt")).unwrap().modified().unwrap();
    let diff = src_mtime
        .duration_since(dst_mtime)
        .unwrap_or_else(|e| e.duration());
    assert!(diff.as_millis() < 2);
}

#[tokio::test]
async fn refuses_escaping_headers() {
    let dst = tempfile::tempdir().unwrap();
    let sink = Sink::new(&dst.path().join("new"), "x").unwrap();
    assert_eq!(sink.root(), dst.path().join("new"));
    let h = FileHeader {
        xfer: 1,
        rel: "../evil".into(),
        dir: false,
        size: 0,
        mode: None,
        mtime_ms: None,
    };
    assert!(sink.write(&h, &mut &b""[..]).await.is_err());
    assert!(Sink::new(dst.path(), "..").is_err());
}
