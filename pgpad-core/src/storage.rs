use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::Context;
use keyring::Entry;
use rand::RngCore;
use rusqlite::{types::Type, Connection};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Gotta match the IDs in the DB
const DB_TYPE_POSTGRES: i32 = 1;
const DB_TYPE_SQLITE: i32 = 2;

/// Keyring service and account holding the app database encryption key.
const SERVICE_NAME: &str = "pgpad";
const APP_KEY_ACCOUNT: &str = "app_storage_key";

use crate::{
    database::types::{ConnectionConfig, ConnectionInfo, Permissions},
    Result,
};

struct Migrator {
    migrations: &'static [&'static str],
}

impl Migrator {
    fn new() -> Self {
        Self {
            migrations: &[
                include_str!("../migrations/001.sql"),
                include_str!("../migrations/002.sql"),
                include_str!("../migrations/003.sql"),
            ],
        }
    }

    fn migrate(&self, conn: &mut Connection) -> anyhow::Result<()> {
        let current_version: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .context("Failed to get current database version")?;

        let target_version = self.migrations.len() as i32;

        if current_version == target_version {
            return Ok(());
        }

        if current_version > target_version {
            anyhow::bail!(
                "Database version ({}) is newer than application version ({}). Please update the application.",
                current_version,
                target_version
            );
        }

        let tx = conn
            .transaction()
            .context("Failed to start migration transaction")?;

        for (i, migration) in self.migrations.iter().enumerate() {
            let migration_version = (i + 1) as i32;

            if migration_version <= current_version {
                continue;
            }

            tx.execute_batch(migration).map_err(|err| {
                anyhow::anyhow!("Failed to execute migration {migration_version}: {err}")
            })?;

            tx.pragma_update(None, "user_version", migration_version)
                .with_context(|| format!("Failed to update version to {}", migration_version))?;
        }

        let integrity_check: String = tx
            .pragma_query_value(None, "integrity_check", |row| row.get(0))
            .context("Failed to check database integrity")?;

        anyhow::ensure!(
            integrity_check == "ok",
            "Database integrity check failed: {}",
            integrity_check
        );

        tx.commit()
            .context("Failed to commit migration transaction")?;

        conn.execute("PRAGMA optimize", [])
            .context("Failed to optimize database")?;

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryHistoryEntry {
    pub id: i64,
    pub connection_id: String,
    pub query_text: String,
    pub executed_at: i64,
    pub duration_ms: Option<i64>,
    pub status: String,
    pub row_count: i64,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SavedQuery {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub query_text: String,
    pub connection_id: Option<Uuid>,
    pub tags: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub favorite: bool,
}

#[derive(Debug)]
pub struct Storage {
    conn: Mutex<Connection>,
}

impl Storage {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create database directory: {}", parent.display())
            })?;
        }

        let key = Self::get_or_create_app_key()?;
        let mut conn = Self::open_or_migrate_encrypted(&db_path, &key)?;

