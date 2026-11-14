//! `ywza cp`: copy files and folders between this machine and paired devices, scp style.

use std::time::Instant;

use anyhow::{bail, Result};
use ywza_proto::client::{self, copy};

pub async fn run(paths: &[String], recursive: bool) -> Result<()> {
    let (dst, srcs) = paths.split_last().expect("clap requires two paths");
    let names = copy::device_names(paths);
    let names: Vec<&str> = names.iter().map(String::as_str).collect();

    let start = Instant::now();
    let ep = client::bind_devices(&names).await?;
    let r = copy::copy(&ep, srcs, dst, recursive).await;
    client::shut(ep).await;
    let st = r?;
    for e in &st.errors {
        eprintln!("error: {e}");
    }
    println!(
        "copied {} entries, {} in {:.1}s",
        st.files,
        human(st.bytes),
        start.elapsed().as_secs_f64()
    );
    if !st.errors.is_empty() {
        bail!("{} entries failed", st.errors.len());
    }
    Ok(())
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
    fn sizes() {
        assert_eq!(human(512), "512 B");
        assert_eq!(human(1536), "1.5 KiB");
    }
}
