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

    if config.get_hosts().is_empty() {
        return Err(
            anyhow::anyhow!("No host provided in Postgres connection configuration").into(),
        );
    }

    let client = match config.get_ssl_mode() {
        SslMode::Require => {
            let certificate_store = if let Some(cert_path) = ca_cert_path {
                certificates.with_custom_cert(cert_path).await?
            } else if let Ok(env_path) = std::env::var("PGPAD_CA_CERT_PATH") {
                if !env_path.is_empty() {
                    certificates.with_custom_cert(&env_path).await?
                } else {
                    certificates.read().await?
                }
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

            if let Err(e) = client.batch_execute("SET application_name = 'pgpad'").await {
                log::warn!("Failed to set application_name: {}", e);
            }
            apply_optional_session_settings(&client).await;
            let conn_check =
                tauri::async_runtime::spawn(check_connection::<MakeRustlsConnect>(conn));

            (client, conn_check)
        }
        SslMode::Prefer => {
            let certificate_store = if let Some(cert_path) = ca_cert_path {
                certificates.with_custom_cert(cert_path).await?
            } else if let Ok(env_path) = std::env::var("PGPAD_CA_CERT_PATH") {
                if !env_path.is_empty() {
                    certificates.with_custom_cert(&env_path).await?
                } else {
                    certificates.read().await?
                }
            } else {
                certificates.read().await?
            };

            let rustls_config = rustls::ClientConfig::builder()
                .with_root_certificates(certificate_store)
                .with_no_client_auth();
            let tls = tokio_postgres_rustls::MakeRustlsConnect::new(rustls_config);

            match config.connect(tls).await {
                Ok((client, conn)) => {
                    if let Err(e) = client.batch_execute("SET application_name = 'pgpad'").await {
                        log::warn!("Failed to set application_name: {}", e);
                    }
                    apply_optional_session_settings(&client).await;
                    let conn_check =
                        tauri::async_runtime::spawn(check_connection::<MakeRustlsConnect>(conn));
                    (client, conn_check)
                }
                Err(e) => {
                    log::warn!(
                        "TLS connection failed under sslmode=Prefer, falling back to NoTls: {}",
                        e
                    );
                    let (client, conn) = config
                        .connect(NoTls)
                        .await
                        .with_context(|| format!("Failed to connect to Postgres '{config:?}'"))?;
                    let conn_check = tauri::async_runtime::spawn(check_connection::<NoTls>(conn));
                    (client, conn_check)
                }
            }
        }
        // Other modes (including SslMode::Disable)
        _other => {
            let (client, conn) = config
                .connect(NoTls)
                .await
                .with_context(|| format!("Failed to connect to Postgres '{config:?}'",))?;

            if let Err(e) = client.batch_execute("SET application_name = 'pgpad'").await {
                log::warn!("Failed to set application_name: {}", e);
            }
            apply_optional_session_settings(&client).await;
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

async fn apply_optional_session_settings(client: &Client) {
    if let Ok(v) = std::env::var("PGPAD_STATEMENT_TIMEOUT_MS") {
        if let Ok(ms) = v.parse::<u64>() {
            if ms > 0 {
                let stmt = format!("SET statement_timeout = '{}ms'", ms);
                if let Err(e) = client.batch_execute(&stmt).await {
                    log::warn!("Failed to set statement_timeout: {}", e);
                }
            }
        }
    }
    if let Ok(v) = std::env::var("PGPAD_IDLE_TX_TIMEOUT_MS") {
        if let Ok(ms) = v.parse::<u64>() {
            if ms > 0 {
                let stmt = format!("SET idle_in_transaction_session_timeout = '{}ms'", ms);
                if let Err(e) = client.batch_execute(&stmt).await {
                    log::warn!("Failed to set idle_in_transaction_session_timeout: {}", e);
                }
            }
        }
    }
    if let Ok(v) = client.query_one("SHOW server_version", &[]).await {
        let ver: &str = v.get(0);
        log::info!("Connected to Postgres server_version={}", ver);
    }
    if let Ok(sp) = std::env::var("PGPAD_SEARCH_PATH") {
        if !sp.trim().is_empty() {
            let stmt = format!("SET search_path = {}", sp);
            if let Err(e) = client.batch_execute(&stmt).await {
                log::warn!("Failed to set search_path: {}", e);
            }
        }
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
