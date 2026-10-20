//! End-to-end tests for the DuckDB driver: a statement goes through the
//! parser, the executor and the row writer, and comes back as JSON.

use std::sync::{Arc, Mutex};

use serde_json::{json, value::RawValue};

use crate::database::stmt_manager::StatementManager;
use crate::database::types::RuntimeClient;

fn client() -> RuntimeClient {
    RuntimeClient::DuckDB {
        connection: Arc::new(Mutex::new(duckdb::Connection::open_in_memory().unwrap())),
    }
}

async fn run_query(query: &str) -> (Box<RawValue>, Box<RawValue>) {
    let stmt_manager = StatementManager::new();

    let query_ids = stmt_manager.submit_query(client(), query).unwrap();
    assert_eq!(query_ids, vec![0]);

    let snapshot = stmt_manager
        .fetch_initial_renderable_state(0)
        .await
        .unwrap();

    (
        snapshot.columns.expect("columns returned None"),
        snapshot.first_page.expect("first page returned None"),
    )
}

#[tokio::test]
async fn a_select_returns_its_rows() {
    let (columns, page) = run_query("SELECT 1").await;

    assert_eq!(
        serde_json::from_str::<serde_json::Value>(columns.get()).unwrap(),
        json!(["1"])
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(page.get()).unwrap(),
        json!([[1]])
    );
}

#[tokio::test]
async fn a_table_is_read_back() {
    let stmt_manager = StatementManager::new();
    let connection = Arc::new(Mutex::new(duckdb::Connection::open_in_memory().unwrap()));
    connection
        .lock()
        .unwrap()
        .execute_batch("CREATE TABLE fruit (id INTEGER, name VARCHAR);")
        .unwrap();
    connection
        .lock()
        .unwrap()
        .execute("INSERT INTO fruit VALUES (1, 'apple'), (2, 'banana')", [])
        .unwrap();

    let client = RuntimeClient::DuckDB { connection };
    stmt_manager
        .submit_query(client, "SELECT name FROM fruit ORDER BY id")
        .unwrap();
    let snapshot = stmt_manager
        .fetch_initial_renderable_state(0)
        .await
        .unwrap();
    let page = snapshot.first_page.expect("first page returned None");

    assert_eq!(
        serde_json::from_str::<serde_json::Value>(page.get()).unwrap(),
        json!([["apple"], ["banana"]])
    );
}
