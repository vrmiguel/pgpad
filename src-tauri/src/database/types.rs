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
    pub is_explain_plan: bool,
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
    Postgres {
        connection_string: String,
        ca_cert_path: Option<String>,
    },
    SQLite {
        db_path: String,
        passphrase: Option<String>,
    },
    DuckDB {
        db_path: String,
    },
    Oracle {
        connection_string: String,
        wallet_path: Option<String>,
        tns_alias: Option<String>,
    },
    Mssql {
        connection_string: String,
        ca_cert_path: Option<String>,
    },
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
    DuckDB {
        connection: Arc<Mutex<duckdb::Connection>>,
    },
    Oracle {
        connection: Arc<Mutex<oracle::Connection>>,
    },
    Mssql {
        connection: Arc<Mutex<tiberius::Client<crate::database::mssql::connect::MssqlStream>>>,
    },
}

#[derive(Debug)]
pub enum Database {
    Postgres {
        connection_string: String,
        ca_cert_path: Option<String>,
        client: Option<Arc<tokio_postgres::Client>>,
        backend_pid: Option<i32>,
    },
    SQLite {
        db_path: String,
        connection: Option<Arc<Mutex<rusqlite::Connection>>>,
    },
    DuckDB {
        db_path: String,
        connection: Option<Arc<Mutex<duckdb::Connection>>>,
    },
    Oracle {
        connection_string: String,
        wallet_path: Option<String>,
        tns_alias: Option<String>,
        connection: Option<Arc<Mutex<oracle::Connection>>>,
    },
    Mssql {
        connection_string: String,
        ca_cert_path: Option<String>,
        connection:
            Option<Arc<Mutex<tiberius::Client<crate::database::mssql::connect::MssqlStream>>>>,
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
                    connection_string,
                    ca_cert_path,
                    ..
                } => DatabaseInfo::Postgres {
                    connection_string: connection_string.clone(),
                    ca_cert_path: ca_cert_path.clone(),
                },
                Database::SQLite { db_path, .. } => DatabaseInfo::SQLite {
                    db_path: db_path.clone(),
                    passphrase: None,
                },
                Database::DuckDB { db_path, .. } => DatabaseInfo::DuckDB {
                    db_path: db_path.clone(),
                },
                Database::Oracle {
                    connection_string,
                    wallet_path,
                    tns_alias,
                    ..
                } => DatabaseInfo::Oracle {
                    connection_string: connection_string.clone(),
                    wallet_path: wallet_path.clone(),
                    tns_alias: tns_alias.clone(),
                },
                Database::Mssql {
                    connection_string,
                    ca_cert_path,
                    ..
                } => DatabaseInfo::Mssql {
                    connection_string: connection_string.clone(),
                    ca_cert_path: ca_cert_path.clone(),
                },
            },
        }
    }

    pub fn new(id: Uuid, name: String, database_info: DatabaseInfo) -> Self {
        let database = match database_info {
            DatabaseInfo::Postgres {
                connection_string,
                ca_cert_path,
            } => Database::Postgres {
                connection_string,
                ca_cert_path,
                client: None,
                backend_pid: None,
            },
            DatabaseInfo::SQLite { db_path, .. } => Database::SQLite {
                db_path,
                connection: None,
            },
            DatabaseInfo::DuckDB { db_path } => Database::DuckDB {
                db_path,
                connection: None,
            },
            DatabaseInfo::Oracle {
                connection_string,
                wallet_path,
                tns_alias,
            } => Database::Oracle {
                connection_string,
                wallet_path,
                tns_alias,
                connection: None,
            },
            DatabaseInfo::Mssql {
                connection_string,
                ca_cert_path,
            } => Database::Mssql {
                connection_string,
                ca_cert_path,
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
            Database::DuckDB { connection, .. } => connection.is_some(),
            Database::Oracle { connection, .. } => connection.is_some(),
            Database::Mssql { connection, .. } => connection.is_some(),
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
            Database::DuckDB {
                connection: Some(duckdb_conn),
                ..
            } => DatabaseClient::DuckDB {
                connection: duckdb_conn.clone(),
            },
            Database::DuckDB {
                connection: None, ..
            } => {
                return Err(Error::Any(anyhow::anyhow!("DuckDB connection not active")));
            }
            Database::Oracle {
                connection: Some(oracle_conn),
                ..
            } => DatabaseClient::Oracle {
                connection: oracle_conn.clone(),
            },
            Database::Oracle {
                connection: None, ..
            } => {
                return Err(Error::Any(anyhow::anyhow!("Oracle connection not active")));
            }
            Database::Mssql {
                connection: Some(mssql_conn),
                ..
            } => DatabaseClient::Mssql {
                connection: mssql_conn.clone(),
            },
            Database::Mssql {
                connection: None, ..
            } => {
                return Err(Error::Any(anyhow::anyhow!("MSSQL connection not active")));
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
    #[allow(dead_code)]
    BlobChunk {
        #[allow(dead_code)]
        row_index: usize,
        #[allow(dead_code)]
        column_index: usize,
        #[allow(dead_code)]
        offset: usize,
        #[allow(dead_code)]
        hex_chunk: String,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleSettings {
    pub raw_format: Option<String>,
    pub raw_chunk_size: Option<usize>,
    pub blob_stream: Option<String>,
    pub blob_chunk_size: Option<usize>,
    pub allow_db_link_ping: Option<bool>,
    pub xplan_format: Option<String>,
    pub xplan_mode: Option<String>,
    pub reconnect_max_retries: Option<u32>,
    pub reconnect_backoff_ms: Option<u64>,
    pub stmt_cache_size: Option<u32>,
    pub batch_size: Option<usize>,
    pub bytes_format: Option<String>,
    pub bytes_chunk_size: Option<usize>,
    pub timestamp_tz_mode: Option<String>,
    pub numeric_string_policy: Option<String>,
    pub numeric_precision_threshold: Option<usize>,
    pub json_detection: Option<String>,
    pub json_min_length: Option<usize>,
    pub money_as_string: Option<bool>,
    pub money_decimals: Option<usize>,
}
