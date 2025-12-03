use anyhow::Context;
use rusqlite::Connection;
use secrecy::{ExposeSecret, SecretString};

pub fn apply_cipher_settings(conn: &Connection, key: &SecretString) -> anyhow::Result<()> {
    conn.pragma_update(None, "key", key.expose_secret())
        .context("Failed to apply SQLCipher key")?;
    conn.pragma_update(None, "cipher_compatibility", 4)
        .context("Failed to set cipher_compatibility")?;
    conn.pragma_update(None, "cipher_page_size", 4096)
        .context("Failed to set cipher_page_size")?;
    conn.pragma_update(None, "kdf_iter", 256000)
        .context("Failed to set kdf_iter")?;
    let _ = conn.execute("PRAGMA cipher_use_hmac = ON", []);
    let _ = conn.execute("PRAGMA cipher_memory_security = ON", []);
    Ok(())
}

pub fn apply_common_settings(conn: &Connection) -> anyhow::Result<()> {
    let busy = std::env::var("PGPAD_SQLITE_BUSY_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30000);
    conn.execute_batch(&format!(
        "PRAGMA foreign_keys = ON;\nPRAGMA journal_mode = WAL;\nPRAGMA synchronous = FULL;\nPRAGMA busy_timeout = {};\nPRAGMA case_sensitive_like = ON;\nPRAGMA extended_result_codes = ON;",
        busy
    ))
    .context("Failed to apply common SQLite settings")?;
    if let Ok(mode) = std::env::var("PGPAD_SQLITE_JOURNAL_MODE") {
        if !mode.trim().is_empty() {
            let sql = format!("PRAGMA journal_mode = {}", mode);
            let _ = conn.execute_batch(&sql);
        }
    }
    if let Ok(sync) = std::env::var("PGPAD_SQLITE_SYNCHRONOUS") {
        if !sync.trim().is_empty() {
            let sql = format!("PRAGMA synchronous = {}", sync);
            let _ = conn.execute_batch(&sql);
        }
    }
    Ok(())
}

pub fn verify_cipher_ok(conn: &Connection) -> anyhow::Result<()> {
    let ck = conn
        .pragma_query_value(None, "cipher_integrity_check", |row| {
            row.get::<_, String>(0)
        })
        .unwrap_or_else(|_| String::from("error"));
    anyhow::ensure!(ck == "ok", "wrong key or not SQLCipher");
    Ok(())
}

pub fn attach_and_export_plain_to_encrypted(
    conn: &Connection,
    new_path: &std::path::Path,
    key: &SecretString,
) -> anyhow::Result<()> {
    let attach_sql = format!(
        "ATTACH DATABASE '{}' AS cipher_db KEY '{}';",
        new_path.display(),
        key.expose_secret()
    );
    conn.execute_batch(&attach_sql)
        .context("Failed to attach encrypted database")?;
    conn.execute_batch("SELECT sqlcipher_export('cipher_db');")
        .context("Failed to export to encrypted database")?;
    conn.execute_batch("DETACH DATABASE cipher_db;")
        .context("Failed to detach encrypted database")?;
    Ok(())
}
