use std::collections::{HashMap, HashSet};

use anyhow::Context;
use futures_util::{pin_mut, TryStreamExt};
use tauri::Emitter;
use tokio_postgres::types::{ToSql, Type};
use uuid::Uuid;

use crate::{
    error::Error,
    postgres::{
        connect::connect,
        types::{
            ColumnInfo, ConnectionConfig, ConnectionInfo, DatabaseConnection, DatabaseSchema,
            QueryStreamData, QueryStreamError, QueryStreamStart, TableInfo,
        },
    },
    storage::{QueryHistoryEntry, SavedQuery},
    AppState,
};

/// Taken from https://github.com/sfackler/rust-postgres/issues/879#issuecomment-1084149480
#[derive(Debug)]
struct NullChecker(bool);

impl tokio_postgres::types::FromSql<'_> for NullChecker {
    fn from_sql(
        _ty: &tokio_postgres::types::Type,
        _raw: &[u8],
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync + 'static>> {
        Ok(Self(false))
    }

    fn from_sql_null(
        _ty: &tokio_postgres::types::Type,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync + 'static>> {
        Ok(Self(true))
    }

    fn accepts(_ty: &tokio_postgres::types::Type) -> bool {
        true
    }
}

/// Move to a separate file
fn postgres_value_to_json(
    row: &tokio_postgres::Row,
    column_index: usize,
) -> Result<serde_json::Value, anyhow::Error> {
    let column = &row.columns()[column_index];
    let pg_type = column.type_();

    // Check for NULL
    if row.try_get::<_, NullChecker>(column_index)?.0 {
        return Ok(serde_json::Value::Null);
    }

    match *pg_type {
        Type::BOOL => {
            let value: bool = row.try_get(column_index)?;
            Ok(serde_json::Value::Bool(value))
        }

        Type::INT2 => {
            let value: i16 = row.try_get(column_index)?;
            Ok(serde_json::Value::Number(value.into()))
        }
        Type::INT4 => {
            let value: i32 = row.try_get(column_index)?;
            Ok(serde_json::Value::Number(value.into()))
        }
        Type::INT8 => {
            let value: i64 = row.try_get(column_index)?;
            Ok(serde_json::Value::Number(value.into()))
        }

        Type::FLOAT4 => {
            let value: f32 = row.try_get(column_index)?;
            if let Some(num) = serde_json::Number::from_f64(value as f64) {
                Ok(serde_json::Value::Number(num))
            } else {
                Ok(serde_json::Value::String(value.to_string()))
            }
        }
        Type::FLOAT8 => {
            let value: f64 = row.try_get(column_index)?;
            if let Some(num) = serde_json::Number::from_f64(value) {
                Ok(serde_json::Value::Number(num))
            } else {
                Ok(serde_json::Value::String(value.to_string()))
            }
        }

        Type::NUMERIC => {
            let value: String = row.try_get(column_index)?;
            if let Ok(parsed) = value.parse::<i64>() {
                Ok(serde_json::Value::Number(parsed.into()))
            } else if let Ok(parsed) = value.parse::<f64>() {
                if let Some(num) = serde_json::Number::from_f64(parsed) {
                    Ok(serde_json::Value::Number(num))
                } else {
                    Ok(serde_json::Value::String(value))
                }
            } else {
                Ok(serde_json::Value::String(value))
            }
        }

        Type::JSON | Type::JSONB => {
            let value: &str = row.try_get(column_index)?;
            match serde_json::from_str(&value) {
                Ok(json_value) => Ok(json_value),
                Err(_) => Ok(serde_json::Value::String(value.to_string())),
            }
        }

        Type::TEXT_ARRAY => {
            let value: Vec<String> = row.try_get(column_index)?;
            Ok(serde_json::Value::Array(
                value.into_iter().map(serde_json::Value::String).collect(),
            ))
        }
        Type::INT4_ARRAY => {
            let value: Vec<i32> = row.try_get(column_index)?;
            Ok(serde_json::Value::Array(
                value
                    .into_iter()
                    .map(|v| serde_json::Value::Number(v.into()))
                    .collect(),
            ))
        }
        Type::INT8_ARRAY => {
            let value: Vec<i64> = row.try_get(column_index)?;
            Ok(serde_json::Value::Array(
                value
                    .into_iter()
                    .map(|v| serde_json::Value::Number(v.into()))
                    .collect(),
            ))
        }

        // Text and string types (default case)
        _ => {
            let value: String = row.try_get(column_index)?;
            Ok(serde_json::Value::String(value))
        }
    }
}

