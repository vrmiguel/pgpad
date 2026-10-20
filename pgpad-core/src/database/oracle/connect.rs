use url::Url;

use crate::error::Error;

/// The parts of an Oracle connection string.
///
/// The wallet directory and the TNS alias travel as query parameters, so the
/// stored configuration is a single string.
pub struct OracleTarget {
    pub user: String,
    pub password: String,
    pub connect_str: String,
}

pub fn parse_target(
    connection_string: &str,
    password_override: Option<String>,
) -> Result<OracleTarget, Error> {
    let url = Url::parse(connection_string)
        .map_err(|e| Error::Any(anyhow::anyhow!("Failed to parse connection string: {}", e)))?;

    let user = url.username().to_string();
    let password = url
        .password()
        .map(ToOwned::to_owned)
        .or(password_override)
        .unwrap_or_default();

    let host = url.host_str().unwrap_or("localhost");
    let port = url.port().unwrap_or(1521);
    let service = url.path().trim_start_matches('/');

    let mut wallet = None;
    let mut tns = None;
    for (key, value) in url.query_pairs() {
        match key.to_ascii_lowercase().as_str() {
            "wallet" => wallet = Some(value.into_owned()),
            "tns" => tns = Some(value.into_owned()),
            _ => {}
        }
    }

    let connect_str = if let Some(alias) = tns {
        alias
    } else if wallet.is_some() {
        format!("//{}:{}/{}", host, port, service)
    } else if url.scheme().eq_ignore_ascii_case("tcps") {
        format!("tcps://{}:{}/{}", host, port, service)
    } else {
        format!("//{}:{}/{}", host, port, service)
    };

    Ok(OracleTarget {
        user,
        password,
        connect_str,
    })
}

pub fn connect(user: &str, password: &str, connect_str: &str) -> Result<oracle::Connection, Error> {
    match oracle::Connection::connect(user, password, connect_str) {
        Ok(conn) => {
            let cache_size = std::env::var("ORACLE_STMT_CACHE_SIZE")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .filter(|&n| n > 0)
                .unwrap_or(64);
            let _ = conn.set_stmt_cache_size(cache_size);
            Ok(conn)
        }
        Err(e) => Err(Error::Any(anyhow::anyhow!(
            "Failed to connect to Oracle: {}",
            e
        ))),
    }
}
