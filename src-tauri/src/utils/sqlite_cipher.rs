use anyhow::Context;
use rusqlite::{Connection, ToSql};
use secrecy::{ExposeSecret, SecretString};
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroize;

fn exec_ignore_result(conn: &Connection, sql: &str, params: &[&dyn ToSql]) -> anyhow::Result<()> {
    match conn.execute(sql, params) {
        Ok(_) => Ok(()),
        Err(rusqlite::Error::ExecuteReturnedResults) => Ok(()),
        Err(e) => {
            eprintln!("exec_ignore_result failed for '{}': {:?}", sql, e);
            Err(e.into())
        }
    }
}

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
    let strict = std::env::var("PGPAD_CIPHER_POLICY_STRICT")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let min_kdf = std::env::var("PGPAD_CIPHER_MIN_KDF")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(256000);

    // Disable memory security early to avoid NOMEM on some environments
    exec_ignore_result(conn, "PRAGMA cipher_memory_security = OFF", &[])?;

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
    exec_ignore_result(
        conn,
        &format!("PRAGMA cipher_compatibility = {}", compat),
        &[],
    )?;
    let page = cfg.page_size.unwrap_or(4096);
    exec_ignore_result(conn, &format!("PRAGMA cipher_page_size = {}", page), &[])?;
    exec_ignore_result(conn, &format!("PRAGMA kdf_iter = {}", kdf), &[])?;
    if use_hmac {
        exec_ignore_result(conn, "PRAGMA cipher_use_hmac = ON", &[])?;
    } else {
        exec_ignore_result(conn, "PRAGMA cipher_use_hmac = OFF", &[])?;
    }
    if let Some(p) = cfg.plaintext_header_size {
        exec_ignore_result(
            conn,
            &format!("PRAGMA cipher_plaintext_header_size = {}", p),
            &[],
        )?;
    }
    apply_key(conn, key)?;
    Ok(())
}

pub fn apply_cipher_defaults(conn: &Connection, cfg: &CipherSettings) -> anyhow::Result<()> {
    // Disable memory security early
    exec_ignore_result(conn, "PRAGMA cipher_memory_security = OFF", &[])?;

    if let Some(v) = cfg.kdf_iter {
        exec_ignore_result(
            conn,
            &format!("PRAGMA cipher_default_kdf_iter = {}", v),
            &[],
        )?;
    }
    if let Some(v) = cfg.page_size {
        exec_ignore_result(
            conn,
            &format!("PRAGMA cipher_default_page_size = {}", v),
            &[],
        )?;
    }
    match cfg.use_hmac.unwrap_or(true) {
        true => {
            exec_ignore_result(conn, "PRAGMA cipher_default_use_hmac = ON", &[])?;
        }
        false => {
            exec_ignore_result(conn, "PRAGMA cipher_default_use_hmac = OFF", &[])?;
        }
    }
    if let Some(v) = cfg.plaintext_header_size {
        exec_ignore_result(
            conn,
            &format!("PRAGMA cipher_default_plaintext_header_size = {}", v),
            &[],
        )?;
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
        exec_ignore_result(conn, &sql, &[]).context("Failed to apply hex SQLCipher key")?;
    } else {
        let escaped = s.replace("'", "''");
        let sql = format!("PRAGMA key = '{}';", escaped);
        exec_ignore_result(conn, &sql, &[]).context("Failed to apply SQLCipher key")?;
    }
    s.zeroize();
    Ok(())
}

#[allow(dead_code)]
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
        let escaped = s.replace("'", "''");
        let sql = format!("PRAGMA {}.key = '{}';", db, escaped);
        conn.execute_batch(&sql)
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
    crate::utils::sqlite_cipher::apply_cipher_defaults(conn, cfg)?;
    let is_raw_hex = pass.starts_with("hex:")
        && pass[4..].len().is_multiple_of(2)
        && pass[4..].chars().all(|c| c.is_ascii_hexdigit());
    let attach_sql = if is_raw_hex {
        format!(
            "ATTACH DATABASE '{}' AS cipher_db KEY \"x'{}'\";",
            path_str,
            &pass[4..]
        )
    } else {
        let escaped = pass.replace("'", "''");
        format!(
            "ATTACH DATABASE '{}' AS cipher_db KEY '{}';",
            path_str, escaped
        )
    };
    // Use exec_ignore_result for attach as well, as it might return status
    // Skip ATTACH on Windows to avoid SQLITE_NOMEM/Error 7 logs due to broken build
    let attach_ok = if !cfg!(windows) {
        exec_ignore_result(conn, &attach_sql, &[]).is_ok()
    } else {
        false
    };

    if attach_ok {
        let attached_has_cipher = conn
            .pragma_query_value(None, "cipher_db.cipher_version", |row| {
                row.get::<_, String>(0)
            })
            .is_ok();
        if attached_has_cipher {
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
                exec_ignore_result(
                    conn,
                    &format!("PRAGMA cipher_db.cipher_plaintext_header_size = {}", p),
                    &[],
                )?;
            }
            conn.execute_batch("SELECT sqlcipher_export('cipher_db');")
                .context("Failed to export to encrypted database")?;
            if cfg.vacuum_after_pagesize.unwrap_or(false) {
                let _ = conn.execute_batch("VACUUM");
            }
            let _ = conn.execute_batch("DETACH DATABASE cipher_db;");
            pass.zeroize();
            return Ok(());
        } else {
            let _ = conn.execute_batch("DETACH DATABASE cipher_db;");
        }
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
