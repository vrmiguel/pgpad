use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub id: Uuid,
    pub name: String,
    pub connected: bool,
    pub database_type: DatabaseInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatabaseInfo {
    Postgres { connection_string: String },
    SQLite { db_path: String },
}

#[derive(Debug)]
pub struct DatabaseConnection {
    pub id: Uuid,
    pub name: String,
    pub connected: bool,
    pub database: Database,
}

#[derive(Debug)]
pub enum Database {
    Postgres {
        connection_string: String,
        client: Option<tokio_postgres::Client>,
    },
    SQLite {
        db_path: String,
        connection: Option<Arc<Mutex<rusqlite::Connection>>>,
    },
}

impl DatabaseConnection {
    pub fn to_connection_info(&self) -> ConnectionInfo {
        ConnectionInfo {
            id: self.id,
            name: self.name.clone(),
            connected: self.connected,
            database_type: match &self.database {
                Database::Postgres {
                    connection_string, ..
                } => DatabaseInfo::Postgres {
                    connection_string: connection_string.clone(),
                },
                Database::SQLite { db_path, .. } => DatabaseInfo::SQLite {
                    db_path: db_path.clone(),
                },
            },
        }
    }

    pub fn new(id: Uuid, name: String, database_info: DatabaseInfo) -> Self {
        let database = match database_info {
            DatabaseInfo::Postgres { connection_string } => Database::Postgres {
                connection_string,
                client: None,
            },
            DatabaseInfo::SQLite { db_path } => Database::SQLite {
                db_path,
                connection: None,
            },
        };

        Self {
            id,
            name,
            connected: false,
            database,
        }
    }

    pub fn is_client_connected(&self) -> bool {
        match &self.database {
            Database::Postgres { client, .. } => client.is_some(),
            Database::SQLite { connection, .. } => connection.is_some(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub schema: String,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSchema {
    pub tables: Vec<TableInfo>,
    pub schemas: Vec<String>,
    // Deduplicated list of column names across all tables, for autocomplete purposes
    pub unique_columns: Vec<String>,
}

#[derive(Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "event",
    content = "data"
)]
pub enum QueryStreamEvent {
    StatementStart {
        statement_index: usize,
        total_statements: usize,
        statement: String,
        returns_values: bool,
    },

    /// For queries that return data
    ResultStart {
        statement_index: usize,
        // Serialized Vec<String>, because I can't help myself
        columns: Box<RawValue>,
    },
    ResultBatch {
        statement_index: usize,
        rows: Box<RawValue>,
    },

    /// For queries that do not return data
    StatementComplete {
        statement_index: usize,
        affected_rows: u64,
    },
    StatementFinish {
        statement_index: usize,
    },

    /// All statements completed
    AllFinished {},
    StatementError {
        statement_index: usize,
        statement: String,
        error: String,
    },
}
