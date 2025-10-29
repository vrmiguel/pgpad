use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc, RwLock,
};

use anyhow::Context;
use serde_json::value::RawValue;
use tauri::async_runtime::{spawn, spawn_blocking};

use dashmap::DashMap;

use crate::{
    database::{
        parser::ParsedStatement,
        postgres, sqlite,
        types::{channel, DatabaseClient, Page, QueryId, QueryStatus, StatementInfo},
        QueryExecEvent,
    },
    Error,
};

/// The storage/state for an individual statement being executed
struct ExecState {
    status: AtomicU8,
    pages: RwLock<Vec<Page>>,
    error: RwLock<Option<String>>,
    columns: RwLock<Option<Box<RawValue>>>,
    /// True if this query is expected to return some amount of rows
    /// False if this is a query that will never return anything (e.g. an UPDATE without a RETURNING clause)
    // TODO(vini): we could refactor this into an enum with a variant with `pages`, `columns`, and one with just `rows_affected`
    returns_values: bool,
    rows_affected: RwLock<Option<usize>>,
}

/// Executes and keeps track of the execution of queries.
pub struct StatementManager {
    queries: DashMap<QueryId, Arc<ExecState>>,
}

impl std::fmt::Debug for StatementManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StatementManager")
    }
}

#[allow(clippy::new_without_default)]
impl StatementManager {
    pub fn new() -> Self {
        Self {
            queries: DashMap::new(),
        }
    }

    /// Submits a new query (possibly containing multiple statements) for execution.
    ///
    /// Note that this _will_ cancel the execution of any ongoing query, and replace it with the new one.
    // TODO(vini): not sure if this will actually cancel the ongoing query.
    // Might need to store the joinhandles so we can properly cancel them
    pub fn submit_query(&self, client: DatabaseClient, query: &str) -> Result<Vec<QueryId>, Error> {
        self.queries.clear();

        let parse_statements = match &client {
            DatabaseClient::Postgres { .. } => postgres::parser::parse_statements,
            DatabaseClient::SQLite { .. } => sqlite::parser::parse_statements,
        };

        let statements = parse_statements(query)?;
        let mut query_ids = Vec::with_capacity(statements.len());

        for (idx, statement) in statements.into_iter().enumerate() {
            self.create_worker(idx as QueryId, client.clone(), statement);
            query_ids.push(idx);
        }

        Ok(query_ids)
    }

    /// Fetches some general data on a query execution.
    /// Useful for the front-end to poll the execution status, mainly when it is still trying to load the first page of results
    pub fn fetch_query(&self, query_id: QueryId) -> Result<StatementInfo, Error> {
        let exec_state = self.get(query_id)?;
        let returns_values = exec_state.returns_values;

        let info = StatementInfo {
            returns_values,
            status: exec_state.status.load(Ordering::Relaxed).into(),
            first_page: if returns_values {
                let pages = exec_state.pages.read().expect("RwLock poisoned");
                pages.first().cloned()
            } else {
                None
            },
            affected_rows: *exec_state.rows_affected.read().expect("RwLock poisoned"),
            error: exec_state.error.read().expect("RwLock poisoned").clone(),
        };

        Ok(info)
    }

    /// Fetches a page of results for a given query.
    pub fn fetch_page(&self, query_id: QueryId, page_idx: usize) -> Result<Option<Page>, Error> {
        let exec_state = self.get(query_id)?;
        let pages = exec_state.pages.read().expect("RwLock poisoned");
        Ok(pages.get(page_idx).cloned())
    }

    pub fn get_query_status(&self, query_id: QueryId) -> Result<QueryStatus, Error> {
        let exec_state = self.get(query_id)?;

        Ok(exec_state.status.load(Ordering::Relaxed).into())
    }

    pub fn get_page_count(&self, query_id: QueryId) -> Result<usize, Error> {
        let exec_state = self.get(query_id)?;
        let page_count = exec_state.pages.read().expect("RwLock poisoned").len();
        Ok(page_count)
    }

