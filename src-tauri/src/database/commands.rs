use std::collections::{HashMap, HashSet};

use anyhow::Context;
use tauri::ipc::Channel;
use uuid::Uuid;

use crate::{
    database::{
        postgres::{self, connect::connect},
        types::{
            Client, ColumnInfo, ConnectionConfig, ConnectionInfo, DatabaseConnection, DatabaseSchema, QueryStreamEvent, TableInfo
        },
        Certificates, ConnectionMonitor,
    },
    error::Error,
    storage::{QueryHistoryEntry, SavedQuery},
    AppState,
};

#[tauri::command]
pub async fn add_connection(
    config: ConnectionConfig,
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionInfo, Error> {
    let id = Uuid::new_v4();
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
pub async fn update_connection(
    conn_id: Uuid,
    config: ConnectionConfig,
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionInfo, Error> {
    let mut updated_info = ConnectionInfo {
        id: conn_id,
        name: config.name,
        connection_string: config.connection_string,
        connected: false,
    };

    if let Some(mut connection_entry) = state.connections.get_mut(&conn_id) {
        let connection = connection_entry.value_mut();

        // Connstring changed, we'll need to reconnect
        if connection.info.connection_string != updated_info.connection_string {
            connection.client = None;
            connection.info.connected = false;
        }

        updated_info.connected = connection.info.connected;
        connection.info = updated_info.clone();
    }

    state.storage.update_connection(&updated_info)?;

    Ok(updated_info)
}

#[tauri::command]
pub async fn connect_to_database(
    connection_id: Uuid,
    state: tauri::State<'_, AppState>,
    monitor: tauri::State<'_, ConnectionMonitor>,
    certificates: tauri::State<'_, Certificates>,
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

    match connect(&connection.info.connection_string, &certificates).await {
        Ok((client, conn_check)) => {
            connection.client = Some(client.into());
            connection.info.connected = true;

            if let Err(e) = state.storage.update_last_connected(&connection_id) {
                log::warn!("Failed to update last connected timestamp: {}", e);
            }

            monitor.add_connection(connection_id, conn_check).await;

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
    connection_id: Uuid,
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
    connection_id: Uuid,
    query: &str,
    state: tauri::State<'_, AppState>,
    channel: Channel<QueryStreamEvent<'_>>,
) -> Result<(), Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;

    let connection = connection_entry.value();

    let client = connection
        .client
        .as_ref()
        .with_context(|| format!("Connection not found: {}", connection_id))?;

    let Client::Postgres(client) = client else {
        unimplemented!("SQLite");
    };

    postgres::execute::execute_query(client, query, &channel).await?;
    
    Ok(())
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
    connection_id: Uuid,
    state: tauri::State<'_, AppState>,
) -> Result<(), Error> {
    state.storage.remove_connection(&connection_id)?;

    state.connections.remove(&connection_id);

    Ok(())
}

#[tauri::command]
pub async fn test_connection(
    config: ConnectionConfig,
    certificates: tauri::State<'_, Certificates>,
) -> Result<bool, Error> {
    log::info!("Testing connection: {}", config.connection_string);
    match connect(&config.connection_string, &certificates).await {
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
            .insert(connection.info.id, connection);
    }

    log::info!(
        "Initialized {} connections from storage",
        state.connections.len()
    );
    Ok(())
}

#[tauri::command]
pub async fn get_database_schema(
    connection_id: Uuid,
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

    let Client::Postgres(client) = client else {
        unimplemented!("SQLite");
    };

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

    // Key is (schema, table_name)
    let mut tables_map = HashMap::new();
    let mut schemas_set = HashSet::new();
    let mut unique_columns_set = HashSet::new();

    for row in &rows {
        let schema: &str = row.get(0);
        let table_name: &str = row.get(1);
        let column_name: &str = row.get(2);
        let data_type: &str = row.get(3);
        let is_nullable: bool = row.get(4);
        let default_value: Option<&str> = row.get(5);

        schemas_set.insert(schema);
        unique_columns_set.insert(column_name);

        let table_key = (schema, table_name);

        let table_info = tables_map.entry(table_key).or_insert_with(|| TableInfo {
            name: table_name.to_owned(),
            schema: schema.to_owned(),
            columns: Vec::new(),
        });

        table_info.columns.push(ColumnInfo {
            name: column_name.to_owned(),
            data_type: data_type.to_owned(),
            is_nullable,
            default_value: default_value.map(|s| s.to_owned()),
        });
    }

    let tables = tables_map.into_values().collect();
    let schemas = schemas_set.into_iter().map(ToOwned::to_owned).collect();
    let unique_columns = unique_columns_set
        .into_iter()
        .map(ToOwned::to_owned)
        .collect();

    Ok(DatabaseSchema {
        tables,
        schemas,
        unique_columns,
    })
}

// Script management commands
#[tauri::command]
pub async fn save_script(
    name: String,
    content: String,
    connection_id: Option<Uuid>,
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
    connection_id: Option<Uuid>,
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
    connection_id: Option<Uuid>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SavedQuery>, Error> {
    let scripts = state.storage.get_saved_queries(connection_id.as_ref())?;
    Ok(scripts)
}

#[tauri::command]
pub async fn delete_script(id: i64, state: tauri::State<'_, AppState>) -> Result<(), Error> {
    state.storage.delete_saved_query(id)?;
    Ok(())
}

#[tauri::command]
pub async fn save_session_state(
    session_data: &str,
    state: tauri::State<'_, AppState>,
) -> Result<(), Error> {
    state.storage.set_setting("session_state", session_data)?;
    Ok(())
}

#[tauri::command]
pub async fn get_session_state(state: tauri::State<'_, AppState>) -> Result<Option<String>, Error> {
    let session_data = state.storage.get_setting("session_state")?;
    Ok(session_data)
}
