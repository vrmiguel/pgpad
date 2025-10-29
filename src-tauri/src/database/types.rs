use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use uuid::Uuid;

use crate::Error;

pub type QueryId = usize;

/// A row of data serialized by one of our writers
/// You can conceptually think of this as a Vec<Vec<Json>>.
///
/// Example:
/// - Query: `SELECT 1,2,3 UNION ALL SELECT 4,5,6`
/// - Page: `[[1,2,3],[4,5,6]]`
pub type Page = Box<RawValue>;

pub type ExecSender = UnboundedSender<QueryExecEvent>;

#[derive(Debug, Clone, Serialize)]
pub struct StatementInfo {
    pub returns_values: bool,
    pub status: QueryStatus,
    pub first_page: Option<Box<RawValue>>,
    pub affected_rows: Option<usize>,
    pub error: Option<String>,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum QueryStatus {
    Pending = 0,
    Running = 1,
    Completed = 2,
    Error = 3,
}

impl From<u8> for QueryStatus {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Pending,
            1 => Self::Running,
            2 => Self::Completed,
            _ => Self::Error,
        }
    }
}

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

#[derive(Clone)]
pub enum DatabaseClient {
    Postgres {
        client: Arc<tokio_postgres::Client>,
    },
    SQLite {
        connection: Arc<Mutex<rusqlite::Connection>>,
    },
}

#[derive(Debug)]
pub enum Database {
    Postgres {
        connection_string: String,
        client: Option<Arc<tokio_postgres::Client>>,
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

    /// Get the inner client object
    pub fn get_client(&self) -> Result<DatabaseClient, Error> {
        let client = match &self.database {
            Database::Postgres {
                client: Some(client),
                ..
            } => DatabaseClient::Postgres {
                client: client.clone(),
            },
            Database::Postgres { client: None, .. } => {
                return Err(Error::Any(anyhow::anyhow!(
                    "Postgres connection not active"
                )));
            }
            Database::SQLite {
                connection: Some(sqlite_conn),
                ..
            } => DatabaseClient::SQLite {
                connection: sqlite_conn.clone(),
            },
            Database::SQLite {
                connection: None, ..
            } => {
                return Err(Error::Any(anyhow::anyhow!("SQLite connection not active")));
            }
        };

        Ok(client)
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

pub fn channel() -> (
    UnboundedSender<QueryExecEvent>,
    UnboundedReceiver<QueryExecEvent>,
) {
    mpsc::unbounded_channel()
}

#[derive(Debug)]
/// An event sent by a query executor to the main thread
pub enum QueryExecEvent {
    /// Sent by a query executor when the column types of a query are now known
    TypesResolved {
        // Serialized Vec<String>, because I can't help myself
        columns: Box<RawValue>,
    },
    /// Sent by a query executor when a page of results is available
    Page {
        #[allow(unused)]
        page_amount: usize,
        /// JSON-serialized Vec<Vec<Json>>
        page: Page,
    },
    Finished {
        #[allow(unused)]
        elapsed_ms: u64,
        /// Number of rows affected by the query
        /// Relevant only for modification queries
        affected_rows: usize,
        /// If the query failed, this will contain the error message
        error: Option<String>,
    },
}
