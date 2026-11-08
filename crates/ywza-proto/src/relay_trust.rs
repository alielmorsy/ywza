//! Trust for a custom relay without a public CA: `relay_url#pin=<fingerprint>`, or else
//! trust on first use recorded in `known_relays.json`. Other hosts get iroh's default checks.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use data_encoding::BASE32_NOPAD;
use iroh::{endpoint::Builder, tls::CaTlsConfig, RelayUrl};
use rustls::{
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::CryptoProvider,
    pki_types::{CertificateDer, ServerName, UnixTime},
    DigitallySignedStruct, SignatureScheme,
};
use sha2::{Digest, Sha256};

use crate::{config_dir, relay_mode, Error};

const STORE: &str = "known_relays.json";

/// Lowercase base32 (no padding) of the SHA-256 of a DER certificate.
pub fn fingerprint(der: &[u8]) -> String {
    BASE32_NOPAD
        .encode(&Sha256::digest(der))
        .to_ascii_lowercase()
}

/// Splits `url#pin=<fingerprint>` into the plain url and the pin.
pub fn split_pin(s: &str) -> Result<(&str, Option<String>), Error> {
    let s = s.trim();
    let Some((url, frag)) = s.split_once('#') else {
        return Ok((s, None));
    };
    match frag
        .strip_prefix("pin=")
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        Some(pin) => Ok((url, Some(pin.to_ascii_lowercase()))),
        None => Err(Error::RelayUrl(
            s.into(),
            "the only supported fragment is #pin=<fingerprint>".into(),
        )),
    }
}

#[derive(Debug, Clone)]
struct Target {
    host: String,
    key: String,
    pin: Option<String>,
}

impl Target {
    fn matches(&self, name: &ServerName<'_>) -> bool {
        match name {
            ServerName::IpAddress(ip) => std::net::IpAddr::from(*ip).to_string() == self.host,
            other => other.to_str().eq_ignore_ascii_case(&self.host),
        }
    }
}

/// The https relay to special-case; `None` for plain http, which has no TLS to check.
fn target(relay: &str) -> Result<Option<Target>, Error> {
    let (clean, pin) = split_pin(relay)?;
    let url: RelayUrl = clean
        .parse()
        .map_err(|e: iroh::RelayUrlParseError| Error::RelayUrl(relay.into(), e.to_string()))?;
    if url.scheme() != "https" {
        return Ok(None);
    }
    let raw = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let port = url.port_or_known_default().unwrap_or(443);
    Ok(Some(Target {
        host: raw.trim_matches(|c| c == '[' || c == ']').into(),
        key: format!("{raw}:{port}"),
        pin,
    }))
}

