use std::{io::Cursor, sync::Arc};

use tokio::sync::OnceCell;

pub const RDS_CERTIFICATES: &str = include_str!("../../../../certs/aws-rds-global-bundle.pem");
pub const AZURE_CERTIFICATES: &str = include_str!("../../../../certs/azure-baltimore-root.pem");

#[derive(Clone)]
pub struct Certificates {
    pub certs: Arc<OnceCell<Arc<rustls::RootCertStore>>>,
}

impl Certificates {
    pub fn new() -> Self {
        Self {
            certs: Arc::new(OnceCell::new()),
        }
    }

    pub async fn read(&self) -> anyhow::Result<Arc<rustls::RootCertStore>> {
        Ok(self
            .certs
            .get_or_init(|| async { Arc::new(load_certificates().await) })
            .await
            .clone())
    }

    pub async fn with_custom_cert(
        &self,
        cert_path: &str,
    ) -> anyhow::Result<Arc<rustls::RootCertStore>> {
        use std::fs::File;
        use std::io::BufReader;

        let base_store = self.read().await?;
        let mut cert_store = (*base_store).clone();

        let cert_file = File::open(cert_path).map_err(|e| {
            anyhow::anyhow!("Failed to open certificate file '{}': {}", cert_path, e)
        })?;

        let mut reader = BufReader::new(cert_file);
        let certs = rustls_pemfile::certs(&mut reader);

        let mut cert_count = 0;
        for cert_result in certs {
            match cert_result {
                Ok(cert) => {
                    cert_store.add_parsable_certificates([cert]);
                    cert_count += 1;
                }
                Err(e) => {
                    log::warn!("Failed to parse certificate in file '{}': {}", cert_path, e);
                }
            }
        }

        if cert_count == 0 {
            anyhow::bail!(
                "No valid certificates found in file '{}'. Please ensure the file is in PEM format.",
                cert_path
            );
        }

        log::info!(
            "Loaded {} custom certificate(s) from '{}'",
            cert_count,
            cert_path
        );

        Ok(Arc::new(cert_store))
    }
}

async fn load_certificates() -> rustls::RootCertStore {
    let mut cert_store = webpki_certificates();

    let native_certs_handle = tokio::task::spawn_blocking(rustls_native_certs::load_native_certs);

    if let Err(e) = load_pem_certificates(RDS_CERTIFICATES, &mut cert_store) {
        eprintln!("Error loading RDS certificates: {}", e);
    }

    if let Err(e) = load_pem_certificates(AZURE_CERTIFICATES, &mut cert_store) {
        eprintln!("Error loading Azure certificates: {}", e);
    }

    if let Ok(native_certs) = native_certs_handle.await {
        cert_store.add_parsable_certificates(native_certs.certs);
    } else {
        eprintln!("Error loading native certificates");
    }

    cert_store
}

fn load_pem_certificates(
    pem_data: &str,
    cert_store: &mut rustls::RootCertStore,
) -> anyhow::Result<()> {
    let mut cursor = Cursor::new(pem_data.as_bytes());
    let certs = rustls_pemfile::certs(&mut cursor);

    for cert in certs {
        cert_store.add_parsable_certificates(cert);
    }

    Ok(())
}

fn webpki_certificates() -> rustls::RootCertStore {
    rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    }
}