#[tauri::command]
pub async fn add_connection(
    config: ConnectionConfig,
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionInfo, Error> {
    let id = Uuid::new_v4().to_string();
    let info = ConnectionInfo {
        id: id.clone(),
        name: config.name.clone(),
        connection_string: config.connection_string.clone(),
        connected: false,
    };

    state.storage.save_connection(&info)?;

    let connection = DatabaseConnection {
        info: info.clone(),
        client: None,
    };
    state.connections.insert(id, connection);

    Ok(info)
}

#[tauri::command]
pub async fn connect_to_database(
    connection_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<bool, Error> {
    if !state.connections.contains_key(&connection_id) {
        let stored_connections = state.storage.get_connections()?;
        if let Some(stored_connection) = stored_connections.iter().find(|c| c.id == connection_id) {
            let connection = DatabaseConnection {
                info: stored_connection.clone(),
                client: None,
            };
            state.connections.insert(connection_id.clone(), connection);
        }
    }

    let mut connection_entry = state
        .connections
        .get_mut(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;

    let connection = connection_entry.value_mut();

    match connect(&connection.info.connection_string).await {
        Ok(client) => {
            connection.client = Some(client);
            connection.info.connected = true;

            if let Err(e) = state.storage.update_last_connected(&connection_id) {
                log::warn!("Failed to update last connected timestamp: {}", e);
            }

            Ok(true)
        }
        Err(e) => {
            log::error!("Failed to connect: {}", e);
            connection.info.connected = false;
            Ok(false)
        }
    }
}

#[tauri::command]
pub async fn disconnect_from_database(
    connection_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), Error> {
    let mut connection_entry = state
        .connections
        .get_mut(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value_mut();
    connection.client = None;
    connection.info.connected = false;
    Ok(())
}

#[tauri::command]
pub async fn execute_query_stream(
    connection_id: String,
    query: String,
    query_id: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let query_id = query_id.unwrap_or_else(|| Uuid::new_v4().to_string());

    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;

    let connection = connection_entry.value();

    let client = connection
        .client
        .as_ref()
        .with_context(|| format!("Connection not found: {}", connection_id))?;

    let start = std::time::Instant::now();

    log::info!("Starting streaming query: {}", query);

    fn slice_iter<'a>(
        s: &'a [&'a (dyn ToSql + Sync)],
    ) -> impl ExactSizeIterator<Item = &'a dyn ToSql> + 'a {
        s.iter().map(|s| *s as _)
    }

    match client.query_raw(&query, slice_iter(&[])).await {
        Ok(stream) => {
            pin_mut!(stream);

            let mut columns_sent = false;
            let mut batch = Vec::new();
            let mut batch_size = 50;
            let max_batch_size = 150;
            let mut total_rows = 0;

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

                            if let Err(e) = app.emit(
                                "query-stream-start",
                                QueryStreamStart {
                                    query_id: query_id.clone(),
                                    columns: columns.clone(),
                                },
                            ) {
                                log::error!("❌ Failed to emit stream start: {}", e);
                            } else {
                                log::info!(
                                    "✅ Successfully emitted stream start with {} columns",
                                    columns.len()
                                );
                            }
                            columns_sent = true;
                        }

                        // Serialize row data
                        let mut result_row = Vec::new();
                        for i in 0..row.len() {
                            let value = match postgres_value_to_json(&row, i) {
                                Ok(value) => value,
                                Err(e) => {
                                    log::error!("Error deserializing column: {:?}", e);
                                    serde_json::Value::Null
                                }
                            };
                            result_row.push(value);
                        }

                        batch.push(result_row);
                        total_rows += 1;

                        if batch.len() >= batch_size {
                            if let Err(e) = app.emit(
                                "query-stream-data",
                                QueryStreamData {
                                    query_id: query_id.clone(),
                                    rows: batch.clone(),
                                    is_complete: false,
                                },
                            ) {
                                log::error!("❌ Failed to emit stream data: {}", e);
                            } else {
                                log::info!("✅ Successfully emitted batch of {} rows", batch.len());
                            }

                            batch.clear();
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

                        if let Err(emit_err) = app.emit(
                            "query-stream-error",
                            QueryStreamError {
                                query_id: query_id.clone(),
                                error: error_msg.clone(),
                            },
                        ) {
                            log::error!("Failed to emit stream error: {}", emit_err);
                        }

                        return Err(Error::Any(anyhow::anyhow!(error_msg)));
                    }
                }
            }

            // Send any remaining rows in the batch
            if !batch.is_empty() {
                if let Err(e) = app.emit(
                    "query-stream-data",
                    QueryStreamData {
                        query_id: query_id.clone(),
                        rows: batch,
                        is_complete: false,
                    },
                ) {
                    log::error!("Failed to emit final batch: {}", e);
                }
            }

            log::info!("🏁 Emitting stream completion for query_id: {}", query_id);
            if let Err(e) = app.emit(
                "query-stream-data",
                QueryStreamData {
                    query_id: query_id.clone(),
                    rows: Vec::new(),
                    is_complete: true,
                },
            ) {
                log::error!("❌ Failed to emit stream completion: {}", e);
            } else {
                log::info!("✅ Successfully emitted stream completion");
            }

            let duration = start.elapsed().as_millis() as u64;
            log::info!(
                "Streaming query completed: {} rows in {}ms",
                total_rows,
                duration
            );

            Ok(query_id)
        }
        Err(e) => {
            log::error!("Query execution failed: {:?}", e);
            let error_msg = format!("Query failed: {}", e);

            if let Err(emit_err) = app.emit(
                "query-stream-error",
                QueryStreamError {
                    query_id: query_id.clone(),
                    error: error_msg.clone(),
                },
            ) {
                log::error!("Failed to emit stream error: {}", emit_err);
            }

            Err(Error::Any(anyhow::anyhow!(error_msg)))
        }
    }
}

