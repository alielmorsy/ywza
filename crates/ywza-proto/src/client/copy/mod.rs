//! Copying files and folders between this machine and paired devices, scp style. Used by
//! `ywza cp`, the agent's AI and the phone app. Entries that fail are collected in
//! [`Stats::errors`]; a transfer that cannot go on at all is an `Err`.

mod peer;
mod transfer;

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use iroh::Endpoint;

use super::{dial, valid_device_name};
use peer::Peer;

const CLIENT_NAME: &str = "ywza-copy";

#[derive(Debug, PartialEq)]
pub enum Loc {
    Local(PathBuf),
    Remote { device: String, path: String },
}

/// `device:path` is on a paired device, `local:path` or a bare path on this machine. On
/// Windows a one-letter prefix is a drive (`E:\files`).
pub fn parse_loc(s: &str) -> Loc {
    if let Some((dev, path)) = s.split_once(':') {
        if dev == "local" {
            return Loc::Local(path.into());
        }
        let drive = cfg!(windows) && dev.len() == 1;
        if !drive && valid_device_name(dev) {
            return Loc::Remote {
                device: dev.into(),
                path: path.into(),
            };
        }
    }
    Loc::Local(s.into())
}

#[derive(Debug, Default)]
pub struct Stats {
    pub files: u64,
    pub bytes: u64,
    /// One line per entry that failed, `device:path: reason`.
    pub errors: Vec<String>,
}

impl Stats {
    fn error(&mut self, msg: String) {
        self.errors.push(msg);
    }
}

/// The paired devices named by `paths` (sources and destination), without repeats; what to
/// bind the endpoint for.
pub fn device_names(paths: &[String]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for p in paths {
        if let Loc::Remote { device, .. } = parse_loc(p) {
            if !names.contains(&device) {
                names.push(device);
            }
        }
    }
    names
}

/// Copies every `srcs` entry to `dst`. The last-path rules of `scp` apply: several sources
/// need a folder destination. Stops at the first source that fails as a whole.
pub async fn copy(ep: &Endpoint, srcs: &[String], dst: &str, recursive: bool) -> Result<Stats> {
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
    if !srcs.iter().chain([&dst]).any(|l| matches!(l, Loc::Remote { .. })) {
        bail!("both sides are on this machine; use your shell's copy");
    }
    let mut st = Stats::default();
    for (i, src) in srcs.iter().enumerate() {
        let r = copy_one(ep, i as u64 + 1, src, &dst, recursive, &mut st).await;
        finish(r, &st)?;
    }
    Ok(st)
}

/// A local file or folder to `dest` on `device`.
pub async fn push(
    ep: &Endpoint,
    device: &str,
    local: &Path,
    dest: &str,
    recursive: bool,
) -> Result<Stats> {
    let src = Loc::Local(local.into());
    let dst = Loc::Remote {
        device: device.into(),
        path: dest.into(),
    };
    one(ep, &src, &dst, recursive).await
}

/// `remote` on `device` into the local folder `local_dir`.
pub async fn pull(
    ep: &Endpoint,
    device: &str,
    remote: &str,
    local_dir: &Path,
    recursive: bool,
) -> Result<Stats> {
    let src = Loc::Remote {
        device: device.into(),
        path: remote.into(),
    };
    one(ep, &src, &Loc::Local(local_dir.into()), recursive).await
}

async fn one(ep: &Endpoint, src: &Loc, dst: &Loc, recursive: bool) -> Result<Stats> {
    let mut st = Stats::default();
    let r = copy_one(ep, 1, src, dst, recursive, &mut st).await;
    finish(r, &st)?;
    Ok(st)
}

/// A failed transfer keeps the entry errors that came before it.
fn finish(r: Result<()>, st: &Stats) -> Result<()> {
    r.map_err(|e| {
        let before: String = st.errors.iter().map(|m| format!("\n{m}")).collect();
        anyhow::anyhow!("{e:#}{before}")
    })
}

/// Each source gets its own connections, so a device can be both source and destination.
async fn copy_one(
    ep: &Endpoint,
    xfer: u64,
    src: &Loc,
    dst: &Loc,
    recursive: bool,
    st: &mut Stats,
) -> Result<()> {
    let open = |name: &str| {
        let name = name.to_string();
        async move {
            Ok::<_, anyhow::Error>(Peer::new(&name, dial(ep, &name, CLIENT_NAME).await?.0))
        }
    };
    match (src, dst) {
        (Loc::Local(_), Loc::Local(_)) => bail!("both sides are on this machine"),
        (Loc::Remote { device, path }, Loc::Local(d)) => {
            let mut p = open(device).await?;
            let r = transfer::pull(&mut p, xfer, path, recursive, d, st).await;
            p.close();
            r
        }
        (Loc::Local(s), Loc::Remote { device, path }) => {
            let mut p = open(device).await?;
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
            let mut a = open(a).await?;
            let mut b = open(b).await?;
            let r = transfer::relay(&mut a, &mut b, xfer, path, recursive, dest, st).await;
            a.close();
            b.close();
            r
        }
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
    }

    #[test]
    fn device_names_are_unique_and_skip_local_paths() {
        let paths: Vec<String> = ["vm:/a", "./b", "pc:x", "vm:/c", "local:/d"]
            .map(String::from)
            .into();
        assert_eq!(device_names(&paths), ["vm", "pc"]);
    }

    #[test]
    fn a_failed_transfer_keeps_the_entry_errors_before_it() {
        let mut st = Stats::default();
        st.error("vm:/a: denied".into());
        let e = finish(Err(anyhow::anyhow!("vm: gone")), &st).unwrap_err();
        assert_eq!(e.to_string(), "vm: gone\nvm:/a: denied");
        assert!(finish(Ok(()), &st).is_ok());
    }
}
