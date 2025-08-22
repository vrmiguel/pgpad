use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use tokio_postgres::Client;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub name: String,
    pub connection_string: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub id: Uuid,
    pub name: String,
    pub connection_string: String,
    pub connected: bool,
}

#[derive(Debug)]
pub struct DatabaseConnection {
    pub info: ConnectionInfo,
    pub client: Option<Client>,
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
pub enum QueryStreamEvent<'a> {
    StatementStart {
        statement_index: usize,
        total_statements: usize,
        statement: String,
        returns_values: bool,
    },

    /// For queries that return data
    ResultStart {
        statement_index: usize,
        columns: Vec<&'a str>,
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
