use crate::{database::Certificates, error::Error};

use anyhow::Context;
use tauri::async_runtime::JoinHandle;
use tokio_postgres::{tls::MakeTlsConnect, Client, Connection, NoTls, Socket};
use tokio_postgres_rustls::MakeRustlsConnect;

pub type ConnectionCheck = JoinHandle<()>;

pub async fn connect(
    config: &tokio_postgres::Config,
    certificates: &Certificates,
    ca_cert_path: Option<&str>,
) -> Result<(Client, ConnectionCheck), Error> {
    use tokio_postgres::config::SslMode;

    let client = match config.get_ssl_mode() {
        SslMode::Require | SslMode::Prefer => {
            let certificate_store = if let Some(cert_path) = ca_cert_path {
                certificates.with_custom_cert(cert_path).await?
            } else {
                certificates.read().await?
            };

            let rustls_config = rustls::ClientConfig::builder()
                .with_root_certificates(certificate_store)
                .with_no_client_auth();
            let tls = tokio_postgres_rustls::MakeRustlsConnect::new(rustls_config);
            let (client, conn) = config
                .connect(tls)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to connect to Postgres: {}", e))?;

            let conn_check =
                tauri::async_runtime::spawn(check_connection::<MakeRustlsConnect>(conn));

            (client, conn_check)
        }
        // Mostly SslMode::Disable, but the enum was marked as non_exhaustive
        _other => {
            let (client, conn) = config
                .connect(NoTls)
                .await
                .with_context(|| format!("Failed to connect to Postgres '{config:?}'",))?;

            let conn_check = tauri::async_runtime::spawn(check_connection::<NoTls>(conn));

            (client, conn_check)
        }
    };

    Ok(client)
}

async fn check_connection<T>(conn: Connection<Socket, T::Stream>)
where
    T: MakeTlsConnect<Socket>,
{
    let res = conn.await;
    log::info!("Connection finished");
    match res {
        Ok(()) => log::info!("Connected successfully"),
        Err(err) => log::error!("Error or disconnect: {:?}", err),
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_connect() {
        let connection_string = "postgres://postgres@localhost:5432/postgres";
        let config: tokio_postgres::Config = connection_string.parse().unwrap();
        assert_eq!(config.get_password(), None);

        let connection_string = "postgres://postgres:postgres@localhost:5432/postgres";
        let config: tokio_postgres::Config = connection_string.parse().unwrap();
        assert_eq!(config.get_password(), Some(&b"postgres"[..]));
    }
}
