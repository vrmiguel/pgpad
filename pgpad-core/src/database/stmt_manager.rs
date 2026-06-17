use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::Context;
use serde_json::value::RawValue;
use tokio::task::{self, JoinHandle};

use dashmap::DashMap;

use crate::{
    database::{
        parser::ParsedStatement,
        postgres, sqlite,
        types::{channel, Page, QueryId, QuerySnapshot, QueryStatus, RuntimeClient},
        QueryExecEvent,
    },
    utils::Condvar,
    Error,
};

/// The storage/state for an individual statement being executed
struct ExecState {
    /// True if this query is expected to return some amount of rows
    /// False if this is a query that will never return anything (e.g. an UPDATE without a RETURNING clause)
    returns_values: bool,
    inner: Mutex<ExecInner>,

    /// If set, the UI can now render the results of this query,
    /// even if it's still on-going (e.g. we already have enough data to render the first page)
    renderable: Condvar,
}

struct ExecInner {
    status: QueryStatus,
    output: ExecOutput,
    error: Option<String>,
}

enum ExecOutput {
    Pending,
    ResultSet {
        columns: Option<Box<RawValue>>,
        pages: Vec<Page>,
    },
    Modification {
        affected_rows: Option<usize>,
    },
}

impl ExecState {
    fn new(returns_values: bool) -> Self {
        Self {
            returns_values,
            inner: Mutex::new(ExecInner {
                status: QueryStatus::Pending,
                output: ExecOutput::Pending,
                error: None,
            }),
            renderable: Condvar::new(),
        }
    }

    fn inner(&self) -> MutexGuard<'_, ExecInner> {
        self.inner.lock().expect("Mutex poisoned")
    }

    fn mark_running(&self) {
        self.inner().status = QueryStatus::Running;
    }

    fn set_columns(&self, columns: Box<RawValue>) {
        let mut inner = self.inner();

        match &mut inner.output {
            ExecOutput::Pending => {
                inner.output = ExecOutput::ResultSet {
                    columns: Some(columns),
                    pages: vec![],
                };
            }
            ExecOutput::ResultSet {
                columns: existing, ..
            } => {
                *existing = Some(columns);
            }
            ExecOutput::Modification { .. } => {}
        }
    }

    fn push_page(&self, page: Page) {
        {
            let mut inner = self.inner();

            match &mut inner.output {
                ExecOutput::Pending => {
                    inner.output = ExecOutput::ResultSet {
                        columns: None,
                        pages: vec![page],
                    };
                }
                ExecOutput::ResultSet { pages, .. } => {
                    pages.push(page);
                }
                ExecOutput::Modification { .. } => {}
            }
        }

        self.renderable.set();
    }

    fn finish(&self, affected_rows: usize, error: Option<String>) {
        {
            let mut inner = self.inner();

            if let Some(message) = error {
                inner.status = QueryStatus::Error;
                inner.error = Some(message);
            } else {
                inner.status = QueryStatus::Completed;
                inner.error = None;

                if self.returns_values {
                    if matches!(inner.output, ExecOutput::Pending) {
                        inner.output = ExecOutput::ResultSet {
                            columns: None,
                            pages: vec![],
                        };
                    }
                } else {
                    inner.output = ExecOutput::Modification {
                        affected_rows: Some(affected_rows),
                    };
                }
            }
        }

        self.renderable.set();
    }

    fn snapshot(&self) -> QuerySnapshot {
        let inner = self.inner();

        match &inner.output {
            ExecOutput::Pending => QuerySnapshot {
                returns_values: self.returns_values,
                status: inner.status,
                first_page: None,
                affected_rows: None,
                error: inner.error.clone(),
                columns: None,
            },
            ExecOutput::ResultSet { columns, pages } => QuerySnapshot {
                returns_values: true,
                status: inner.status,
                first_page: pages.first().cloned(),
                affected_rows: None,
                error: inner.error.clone(),
                columns: columns.clone(),
            },
            ExecOutput::Modification { affected_rows } => QuerySnapshot {
                returns_values: false,
                status: inner.status,
                first_page: None,
                affected_rows: *affected_rows,
                error: inner.error.clone(),
                columns: None,
            },
        }
    }

    fn get_columns(&self) -> Option<Box<RawValue>> {
        let inner = self.inner();

        match &inner.output {
            ExecOutput::ResultSet { columns, .. } => columns.clone(),
            ExecOutput::Pending | ExecOutput::Modification { .. } => None,
        }
    }

    fn fetch_page(&self, page_idx: usize) -> Option<Page> {
        let inner = self.inner();

        match &inner.output {
            ExecOutput::ResultSet { pages, .. } => pages.get(page_idx).cloned(),
            ExecOutput::Pending | ExecOutput::Modification { .. } => None,
        }
    }

    fn get_query_status(&self) -> QueryStatus {
        self.inner().status
    }

    fn get_page_count(&self) -> usize {
        let inner = self.inner();

        match &inner.output {
            ExecOutput::ResultSet { pages, .. } => pages.len(),
            ExecOutput::Pending | ExecOutput::Modification { .. } => 0,
        }
    }
}

