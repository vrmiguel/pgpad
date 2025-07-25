use crate::error::Error;
use crate::postgres::tls::load_certificates;

use tokio_postgres_rustls::MakeRustlsConnect;
use tokio_postgres::{tls::MakeTlsConnect, Client, Connection, NoTls, Socket};

pub async fn connect(connection_string: &str) -> Result<Client, Error> {
    use tokio_postgres::config::SslMode;

    let config: tokio_postgres::Config = connection_string.parse()?;

    let client = match config.get_ssl_mode() {
        SslMode::Require | SslMode::Prefer => {
            let certificate_store = load_certificates().await;
            let rustls_config = rustls::ClientConfig::builder()
                .with_root_certificates(certificate_store)
                .with_no_client_auth();
            let tls = tokio_postgres_rustls::MakeRustlsConnect::new(rustls_config);
            let (client, conn) = config.connect(tls).await?;

            tokio::spawn(check_connection::<MakeRustlsConnect>(conn));

            client
        },
        // Mostly SslMode::Disable, but the enum was marked as non_exhaustive
        _other => {
            let (client, conn) = config.connect(NoTls).await?;

            tokio::spawn(check_connection::<NoTls>(conn));  

            client
        }
    };

    Ok(client)
}

async fn check_connection<T>(conn: Connection<Socket, T::Stream>)
where
    T: MakeTlsConnect<Socket>,
{
    match conn.await {
        Ok(()) => println!("Connected successfully"),
        Err(err) => eprintln!("Failed to connect to Postgres: {err}"),
    }
}