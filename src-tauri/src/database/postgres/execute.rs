use futures_util::{pin_mut, TryStreamExt};
use tokio_postgres::{types::ToSql, Client};
use serde_json;
use tauri::Emitter;
use uuid::Uuid;

use crate::{
    database::{
        postgres::{parser::parse_statements, row_writer::RowWriter},
        types::QueryStreamEvent,
    },
    utils::serialize_as_json_array,
    Error,
};

pub async fn execute_query(
    client: &Client,
    query: &str,
    session_id: Uuid,
    app_handle: &tauri::AppHandle,
) -> Result<(), Error> {
    let statements = parse_statements(query)?;
    let total_statements = statements.len();

    for (index, statement) in statements.iter().enumerate() {
        emit_event(app_handle, session_id, QueryStreamEvent::StatementStart {
            statement_index: index,
            total_statements,
            statement: statement.statement.clone(),
            returns_values: statement.returns_values,
        })?;

        if statement.returns_values {
            execute_query_with_results(client, &statement.statement, index, session_id, app_handle).await?;
        } else {
            execute_modification_query(client, &statement.statement, index, session_id, app_handle).await?;
        }

        emit_event(app_handle, session_id, QueryStreamEvent::StatementFinish {
            statement_index: index,
        })?;
    }

    emit_event(app_handle, session_id, QueryStreamEvent::AllFinished {})?;

    Ok(())
}

fn emit_event(
    app_handle: &tauri::AppHandle,
    session_id: Uuid,
    event: QueryStreamEvent,
) -> Result<(), Error> {
    let payload = serde_json::json!({
        "session_id": session_id,
        "event": event
    });
    
    app_handle
        .emit("query-stream-event", payload)
        .map_err(|e| Error::Any(anyhow::anyhow!("Failed to emit event: {}", e)))?;
    
    Ok(())
}

async fn execute_query_with_results(
    client: &Client,
    query: &str,
    statement_index: usize,
    session_id: Uuid,
    app_handle: &tauri::AppHandle,
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
                            let columns = row.columns().iter().map(|col| col.name());
                            let columns = serialize_as_json_array(columns)?;

                            emit_event(app_handle, session_id, QueryStreamEvent::ResultStart {
                                statement_index,
                                columns,
                            })?;

                            columns_sent = true;
                        }

                        writer.add_row(&row)?;

                        total_rows += 1;

                        // let s = serde_json::from_str::<&RawValue>("hey").unwrap();
                        // TODO: maybe writer.finish returns RawValue directly?

                        if writer.len() >= batch_size {
                            emit_event(app_handle, session_id, QueryStreamEvent::ResultBatch {
                                statement_index,
                                rows: writer.finish(),
                            })?;

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

                        emit_event(app_handle, session_id, QueryStreamEvent::StatementError {
                            statement_index,
                            statement: query.to_string(),
                            error: error_msg.clone(),
                        })?;

                        return Err(Error::Any(anyhow::anyhow!(error_msg)));
                    }
                }
            }

            if !writer.is_empty() {
                emit_event(app_handle, session_id, QueryStreamEvent::ResultBatch {
                    statement_index,
                    rows: writer.finish(),
                })?;
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

            emit_event(app_handle, session_id, QueryStreamEvent::StatementError {
                statement_index,
                statement: query.to_string(),
                error: error_msg.clone(),
            })?;

            Err(Error::Any(anyhow::anyhow!(error_msg)))
        }
    }
}

async fn execute_modification_query(
    client: &Client,
    query: &str,
    statement_index: usize,
    session_id: Uuid,
    app_handle: &tauri::AppHandle,
) -> Result<(), Error> {
    log::info!("Executing modification query: {}", query);

    match client.execute(query, &[]).await {
        Ok(rows_affected) => {
            emit_event(app_handle, session_id, QueryStreamEvent::StatementComplete {
                statement_index,
                affected_rows: rows_affected,
            })?;
            Ok(())
        }
        Err(e) => {
            log::error!("Modification query failed: {:?}", e);
            let error_msg = format!("Query failed: {}", e);

            emit_event(app_handle, session_id, QueryStreamEvent::StatementError {
                statement_index,
                statement: query.to_string(),
                error: error_msg.clone(),
            })?;

            Err(Error::Any(anyhow::anyhow!(error_msg)))
        }
    }
}
