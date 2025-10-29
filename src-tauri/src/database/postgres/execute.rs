use futures_util::{pin_mut, TryStreamExt};
use tokio_postgres::{types::ToSql, Client};

use crate::{
    database::{
        parser::ParsedStatement, postgres::row_writer::RowWriter, types::ExecSender, QueryExecEvent,
    },
    utils::serialize_as_json_array,
    Error,
};

pub async fn execute_query(
    client: &Client,
    stmt: ParsedStatement,
    sender: &ExecSender,
) -> Result<(), Error> {
    if stmt.returns_values {
        execute_query_with_results(client, &stmt.statement, sender).await?;
    } else {
        execute_modification_query(client, &stmt.statement, sender).await?;
    }

    Ok(())
}

async fn execute_query_with_results(
    client: &Client,
    query: &str,
    sender: &ExecSender,
) -> Result<(), Error> {
    let started_at = std::time::Instant::now();
    log::info!("Starting streaming query: {}", query);

    fn slice_iter<'a>(
        s: &'a [&'a (dyn ToSql + Sync)],
    ) -> impl ExactSizeIterator<Item = &'a dyn ToSql> + 'a {
        s.iter().map(|s| *s as _)
    }

    let prepared_stmt = match client.prepare(query).await {
        Ok(stmt) => stmt,
        Err(e) => {
            log::error!("Query preparation failed: {:?}", e);
            let error_msg = format!("Query failed: {}", e);

            sender.send(QueryExecEvent::Finished {
                elapsed_ms: started_at.elapsed().as_millis() as u64,
                affected_rows: 0,
                error: Some(error_msg.clone()),
            })?;

            return Err(Error::Any(anyhow::anyhow!(error_msg)));
        }
    };

    let columns = prepared_stmt.columns().iter().map(|col| col.name());
    let columns = serialize_as_json_array(columns)?;

    sender.send(QueryExecEvent::TypesResolved { columns })?;

    match client.query_raw(&prepared_stmt, slice_iter(&[])).await {
        Ok(stream) => {
            pin_mut!(stream);

            let batch_size = 50;
            let mut total_rows = 0;

            let mut writer = RowWriter::new();

            loop {
                match stream.try_next().await {
                    Ok(Some(row)) => {
                        writer.add_row(&row)?;

                        total_rows += 1;

                        if writer.len() >= batch_size {
                            sender.send(QueryExecEvent::Page {
                                page_amount: writer.len(),
                                page: writer.finish(),
                            })?;
                        }
                    }
                    Ok(None) => {
                        // End of stream
                        break;
                    }
                    Err(e) => {
                        log::error!("Error processing row: {}", e);
                        let error_msg = format!("Query failed: {}", e);

                        sender.send(QueryExecEvent::Finished {
                            elapsed_ms: started_at.elapsed().as_millis() as u64,
                            affected_rows: 0,
                            error: Some(error_msg.clone()),
                        })?;

                        return Err(Error::Any(anyhow::anyhow!(error_msg)));
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

            sender.send(QueryExecEvent::Finished {
                elapsed_ms: started_at.elapsed().as_millis() as u64,
                affected_rows: 0,
                error: None,
            })?;

            log::info!(
                "Streaming query completed: {} rows in {}ms",
                total_rows,
                duration
            );

            Ok(())
        }
        Err(e) => {
            log::error!("Query execution failed: {:?}", e);
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

async fn execute_modification_query(
    client: &Client,
    query: &str,
    sender: &ExecSender,
) -> Result<(), Error> {
    log::info!("Executing modification query: {}", query);
    let started_at = std::time::Instant::now();

    match client.execute(query, &[]).await {
        Ok(rows_affected) => {
            sender.send(QueryExecEvent::Finished {
                elapsed_ms: started_at.elapsed().as_millis() as u64,
                affected_rows: rows_affected as usize,
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
    use std::{collections::HashSet, ops::Not, sync::Arc};

    use pgtemp::PgTempDB;

    use super::execute_query;
    use crate::database::{postgres::parser::parse_statements, types::channel, QueryExecEvent};

    async fn run_query(
        conn: Arc<tokio_postgres::Client>,
        query: &str,
    ) -> anyhow::Result<Vec<QueryExecEvent>> {
        let mut parsed_stmt = parse_statements(query).unwrap();
        assert_eq!(parsed_stmt.len(), 1);
        assert!(parsed_stmt[0].returns_values);
        let stmt = parsed_stmt.pop().unwrap();

        let (sender, mut recv) = channel();

        tokio::task::spawn(async move {
            execute_query(&conn, stmt, &sender).await.unwrap();
        });

        let mut events = Vec::new();

        while let Some(event) = recv.recv().await {
            events.push(event);
        }

        Ok(events)
    }

    /// Run a query that returns no results (modification-only?), returning the number of rows affected.
    async fn run_modification_query(
        conn: Arc<tokio_postgres::Client>,
        query: &str,
    ) -> anyhow::Result<usize> {
        let mut parsed_stmt = parse_statements(query).unwrap();
        assert_eq!(parsed_stmt.len(), 1);
        assert!(parsed_stmt[0].returns_values.not());
        let stmt = parsed_stmt.pop().unwrap();

        let (sender, mut recv) = channel();

        tokio::task::spawn(async move {
            execute_query(&conn, stmt, &sender).await.unwrap();
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
    async fn test_queries() -> anyhow::Result<()> {
        let db = PgTempDB::async_new().await;

        let (client, conn) = tokio_postgres::connect(&db.connection_uri(), tokio_postgres::NoTls)
            .await
            .unwrap();

        tokio::task::spawn(async move {
            if let Err(e) = conn.await {
                eprintln!("Connection error: {}", e);
            }
        });

        let client = Arc::new(client);
        let affected_rows = run_modification_query(
            client.clone(),
            "CREATE TABLE users (id INTEGER GENERATED ALWAYS AS IDENTITY PRIMARY KEY, name TEXT, age INTEGER);",
        )
        .await?;
        assert_eq!(affected_rows, 0);

        let query =
            "INSERT INTO users (name, age) VALUES ('Alice', 25), ('Bob', 30),('Charlie', 35), ('Diana', 28);";
        let affected_rows = run_modification_query(client.clone(), query).await?;
        assert_eq!(affected_rows, 4);

        let query = "UPDATE users SET age = age + 1 WHERE name = 'Alice';";
        let affected_rows = run_modification_query(client.clone(), query).await?;
        assert_eq!(affected_rows, 1);

        let query = "DELETE FROM users WHERE name = 'Bob';";
        let affected_rows = run_modification_query(client.clone(), query).await?;
        assert_eq!(affected_rows, 1);

        let query = "DELETE FROM users WHERE name = 'Joe';";
        let affected_rows = run_modification_query(client.clone(), query).await?;
        assert_eq!(affected_rows, 0);

        let query = "SELECT * FROM users";
        let mut events = run_query(client.clone(), query).await?.into_iter();
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
                    {
                        let val = serde_json::to_value(&page).unwrap();
                        let serde_json::Value::Array(val) = val else {
                            panic!("Expected array");
                        };
                        val.into_iter().collect::<HashSet<_>>()
                    },
                    {
                        HashSet::from([
                            serde_json::json!([1, "Alice", 26]),
                            serde_json::json!([3, "Charlie", 35]),
                            serde_json::json!([4, "Diana", 28]),
                        ])
                    }
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
                let _ = elapsed_ms;
                assert!(error.is_none());
                assert_eq!(affected_rows, 0);
            }
            other => panic!("Expected Finished event, got {:?}", other),
        }
        Ok(())
    }
}
