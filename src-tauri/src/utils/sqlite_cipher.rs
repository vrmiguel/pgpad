use anyhow::Context;
use rusqlite::Connection;
use secrecy::{ExposeSecret, SecretString};

#[derive(Default)]
pub struct CipherSettings {
    pub kdf_iter: Option<u32>,
    pub page_size: Option<u32>,
    pub use_hmac: Option<bool>,
    pub plaintext_header_size: Option<u32>,
    pub compatibility: Option<u32>,
    pub vacuum_after_pagesize: Option<bool>,
}

pub fn apply_cipher_settings_with(
    conn: &Connection,
    key: &SecretString,
    cfg: &CipherSettings,
) -> anyhow::Result<()> {
    conn.pragma_update(None, "key", key.expose_secret())
        .context("Failed to apply SQLCipher key")?;
    let compat = cfg.compatibility.unwrap_or(4);
    conn.pragma_update(None, "cipher_compatibility", compat)
        .context("Failed to set cipher_compatibility")?;
    let page = cfg.page_size.unwrap_or(4096);
    conn.pragma_update(None, "cipher_page_size", page)
        .context("Failed to set cipher_page_size")?;
    let kdf = cfg.kdf_iter.unwrap_or(256000);
    conn.pragma_update(None, "kdf_iter", kdf)
        .context("Failed to set kdf_iter")?;
    if cfg.use_hmac.unwrap_or(true) {
        let _ = conn.execute("PRAGMA cipher_use_hmac = ON", []);
    } else {
        let _ = conn.execute("PRAGMA cipher_use_hmac = OFF", []);
    }
    if let Some(p) = cfg.plaintext_header_size {
        let _ = conn.execute(&format!("PRAGMA cipher_plaintext_header_size = {}", p), []);
    }
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
    if let Ok(temp_store) = std::env::var("PGPAD_SQLITE_TEMP_STORE") {
        if !temp_store.trim().is_empty() {
            let _ = conn.execute_batch(&format!("PRAGMA temp_store = {}", temp_store));
        }
    }
    if let Ok(locking_mode) = std::env::var("PGPAD_SQLITE_LOCKING_MODE") {
        if !locking_mode.trim().is_empty() {
            let _ = conn.execute_batch(&format!("PRAGMA locking_mode = {}", locking_mode));
        }
    }
    if let Ok(autockpt) = std::env::var("PGPAD_SQLITE_WAL_AUTOCHECKPOINT") {
        if let Ok(v) = autockpt.parse::<i64>() {
            let _ = conn.execute_batch(&format!("PRAGMA wal_autocheckpoint = {}", v));
        }
    }
    if let Ok(limit) = std::env::var("PGPAD_SQLITE_JOURNAL_SIZE_LIMIT") {
        if let Ok(v) = limit.parse::<i64>() {
            let _ = conn.execute_batch(&format!("PRAGMA journal_size_limit = {}", v));
        }
    }
    if let Ok(mmap) = std::env::var("PGPAD_SQLITE_MMAP_SIZE") {
        if let Ok(v) = mmap.parse::<i64>() {
            let _ = conn.execute_batch(&format!("PRAGMA mmap_size = {}", v));
        }
    }
    if let Ok(cache) = std::env::var("PGPAD_SQLITE_CACHE_SIZE") {
        if let Ok(v) = cache.parse::<i64>() {
            let _ = conn.execute_batch(&format!("PRAGMA cache_size = {}", v));
        }
    }
    if let Ok(secdel) = std::env::var("PGPAD_SQLITE_SECURE_DELETE") {
        if !secdel.trim().is_empty() {
            let _ = conn.execute_batch(&format!("PRAGMA secure_delete = {}", secdel));
        }
    }
    Ok(())
}

pub fn apply_cipher_defaults(conn: &Connection, cfg: &CipherSettings) -> anyhow::Result<()> {
    if let Some(v) = cfg.kdf_iter {
        let _ = conn.execute("PRAGMA cipher_default_kdf_iter = ?1", [v]);
    }
    if let Some(v) = cfg.page_size {
        let _ = conn.execute("PRAGMA cipher_default_page_size = ?1", [v]);
    }
    match cfg.use_hmac.unwrap_or(true) {
        true => {
            let _ = conn.execute("PRAGMA cipher_default_use_hmac = ON", []);
        }
        false => {
            let _ = conn.execute("PRAGMA cipher_default_use_hmac = OFF", []);
        }
    }
    if let Some(v) = cfg.plaintext_header_size {
        let _ = conn.execute(
            &format!("PRAGMA cipher_default_plaintext_header_size = {}", v),
            [],
        );
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

pub fn attach_and_export_plain_to_encrypted_with(
    conn: &Connection,
    new_path: &std::path::Path,
    key: &SecretString,
    cfg: &CipherSettings,
) -> anyhow::Result<()> {
    let path_str = new_path.display().to_string().replace("'", "''");
    let key_str = key.expose_secret().replace("'", "''");
    let attach_sql = format!(
        "ATTACH DATABASE '{}' AS cipher_db KEY '{}';",
        path_str, key_str
    );
    conn.execute_batch(&attach_sql)
        .context("Failed to attach encrypted database")?;
    if let Some(page) = cfg.page_size {
        let _ = conn.execute("PRAGMA cipher_db.cipher_page_size = ?1", [page]);
    }
    if let Some(kdf) = cfg.kdf_iter {
        let _ = conn.execute("PRAGMA cipher_db.kdf_iter = ?1", [kdf]);
    }
    match cfg.use_hmac.unwrap_or(true) {
        true => {
            let _ = conn.execute("PRAGMA cipher_db.cipher_use_hmac = ON", []);
        }
        false => {
            let _ = conn.execute("PRAGMA cipher_db.cipher_use_hmac = OFF", []);
        }
    }
    if let Some(p) = cfg.plaintext_header_size {
        let _ = conn.execute(
            &format!("PRAGMA cipher_db.cipher_plaintext_header_size = {}", p),
            [],
        );
    }
    conn.execute_batch("SELECT sqlcipher_export('cipher_db');")
        .context("Failed to export to encrypted database")?;
    if cfg.vacuum_after_pagesize.unwrap_or(false) {
        let _ = conn.execute_batch("VACUUM");
    }
    conn.execute_batch("DETACH DATABASE cipher_db;")
        .context("Failed to detach encrypted database")?;
    Ok(())
}
