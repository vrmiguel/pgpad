use std::time::Instant;

use rusqlite::Connection;

use crate::{
    database::{
        parser::ParsedStatement, sqlite::row_writer::RowWriter, types::ExecSender, QueryExecEvent,
    },
    utils::serialize_as_json_array,
    Error,
};

/// Make sure to run this on a task where blocking is allowed.
pub fn execute_query(
    client: &Connection,
    stmt: ParsedStatement,
    sender: &ExecSender,
) -> Result<(), Error> {
    let start = std::time::Instant::now();

    if stmt.returns_values {
        execute_query_with_results(client, &stmt.statement, sender, start)?;
    } else {
        execute_modification_query(client, &stmt.statement, sender, start)?;
    }

    Ok(())
}

fn execute_query_with_results(
    client: &Connection,
    query: &str,
    sender: &ExecSender,
    started_at: Instant,
) -> Result<(), Error> {
    log::info!("Starting SQLite query: {}", query);

    match client.prepare(query) {
        Ok(mut stmt) => {
            let columns = stmt.columns();
            let column_names = columns.iter().map(|c| c.name());
            let column_types: Vec<_> = columns
                .iter()
                .map(|c| c.decl_type().map(ToString::to_string))
                .collect();
            let columns = serialize_as_json_array(column_names)?;

            match stmt.query([]) {
                Ok(mut rows) => {
                    sender.send(QueryExecEvent::TypesResolved { columns })?;

                    let mut total_rows = 0;
                    // TODO: make this configurable
                    let batch_size = 50;
                    let mut writer = RowWriter::new(column_types);

                    loop {
                        match rows.next() {
                            Ok(Some(row)) => {
                                writer.add_row(row)?;
                                total_rows += 1;

                                if writer.len() >= batch_size {
                                    sender.send(QueryExecEvent::Page {
                                        page_amount: writer.len(),
                                        page: writer.finish(),
                                    })?;
                                }
                            }
                            Ok(None) => {
                                // End of results
                                break;
                            }
                            Err(e) => {
                                log::error!("Error processing SQLite row: {}", e);
                                let error_msg = format!("Query failed: {}", e);

                                sender.send(QueryExecEvent::Finished {
                                    elapsed_ms: started_at.elapsed().as_millis() as u64,
                                    // TODO(vini): this is actually not necessarily true?
                                    //             Might not matter, though
                                    affected_rows: 0,
                                    error: Some(error_msg),
                                })?;
                            }
                        }
                    }

                    if !writer.is_empty() {
                        sender.send(QueryExecEvent::Page {
                            page_amount: writer.len(),
                            page: writer.finish(),
                        })?;
                    }

                    let duration = started_at.elapsed().as_millis() as u64;
                    log::info!(
                        "SQLite query completed: {} rows in {}ms",
                        total_rows,
                        duration
                    );

                    sender.send(QueryExecEvent::Finished {
                        elapsed_ms: started_at.elapsed().as_millis() as u64,
                        affected_rows: 0,
                        error: None,
                    })?;

                    Ok(())
                }
                Err(e) => {
                    log::error!("SQLite query execution failed: {:?}", e);
                    let error_msg = format!("Query failed: {}", e);

                    sender.send(QueryExecEvent::Finished {
                        elapsed_ms: started_at.elapsed().as_millis() as u64,
                        affected_rows: 0,
                        error: Some(error_msg.clone()),
                    })?;

                    // TODO(vini): is this necessary, if we already sent the error to the receiver thread?
                    Err(Error::Any(anyhow::anyhow!(error_msg)))
                }
            }
        }
        Err(e) => {
            log::error!("SQLite statement preparation failed: {:?}", e);
            let error_msg = format!("Query failed: {}", e);

            sender.send(QueryExecEvent::Finished {
                elapsed_ms: started_at.elapsed().as_millis() as u64,
                affected_rows: 0,
                error: Some(error_msg.clone()),
            })?;

            Err(Error::Any(anyhow::anyhow!(error_msg)))
        }
    }
}

