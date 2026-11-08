//! A self-signed certificate for a relay with no CA-issued one. Devices trust it by pinning
//! its fingerprint (`relay_url = "https://host:port#pin=<fingerprint>"`).

use std::{
    fs,
    net::Ipv6Addr,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use data_encoding::BASE32_NOPAD;
use rcgen::{date_time_ymd, CertificateParams, KeyPair};
use sha2::{Digest, Sha256};

const SECS_PER_YEAR: u64 = 31_556_952;

/// `<dir>/self-signed.crt` and `.key`, generated for `host` on the first call and reused after.
pub fn ensure(dir: &Path, host: &str) -> Result<(PathBuf, PathBuf)> {
    let (crt, key) = (dir.join("self-signed.crt"), dir.join("self-signed.key"));
    if crt.exists() && key.exists() {
        return Ok((crt, key));
    }
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let mut params = CertificateParams::new(vec![host.to_string()])
        .with_context(|| format!("`{host}` is not a valid certificate name"))?;
    let year = 1970 + SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() / SECS_PER_YEAR;
    params.not_before = date_time_ymd(year as i32 - 1, 1, 1);
    params.not_after = date_time_ymd(year as i32 + 10, 1, 1);
    let pair = KeyPair::generate()?;
    let cert = params.self_signed(&pair)?;
    write_private(&key, &pair.serialize_pem())?;
    fs::write(&crt, cert.pem()).with_context(|| format!("writing {}", crt.display()))?;
    Ok((crt, key))
}

fn write_private(path: &Path, pem: &str) -> Result<()> {
    #[cfg(unix)]
    {
        use std::{io::Write, os::unix::fs::OpenOptionsExt};
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        f.write_all(pem.as_bytes())
            .with_context(|| format!("writing {}", path.display()))
    }
    #[cfg(not(unix))]
    fs::write(path, pem).with_context(|| format!("writing {}", path.display()))
}

/// Lowercase base32 (no padding) of the SHA-256 of the DER certificate.
pub fn fingerprint(der: &[u8]) -> String {
    BASE32_NOPAD
        .encode(&Sha256::digest(der))
        .to_ascii_lowercase()
}

/// The `relay_url` value devices put in their config to trust this relay.
pub fn relay_url(host: &str, port: u16, der: &[u8]) -> String {
    let host = match host.parse::<Ipv6Addr>() {
        Ok(_) => format!("[{host}]"),
        Err(_) => host.to_string(),
    };
    format!("https://{host}:{port}#pin={}", fingerprint(der))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_lowercase_unpadded_base32_of_sha256() {
        assert_eq!(
            fingerprint(b""),
            "4oymiquy7qobjgx36tejs35zeqt24qpemsnzgtfeswmrw6csxbkq"
        );
    }

    #[test]
    fn relay_url_carries_port_and_pin_and_brackets_ipv6() {
        assert_eq!(
            relay_url("203.0.113.7", 443, b""),
            "https://203.0.113.7:443#pin=4oymiquy7qobjgx36tejs35zeqt24qpemsnzgtfeswmrw6csxbkq"
        );
        assert!(relay_url("2001:db8::1", 8443, b"").starts_with("https://[2001:db8::1]:8443#pin="));
        assert!(relay_url("relay.example.com", 443, b"").starts_with("https://relay.example.com:443#"));
    }

    #[test]
    fn certificate_is_generated_once_for_an_ip_or_a_name() {
        for host in ["203.0.113.7", "relay.example.com"] {
            let dir = tempfile::tempdir().unwrap();
            let (crt, key) = ensure(dir.path(), host).unwrap();
            let first = (fs::read(&crt).unwrap(), fs::read(&key).unwrap());
            assert!(first.0.starts_with(b"-----BEGIN CERTIFICATE-----"));
            ensure(dir.path(), host).unwrap();
            assert_eq!(first, (fs::read(&crt).unwrap(), fs::read(&key).unwrap()));
        }
    }
}
