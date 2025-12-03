use anyhow::Context;
use rusqlite::Connection;
use secrecy::{ExposeSecret, SecretString};
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroize;

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
    apply_key(conn, key)?;
    let strict = std::env::var("PGPAD_CIPHER_POLICY_STRICT")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let min_kdf = std::env::var("PGPAD_CIPHER_MIN_KDF")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(256000);
    let kdf = cfg.kdf_iter.unwrap_or(256000);
    let use_hmac = cfg.use_hmac.unwrap_or(true);
    if strict {
        if kdf < min_kdf {
            anyhow::bail!(
                "kdf_iter {} below minimum {} under strict policy",
                kdf,
                min_kdf
            );
        }
        if !use_hmac {
            anyhow::bail!("cipher_use_hmac must be ON under strict policy");
        }
    }
    let compat = cfg.compatibility.unwrap_or(4);
    conn.pragma_update(None, "cipher_compatibility", compat)
        .context("Failed to set cipher_compatibility")?;
    let page = cfg.page_size.unwrap_or(4096);
    conn.pragma_update(None, "cipher_page_size", page)
        .context("Failed to set cipher_page_size")?;
    conn.pragma_update(None, "kdf_iter", kdf)
        .context("Failed to set kdf_iter")?;
    if use_hmac {
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

fn apply_key(conn: &Connection, secret: &SecretString) -> anyhow::Result<()> {
    let raw = secret.expose_secret();
    let mut s = normalize_passphrase(raw);
    let is_raw_hex = s.starts_with("hex:")
        && s[4..].len().is_multiple_of(2)
        && s[4..].chars().all(|c| c.is_ascii_hexdigit());
    if is_raw_hex {
        let sql = format!("PRAGMA key = \"x'{}'\";", &s[4..]);
        conn.execute_batch(&sql)
            .context("Failed to apply hex SQLCipher key")?;
    } else {
        conn.pragma_update(None, "key", &s)
            .context("Failed to apply SQLCipher key")?;
    }
    s.zeroize();
    Ok(())
}

fn apply_key_for_db(conn: &Connection, db: &str, secret: &SecretString) -> anyhow::Result<()> {
    let raw = secret.expose_secret();
    let mut s = normalize_passphrase(raw);
    let is_raw_hex = s.starts_with("hex:")
        && s[4..].len().is_multiple_of(2)
        && s[4..].chars().all(|c| c.is_ascii_hexdigit());
    if is_raw_hex {
        let sql = format!("PRAGMA {}.key = \"x'{}'\";", db, &s[4..]);
        conn.execute_batch(&sql)
            .context("Failed to apply hex SQLCipher key to attached db")?;
    } else {
        conn.pragma_update(Some(db), "key", &s)
            .context("Failed to apply SQLCipher key to attached db")?;
    }
    s.zeroize();
    Ok(())
}

fn normalize_passphrase(p: &str) -> String {
    p.nfc().collect::<String>()
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
    let _ = conn.execute_batch(&format!(
        "PRAGMA foreign_keys = ON;\nPRAGMA journal_mode = WAL;\nPRAGMA synchronous = FULL;\nPRAGMA busy_timeout = {};\nPRAGMA case_sensitive_like = ON;\nPRAGMA extended_result_codes = ON;",
        busy
    ));

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
    if !has_sqlcipher(conn) {
        return Ok(());
    }
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
    let mut pass = normalize_passphrase(key.expose_secret());
    // Prefer attach without KEY then set key via attached alias to cover builds
    let attach_sql = format!("ATTACH DATABASE '{}' AS cipher_db;", path_str);
    let _ = crate::utils::sqlite_cipher::apply_cipher_defaults(conn, cfg);
    let attach_ok = conn.execute_batch(&attach_sql).is_ok();
    if attach_ok {
        // Apply key to attached alias
        apply_key_for_db(conn, "cipher_db", &SecretString::new(pass.clone()))?;
        // Force key derivation on the attached alias
        let _ = conn.query_row("SELECT COUNT(*) FROM cipher_db.sqlite_master", [], |r| {
            r.get::<_, i64>(0)
        });
        // Apply cipher settings to attached database alias
        let compat = cfg.compatibility.unwrap_or(4);
        let page = cfg.page_size.unwrap_or(4096);
        let kdf = cfg.kdf_iter.unwrap_or(256000);
        let _ = conn.execute_batch(&format!(
            "PRAGMA cipher_db.cipher_compatibility = {};
PRAGMA cipher_db.cipher_page_size = {};
PRAGMA cipher_db.kdf_iter = {};",
            compat, page, kdf
        ));
        if cfg.use_hmac.unwrap_or(true) {
            let _ = conn.execute("PRAGMA cipher_db.cipher_use_hmac = ON", []);
        } else {
            let _ = conn.execute("PRAGMA cipher_db.cipher_use_hmac = OFF", []);
        }
        if let Some(p) = cfg.plaintext_header_size {
            let _ = conn.execute(
                &format!("PRAGMA cipher_db.cipher_plaintext_header_size = {}", p),
                [],
            );
        }
        // Export after derivation
        conn.execute_batch("SELECT sqlcipher_export('cipher_db');")
            .context("Failed to export to encrypted database")?;
        if cfg.vacuum_after_pagesize.unwrap_or(false) {
            let _ = conn.execute_batch("VACUUM");
        }
        let _ = conn.execute_batch("DETACH DATABASE cipher_db;");
        pass.zeroize();
        return Ok(());
    }
    // Fallback (non-Windows): plain backup copy when SQLCipher attach path fails
    #[cfg(not(windows))]
    {
        let mut plain_dst =
            rusqlite::Connection::open(new_path).context("Failed to open destination database")?;
        let backup = rusqlite::backup::Backup::new(conn, &mut plain_dst)
            .context("Failed to start backup to destination database")?;
        backup.step(-1).context("Backup step failed")?;
        Ok(())
    }
    #[cfg(windows)]
    {
        // Fallback copy on Windows when SQLCipher attach is unavailable
        let mut dst_conn = rusqlite::Connection::open(new_path)
            .context("Failed to open destination database on Windows")?;
        let backup = rusqlite::backup::Backup::new(conn, &mut dst_conn)
            .context("Failed to start backup to destination database")?;
        backup.step(-1).context("Backup step failed")?;
        Ok(())
    }
}

pub fn has_sqlcipher(conn: &Connection) -> bool {
    conn.pragma_query_value(None, "cipher_version", |row| row.get::<_, String>(0))
        .is_ok()
}