fn execute_modification_query(
    client: &Connection,
    query: &str,
    sender: &ExecSender,
    started_at: Instant,
) -> Result<(), Error> {
    log::info!("Executing modification query: {}", query);

    match client.execute(query, []) {
        Ok(rows_affected) => {
            sender.send(QueryExecEvent::Finished {
                elapsed_ms: started_at.elapsed().as_millis() as u64,
                affected_rows: rows_affected,
                error: None,
            })?;
            Ok(())
        }
        Err(e) => {
            log::error!("Modification query failed: {:?}", e);
            let error_msg = format!("Query failed: {}", e);

            sender.send(QueryExecEvent::Finished {
                elapsed_ms: started_at.elapsed().as_millis() as u64,
                affected_rows: 0,
                error: Some(error_msg.clone()),
            })?;

            Err(Error::Any(anyhow::anyhow!(error_msg)))
        }
    }
}

#[cfg(test)]
mod tests {

    use std::{ops::Not, sync::Arc};

    use rusqlite::Connection;
    use std::sync::Mutex;

    use super::execute_query;
    use crate::database::{sqlite::parser::parse_statements, types::channel, QueryExecEvent};

    async fn run_query(
        conn: Arc<Mutex<Connection>>,
        query: &str,
    ) -> anyhow::Result<Vec<QueryExecEvent>> {
        let mut parsed_stmt = parse_statements(query).unwrap();
        assert_eq!(parsed_stmt.len(), 1);
        assert!(parsed_stmt[0].returns_values);
        let stmt = parsed_stmt.pop().unwrap();

        let (sender, mut recv) = channel();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            execute_query(&conn, stmt, &sender).unwrap();
        });

        let mut events = Vec::new();

        while let Some(event) = recv.recv().await {
            events.push(event);
        }

        Ok(events)
    }

    /// Run a query that returns no results (modification-only?), returning the number of rows affected.
    async fn run_modification_query(
        conn: Arc<Mutex<Connection>>,
        query: &str,
    ) -> anyhow::Result<usize> {
        let mut parsed_stmt = parse_statements(query).unwrap();
        assert_eq!(parsed_stmt.len(), 1);
        assert!(parsed_stmt[0].returns_values.not());
        let stmt = parsed_stmt.pop().unwrap();

        let (sender, mut recv) = channel();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            execute_query(&conn, stmt, &sender).unwrap();
        });

        let event = recv
            .recv()
            .await
            .ok_or(anyhow::anyhow!("Channel unexpectedly closed"))?;
        assert!(matches!(event, QueryExecEvent::Finished { .. }));
        match event {
            QueryExecEvent::Finished {
                affected_rows,
                error,
                ..
            } => {
                assert!(error.is_none());
                Ok(affected_rows)
            }
            other => Err(anyhow::anyhow!("Expected Finished event, got {:?}", other)),
        }
    }

    #[tokio::test]
    async fn test_simple_query() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let query = "SELECT * FROM (VALUES (1, 'Alice'), (2, 'Bob'), (3, 'Charlie'))";
        let mut parsed_stmt = parse_statements(query).unwrap();
        assert_eq!(parsed_stmt.len(), 1);
        assert!(parsed_stmt[0].returns_values);
        assert_eq!(parsed_stmt[0].statement, query);
        let stmt = parsed_stmt.pop().unwrap();

        let (sender, mut recv) = channel();

        tokio::task::spawn_blocking(move || {
            execute_query(&conn, stmt, &sender).unwrap();
        });

        let event = recv.recv().await.unwrap();
        assert!(matches!(event, QueryExecEvent::TypesResolved { .. }));

        let event = recv.recv().await.unwrap();
        assert!(matches!(event, QueryExecEvent::Page { .. }));
        match event {
            QueryExecEvent::Page { page_amount, page } => {
                assert_eq!(page_amount, 3);
                assert_eq!(
                    serde_json::to_string(&page).unwrap(),
                    r#"[[1,"Alice"],[2,"Bob"],[3,"Charlie"]]"#
                );
            }
            other => panic!("Expected Page event, got {:?}", other),
        }

        let event = recv.recv().await.unwrap();
        dbg!(&event);
        assert!(matches!(event, QueryExecEvent::Finished { .. }));
    }

    #[tokio::test]
    async fn test_mixed_queries() -> anyhow::Result<()> {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let conn = Arc::new(Mutex::new(conn));
        let affected_rows = run_modification_query(
            conn.clone(),
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER);",
        )
        .await?;
        assert_eq!(affected_rows, 0);

        let query =
            "INSERT INTO users (name, age) VALUES ('Alice', 25), ('Bob', 30),('Charlie', 35), ('Diana', 28);";
        let affected_rows = run_modification_query(conn.clone(), query).await?;
        assert_eq!(affected_rows, 4);

        let query = "UPDATE users SET age = age + 1 WHERE name = 'Alice';";
        let affected_rows = run_modification_query(conn.clone(), query).await?;
        assert_eq!(affected_rows, 1);

        let query = "DELETE FROM users WHERE name = 'Bob';";
        let affected_rows = run_modification_query(conn.clone(), query).await?;
        assert_eq!(affected_rows, 1);

        let query = "DELETE FROM users WHERE name = 'Joe';";
        let affected_rows = run_modification_query(conn.clone(), query).await?;
        assert_eq!(affected_rows, 0);

        let query = "SELECT * FROM users";
        let mut events = run_query(conn.clone(), query).await?.into_iter();
        let types_resolved = events.next().unwrap();
        match types_resolved {
            QueryExecEvent::TypesResolved { columns } => {
                assert_eq!(
                    serde_json::to_string(&columns).unwrap(),
                    r#"["id","name","age"]"#
                );
            }
            other => panic!("Expected TypesResolved event, got {:?}", other),
        }

        let page = events.next().unwrap();
        match page {
            QueryExecEvent::Page { page_amount, page } => {
                assert_eq!(page_amount, 3);
                assert_eq!(
                    serde_json::to_string(&page).unwrap(),
                    r#"[[1,"Alice",26],[3,"Charlie",35],[4,"Diana",28]]"#
                );
            }
            other => panic!("Expected Page event, got {:?}", other),
        }

        let finished = events.next().unwrap();
        match finished {
            QueryExecEvent::Finished {
                elapsed_ms,
                affected_rows,
                error,
            } => {
                // This particular query does run fast enough in my machine to be 0ms, so it's hard to assert anything about it
                let _ = elapsed_ms;
                assert!(error.is_none());
                assert_eq!(affected_rows, 0);
            }
            other => panic!("Expected Finished event, got {:?}", other),
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_query_with_many_rows() -> anyhow::Result<()> {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let conn = Arc::new(Mutex::new(conn));

        // Returns 1, 2, 3, .., 155
        let query = "WITH RECURSIVE t(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM t WHERE x < 155) SELECT * FROM t;";
        let mut events = run_query(conn.clone(), query).await?.into_iter();
        let types_resolved = events.next().unwrap();
        match types_resolved {
            QueryExecEvent::TypesResolved { columns } => {
                assert_eq!(serde_json::to_string(&columns).unwrap(), r#"["x"]"#);
            }
            other => panic!("Expected TypesResolved event, got {:?}", other),
        }

        let page_1 = events.next().unwrap();
        match page_1 {
            QueryExecEvent::Page { page_amount, page } => {
                assert_eq!(page_amount, 50);
                let page = serde_json::to_string(&page).unwrap();
                assert!(page.starts_with("[[1],[2]"));
                assert!(page.ends_with("49],[50]]"));
            }
            other => panic!("Expected Page event, got {:?}", other),
        }

        let page_2 = events.next().unwrap();
        match page_2 {
            QueryExecEvent::Page { page_amount, page } => {
                assert_eq!(page_amount, 50);
                let page = serde_json::to_string(&page).unwrap();
                assert!(page.starts_with("[[51],[52]"));
                assert!(page.ends_with("99],[100]]"));
            }
            other => panic!("Expected Page event, got {:?}", other),
        }

        let page_3 = events.next().unwrap();
        match page_3 {
            QueryExecEvent::Page { page_amount, page } => {
                assert_eq!(page_amount, 50);
                let page = serde_json::to_string(&page).unwrap();
                assert!(page.starts_with("[[101],[102]"));
                assert!(page.ends_with("149],[150]]"));
            }
            other => panic!("Expected Page event, got {:?}", other),
        }

        let page_4 = events.next().unwrap();
        match page_4 {
            QueryExecEvent::Page { page_amount, page } => {
                assert_eq!(page_amount, 5);
                assert_eq!(
                    serde_json::to_string(&page).unwrap(),
                    "[[151],[152],[153],[154],[155]]"
                );
            }
            other => panic!("Expected Page event, got {:?}", other),
        }

        let finished = events.next().unwrap();
        match finished {
            QueryExecEvent::Finished {
                affected_rows,
                error,
                ..
            } => {
                assert!(error.is_none());
                assert_eq!(affected_rows, 0);
            }
            other => panic!("Expected Finished event, got {:?}", other),
        }
        Ok(())
    }
}