fn load(path: &Path) -> Result<BTreeMap<String, String>, String> {
    match std::fs::read_to_string(path) {
        Ok(t) => {
            serde_json::from_str(&t).map_err(|e| format!("{} is invalid: {e}", path.display()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(format!("reading {}: {e}", path.display())),
    }
}

/// Trust on first use: stores `fp` for `key`, accepts the stored one, rejects a different one.
fn remember(path: &Path, key: &str, fp: &str) -> Result<(), String> {
    let mut known = load(path)?;
    match known.get(key) {
        Some(k) if k == fp => return Ok(()),
        Some(k) => {
            return Err(format!(
                "relay {key} now presents certificate {fp} but {} trusts {k}; if the relay was \
                 reinstalled, delete its entry in that file, otherwise the connection may be \
                 intercepted",
                path.display()
            ))
        }
        None => known.insert(key.into(), fp.into()),
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&known).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| format!("writing {}: {e}", path.display()))
}

#[derive(Debug)]
struct Verifier {
    target: Target,
    store: PathBuf,
    inner: Arc<dyn ServerCertVerifier>,
}

impl ServerCertVerifier for Verifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let normal = || {
            self.inner
                .verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now)
        };
        if !self.target.matches(server_name) {
            return normal();
        }
        let fp = fingerprint(end_entity);
        match &self.target.pin {
            Some(pin) if *pin == fp => {}
            Some(pin) => {
                return Err(rustls::Error::General(format!(
                    "relay {} presented certificate {fp}, not the pinned {pin}",
                    self.target.key
                )))
            }
            // A CA-signed certificate (a renewed Let's Encrypt one) needs no recorded fingerprint.
            None if normal().is_ok() => {}
            None => {
                remember(&self.store, &self.target.key, &fp).map_err(rustls::Error::General)?
            }
        }
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

/// Applies a configured relay to the builder: the relay map, and for an https relay the
/// pin / trust-on-first-use certificate check. `None` and plain http keep iroh's defaults.
pub fn with_relay(mut b: Builder, relay: Option<&str>) -> Result<Builder, Error> {
    let Some(relay) = relay else { return Ok(b) };
    if let Some(mode) = relay_mode(Some(relay))? {
        b = b.relay_mode(mode);
    }
    if let Some(target) = target(relay)? {
        let store = config_dir().join(STORE);
        b = b.ca_tls_config(CaTlsConfig::custom_server_cert_verifier(Arc::new(
            move |provider: Arc<CryptoProvider>| {
                let inner = CaTlsConfig::default().server_cert_verifier(provider)?;
                let verifier: Arc<dyn ServerCertVerifier> = Arc::new(Verifier {
                    target: target.clone(),
                    store: store.clone(),
                    inner,
                });
                Ok(verifier)
            },
        )));
    }
    Ok(b)
}

/// The relay as other devices should configure it: a configured pin stays, an unpinned https
/// relay gets the fingerprint this machine already trusts, if any.
pub fn shareable_relay(relay: &str) -> String {
    let known = || {
        let t = target(relay).ok()??;
        let fp = load(&config_dir().join(STORE)).ok()?.remove(&t.key)?;
        Some(format!("{}#pin={fp}", split_pin(relay).ok()?.0))
    };
    match target(relay) {
        Ok(Some(t)) if t.pin.is_none() => known().unwrap_or_else(|| relay.trim().into()),
        _ => relay.trim().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_lowercase_unpadded_base32_of_sha256() {
        let fp = fingerprint(b"");
        assert_eq!(fp, "4oymiquy7qobjgx36tejs35zeqt24qpemsnzgtfeswmrw6csxbkq");
        assert_eq!(fp.len(), 52);
    }

    #[test]
    fn pin_is_split_off_the_url() {
        assert_eq!(
            split_pin("https://1.2.3.4:443").unwrap(),
            ("https://1.2.3.4:443", None)
        );
        assert_eq!(
            split_pin(" https://1.2.3.4:443#pin=ABC123 ").unwrap(),
            ("https://1.2.3.4:443", Some("abc123".into()))
        );
        assert!(matches!(
            split_pin("https://h#other"),
            Err(Error::RelayUrl(..))
        ));
        assert!(matches!(
            split_pin("https://h#pin="),
            Err(Error::RelayUrl(..))
        ));
    }

    #[test]
    fn target_covers_https_only() {
        let t = target("https://[::1]:8443#pin=x").unwrap().unwrap();
        assert_eq!((t.host.as_str(), t.key.as_str()), ("::1", "[::1]:8443"));
        assert_eq!(t.pin.as_deref(), Some("x"));
        assert!(t.matches(&ServerName::try_from("::1").unwrap()));
        assert!(!t.matches(&ServerName::try_from("example.com").unwrap()));
        assert_eq!(
            target("https://Relay.Example.com").unwrap().unwrap().key,
            "relay.example.com:443"
        );
        assert!(target("http://1.2.3.4:3340").unwrap().is_none());
    }

    #[test]
    fn first_use_is_stored_then_only_the_same_cert_is_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("known_relays.json");
        remember(&path, "1.2.3.4:443", "aaa").unwrap();
        assert!(path.exists());
        remember(&path, "1.2.3.4:443", "aaa").unwrap();
        let err = remember(&path, "1.2.3.4:443", "bbb").unwrap_err();
        assert!(err.contains("known_relays.json") && err.contains("aaa") && err.contains("bbb"));
        remember(&path, "5.6.7.8:443", "bbb").unwrap();
        assert_eq!(load(&path).unwrap().len(), 2);
    }
}
