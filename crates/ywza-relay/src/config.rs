use std::{net::IpAddr, path::PathBuf};

use anyhow::{anyhow, bail, Result};
use iroh_relay::defaults::{DEFAULT_HTTPS_PORT, DEFAULT_HTTP_PORT, DEFAULT_RELAY_QUIC_PORT};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CertMode {
    Letsencrypt,
    Manual,
    #[serde(rename = "self-signed")]
    SelfSigned,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "yes")]
    pub tls: bool,
    pub hostname: Option<String>,
    pub cert_mode: Option<CertMode>,
    #[serde(default = "default_cert_dir")]
    pub cert_dir: PathBuf,
    pub manual_cert: Option<PathBuf>,
    pub manual_key: Option<PathBuf>,
    #[serde(default = "yes")]
    pub letsencrypt_prod: bool,
    pub contact: Option<String>,
    #[serde(default = "default_http_port")]
    pub http_port: u16,
    #[serde(default = "default_https_port")]
    pub https_port: u16,
    #[serde(default = "default_quic_port")]
    pub quic_port: u16,
    #[serde(default = "yes")]
    pub quic_addr_discovery: bool,
    #[serde(default)]
    pub log: crate::log::LogConfig,
}

fn default_cert_dir() -> PathBuf {
    "./certs".into()
}

fn yes() -> bool {
    true
}

fn default_http_port() -> u16 {
    DEFAULT_HTTP_PORT
}

fn default_https_port() -> u16 {
    DEFAULT_HTTPS_PORT
}

fn default_quic_port() -> u16 {
    DEFAULT_RELAY_QUIC_PORT
}

impl Config {
    /// The hostname and certificate mode. The hostname is required when `tls` is on; the mode
    /// defaults to self-signed for an IP address, which no CA will certify.
    pub fn tls_params(&self) -> Result<(&str, CertMode)> {
        let host = self
            .hostname
            .as_deref()
            .filter(|h| !h.is_empty())
            .ok_or_else(|| anyhow!("hostname is required when tls = true"))?;
        let mode = match self.cert_mode {
            Some(mode) => mode,
            None if host.parse::<IpAddr>().is_ok() => CertMode::SelfSigned,
            None => bail!("cert_mode is required when tls = true and hostname is not an IP address"),
        };
        Ok((host, mode))
    }

    pub fn cert_path(&self) -> PathBuf {
        self.manual_cert
            .clone()
            .unwrap_or_else(|| self.cert_dir.join("default.crt"))
    }

    pub fn key_path(&self) -> PathBuf {
        self.manual_key
            .clone()
            .unwrap_or_else(|| self.cert_dir.join("default.key"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_file_gets_the_defaults() {
        let c: Config =
            toml::from_str("hostname = \"relay.example.com\"\ncert_mode = \"letsencrypt\"").unwrap();
        assert_eq!(c.tls_params().unwrap(), ("relay.example.com", CertMode::Letsencrypt));
        assert!(c.tls);
        assert_eq!(c.cert_dir, PathBuf::from("./certs"));
        assert!(c.letsencrypt_prod && c.quic_addr_discovery);
        assert_eq!((c.http_port, c.https_port), (80, 443));
        assert_eq!(c.quic_port, DEFAULT_RELAY_QUIC_PORT);
        assert_eq!(c.contact, None);
        assert_eq!(c.cert_path(), PathBuf::from("./certs/default.crt"));
        assert_eq!(c.key_path(), PathBuf::from("./certs/default.key"));
    }

    #[test]
    fn full_file_overrides_the_defaults() {
        let c: Config = toml::from_str(
            r#"
            hostname = "r.example.com"
            cert_mode = "manual"
            cert_dir = "/etc/relay"
            manual_cert = "/x/c.pem"
            manual_key = "/x/k.pem"
            letsencrypt_prod = false
            contact = "me@example.com"
            http_port = 8080
            https_port = 8443
            quic_port = 9000
            quic_addr_discovery = false
            "#,
        )
        .unwrap();
        assert_eq!(c.tls_params().unwrap().1, CertMode::Manual);
        assert_eq!(c.cert_path(), PathBuf::from("/x/c.pem"));
        assert_eq!(c.key_path(), PathBuf::from("/x/k.pem"));
        assert!(!c.letsencrypt_prod && !c.quic_addr_discovery);
        assert_eq!((c.http_port, c.https_port, c.quic_port), (8080, 8443, 9000));
        assert_eq!(c.contact.as_deref(), Some("me@example.com"));
    }

    #[test]
    fn plain_http_needs_no_hostname_but_tls_does() {
        let c: Config = toml::from_str("tls = false
http_port = 3340").unwrap();
        assert!(!c.tls);
        assert_eq!(c.http_port, 3340);
        let c: Config = toml::from_str("cert_mode = \"manual\"").unwrap();
        assert!(c.tls_params().unwrap_err().to_string().contains("hostname"));
    }

    #[test]
    fn an_ip_hostname_defaults_to_self_signed() {
        let c: Config = toml::from_str("hostname = \"203.0.113.7\"").unwrap();
        assert_eq!(c.tls_params().unwrap(), ("203.0.113.7", CertMode::SelfSigned));
        let c: Config = toml::from_str("hostname = \"::1\"\ncert_mode = \"self-signed\"").unwrap();
        assert_eq!(c.tls_params().unwrap().1, CertMode::SelfSigned);
        let c: Config = toml::from_str("hostname = \"relay.example.com\"").unwrap();
        assert!(c.tls_params().unwrap_err().to_string().contains("cert_mode"));
    }

    #[test]
    fn unknown_keys_and_bad_modes_are_rejected() {
        assert!(toml::from_str::<Config>("hostname = \"h\"\ncert_mode = \"acme\"").is_err());
        assert!(
            toml::from_str::<Config>("hostname = \"h\"\ncert_mode = \"manual\"\nport = 1").is_err()
        );
    }
}
