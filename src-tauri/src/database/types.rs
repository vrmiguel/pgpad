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

/// A "snapshot" of a query
#[derive(Debug, Clone, Serialize)]
pub struct QuerySnapshot {
    pub returns_values: bool,
    pub status: QueryStatus,
    pub first_page: Option<Box<RawValue>>,
    pub affected_rows: Option<usize>,
    pub columns: Option<Box<RawValue>>,
    pub error: Option<String>,
}

pub enum DatabaseKind {
    Postgres,
    Sqlite,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Permissions {
    #[default]
    ReadWrite,
    ProtectedWrite,
    ReadOnly,
}

impl Permissions {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ReadWrite => "read_write",
            Self::ProtectedWrite => "protected_write",
            Self::ReadOnly => "read_only",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "protected_write" => Self::ProtectedWrite,
            "read_only" => Self::ReadOnly,
            _ => Self::ReadWrite,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub id: Uuid,
    pub name: String,
    pub connected: bool,
    pub permissions: Permissions,
    pub config: ConnectionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionConfig {
    Postgres {
        connection_string: String,
        ca_cert_path: Option<String>,
    },
    SQLite {
        db_path: String,
    },
}

#[derive(Debug)]
pub struct Connection {
    pub id: Uuid,
    pub name: String,
    pub permissions: Permissions,
    pub config: ConnectionConfig,
    pub runtime: ConnectionRuntime,
}

#[derive(Debug, Clone)]
pub enum RuntimeClient {
    Postgres {
        client: Arc<tokio_postgres::Client>,
    },
    SQLite {
        connection: Arc<Mutex<rusqlite::Connection>>,
    },
}

#[derive(Debug, Clone)]
pub enum ConnectionRuntime {
    Disconnected,
    Connected(RuntimeClient),
}

impl ConnectionConfig {
    pub fn kind(&self) -> DatabaseKind {
        match self {
            ConnectionConfig::Postgres { .. } => DatabaseKind::Postgres,
            ConnectionConfig::SQLite { .. } => DatabaseKind::Sqlite,
        }
    }
}

impl Connection {
    pub fn to_connection_info(&self) -> ConnectionInfo {
        ConnectionInfo {
            id: self.id,
            name: self.name.clone(),
            connected: self.is_client_connected(),
            permissions: self.permissions,
            config: self.config.clone(),
        }
    }

    pub fn new(id: Uuid, name: String, config: ConnectionConfig, permissions: Permissions) -> Self {
        Self {
            id,
            name,
            permissions,
            config,
            runtime: ConnectionRuntime::Disconnected,
        }
    }

    pub fn is_client_connected(&self) -> bool {
        matches!(self.runtime, ConnectionRuntime::Connected(_))
    }

    /// Get the inner client object
    pub fn get_client(&self) -> Result<RuntimeClient, Error> {
        let client = match &self.runtime {
            ConnectionRuntime::Connected(RuntimeClient::Postgres { client }) => {
                RuntimeClient::Postgres {
                    client: client.clone(),
                }
            }
            ConnectionRuntime::Connected(RuntimeClient::SQLite { connection }) => {
                RuntimeClient::SQLite {
                    connection: connection.clone(),
                }
            }
            ConnectionRuntime::Disconnected => {
                return Err(Error::Any(anyhow::anyhow!("Connection not active")));
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