/// Executes and keeps track of the execution of queries.
pub struct StatementManager {
    queries: DashMap<QueryId, Arc<ExecState>>,
    /// Handles for tasks spawned by the current batch of queries
    task_handles: Mutex<Vec<JoinHandle<()>>>,
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
            task_handles: Mutex::new(Vec::new()),
        }
    }

    fn stop_workers(&self) {
        let mut handles = self.task_handles.lock().unwrap();
        for handle in handles.drain(..) {
            handle.abort();
        }
    }

    /// Submits a new query (possibly containing multiple statements) for execution
    pub fn submit_query(&self, client: RuntimeClient, query: &str) -> Result<Vec<QueryId>, Error> {
        self.stop_workers();
        self.queries.clear();

        let parse_statements = match &client {
            RuntimeClient::Postgres { .. } => postgres::parser::parse_statements,
            RuntimeClient::SQLite { .. } => sqlite::parser::parse_statements,
        };

        let statements = parse_statements(query)?;
        let mut query_ids = Vec::with_capacity(statements.len());
        let mut handles = self.task_handles.lock().unwrap();

        for (idx, statement) in statements.into_iter().enumerate() {
            let new_handles = self.create_worker(idx as QueryId, client.clone(), statement);
            handles.extend(new_handles);
            query_ids.push(idx);
        }

        Ok(query_ids)
    }

    /// Fetches initial data on a query in execution. This will block until said data is available.
    /// Useful for the front-end to poll the execution status, mainly when it is still trying to load the first page of results
    pub async fn fetch_initial_renderable_state(
        &self,
        query_id: QueryId,
    ) -> Result<QuerySnapshot, Error> {
        let exec_state = self.get(query_id)?;
        // Wait for the data to load in
        exec_state.renderable.wait().await;

        Ok(exec_state.snapshot())
    }

    pub fn get_columns(&self, query_id: QueryId) -> Result<Option<Box<RawValue>>, Error> {
        Ok(self.get(query_id)?.get_columns())
    }

    /// Fetches a page of results for a given query.
    pub fn fetch_page(&self, query_id: QueryId, page_idx: usize) -> Result<Option<Page>, Error> {
        Ok(self.get(query_id)?.fetch_page(page_idx))
    }

    pub fn get_query_status(&self, query_id: QueryId) -> Result<QueryStatus, Error> {
        Ok(self.get(query_id)?.get_query_status())
    }

    pub fn get_page_count(&self, query_id: QueryId) -> Result<usize, Error> {
        Ok(self.get(query_id)?.get_page_count())
    }
}

/// Impl block for internal methods
impl StatementManager {
    fn create_worker(
        &self,
        id: QueryId,
        client: RuntimeClient,
        stmt: ParsedStatement,
    ) -> [JoinHandle<()>; 2] {
        let exec_storage = ExecState::new(stmt.returns_values);
        let exec_storage = Arc::new(exec_storage);
        self.queries.insert(id, exec_storage.clone());

        let (sender, recv) = channel();

        let executor_handle = match client {
            RuntimeClient::Postgres { client } => task::spawn(async move {
                if let Err(err) = postgres::execute::execute_query(&client, stmt, &sender).await {
                    log::error!("Error executing Postgres query: {}", err);
                }
            }),
            RuntimeClient::SQLite { connection } => task::spawn_blocking(move || {
                let conn = connection.lock().unwrap();
                if let Err(err) = sqlite::execute::execute_query(&conn, stmt, &sender) {
                    log::error!("Error executing SQLite query: {}", err);
                }
            }),
        };

        let receiver_handle = task::spawn(async move {
            let mut recv = recv;

            exec_storage.mark_running();

            while let Some(event) = recv.recv().await {
                match event {
                    QueryExecEvent::TypesResolved { columns } => {
                        exec_storage.set_columns(columns);
                    }
                    QueryExecEvent::Page {
                        page_amount: _,
                        page,
                    } => {
                        exec_storage.push_page(page);
                    }
                    QueryExecEvent::Finished {
                        elapsed_ms: _,
                        affected_rows,
                        error,
                    } => {
                        exec_storage.finish(affected_rows, error);

                        // TODO(vini): fingerprint query here, and save it?

                        break;
                    }
                }
            }
        });

        [executor_handle, receiver_handle]
    }

    fn get(&self, query_id: QueryId) -> Result<Arc<ExecState>, Error> {
        self.queries
            .get(&query_id)
            .with_context(|| format!("Did not find QueryId({query_id}) in StatementManager"))
            .map_err(Into::into)
            .map(|entry| entry.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::{json, value::RawValue};

    use crate::database::types::RuntimeClient;

    use super::StatementManager;

    #[tokio::test]
    async fn test_basic_functionality() {
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

    async fn run_query(query: &str) -> (Box<RawValue>, Box<RawValue>) {
        let stmt_manager = StatementManager::new();

        let client = RuntimeClient::SQLite {
            connection: Arc::new(Mutex::new(rusqlite::Connection::open_in_memory().unwrap())),
        };
        let query_ids = stmt_manager.submit_query(client, query).unwrap();
        assert_eq!(query_ids, vec![0]);

        let snapshot = stmt_manager
            .fetch_initial_renderable_state(0)
            .await
            .unwrap();

        (
            snapshot.columns.expect("columns returned None"),
            snapshot.first_page.expect("columns returned None"),
        )
    }

    #[tokio::test]
    async fn text_csv_exports() {
        let query = r"
        SELECT column1 AS id, column2 AS name, column3 AS price
        FROM (
            VALUES
                (1, 'apple', 0.99),
                (2, 'banana', 1.25),
                (3, 'cherry', 2.50));";
        let (columns, page) = run_query(query).await;
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(columns.get()).unwrap(),
            json!(["id", "name", "price"])
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(page.get()).unwrap(),
            json!([[1, "apple", 0.99], [2, "banana", 1.25], [3, "cherry", 2.5]])
        );

        let csv_export = crate::database::export::export_to_csv(columns.get(), page.get()).unwrap();
        assert_eq!(
            csv_export,
            "id,name,price\n1,\"apple\",0.99\n2,\"banana\",1.25\n3,\"cherry\",2.5\n"
        );
    }
}
