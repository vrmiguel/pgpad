use std::ops::Not;

use anyhow::bail;
use tokio::task::spawn_blocking;

pub async fn load_certificates() -> rustls::RootCertStore {
    load_native_certificates().await.unwrap_or_else(|err| {
        eprintln!("{err}");
        webpki_certificates()
    })
}

async fn load_native_certificates() -> anyhow::Result<rustls::RootCertStore> {
    let certificate_result = spawn_blocking(rustls_native_certs::load_native_certs).await?;

    if certificate_result.errors.is_empty().not() {
        bail!(
            "Failed to find native certificates: {:?}",
            certificate_result.errors
        );
    }
    let native_certificates = certificate_result.certs;

    let mut roots = rustls::RootCertStore::empty();
    roots.add_parsable_certificates(native_certificates);
    Ok(roots)
}

pub fn webpki_certificates() -> rustls::RootCertStore {
    rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    }
}