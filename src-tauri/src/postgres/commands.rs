use anyhow::Context;
use uuid::Uuid;

use crate::{
    error::Error,
    postgres::{
        connect::connect,
        types::{ConnectionConfig, ConnectionInfo, DatabaseConnection, QueryResult},
    },
    AppState,
};

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
    let mut connection_entry = state
        .connections
        .get_mut(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;

    let connection = connection_entry.value_mut();

    match connect(&connection.info.connection_string).await {
        Ok(client) => {
            connection.client = Some(client);
            connection.info.connected = true;
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
pub async fn execute_query(
    connection_id: String,
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<QueryResult, Error> {
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

    match client.query(&query, &[]).await {
        Ok(rows) => {
            let duration = start.elapsed().as_millis() as u64;

            let columns = if rows.is_empty() {
                Vec::new()
            } else {
                rows[0]
                    .columns()
                    .iter()
                    .map(|col| col.name().to_string())
                    .collect()
            };

            let mut result_rows = Vec::new();
            for row in rows.iter() {
                let mut result_row = Vec::new();
                for i in 0..row.len() {
                    let value: Option<String> = row.try_get(i).unwrap_or(None);
                    result_row.push(
                        value
                            .map(serde_json::Value::String)
                            .unwrap_or(serde_json::Value::Null),
                    );
                }
                result_rows.push(result_row);
            }

            Ok(QueryResult {
                columns,
                row_count: result_rows.len(),
                rows: result_rows,
                duration_ms: duration,
            })
        }
        Err(e) => {
            log::error!("Query execution failed: {}", e);
            Err(Error::Any(anyhow::anyhow!("Query failed: {}", e)))
        }
    }
}

#[tauri::command]
pub async fn get_connections(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ConnectionInfo>, Error> {
    let result: Vec<ConnectionInfo> = state
        .connections
        .iter()
        .map(|entry| entry.value().info.clone())
        .collect();
    Ok(result)
}

#[tauri::command]
pub async fn remove_connection(
    connection_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), Error> {
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