        let migrator = Migrator::new();
        migrator.migrate(&mut conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Returns the application storage key, generating it on first use.
    ///
    /// The key lives in the OS keyring. `PGPAD_APP_KEY` overrides it with a 64
    /// character hexadecimal value, which is the supported way to run where
    /// there is no keyring to talk to, such as a container.
    ///
    /// Refuses to run rather than continuing when the key cannot be persisted:
    /// an unpersisted key would differ on the next start and lock the user out
    /// of their own database.
    fn get_or_create_app_key() -> anyhow::Result<SecretString> {
        match std::env::var("PGPAD_APP_KEY") {
            Ok(env_key)
                if env_key.len() == 64 && env_key.chars().all(|c| c.is_ascii_hexdigit()) =>
            {
                return Ok(SecretString::new(env_key));
            }
            Ok(_) => {
                anyhow::bail!("PGPAD_APP_KEY must be 64 hexadecimal characters (32 bytes)");
            }
            Err(_) => {}
        }

        let entry = Entry::new(SERVICE_NAME, APP_KEY_ACCOUNT)
            .context("Failed to create the keyring entry for the app database key")?;

        match entry.get_password() {
            Ok(pw) if pw.len() == 64 && pw.chars().all(|c| c.is_ascii_hexdigit()) => {
                return Ok(SecretString::new(pw));
            }
            Ok(_) => anyhow::bail!(
                "The app database key stored in the OS keyring is not 64 hexadecimal \
                 characters; remove it and restart, or set PGPAD_APP_KEY"
            ),
            // A keyring we cannot read is not one we can write to either, so
            // there is nowhere to keep a newly generated key.
            Err(keyring::Error::NoEntry) => {}
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Could not read the app database key from the OS keyring ({}). \
                     Set PGPAD_APP_KEY to 64 hexadecimal characters to run without one.",
                    e
                ));
            }
        }

        let mut buf = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut buf);
        let key = hex::encode(buf);
        entry
            .set_password(&key)
            .context("Failed to store the app database key in the OS keyring")?;
        Ok(SecretString::new(key))
    }

    /// Cipher tuning for the app database, read from `PGPAD_CIPHER_*`.
    ///
    /// These come from the environment rather than from `app_settings` because
    /// the database cannot be read before its key is applied, and the defaults
    /// SQLCipher ships with are already the ones we want.
    fn env_cipher_settings() -> crate::utils::sqlite_cipher::CipherSettings {
        let num = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<u32>().ok());
        let flag = |k: &str| {
            std::env::var(k)
                .ok()
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        };
        crate::utils::sqlite_cipher::CipherSettings {
            kdf_iter: num("PGPAD_CIPHER_KDF_ITER"),
            page_size: num("PGPAD_CIPHER_PAGE_SIZE"),
            use_hmac: flag("PGPAD_CIPHER_USE_HMAC"),
            plaintext_header_size: num("PGPAD_CIPHER_PLAINTEXT_HEADER_SIZE"),
            compatibility: num("PGPAD_CIPHER_COMPATIBILITY"),
            vacuum_after_pagesize: flag("PGPAD_SQLITE_VACUUM_ON_PAGESIZE"),
        }
    }

    /// Reads `key` from `PGPAD_SETTING_<KEY>`.
    fn get_setting_static(key: &str) -> Option<String> {
        std::env::var(format!("PGPAD_SETTING_{}", key.to_uppercase())).ok()
    }

    /// Pragmas applied to the app database on every open, read from
    /// `PGPAD_SETTING_*` for the same reason as `env_cipher_settings`.
    fn settings_common_pragmas() -> crate::utils::sqlite_cipher::CommonSettings {
        let num = |key: &str| Self::get_setting_static(key).and_then(|v| v.parse::<i64>().ok());
        let flag = |key: &str| {
            Self::get_setting_static(key).map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        };
        crate::utils::sqlite_cipher::CommonSettings {
            journal_mode: std::env::var("PGPAD_SQLITE_JOURNAL_MODE").ok(),
            synchronous: std::env::var("PGPAD_SQLITE_SYNCHRONOUS").ok(),
            temp_store: Self::get_setting_static("sqlite_temp_store"),
            locking_mode: Self::get_setting_static("sqlite_locking_mode"),
            wal_autocheckpoint: num("sqlite_wal_autocheckpoint"),
            journal_size_limit: num("sqlite_journal_size_limit"),
            mmap_size: num("sqlite_mmap_size"),
            cache_size: num("sqlite_cache_size"),
            secure_delete: Some(
                Self::get_setting_static("sqlite_secure_delete")
                    .unwrap_or_else(|| "ON".to_string()),
            ),
            busy_timeout_ms: Self::get_setting_static("sqlite_busy_timeout_ms")
                .and_then(|v| v.parse::<u64>().ok()),
            case_sensitive_like: flag("sqlite_case_sensitive_like"),
            extended_result_codes: flag("sqlite_extended_result_codes"),
        }
    }

    fn open_encrypted_with_key(
        db_path: &PathBuf,
        key: &SecretString,
        cfg: &crate::utils::sqlite_cipher::CipherSettings,
    ) -> anyhow::Result<Connection> {
        let conn = Connection::open(db_path)
            .with_context(|| format!("Failed to open SQLite database at {}", db_path.display()))?;
        crate::utils::sqlite_cipher::apply_cipher_settings_with(&conn, key, cfg)?;
        let common = Self::settings_common_pragmas();
        crate::utils::sqlite_cipher::apply_common_settings_with(&conn, &common)?;
        Ok(conn)
    }

    fn open_plain(db_path: &PathBuf) -> anyhow::Result<Connection> {
        let conn = Connection::open(db_path)
            .with_context(|| format!("Failed to open SQLite database at {}", db_path.display()))?;
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = FULL;
            PRAGMA busy_timeout = 30000;
            PRAGMA case_sensitive_like = ON;
            PRAGMA extended_result_codes = ON;
            ",
        )
        .context("Failed to execute database initialization SQL")?;
        let common = Self::settings_common_pragmas();
        crate::utils::sqlite_cipher::apply_common_settings_with(&conn, &common)?;
        Ok(conn)
    }

    /// True when the file on disk cannot be read without a key.
    ///
    /// Deciding this by opening the file with no key and reading its schema is
    /// what distinguishes an encrypted app database from a plaintext one that
    /// happens to be openable.
    fn is_encrypted(db_path: &PathBuf) -> bool {
        match Connection::open(db_path) {
            Ok(conn) => conn
                .query_row("SELECT count(*) FROM sqlite_master", [], |row| {
                    row.get::<_, i64>(0)
                })
                .is_err(),
            Err(_) => false,
        }
    }

    /// Opens the app database with SQLCipher, converting an existing plaintext
    /// database on first run. The original file is kept as `<name>.db.bak`.
    ///
    /// Migrations are not applied here; the caller runs them on the result.
    fn open_or_migrate_encrypted(
        db_path: &PathBuf,
        key: &SecretString,
    ) -> anyhow::Result<Connection> {
        let exists = std::fs::metadata(db_path).is_ok();
        let encrypted = exists && Self::is_encrypted(db_path);

        if !exists || encrypted {
            let cfg = Self::env_cipher_settings();
            let conn = Self::open_encrypted_with_key(db_path, key, &cfg)?;
            if encrypted {
                // Guards against a keyring that no longer holds this app's key.
                crate::utils::sqlite_cipher::verify_cipher_ok(&conn)
                    .context("Failed to decrypt the app database with the stored key")?;
            }
            return Ok(conn);
        }

        // Existing plaintext database: rewrite it through an encrypted export.
        // The original is only moved aside once the export has been verified,
        // so a failure here leaves the plaintext database untouched.
        let backup_path = db_path.with_extension("db.bak");
        let new_path = db_path.with_extension("db.enc");
        let _ = std::fs::remove_file(&new_path);

        let conn_plain = Self::open_plain(db_path)?;
        let source_version: i32 =
            conn_plain.pragma_query_value(None, "user_version", |row| row.get(0))?;
        let cfg = Self::env_cipher_settings();
        crate::utils::sqlite_cipher::attach_and_export_plain_to_encrypted_with(
            &conn_plain,
            &new_path,
            key,
            &cfg,
        )?;
        drop(conn_plain);

        std::fs::rename(db_path, &backup_path)?;
        std::fs::rename(&new_path, db_path)?;

        let enc_conn = Self::open_encrypted_with_key(db_path, key, &cfg)?;
        crate::utils::sqlite_cipher::verify_cipher_ok(&enc_conn)?;

        // `sqlcipher_export` copies the schema and the rows but not the
        // `user_version` header, so the migrations would run a second time and
        // trip over the tables that already exist.
        if source_version > 0 {
            enc_conn.pragma_update(None, "user_version", source_version)?;
        }

        Ok(enc_conn)
    }

    pub fn save_connection(&self, connection: &ConnectionInfo) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let conn = self.conn.lock().unwrap();

        let (db_type_id, connection_data, ca_cert_path) = match &connection.config {
            ConnectionConfig::Postgres {
                connection_string,
                ca_cert_path,
            } => (
                DB_TYPE_POSTGRES,
                connection_string.as_str(),
                ca_cert_path.as_deref(),
            ),
            ConnectionConfig::SQLite { db_path, .. } => (DB_TYPE_SQLITE, db_path.as_str(), None),
        };

        conn.execute(
            "INSERT OR REPLACE INTO connections 
             (id, name, connection_data, database_type_id, ca_cert_path, permissions, created_at, updated_at, sort_order) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 
                (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM connections))",
            (
                &connection.id.to_string(),
                &connection.name,
                connection_data,
                db_type_id,
                ca_cert_path,
                connection.permissions.as_str(),
                now,
                now,
            ),
        )
        .context("Failed to save connection")?;

        Ok(())
    }

    pub fn update_connection(&self, connection: &ConnectionInfo) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let conn = self.conn.lock().unwrap();

        let (db_type_id, connection_data, ca_cert_path) = match &connection.config {
            ConnectionConfig::Postgres {
                connection_string,
                ca_cert_path,
            } => (
                DB_TYPE_POSTGRES,
                connection_string.as_str(),
                ca_cert_path.as_deref(),
            ),
            ConnectionConfig::SQLite { db_path, .. } => (DB_TYPE_SQLITE, db_path.as_str(), None),
        };

        let updated_rows = conn
            .execute(
                "UPDATE connections 
             SET name = ?2, connection_data = ?3, database_type_id = ?4, ca_cert_path = ?5, permissions = ?6, updated_at = ?7
             WHERE id = ?1",
                (
                    &connection.id.to_string(),
                    &connection.name,
                    connection_data,
                    db_type_id,
                    ca_cert_path,
                    connection.permissions.as_str(),
                    now,
                ),
            )
            .context("Failed to update connection")?;

        if updated_rows == 0 {
            return Err(crate::Error::Any(anyhow::anyhow!(
                "Connection not found: {}",
                connection.id
            )));
        }

        Ok(())
    }

    // TODO: add `get_connection`
    pub fn get_connections(&self) -> Result<Vec<ConnectionInfo>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT c.id, c.name, c.connection_data, 
                        COALESCE(dt.name, 'postgres') as db_type,
                        c.ca_cert_path,
                        COALESCE(c.permissions, 'read_write') as permissions
                 FROM connections c
                 LEFT JOIN database_types dt ON c.database_type_id = dt.id
                 ORDER BY c.sort_order, c.name",
            )
            .context("Failed to prepare statement")?;

        let rows = stmt
            .query_map([], |row| {
                let connection_data: String = row.get(2)?;
                let db_type: String = row.get(3)?;
                let ca_cert_path: Option<String> = row.get(4)?;
                let permissions_str: String = row.get(5)?;

                let config = match db_type.as_str() {
                    "postgres" => ConnectionConfig::Postgres {
                        connection_string: connection_data,
                        ca_cert_path,
                    },
                    "sqlite" => ConnectionConfig::SQLite {
                        db_path: connection_data,
                        passphrase: None,
                    },
                    _ => ConnectionConfig::Postgres {
                        connection_string: connection_data, // Default to postgres for unknown types
                        ca_cert_path,
                    },
                };

                Ok(ConnectionInfo {
                    id: {
                        let id: String = row.get(0)?;
                        Uuid::parse_str(&id).map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(err))
                        })?
                    },
                    name: row.get(1)?,
                    permissions: Permissions::from_storage_str(&permissions_str),
                    config,
                    connected: false,
                })
            })
            .context("Failed to query connections")?;

        let mut connections = Vec::new();
        for row in rows {
            connections
                .push(row.map_err(|e| anyhow::anyhow!("Failed to process connection row: {}", e))?);
        }

        Ok(connections)
    }

    pub fn remove_connection(&self, connection_id: &Uuid) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM connections WHERE id = ?1",
            [connection_id.to_string()],
        )
        .context("Failed to remove connection")?;
        Ok(())
    }

    pub fn update_last_connected(&self, connection_id: &Uuid) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE connections SET last_connected_at = ?1 WHERE id = ?2",
            (now, connection_id.to_string()),
        )
        .context("Failed to update last connected time")?;
        Ok(())
    }

    pub fn save_query_history(&self, entry: &QueryHistoryEntry) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO query_history 
             (connection_id, query_text, executed_at, duration_ms, status, row_count, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                &entry.connection_id,
                &entry.query_text,
                entry.executed_at,
                entry.duration_ms,
                &entry.status,
                entry.row_count,
                &entry.error_message,
            ),
        )
        .context("Failed to save query history")?;
        Ok(())
    }

    pub fn get_query_history(
        &self,
        connection_id: &str,
        limit: Option<i64>,
    ) -> Result<Vec<QueryHistoryEntry>> {
        let limit = limit.unwrap_or(100);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, connection_id, query_text, executed_at, duration_ms, status, row_count, error_message
             FROM query_history 
             WHERE connection_id = ?1 
             ORDER BY executed_at DESC 
             LIMIT ?2"
        ).context("Failed to prepare query history statement")?;

        let rows = stmt
            .query_map((connection_id, limit), |row| {
                Ok(QueryHistoryEntry {
                    id: row.get(0)?,
                    connection_id: row.get(1)?,
                    query_text: row.get(2)?,
                    executed_at: row.get(3)?,
                    duration_ms: row.get(4)?,
                    status: row.get(5)?,
                    row_count: row.get(6)?,
                    error_message: row.get(7)?,
                })
            })
            .context("Failed to query history")?;

        let mut history = Vec::new();
        for row in rows {
            history.push(row.context("Failed to process history row")?);
        }

        Ok(history)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT value FROM app_settings WHERE key = ?1")
            .context("Failed to prepare settings statement")?;
        let mut rows = stmt
            .query_map([key], |row| row.get::<_, String>(0))
            .context("Failed to query settings")?;

        if let Some(row) = rows.next() {
            Ok(Some(row.context("Failed to get setting value")?))
        } else {
            Ok(None)
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            (key, value, now),
        )
        .context("Failed to set setting")?;
        Ok(())
    }

    pub fn save_query(&self, query: &SavedQuery) -> Result<i64> {
        let now = chrono::Utc::now().timestamp();
        let conn = self.conn.lock().unwrap();

        if query.id == 0 {
            conn.execute(
                "INSERT INTO saved_queries 
                 (name, description, query_text, connection_id, tags, created_at, updated_at, favorite)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                (
                    &query.name,
                    &query.description,
                    &query.query_text,
                    &query.connection_id.map(|id| id.to_string()),
                    &query.tags,
                    now,
                    now,
                    query.favorite,
                ),
            ).context("Failed to insert saved query")?;
            Ok(conn.last_insert_rowid())
        } else {
            conn.execute(
                "UPDATE saved_queries 
                 SET name = ?1, description = ?2, query_text = ?3, connection_id = ?4, 
                     tags = ?5, updated_at = ?6, favorite = ?7
                 WHERE id = ?8",
                (
                    &query.name,
                    &query.description,
                    &query.query_text,
                    &query.connection_id.map(|id| id.to_string()),
                    &query.tags,
                    now,
                    query.favorite,
                    query.id,
                ),
            )
            .context("Failed to update saved query")?;
            Ok(query.id)
        }
    }

    pub fn get_saved_queries(&self, connection_id: Option<&Uuid>) -> Result<Vec<SavedQuery>> {
        let conn = self.conn.lock().unwrap();

        let mut queries = Vec::new();

        if let Some(conn_id) = connection_id {
            let mut stmt = conn.prepare(
                "SELECT id, name, description, query_text, connection_id, tags, created_at, updated_at, favorite
                 FROM saved_queries 
                 WHERE connection_id = ?1 OR connection_id IS NULL
                 ORDER BY favorite DESC, created_at DESC"
            ).context("Failed to prepare saved queries statement")?;

            let rows = stmt
                .query_map([conn_id.to_string()], |row| {
                    Ok(SavedQuery {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        query_text: row.get(3)?,
                        connection_id: {
                            let id: Option<String> = row.get(4)?;
                            match id {
                                Some(id) => Some(Uuid::parse_str(&id).map_err(|err| {
                                    rusqlite::Error::FromSqlConversionFailure(
                                        0,
                                        Type::Text,
                                        Box::new(err),
                                    )
                                })?),
                                None => None,
                            }
                        },
                        tags: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                        favorite: row.get(8)?,
                    })
                })
                .context("Failed to query saved queries")?;

            for row in rows {
                queries.push(row.context("Failed to process saved query row")?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, name, description, query_text, connection_id, tags, created_at, updated_at, favorite
                 FROM saved_queries 
                 ORDER BY favorite DESC, created_at DESC"
            ).context("Failed to prepare saved queries statement")?;

            let rows = stmt
                .query_map([], |row| {
                    Ok(SavedQuery {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        query_text: row.get(3)?,
                        connection_id: {
                            let id: Option<String> = row.get(4)?;
                            match id {
                                Some(id) => Some(Uuid::parse_str(&id).map_err(|err| {
                                    rusqlite::Error::FromSqlConversionFailure(
                                        0,
                                        Type::Text,
                                        Box::new(err),
                                    )
                                })?),
                                None => None,
                            }
                        },
                        tags: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                        favorite: row.get(8)?,
                    })
                })
                .context("Failed to query saved queries")?;

            for row in rows {
                queries.push(row.context("Failed to process saved query row")?);
            }
        }

        Ok(queries)
    }

    pub fn delete_saved_query(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM saved_queries WHERE id = ?1", [id])
            .context("Failed to delete saved query")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    fn test_key() -> SecretString {
        SecretString::new(hex::encode([7u8; 32]))
    }

    fn sqlcipher_available() -> bool {
        Connection::open_in_memory()
            .map(|conn| crate::utils::sqlite_cipher::has_sqlcipher(&conn))
            .unwrap_or(false)
    }

    fn seeded_value(conn: &Connection) -> anyhow::Result<String> {
        Ok(conn.query_row(
            "SELECT value FROM app_settings WHERE key = 'seed'",
            [],
            |row| row.get(0),
        )?)
    }

    /// A plaintext app database left over from an earlier version is rewritten
    /// encrypted, the original is kept as `<name>.db.bak`, and the result still
    /// reopens and answers queries.
    #[test]
    fn plaintext_storage_is_migrated_to_cipher() -> anyhow::Result<()> {
        if !sqlcipher_available() {
            eprintln!("skipped: SQLCipher is not compiled in");
            return Ok(());
        }

        let dir = std::env::temp_dir().join("pgpad-storage-migration");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        let db_path = dir.join("pgpad.db");

        // A plaintext database, as written by a build without SQLCipher.
        {
            let mut conn = Connection::open(&db_path)?;
            Migrator::new().migrate(&mut conn)?;
            conn.execute(
                "INSERT INTO app_settings (key, value, updated_at) VALUES ('seed', '1', 0)",
                [],
            )?;
        }
        assert!(!Storage::is_encrypted(&db_path));

        let conn = Storage::open_or_migrate_encrypted(&db_path, &test_key())?;
        assert!(Storage::is_encrypted(&db_path));
        assert!(db_path.with_extension("db.bak").exists());
        assert_eq!(seeded_value(&conn)?, "1");
        // The exported copy keeps the source schema version, so the migrations
        // do not run again and trip over the tables that are already there.
        let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        assert_eq!(version, Migrator::new().migrations.len() as i32);
        drop(conn);

        // Reopening must take the encrypted branch and keep working.
        let conn = Storage::open_or_migrate_encrypted(&db_path, &test_key())?;
        assert_eq!(seeded_value(&conn)?, "1");
        drop(conn);

        // A different key must not be able to read it.
        let other = SecretString::new(hex::encode([9u8; 32]));
        assert!(Storage::open_or_migrate_encrypted(&db_path, &other).is_err());

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// The key that encrypts the app database has to survive a restart. This
    /// locks that in: two reads either agree, or the second one refuses
    /// instead of handing back a different key.
    #[test]
    fn app_key_is_stable_or_refused() -> anyhow::Result<()> {
        match Storage::get_or_create_app_key() {
            Ok(first) => {
                let second = Storage::get_or_create_app_key()
                    .expect("a key that was just read must be readable again");
                assert_eq!(first.expose_secret(), second.expose_secret());
                assert_eq!(first.expose_secret().len(), 64);
                Ok(())
            }
            Err(e) => {
                // No usable keyring here (headless CI, container). Refusing is
                // the point; silently returning a fresh key would lock the user
                // out of their database on the next start.
                eprintln!("skipped: no persistable app key: {}", e);
                Ok(())
            }
        }
    }

    /// When the encrypted export cannot be produced the conversion must not be
    /// destructive: the plaintext database stays where it is and stays
    /// readable, so a platform that cannot do the export is recoverable rather
    /// than a lost database.
    #[test]
    fn failed_export_leaves_plaintext_database_intact() -> anyhow::Result<()> {
        if !sqlcipher_available() {
            eprintln!("skipped: SQLCipher is not compiled in");
            return Ok(());
        }

        let dir = std::env::temp_dir().join("pgpad-export-failure");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        let db_path = dir.join("pgpad.db");

        {
            let mut conn = Connection::open(&db_path)?;
            Migrator::new().migrate(&mut conn)?;
            conn.execute(
                "INSERT INTO app_settings (key, value, updated_at) VALUES ('seed', '1', 0)",
                [],
            )?;
        }

        // A destination inside a directory that does not exist fails the export
        // the same way a platform quirk would. The handle is closed before the
        // assertions because Windows refuses to delete a file that is open.
        let unreachable = dir.join("missing").join("out.db");
        let key = SecretString::new(hex::encode([3u8; 32]));
        let result = {
            let plain = Connection::open(&db_path)?;
            crate::utils::sqlite_cipher::attach_and_export_plain_to_encrypted_with(
                &plain,
                &unreachable,
                &key,
                &Default::default(),
            )
        };
        assert!(result.is_err(), "the export was expected to fail");

        assert!(db_path.exists(), "the original database must survive");
        assert!(!Storage::is_encrypted(&db_path));
        let conn = Connection::open(&db_path)?;
        assert_eq!(seeded_value(&conn)?, "1");
        drop(conn);

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// A database created from scratch has to be created with the configured
    /// cipher parameters and reopen with the same ones.
    ///
    /// The settings are per database, so applying the `cipher_default_*`
    /// pragmas to an existing database changes the parameters it was created
    /// with and makes it fail to decrypt.
    #[test]
    fn new_database_uses_the_configured_settings() -> anyhow::Result<()> {
        if !sqlcipher_available() {
            eprintln!("skipped: SQLCipher is not compiled in");
            return Ok(());
        }

        let variants = [
            crate::utils::sqlite_cipher::CipherSettings {
                kdf_iter: Some(300000),
                page_size: Some(4096),
                use_hmac: Some(true),
                plaintext_header_size: None,
                compatibility: Some(3),
                vacuum_after_pagesize: Some(false),
            },
            crate::utils::sqlite_cipher::CipherSettings {
                kdf_iter: Some(300000),
                page_size: Some(8192),
                use_hmac: Some(true),
                plaintext_header_size: None,
                compatibility: Some(4),
                vacuum_after_pagesize: Some(false),
            },
        ];

        for (index, cfg) in variants.iter().enumerate() {
            let dir = std::env::temp_dir().join(format!("pgpad-storage-new-settings-{}", index));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join("pgpad.db");
            let key = test_key();

            {
                let conn = Storage::open_encrypted_with_key(&db_path, &key, cfg)?;
                // The first write is what creates the header, so the database
                // can only be read back after it.
                conn.execute_batch("CREATE TABLE t (a INTEGER); INSERT INTO t VALUES (5);")?;
                crate::utils::sqlite_cipher::verify_cipher_ok(&conn)?;
            }

            // Reopening with the same settings has to work.
            {
                let conn = Storage::open_encrypted_with_key(&db_path, &key, cfg)?;
                crate::utils::sqlite_cipher::verify_cipher_ok(&conn)?;
                let value: i64 = conn.query_row("SELECT a FROM t", [], |row| row.get(0))?;
                assert_eq!(value, 5, "variant {} lost the row", index);
            }

            // A different key must not be able to read it. Opening alone does
            // not tell, because the key is only exercised on the first read.
            let other = SecretString::new(hex::encode([9u8; 32]));
            let conn = Storage::open_encrypted_with_key(&db_path, &other, cfg)?;
            assert!(crate::utils::sqlite_cipher::verify_cipher_ok(&conn).is_err());

            let _ = std::fs::remove_dir_all(&dir);
        }
        Ok(())
    }
}
