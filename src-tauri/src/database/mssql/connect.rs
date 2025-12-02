use std::sync::Arc;

use rustls::ClientConfig;
use rustls_pki_types::ServerName;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_util::compat::TokioAsyncReadCompatExt;
use url::Url;

use crate::database::Certificates;
use crate::Error;

pub type MssqlStream = tokio_util::compat::Compat<tokio_rustls::client::TlsStream<TcpStream>>;

pub fn parse_mssql_url(conn_str: &str) -> anyhow::Result<(Url, String, Option<String>)> {
    let url = Url::parse(conn_str)?;
    let user = url.username().to_string();
    let password = url.password().map(|p| p.to_string());
    Ok((url, user, password))
}

pub async fn connect(
    conn_str: &str,
    certificates: &Certificates,
    ca_cert_path: Option<&str>,
    password_override: Option<String>,
) -> Result<tiberius::Client<MssqlStream>, Error> {
    let (url, user, pwd_in_url) =
        parse_mssql_url(conn_str).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;

    let host = url.host_str().unwrap_or("localhost");
    let port = url.port().unwrap_or(1433);
    let database = url.path().trim_start_matches('/');
    let encrypt = url.query_pairs().any(|(k, v)| {
        k.eq_ignore_ascii_case("encrypt") && (v.eq_ignore_ascii_case("true") || v == "1")
    });

    let mut config = tiberius::Config::new();
    config.host(host);
    if !database.is_empty() {
        config.database(database);
    }
    config.port(port);

    let password = password_override.or(pwd_in_url).unwrap_or_default();
    config.authentication(tiberius::AuthMethod::sql_server(user, password));
    if encrypt {
        config.encryption(tiberius::EncryptionLevel::Required);
    }

    let tcp = TcpStream::connect((host, port))
        .await
        .map_err(|e| Error::Any(anyhow::anyhow!(format!("TCP connect failed: {}", e))))?;
    tcp.set_nodelay(true)
        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;

    let cert_store = if let Some(path) = ca_cert_path {
        certificates
            .with_custom_cert(path)
            .await
            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?
    } else {
        certificates
            .read()
            .await
            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?
    };

    let rustls_config = ClientConfig::builder()
        .with_root_certificates((*cert_store).clone())
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(rustls_config));
    let server_name = ServerName::try_from(host.to_string())
        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
    let tls = connector
        .connect(server_name, tcp)
        .await
        .map_err(|e| Error::Any(anyhow::anyhow!(format!("TLS connect failed: {}", e))))?;

    let client = tiberius::Client::connect(config, tls.compat())
        .await
        .map_err(|e| Error::Any(anyhow::anyhow!(format!("MSSQL connect failed: {}", e))))?;
    Ok(client)
}
