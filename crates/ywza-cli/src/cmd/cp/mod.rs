//! `ywza cp`: copy files and folders between this machine and paired devices, scp style.

mod peer;
mod transfer;

use std::{path::PathBuf, time::Instant};

use anyhow::{bail, Result};

use peer::Peer;
use ywza_proto::client;

#[derive(Debug, PartialEq)]
enum Loc {
    Local(PathBuf),
    Remote { device: String, path: String },
}

/// `device:path` is on a paired device, `local:path` or a bare path on this machine. On
/// Windows a one-letter prefix is a drive (`E:\files`).
fn parse_loc(s: &str) -> Loc {
    if let Some((dev, path)) = s.split_once(':') {
        if dev == "local" {
            return Loc::Local(path.into());
        }
        let drive = cfg!(windows) && dev.len() == 1;
        if !drive && crate::parse_device(dev).is_ok() {
            return Loc::Remote {
                device: dev.into(),
                path: path.into(),
            };
        }
    }
    Loc::Local(s.into())
}

#[derive(Default)]
pub struct Stats {
    files: u64,
    bytes: u64,
    errors: u64,
}

impl Stats {
    fn error(&mut self, msg: String) {
        self.errors += 1;
        eprintln!("error: {msg}");
    }
}

pub async fn run(paths: &[String], recursive: bool) -> Result<()> {
    let (dst, srcs) = paths.split_last().expect("clap requires two paths");
    let dst = parse_loc(dst);
    let srcs: Vec<Loc> = srcs.iter().map(|s| parse_loc(s)).collect();
    if let (Loc::Local(d), true) = (&dst, srcs.len() > 1) {
        if !d.is_dir() {
            bail!(
                "{} is not a folder (needed for several sources)",
                d.display()
            );
        }
    }
    let mut names: Vec<&str> = Vec::new();
    for l in srcs.iter().chain([&dst]) {
        if let Loc::Remote { device, .. } = l {
            if !names.contains(&device.as_str()) {
                names.push(device);
            }
        }
    }
    if names.is_empty() {
        bail!("both sides are on this machine; use your shell's copy");
    }

    let start = Instant::now();
    let ep = client::bind_devices(&names).await?;
    let mut st = Stats::default();
    let mut r = Ok(());
    for (i, src) in srcs.iter().enumerate() {
        r = copy_one(&ep, i as u64 + 1, src, &dst, recursive, &mut st).await;
        if r.is_err() {
            break;
        }
    }
    client::shut(ep).await;
    r?;
    println!(
        "copied {} entries, {} in {:.1}s",
        st.files,
        human(st.bytes),
        start.elapsed().as_secs_f64()
    );
    if st.errors > 0 {
        bail!("{} entries failed", st.errors);
    }
    Ok(())
}

/// Each source gets its own connections, so a device can be both source and destination.
async fn copy_one(
    ep: &iroh::Endpoint,
    xfer: u64,
    src: &Loc,
    dst: &Loc,
    recursive: bool,
    st: &mut Stats,
) -> Result<()> {
    let dial = |name: &str| {
        let name = name.to_string();
        async move {
            Ok::<_, anyhow::Error>(Peer::new(
                &name,
                client::dial(ep, &name, "ywza-cli").await?.0,
            ))
        }
    };
    match (src, dst) {
        (Loc::Local(_), Loc::Local(_)) => bail!("both sides are on this machine"),
        (Loc::Remote { device, path }, Loc::Local(d)) => {
            let mut p = dial(device.as_str()).await?;
            let r = transfer::pull(&mut p, xfer, path, recursive, d, st).await;
            p.close();
            r
        }
        (Loc::Local(s), Loc::Remote { device, path }) => {
            let mut p = dial(device.as_str()).await?;
            let r = transfer::push(&mut p, xfer, s, recursive, path, st).await;
            p.close();
            r
        }
        (
            Loc::Remote { device: a, path },
            Loc::Remote {
                device: b,
                path: dest,
            },
        ) => {
            let mut a = dial(a.as_str()).await?;
            let mut b = dial(b.as_str()).await?;
            let r = transfer::relay(&mut a, &mut b, xfer, path, recursive, dest, st).await;
            a.close();
            b.close();
            r
        }
    }
}

fn human(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations() {
        assert_eq!(
            parse_loc("vm:/home/vm/folder"),
            Loc::Remote {
                device: "vm".into(),
                path: "/home/vm/folder".into()
            }
        );
        assert_eq!(
            parse_loc("vm:"),
            Loc::Remote {
                device: "vm".into(),
                path: String::new()
            }
        );
        assert_eq!(parse_loc("local:/tmp/x"), Loc::Local("/tmp/x".into()));
        assert_eq!(parse_loc("./a"), Loc::Local("./a".into()));
        let drive = parse_loc("E:\\files");
        if cfg!(windows) {
            assert_eq!(drive, Loc::Local("E:\\files".into()));
        } else {
            assert!(matches!(drive, Loc::Remote { .. }));
        }
        assert_eq!(human(512), "512 B");
        assert_eq!(human(1536), "1.5 KiB");
    }
}
