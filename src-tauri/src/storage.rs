use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::Context;
use keyring::Entry;
use rand::RngCore;
use rusqlite::{types::Type, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Gotta match the IDs in the DB
const DB_TYPE_POSTGRES: i32 = 1;
const DB_TYPE_SQLITE: i32 = 2;
const DB_TYPE_DUCKDB: i32 = 3;
const DB_TYPE_ORACLE: i32 = 4;
const DB_TYPE_MSSQL: i32 = 5;

use crate::{database::types::ConnectionInfo, Result};

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
                include_str!("../migrations/004.sql"),
                include_str!("../migrations/005.sql"),
                include_str!("../migrations/006.sql"),
                include_str!("../migrations/007.sql"),
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

        let conn = Self::open_or_migrate_encrypted(&db_path)
            .with_context(|| format!("Failed to open SQLite database at {}", db_path.display()))?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn save_connection(&self, connection: &ConnectionInfo) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::Error::Any(anyhow::anyhow!("Mutex poisoned: {}", e)))?;

        let (db_type_id, connection_data, ca_cert_path, wallet_path, tns_alias) =
            match &connection.database_type {
                crate::database::types::DatabaseInfo::Postgres {
                    connection_string,
                    ca_cert_path,
                } => (
                    DB_TYPE_POSTGRES,
                    connection_string.as_str(),
                    ca_cert_path.as_deref(),
                    None,
                    None,
                ),
                crate::database::types::DatabaseInfo::SQLite { db_path, .. } => {
                    (DB_TYPE_SQLITE, db_path.as_str(), None, None, None)
                }
                crate::database::types::DatabaseInfo::DuckDB { db_path } => {
                    (DB_TYPE_DUCKDB, db_path.as_str(), None, None, None)
                }
                crate::database::types::DatabaseInfo::Oracle {
                    connection_string,
                    wallet_path,
                    tns_alias,
                } => (
                    DB_TYPE_ORACLE,
                    connection_string.as_str(),
                    None,
                    wallet_path.as_deref(),
                    tns_alias.as_deref(),
                ),
                crate::database::types::DatabaseInfo::Mssql {
                    connection_string,
                    ca_cert_path,
                } => (
                    DB_TYPE_MSSQL,
                    connection_string.as_str(),
                    ca_cert_path.as_deref(),
                    None,
                    None,
                ),
            };

        conn.execute(
            "INSERT OR REPLACE INTO connections
             (id, name, connection_data, database_type_id, ca_cert_path, wallet_path, tns_alias, created_at, updated_at, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                 ?9, (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM connections))",
            (
                &connection.id.to_string(),
                &connection.name,
                connection_data,
                db_type_id,
                ca_cert_path,
                wallet_path,
                tns_alias,
                now,
                now,
            ),
        )
        .context("Failed to save connection")?;

        Ok(())
    }

    pub fn update_connection(&self, connection: &ConnectionInfo) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::Error::Any(anyhow::anyhow!("Mutex poisoned: {}", e)))?;

        let (db_type_id, connection_data, ca_cert_path, wallet_path, tns_alias) =
            match &connection.database_type {
                crate::database::types::DatabaseInfo::Postgres {
                    connection_string,
                    ca_cert_path,
                } => (
                    DB_TYPE_POSTGRES,
                    connection_string.as_str(),
                    ca_cert_path.as_deref(),
                    None,
                    None,
                ),
                crate::database::types::DatabaseInfo::SQLite { db_path, .. } => {
                    (DB_TYPE_SQLITE, db_path.as_str(), None, None, None)
                }
                crate::database::types::DatabaseInfo::DuckDB { db_path } => {
                    (DB_TYPE_DUCKDB, db_path.as_str(), None, None, None)
                }
                crate::database::types::DatabaseInfo::Oracle {
                    connection_string,
                    wallet_path,
                    tns_alias,
                } => (
                    DB_TYPE_ORACLE,
                    connection_string.as_str(),
                    None,
                    wallet_path.as_deref(),
                    tns_alias.as_deref(),
                ),
                crate::database::types::DatabaseInfo::Mssql {
                    connection_string,
                    ca_cert_path,
                } => (
                    DB_TYPE_MSSQL,
                    connection_string.as_str(),
                    ca_cert_path.as_deref(),
                    None,
                    None,
                ),
            };

        let updated_rows = conn
            .execute(
                "UPDATE connections
             SET name = ?2, connection_data = ?3, database_type_id = ?4, ca_cert_path = ?5, wallet_path = ?7, tns_alias = ?8, updated_at = ?6
             WHERE id = ?1",
                (
                    &connection.id.to_string(),
                    &connection.name,
                    connection_data,
                    db_type_id,
                    ca_cert_path,
                    now,
                    wallet_path,
                    tns_alias,
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
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::Error::Any(anyhow::anyhow!("Mutex poisoned: {}", e)))?;
        let mut stmt = conn
            .prepare(
                "SELECT c.id, c.name, c.connection_data,
                        COALESCE(dt.name, 'postgres') as db_type,
                        c.ca_cert_path, c.wallet_path, c.tns_alias
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
                let wallet_path: Option<String> = row.get(5)?;
                let tns_alias: Option<String> = row.get(6)?;

                let database_type = match db_type.as_str() {
                    "postgres" => crate::database::types::DatabaseInfo::Postgres {
                        connection_string: connection_data,
                        ca_cert_path,
                    },
                    "sqlite" => crate::database::types::DatabaseInfo::SQLite {
                        db_path: connection_data,
                        passphrase: None,
                    },
                    "duckdb" => crate::database::types::DatabaseInfo::DuckDB {
                        db_path: connection_data,
                    },
                    "oracle" => crate::database::types::DatabaseInfo::Oracle {
                        connection_string: connection_data,
                        wallet_path,
                        tns_alias,
                    },
                    "mssql" => crate::database::types::DatabaseInfo::Mssql {
                        connection_string: connection_data,
                        ca_cert_path,
                    },
                    _ => crate::database::types::DatabaseInfo::Postgres {
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
                    database_type,
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
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::Error::Any(anyhow::anyhow!("Mutex poisoned: {}", e)))?;
        conn.execute(
            "DELETE FROM connections WHERE id = ?1",
            [connection_id.to_string()],
        )
        .context("Failed to remove connection")?;
        Ok(())
    }

    pub fn update_last_connected(&self, connection_id: &Uuid) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::Error::Any(anyhow::anyhow!("Mutex poisoned: {}", e)))?;
        conn.execute(
            "UPDATE connections SET last_connected_at = ?1 WHERE id = ?2",
            (now, connection_id.to_string()),
        )
        .context("Failed to update last connected time")?;
        Ok(())
    }

    pub fn save_query_history(&self, entry: &QueryHistoryEntry) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::Error::Any(anyhow::anyhow!("Mutex poisoned: {}", e)))?;
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
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::Error::Any(anyhow::anyhow!("Mutex poisoned: {}", e)))?;
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
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::Error::Any(anyhow::anyhow!("Mutex poisoned: {}", e)))?;
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
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::Error::Any(anyhow::anyhow!("Mutex poisoned: {}", e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            (key, value, now),
        )
        .context("Failed to set setting")?;
        Ok(())
    }

    pub fn save_query(&self, query: &SavedQuery) -> Result<i64> {
        let now = chrono::Utc::now().timestamp();
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::Error::Any(anyhow::anyhow!("Mutex poisoned: {}", e)))?;

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
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::Error::Any(anyhow::anyhow!("Mutex poisoned: {}", e)))?;

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
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::Error::Any(anyhow::anyhow!("Mutex poisoned: {}", e)))?;
        conn.execute("DELETE FROM saved_queries WHERE id = ?1", [id])
            .context("Failed to delete saved query")?;
        Ok(())
    }

    fn get_or_create_app_key() -> anyhow::Result<String> {
        if let Ok(env_key) = std::env::var("PGPAD_APP_KEY") {
            let valid = env_key.len() == 64 && env_key.chars().all(|c| c.is_ascii_hexdigit());
            if valid {
                return Ok(env_key);
            }
        }

        let entry = Entry::new("pgpad", "app_storage_key")?;
        if let Ok(pw) = entry.get_password() {
            let valid = pw.len() == 64 && pw.chars().all(|c| c.is_ascii_hexdigit());
            if valid {
                return Ok(pw);
            }
        }

        let mut buf = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut buf);
        let key = hex::encode(buf);

        if entry.set_password(&key).is_ok() {
            return Ok(key);
        }

        let mut path = dirs::config_dir().unwrap_or_else(std::env::temp_dir);
        path.push("pgpad");
        let _ = std::fs::create_dir_all(&path);
        path.push("app_key");

        if let Ok(contents) = std::fs::read_to_string(&path) {
            let s = contents.trim().to_string();
            let valid = s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit());
            if valid {
                return Ok(s);
            }
        }

        {
            use std::fs::OpenOptions;
            let _ = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&path)
                .and_then(|mut f| {
                    use std::io::Write;
                    f.write_all(key.as_bytes())
                });
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&path) {
                let mut perm = meta.permissions();
                perm.set_mode(0o600);
                let _ = std::fs::set_permissions(&path, perm);
            }
        }

        Ok(key)
    }

    fn apply_cipher_pragmas(conn: &mut Connection) -> anyhow::Result<()> {
        conn.pragma_update(None, "cipher_compatibility", 4)?;
        conn.pragma_update(None, "cipher_page_size", 4096)?;
        let _ = conn.execute("PRAGMA cipher_memory_security = ON", []);
        Ok(())
    }

    fn apply_common_pragmas(conn: &mut Connection) -> anyhow::Result<()> {
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = FULL;
            PRAGMA busy_timeout = 30000;
            PRAGMA case_sensitive_like = ON;
            PRAGMA extended_result_codes = ON;
            ",
        )?;
        Ok(())
    }

    fn open_encrypted(db_path: &PathBuf) -> anyhow::Result<Connection> {
        let mut conn = Connection::open(db_path)?;
        let key = Self::get_or_create_app_key()?;
        conn.pragma_update(None, "key", &key)?;
        Self::apply_cipher_pragmas(&mut conn)?;
        Self::apply_common_pragmas(&mut conn)?;
        Ok(conn)
    }

    fn open_plain(db_path: &PathBuf) -> anyhow::Result<Connection> {
        let mut conn = Connection::open(db_path)?;
        Self::apply_common_pragmas(&mut conn)?;
        Ok(conn)
    }

    fn is_encrypted(conn: &mut Connection) -> bool {
        match conn.pragma_query_value(None, "cipher_integrity_check", |row| {
            row.get::<_, String>(0)
        }) {
            Ok(v) => v == "ok",
            Err(_) => false,
        }
    }

    fn open_or_migrate_encrypted(db_path: &PathBuf) -> anyhow::Result<Connection> {
        let exists = std::fs::metadata(db_path).is_ok();
        if !exists {
            let mut conn = Self::open_encrypted(db_path)?;
            let migrator = Migrator::new();
            migrator.migrate(&mut conn)?;
            return Ok(conn);
        }

        let mut conn_enc = match Self::open_encrypted(db_path) {
            Ok(c) => c,
            Err(_) => Self::open_plain(db_path)?,
        };

        if Self::is_encrypted(&mut conn_enc) {
            let migrator = Migrator::new();
            migrator.migrate(&mut conn_enc)?;
            return Ok(conn_enc);
        }

        let plain = Self::open_plain(db_path)?;
        drop(conn_enc);

        let backup_path = db_path.with_extension("db.bak");
        let new_path = db_path.with_extension("db.enc");
        if std::fs::metadata(&new_path).is_ok() {
            let _ = std::fs::remove_file(&new_path);
        }

        let plain_conn = plain;
        let key = Self::get_or_create_app_key()?;
        let attach_sql = format!(
            "ATTACH DATABASE '{}' AS cipher_db KEY '{}';",
            new_path.display(),
            key
        );
        plain_conn.execute_batch(&attach_sql)?;
        plain_conn.execute_batch("SELECT sqlcipher_export('cipher_db');")?;
        plain_conn.execute_batch("DETACH DATABASE cipher_db;")?;

        std::fs::rename(db_path, &backup_path)?;
        std::fs::rename(&new_path, db_path)?;

        let mut enc_conn = Self::open_encrypted(db_path)?;
        let ci = enc_conn
            .pragma_query_value(None, "cipher_integrity_check", |row| {
                row.get::<_, String>(0)
            })
            .unwrap_or_else(|_| String::from("error"));
        anyhow::ensure!(ci == "ok", "cipher integrity check failed: {}", ci);
        let migrator = Migrator::new();
        migrator.migrate(&mut enc_conn)?;
        Ok(enc_conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_key_is_hex32bytes() {
        let key = Storage::get_or_create_app_key().expect("key");
        assert_eq!(key.len(), 64);
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
