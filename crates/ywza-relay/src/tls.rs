use std::{path::Path, sync::Arc};

use anyhow::{Context, Result};
use iroh_relay::server::{AcmeConfig, CertConfig};
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer},
    server::WantsServerCert,
    ConfigBuilder, ServerConfig,
};
use rustls_pki_types::pem::PemObject;

use crate::{
    config::{CertMode, Config},
    selfsigned,
};

fn builder() -> ConfigBuilder<ServerConfig, WantsServerCert> {
    ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .expect("protocols supported by ring")
        .with_no_client_auth()
}

fn load_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>> {
    CertificateDer::pem_file_iter(path)
        .with_context(|| format!("opening certificate {}", path.display()))?
        .collect::<Result<_, _>>()
        .with_context(|| format!("reading certificates from {}", path.display()))
}

fn load_key(path: &Path) -> Result<PrivateKeyDer<'static>> {
    PrivateKeyDer::from_pem_file(path)
        .with_context(|| format!("reading private key {}", path.display()))
}

fn manual(certs: Vec<CertificateDer<'static>>, key: PrivateKeyDer<'static>) -> Result<CertConfig> {
    let server_config = builder()
        .with_single_cert(certs, key)
        .context("certificate and key do not match")?;
    Ok(CertConfig::Manual { server_config })
}

pub fn cert_config(cfg: &Config) -> Result<CertConfig> {
    let (hostname, mode) = cfg.tls_params()?;
    match mode {
        CertMode::Manual => manual(load_certs(&cfg.cert_path())?, load_key(&cfg.key_path())?),
        CertMode::SelfSigned => {
            let (crt, key) = selfsigned::ensure(&cfg.cert_dir, hostname)?;
            let certs = load_certs(&crt)?;
            let first = certs.first().context("self-signed certificate file is empty")?;
            tracing::info!(
                "self-signed certificate {}; set on every device: relay_url = \"{}\"",
                crt.display(),
                selfsigned::relay_url(hostname, cfg.https_port, first)
            );
            manual(certs, load_key(&key)?)
        }
        CertMode::Letsencrypt => {
            let mut acme = AcmeConfig::letsencrypt(cfg.letsencrypt_prod)
                .domains(vec![hostname.to_string()])
                .cache_path(cfg.cert_dir.clone());
            if let Some(email) = &cfg.contact {
                acme = acme.contact(vec![format!("mailto:{email}")]);
            }
            Ok(CertConfig::LetsEncrypt {
                acme_config: acme,
                server_config_builder: builder(),
            })
        }
    }
}
