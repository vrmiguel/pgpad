use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::Context;
use rusqlite::{types::Type, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Gotta match the IDs in the DB
const DB_TYPE_POSTGRES: i32 = 1;
const DB_TYPE_SQLITE: i32 = 2;

use crate::{database::types::ConnectionInfo, Result};

struct Migrator {
    migrations: &'static [&'static str],
}

impl Migrator {
    fn new() -> Self {
        Self {
            migrations: &[include_str!("../migrations/001.sql")],
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

        let mut conn = Connection::open(&db_path)
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

        let migrator = Migrator::new();
        migrator.migrate(&mut conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn save_connection(&self, connection: &ConnectionInfo) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let conn = self.conn.lock().unwrap();

        let (db_type_id, connection_data) = match &connection.database_type {
            crate::database::types::DatabaseInfo::Postgres { connection_string } => {
                (DB_TYPE_POSTGRES, connection_string.as_str())
            }
            crate::database::types::DatabaseInfo::SQLite { db_path } => {
                (DB_TYPE_SQLITE, db_path.as_str())
            }
        };

        conn.execute(
            "INSERT OR REPLACE INTO connections 
             (id, name, connection_data, database_type_id, created_at, updated_at, sort_order) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 
                (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM connections))",
            (
                &connection.id.to_string(),
                &connection.name,
                connection_data,
                db_type_id,
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

        let (db_type_id, connection_data) = match &connection.database_type {
            crate::database::types::DatabaseInfo::Postgres { connection_string } => {
                (DB_TYPE_POSTGRES, connection_string.as_str())
            }
            crate::database::types::DatabaseInfo::SQLite { db_path } => {
                (DB_TYPE_SQLITE, db_path.as_str())
            }
        };

        let updated_rows = conn
            .execute(
                "UPDATE connections 
             SET name = ?2, connection_data = ?3, database_type_id = ?4, updated_at = ?5
             WHERE id = ?1",
                (
                    &connection.id.to_string(),
                    &connection.name,
                    connection_data,
                    db_type_id,
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
                        COALESCE(dt.name, 'postgres') as db_type
                 FROM connections c
                 LEFT JOIN database_types dt ON c.database_type_id = dt.id
                 ORDER BY c.sort_order, c.name",
            )
            .context("Failed to prepare statement")?;

        let rows = stmt
            .query_map([], |row| {
                let connection_data: String = row.get(2)?;
                let db_type: String = row.get(3)?;

                let database_type = match db_type.as_str() {
                    "postgres" => crate::database::types::DatabaseInfo::Postgres {
                        connection_string: connection_data,
                    },
                    "sqlite" => crate::database::types::DatabaseInfo::SQLite {
                        db_path: connection_data,
                    },
                    _ => crate::database::types::DatabaseInfo::Postgres {
                        connection_string: connection_data, // Default to postgres for unknown types
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
