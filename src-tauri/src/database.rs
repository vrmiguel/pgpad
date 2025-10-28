pub type QueryId = usize;

/// A row of data serialized by one of our writers
/// You can conceptually think of this as a Vec<Vec<Json>>.
///
/// Example:
/// - Query: `SELECT 1,2,3 UNION ALL SELECT 4,5,6`
/// - Page: `[[1,2,3],[4,5,6]]`
pub type Page = Box<RawValue>;

pub type ExecSender = UnboundedSender<QueryExecEvent>;
// pub type ExecReceiver = UnboundedReceiver<QueryExecEvent>;

pub mod postgres;
pub mod sqlite;

pub use postgres::tls::Certificates;

pub mod commands;
mod connect;
mod connection_monitor;
pub mod parser;
pub mod stmt_manager;
pub mod types;

pub use connection_monitor::ConnectionMonitor;
use serde_json::value::RawValue;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

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
        page_amount: usize,
        /// JSON-serialized Vec<Vec<Json>>
        page: Page,
    },
    Finished {
        elapsed_ms: u64,
        /// Number of rows affected by the query
        /// Relevant only for modification queries
        affected_rows: u64,
        /// If the query failed, this will contain the error message
        error: Option<String>,
    },
}

// #[expect(dead_code)]
