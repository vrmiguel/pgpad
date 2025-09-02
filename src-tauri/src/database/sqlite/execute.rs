use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tauri::{async_runtime::spawn_blocking, Emitter};
use serde_json;
use uuid::Uuid;

use crate::{
    database::{
        sqlite::{parser::parse_statements, row_writer::RowWriter},
        types::QueryStreamEvent,
    },
    utils::serialize_as_json_array,
    Error,
};

pub async fn execute_query(
    client: Arc<Mutex<Connection>>,
    query: &str,
    session_id: Uuid,
    app_handle: &tauri::AppHandle,
) -> Result<(), Error> {
    let statements = parse_statements(query)?;
    let session_id = session_id.to_string();
    let app_handle = app_handle.clone();

    let handle = spawn_blocking(move || {
        let total_statements = statements.len();
        let client = client.lock().unwrap();

        for (index, statement) in statements.iter().enumerate() {
            emit_event(&app_handle, &session_id, QueryStreamEvent::StatementStart {
                statement_index: index,
                total_statements,
                statement: statement.statement.clone(),
                returns_values: statement.returns_values,
            })?;

            if statement.returns_values {
                execute_query_with_results(&client, &statement.statement, index, &session_id, &app_handle)?;
            } else {
                execute_modification_query(&client, &statement.statement, index, &session_id, &app_handle)?;
            }

            emit_event(&app_handle, &session_id, QueryStreamEvent::StatementFinish {
                statement_index: index,
            })?;
        }

        emit_event(&app_handle, &session_id, QueryStreamEvent::AllFinished {})?;

        Ok(()) as Result<(), Error>
    });

    handle.await??;

    Ok(())
}

fn emit_event(
    app_handle: &tauri::AppHandle,
    session_id: &str,
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

fn execute_query_with_results(
    client: &Connection,
    query: &str,
    statement_index: usize,
    session_id: &str,
    app_handle: &tauri::AppHandle,
) -> Result<(), Error> {
    let start = std::time::Instant::now();
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
                    emit_event(app_handle, session_id, QueryStreamEvent::ResultStart {
                        statement_index,
                        columns,
                    })?;

                    let mut total_rows = 0;
                    let mut batch_size = 50;
                    let max_batch_size = 500;
                    let mut writer = RowWriter::new(column_types);

                    loop {
                        match rows.next() {
                            Ok(Some(row)) => {
                                writer.add_row(row)?;
                                total_rows += 1;

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
                                // End of results
                                break;
                            }
                            Err(e) => {
                                log::error!("Error processing SQLite row: {}", e);
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
                        "SQLite query completed: {} rows in {}ms",
                        total_rows,
                        duration
                    );

                    Ok(())
                }
                Err(e) => {
                    log::error!("SQLite query execution failed: {:?}", e);
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
        Err(e) => {
            log::error!("SQLite statement preparation failed: {:?}", e);
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

fn execute_modification_query(
    client: &Connection,
    query: &str,
    statement_index: usize,
    session_id: &str,
    app_handle: &tauri::AppHandle,
) -> Result<(), Error> {
    log::info!("Executing modification query: {}", query);

    match client.execute(query, []) {
        Ok(rows_affected) => {
            emit_event(app_handle, session_id, QueryStreamEvent::StatementComplete {
                statement_index,
                affected_rows: rows_affected as u64,
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
