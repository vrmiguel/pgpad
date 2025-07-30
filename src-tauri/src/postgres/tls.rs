use std::{
    io::Cursor,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::ensure;
use tauri::async_runtime::{spawn, spawn_blocking};
use tokio::sync::OnceCell;

pub const RDS_CERTIFICATES: &str = include_str!("../../../certs/aws-rds-global-bundle.pem");
pub const AZURE_CERTIFICATES: &str = include_str!("../../../certs/azure-baltimore-root.pem");

pub struct Certificates {
    pub certs: Arc<OnceCell<Arc<rustls::RootCertStore>>>,
}

impl Certificates {
    /// Returns a 'handle' to the certificates, while loading them in the background.
    pub fn new() -> Self {
        let certs = Arc::new(OnceCell::new());

        let certs_for_init = certs.clone();
        spawn(async move {
            let now = Instant::now();
            let certificates = load_certificates().await;
            println!("Certificates loaded in {:?}ms", now.elapsed().as_millis());

            // rustls wants an Arc in `with_root_certificates` so let's just create it once here
            // ¯\_(ツ)_/¯
            let certificates = Arc::new(certificates);

            certs_for_init
                .set(certificates)
                .expect("Certificates set twice");
        });

        Self { certs }
    }

    pub async fn read(&self) -> anyhow::Result<Arc<rustls::RootCertStore>> {
        let mut max_wait = 10;
        while !self.certs.initialized() {
            tokio::time::sleep(Duration::from_millis(100)).await;
            max_wait -= 1;
            if max_wait == 0 {
                panic!("Certificates not initialized");
            }

            ensure!(
                max_wait > 0,
                "Certificates not initialized within the expected time"
            );
        }

        // Safety: `initialized` just returned true
        Ok(self.certs.get().unwrap().clone())
    }
}

async fn load_certificates() -> rustls::RootCertStore {
    let mut cert_store = webpki_certificates();

    let native_certs_handle = spawn_blocking(rustls_native_certs::load_native_certs);

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