    pub fn get_columns(&self, query_id: QueryId) -> Result<Option<Box<RawValue>>, Error> {
        let exec_state = self.get(query_id)?;

        let columns = exec_state.columns.read().expect("RwLock poisoned");

        Ok(columns.clone())
    }
}

/// Impl block for internal methods
impl StatementManager {
    fn create_worker(&self, id: QueryId, client: DatabaseClient, stmt: ParsedStatement) {
        let exec_storage = ExecState {
            status: AtomicU8::new(QueryStatus::Pending as u8),
            pages: RwLock::new(vec![]),
            error: RwLock::new(None),
            columns: RwLock::new(None),
            returns_values: stmt.returns_values,
            rows_affected: RwLock::new(None),
        };

        let exec_storage = Arc::new(exec_storage);
        self.queries.insert(id, exec_storage.clone());

        let (sender, recv) = channel();

        match client {
            DatabaseClient::Postgres { client } => {
                spawn(async move {
                    if let Err(err) = postgres::execute::execute_query(&client, stmt, &sender).await
                    {
                        log::error!("Error executing Postgres query: {}", err);
                    }
                });
            }
            DatabaseClient::SQLite { connection } => {
                spawn_blocking(move || {
                    let conn = connection.lock().unwrap();
                    if let Err(err) = sqlite::execute::execute_query(&conn, stmt, &sender) {
                        log::error!("Error executing SQLite query: {}", err);
                    }
                });
            }
        }

        tokio::task::spawn(async move {
            let mut recv = recv;

            exec_storage
                .status
                .store(QueryStatus::Running as u8, Ordering::Relaxed);

            while let Some(event) = recv.recv().await {
                match event {
                    QueryExecEvent::TypesResolved { columns } => {
                        *exec_storage.columns.write().unwrap() = Some(columns);
                    }
                    QueryExecEvent::Page {
                        page_amount: _,
                        page,
                    } => {
                        exec_storage.pages.write().unwrap().push(page);

                        // TODO(vini): emit progress event to frontend?
                    }
                    QueryExecEvent::Finished {
                        elapsed_ms: _,
                        affected_rows,
                        error,
                    } => {
                        if let Some(err) = error {
                            *exec_storage.error.write().unwrap() = Some(err);
                            exec_storage
                                .status
                                .store(QueryStatus::Error as u8, Ordering::Relaxed);
                        } else {
                            exec_storage
                                .status
                                .store(QueryStatus::Completed as u8, Ordering::Relaxed);

                            *exec_storage.rows_affected.write().unwrap() = Some(affected_rows);
                        }

                        // TODO(vini): emit completion event to frontend?

                        break;
                    }
                }
            }
        });
    }

    fn get(
        &self,
        query_id: QueryId,
    ) -> Result<dashmap::mapref::one::Ref<'_, usize, Arc<ExecState>>, Error> {
        self.queries
            .get(&query_id)
            .with_context(|| format!("Did not find QueryId({query_id}) in StatementManager"))
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    use crate::database::{stmt_manager::QueryStatus, types::DatabaseClient};

    use super::StatementManager;

    #[tokio::test]
    async fn test_basic_functionality() {
        let stmt_manager = StatementManager::new();
        let client = DatabaseClient::SQLite {
            connection: Arc::new(Mutex::new(rusqlite::Connection::open_in_memory().unwrap())),
        };
        let query_ids = stmt_manager.submit_query(client, "SELECT 1").unwrap();
        assert_eq!(query_ids, vec![0]);

        let mut attempt = 0;
        while attempt < 3 {
            attempt += 1;
            let page = stmt_manager.get_query_status(0).unwrap();
            if page == QueryStatus::Completed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        let columns = stmt_manager
            .get_columns(0)
            .unwrap()
            .expect("get_columns returned None");
        assert_eq!(serde_json::to_string(&columns).unwrap(), "[\"1\"]");

        let page = stmt_manager
            .fetch_page(0, 0)
            .unwrap()
            .expect("Page not found after 3 attempts");
        assert_eq!(serde_json::to_string(&page).unwrap(), r#"[[1]]"#);
    }
}
