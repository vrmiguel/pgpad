use crate::{
    database::{Certificates, ConnectionDropNotifier},
    error::Error,
};

use anyhow::Context;
use tokio_postgres::{tls::MakeTlsConnect, Client, Connection, NoTls, Socket};
use tokio_postgres_rustls::MakeRustlsConnect;

pub async fn connect(
    config: &tokio_postgres::Config,
    certificates: &Certificates,
    ca_cert_path: Option<&str>,
    drop_notifier: ConnectionDropNotifier,
) -> Result<Client, Error> {
    connect_inner(
        config,
        certificates,
        ca_cert_path,
        ConnectionMode::Monitored(drop_notifier),
    )
    .await
}

pub async fn test_connection(
    config: &tokio_postgres::Config,
    certificates: &Certificates,
    ca_cert_path: Option<&str>,
) -> Result<(), Error> {
    connect_inner(
        config,
        certificates,
        ca_cert_path,
        ConnectionMode::Unmonitored,
    )
    .await?;
    Ok(())
}

enum ConnectionMode {
    Monitored(ConnectionDropNotifier),
    Unmonitored,
}

async fn connect_inner(
    config: &tokio_postgres::Config,
    certificates: &Certificates,
    ca_cert_path: Option<&str>,
    mode: ConnectionMode,
) -> Result<Client, Error> {
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

            match mode {
                ConnectionMode::Monitored(drop_notifier) => {
                    tokio::spawn(check_connection::<MakeRustlsConnect>(conn, drop_notifier));
                }
                ConnectionMode::Unmonitored => {
                    tokio::spawn(log_connection::<MakeRustlsConnect>(conn));
                }
            }

            client
        }
        // Mostly SslMode::Disable, but the enum was marked as non_exhaustive
        _other => {
            let (client, conn) = config
                .connect(NoTls)
                .await
                .with_context(|| format!("Failed to connect to Postgres '{config:?}'",))?;

            match mode {
                ConnectionMode::Monitored(drop_notifier) => {
                    tokio::spawn(check_connection::<NoTls>(conn, drop_notifier));
                }
                ConnectionMode::Unmonitored => {
                    tokio::spawn(log_connection::<NoTls>(conn));
                }
            }

            client
        }
    };

    Ok(client)
}

async fn check_connection<T>(
    conn: Connection<Socket, T::Stream>,
    drop_notifier: ConnectionDropNotifier,
) where
    T: MakeTlsConnect<Socket>,
{
    log_connection::<T>(conn).await;
    drop_notifier.notify();
}

async fn log_connection<T>(conn: Connection<Socket, T::Stream>)
where
    T: MakeTlsConnect<Socket>,
{
    let res = conn.await;
    log::info!("Connection finished");
    match res {
        Ok(()) => println!("Connected successfully"),
        Err(err) => eprintln!("Error or disconnect: {err:?}"),
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
