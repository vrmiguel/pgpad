use futures_util::{pin_mut, TryStreamExt};
use tauri::ipc::Channel;
use tokio_postgres::{types::ToSql, Client};

use crate::{
    database::{
        postgres::{parser::parse_statements, row_writer::RowWriter},
        types::QueryStreamEvent,
    },
    Error,
};

pub async fn execute_query(
    client: &Client,
    query: &str,
    channel: &Channel<QueryStreamEvent>,
) -> Result<(), Error> {
    let statements = parse_statements(query)?;
    let total_statements = statements.len();

    for (index, statement) in statements.iter().enumerate() {
        channel
            .send(QueryStreamEvent::StatementStart {
                statement_index: index,
                total_statements,
                statement: statement.statement.clone(),
                returns_values: statement.returns_values,
            })
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;

        if statement.returns_values {
            execute_query_with_results(client, &statement.statement, index, &channel).await?;
        } else {
            execute_modification_query(client, &statement.statement, index, &channel).await?;
        }

        channel
            .send(QueryStreamEvent::StatementFinish {
                statement_index: index,
            })
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
    }

    channel
        .send(QueryStreamEvent::AllFinished {})
        .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;

    Ok(())
}

async fn execute_query_with_results(
    client: &Client,
    query: &str,
    statement_index: usize,
    channel: &Channel<QueryStreamEvent>,
) -> Result<(), Error> {
    let start = std::time::Instant::now();
    log::info!("Starting streaming query: {}", query);

    fn slice_iter<'a>(
        s: &'a [&'a (dyn ToSql + Sync)],
    ) -> impl ExactSizeIterator<Item = &'a dyn ToSql> + 'a {
        s.iter().map(|s| *s as _)
    }

    match client.query_raw(query, slice_iter(&[])).await {
        Ok(stream) => {
            pin_mut!(stream);

            let mut columns_sent = false;
            let mut batch_size = 50;
            let max_batch_size = 500;
            let mut total_rows = 0;

            let mut writer = RowWriter::new();

            loop {
                match stream.try_next().await {
                    Ok(Some(row)) => {
                        // Send column info on first row
                        if !columns_sent {
                            let columns: Vec<String> = row
                                .columns()
                                .iter()
                                .map(|col| col.name().to_string())
                                .collect();

                            channel
                                .send(QueryStreamEvent::ResultStart {
                                    statement_index,
                                    columns,
                                })
                                .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;

                            columns_sent = true;
                        }

                        writer.add_row(&row)?;

                        total_rows += 1;

                        // let s = serde_json::from_str::<&RawValue>("hey").unwrap();
                        // TODO: maybe writer.finish returns RawValue directly?

                        if writer.len() >= batch_size {
                            channel
                                .send(QueryStreamEvent::ResultBatch {
                                    statement_index,
                                    rows: writer.finish(),
                                })
                                .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;

                            writer.clear();
                            batch_size = (batch_size * 2).min(max_batch_size);
                        }
                    }
                    Ok(None) => {
                        // End of stream
                        break;
                    }
                    Err(e) => {
                        log::error!("Error processing row: {}", e);
                        let error_msg = format!("Query failed: {}", e);

                        channel
                            .send(QueryStreamEvent::StatementError {
                                statement_index,
                                statement: query.to_string(),
                                error: error_msg.clone(),
                            })
                            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;

                        return Err(Error::Any(anyhow::anyhow!(error_msg)));
                    }
                }
            }

            if !writer.is_empty() {
                channel
                    .send(QueryStreamEvent::ResultBatch {
                        statement_index,
                        rows: writer.finish(),
                    })
                    .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            }

            let duration = start.elapsed().as_millis() as u64;
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

            channel
                .send(QueryStreamEvent::StatementError {
                    statement_index,
                    statement: query.to_string(),
                    error: error_msg.clone(),
                })
                .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;

            Err(Error::Any(anyhow::anyhow!(error_msg)))
        }
    }
}

async fn execute_modification_query(
    client: &Client,
    query: &str,
    statement_index: usize,
    channel: &Channel<QueryStreamEvent>,
) -> Result<(), Error> {
    log::info!("Executing modification query: {}", query);

    match client.execute(query, &[]).await {
        Ok(rows_affected) => {
            channel
                .send(QueryStreamEvent::StatementComplete {
                    statement_index,
                    affected_rows: rows_affected,
                })
                .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
            Ok(())
        }
        Err(e) => {
            log::error!("Modification query failed: {:?}", e);
            let error_msg = format!("Query failed: {}", e);

            channel
                .send(QueryStreamEvent::StatementError {
                    statement_index,
                    statement: query.to_string(),
                    error: error_msg.clone(),
                })
                .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;

            Err(Error::Any(anyhow::anyhow!(error_msg)))
        }
    }
}
