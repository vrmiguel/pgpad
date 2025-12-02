use crate::error::Error;

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