#[tauri::command]
pub async fn get_connections(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ConnectionInfo>, Error> {
    let mut stored_connections = state.storage.get_connections()?;

    for connection in &mut stored_connections {
        if let Some(runtime_connection) = state.connections.get(&connection.id) {
            connection.connected = runtime_connection.info.connected;
        } else {
            connection.connected = false;
        }
    }

    Ok(stored_connections)
}

#[tauri::command]
pub async fn remove_connection(
    connection_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), Error> {
    state.storage.remove_connection(&connection_id)?;

    state.connections.remove(&connection_id);

    Ok(())
}

#[tauri::command]
pub async fn test_connection(config: ConnectionConfig) -> Result<bool, Error> {
    log::info!("Testing connection: {}", config.connection_string);
    match connect(&config.connection_string).await {
        Ok(_) => Ok(true),
        Err(e) => {
            log::error!("Connection test failed: {}", e);
            Ok(false)
        }
    }
}

#[tauri::command]
pub async fn save_query_to_history(
    connection_id: String,
    query: String,
    duration_ms: Option<u64>,
    status: String,
    row_count: u64,
    error_message: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), Error> {
    let entry = QueryHistoryEntry {
        id: 0, // Sqlite will assign,
        connection_id,
        query_text: query,
        executed_at: chrono::Utc::now().timestamp(),
        duration_ms: duration_ms.map(|d| d as i64),
        status,
        row_count: row_count as i64,
        error_message,
    };

    state.storage.save_query_history(&entry)?;
    Ok(())
}

#[tauri::command]
pub async fn get_query_history(
    connection_id: String,
    limit: Option<u32>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<QueryHistoryEntry>, Error> {
    Ok(state
        .storage
        .get_query_history(&connection_id, limit.map(|l| l as i64))?)
}

#[tauri::command]
pub async fn initialize_connections(state: tauri::State<'_, AppState>) -> Result<(), Error> {
    let stored_connections = state.storage.get_connections()?;

    for stored_connection in stored_connections {
        let connection = DatabaseConnection {
            info: stored_connection,
            client: None,
        };
        state
            .connections
            .insert(connection.info.id.clone(), connection);
    }

    log::info!(
        "Initialized {} connections from storage",
        state.connections.len()
    );
    Ok(())
}

#[tauri::command]
pub async fn get_database_schema(
    connection_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<DatabaseSchema, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;

    let connection = connection_entry.value();

    let client = connection
        .client
        .as_ref()
        .with_context(|| format!("Connection not active: {}", connection_id))?;

    let schema_query = r#"
        SELECT 
            t.table_schema,
            t.table_name,
            c.column_name,
            c.data_type,
            c.is_nullable::boolean,
            c.column_default
        FROM 
            information_schema.tables t
        JOIN 
            information_schema.columns c 
            ON t.table_name = c.table_name 
            AND t.table_schema = c.table_schema
        WHERE 
            t.table_type = 'BASE TABLE'
            AND t.table_schema NOT IN ('information_schema', 'pg_catalog', 'pg_toast')
        ORDER BY 
            t.table_schema, t.table_name, c.ordinal_position
    "#;

    let rows = client
        .query(schema_query, &[])
        .await
        .context("Failed to query database schema")?;

    let mut tables_map: HashMap<String, TableInfo> = std::collections::HashMap::new();
    let mut schemas_set: HashSet<String> = std::collections::HashSet::new();

    for row in rows {
        let schema: String = row.get(0);
        let table_name: String = row.get(1);
        let column_name: String = row.get(2);
        let data_type: String = row.get(3);
        let is_nullable: bool = row.get(4);
        let default_value: Option<String> = row.get(5);

        schemas_set.insert(schema.clone());

        let table_key = format!("{}.{}", schema, table_name);

        let table_info = tables_map
            .entry(table_key.clone())
            .or_insert_with(|| TableInfo {
                name: table_name.clone(),
                schema: schema.clone(),
                columns: Vec::new(),
            });

        table_info.columns.push(ColumnInfo {
            name: column_name,
            data_type,
            is_nullable,
            default_value,
        });
    }

    let tables: Vec<TableInfo> = tables_map.into_values().collect();
    let schemas: Vec<String> = schemas_set.into_iter().collect();

    Ok(DatabaseSchema { tables, schemas })
}

// Script management commands
#[tauri::command]
pub async fn save_script(
    name: String,
    content: String,
    connection_id: Option<String>,
    description: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<i64, Error> {
    let script = SavedQuery {
        id: 0, // New script
        name,
        description,
        query_text: content,
        connection_id,
        tags: None,
        created_at: 0, // Will be set by storage
        updated_at: 0, // Will be set by storage
        favorite: false,
    };

    let script_id = state.storage.save_query(&script)?;
    Ok(script_id)
}

#[tauri::command]
pub async fn update_script(
    id: i64,
    name: String,
    content: String,
    connection_id: Option<String>,
    description: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), Error> {
    let script = SavedQuery {
        id,
        name,
        description,
        query_text: content,
        connection_id,
        tags: None,
        created_at: 0, // Will be ignored for updates
        updated_at: 0, // Will be set by storage
        favorite: false,
    };

    state.storage.save_query(&script)?;
    Ok(())
}

#[tauri::command]
pub async fn get_scripts(
    connection_id: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SavedQuery>, Error> {
    let scripts = state.storage.get_saved_queries(connection_id.as_deref())?;
    Ok(scripts)
}

#[tauri::command]
pub async fn delete_script(id: i64, state: tauri::State<'_, AppState>) -> Result<(), Error> {
    state.storage.delete_saved_query(id)?;
    Ok(())
}
