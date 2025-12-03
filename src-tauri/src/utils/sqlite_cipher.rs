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

#[derive(Default)]
pub struct CommonSettings {
    pub journal_mode: Option<String>,
    pub synchronous: Option<String>,
    pub temp_store: Option<String>,
    pub locking_mode: Option<String>,
    pub wal_autocheckpoint: Option<i64>,
    pub journal_size_limit: Option<i64>,
    pub mmap_size: Option<i64>,
    pub cache_size: Option<i64>,
    pub secure_delete: Option<String>,
    pub busy_timeout_ms: Option<u64>,
    pub case_sensitive_like: Option<bool>,
    pub extended_result_codes: Option<bool>,
}

pub fn apply_common_settings_with(conn: &Connection, cfg: &CommonSettings) -> anyhow::Result<()> {
    let busy = cfg.busy_timeout_ms.unwrap_or_else(|| {
        std::env::var("PGPAD_SQLITE_BUSY_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(30000)
    });
    conn.execute_batch(&format!(
        "PRAGMA foreign_keys = ON;\nPRAGMA journal_mode = WAL;\nPRAGMA synchronous = FULL;\nPRAGMA busy_timeout = {};\nPRAGMA case_sensitive_like = ON;\nPRAGMA extended_result_codes = ON;",
        busy
    ))
    .context("Failed to apply base SQLite settings")?;

    if let Some(v) = &cfg.journal_mode {
        if !v.trim().is_empty() {
            let _ = conn.execute_batch(&format!("PRAGMA journal_mode = {}", v));
        }
    }
    if let Some(v) = &cfg.synchronous {
        if !v.trim().is_empty() {
            let _ = conn.execute_batch(&format!("PRAGMA synchronous = {}", v));
        }
    }
    if let Some(v) = &cfg.temp_store {
        if !v.trim().is_empty() {
            let _ = conn.execute_batch(&format!("PRAGMA temp_store = {}", v));
        }
    }
    if let Some(v) = &cfg.locking_mode {
        if !v.trim().is_empty() {
            let _ = conn.execute_batch(&format!("PRAGMA locking_mode = {}", v));
        }
    }
    if let Some(v) = cfg.wal_autocheckpoint {
        let _ = conn.execute_batch(&format!("PRAGMA wal_autocheckpoint = {}", v));
    }
    if let Some(v) = cfg.journal_size_limit {
        let _ = conn.execute_batch(&format!("PRAGMA journal_size_limit = {}", v));
    }
    if let Some(v) = cfg.mmap_size {
        let _ = conn.execute_batch(&format!("PRAGMA mmap_size = {}", v));
    }
    if let Some(v) = cfg.cache_size {
        let _ = conn.execute_batch(&format!("PRAGMA cache_size = {}", v));
    }
    if let Some(v) = &cfg.secure_delete {
        if !v.trim().is_empty() {
            let _ = conn.execute_batch(&format!("PRAGMA secure_delete = {}", v));
        }
    }
    if let Some(v) = cfg.case_sensitive_like {
        let _ = conn.execute_batch(if v {
            "PRAGMA case_sensitive_like = ON"
        } else {
            "PRAGMA case_sensitive_like = OFF"
        });
    }
    if let Some(v) = cfg.extended_result_codes {
        let _ = conn.execute_batch(if v {
            "PRAGMA extended_result_codes = ON"
        } else {
            "PRAGMA extended_result_codes = OFF"
        });
    }

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

pub fn attach_and_export_plain_to_encrypted_with(
    conn: &Connection,
    new_path: &std::path::Path,
    key: &SecretString,
    cfg: &CipherSettings,
) -> anyhow::Result<()> {
    let path_str = new_path.display().to_string().replace("'", "''");
    let key_str = key.expose_secret().replace("'", "''");
    let _ = crate::utils::sqlite_cipher::apply_cipher_defaults(conn, cfg);
    let attach_sql = format!(
        "ATTACH DATABASE '{}' AS cipher_db KEY '{}';",
        path_str, key_str
    );
    if conn.execute_batch(&attach_sql).is_ok() {
        // Force key derivation on the attached alias
        let _ = conn.query_row(
            "SELECT COUNT(*) FROM cipher_db.sqlite_master",
            [],
            |r| r.get::<_, i64>(0),
        );
        // Minimal export path after derivation
        conn.execute_batch("SELECT sqlcipher_export('cipher_db');")
            .context("Failed to export to encrypted database")?;
        if cfg.vacuum_after_pagesize.unwrap_or(false) {
            let _ = conn.execute_batch("VACUUM");
        }
        let _ = conn.execute_batch("DETACH DATABASE cipher_db;");
        return Ok(());
    }
    // Fallback (non-Windows): use sqlite backup API to copy plain -> encrypted
    #[cfg(not(windows))]
    {
        let mut enc_conn = rusqlite::Connection::open(new_path)
            .context("Failed to open destination encrypted database")?;
        crate::utils::sqlite_cipher::apply_cipher_settings_with(&enc_conn, key, cfg)?;
        if cfg.vacuum_after_pagesize.unwrap_or(false) {
            let _ = enc_conn.execute_batch("VACUUM");
        }
        let backup = rusqlite::backup::Backup::new(conn, &mut enc_conn)
            .context("Failed to start backup to encrypted database")?;
        backup.step(-1).context("Backup step failed")?;
        return Ok(());
    }
    #[cfg(windows)]
    {
        // If we reach here on Windows, return an error for visibility
        anyhow::bail!("Failed to export using SQLCipher attach path on Windows");
    }
}
