use std::sync::Arc;

use pgpad_core::{
    database::{
        services as core,
        types::{
            ConnectionConfig, ConnectionInfo, DatabaseSchema, Permissions, QuerySnapshot,
            QueryStatus,
        },
        Certificates, ConnectionMonitor,
    },
    storage::{QueryHistoryEntry, SavedQuery},
    AppState,
};
use serde_json::value::RawValue;
use uuid::Uuid;

use crate::error::Result;

#[tauri::command]
pub async fn add_connection(
    name: String,
    config: ConnectionConfig,
    permissions: Permissions,
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionInfo> {
    Ok(core::add_connection(name, config, permissions, &state).await?)
}

#[tauri::command]
pub async fn update_connection(
    conn_id: Uuid,
    name: String,
    config: ConnectionConfig,
    permissions: Permissions,
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionInfo> {
    Ok(core::update_connection(conn_id, name, config, permissions, &state).await?)
}

#[tauri::command]
pub async fn connect_to_database(
    connection_id: Uuid,
    state: tauri::State<'_, AppState>,
    monitor: tauri::State<'_, ConnectionMonitor>,
    certificates: tauri::State<'_, Certificates>,
) -> Result<bool> {
    Ok(core::connect_to_database(connection_id, &state, &monitor, &certificates).await?)
}

#[tauri::command]
pub async fn disconnect_from_database(
    connection_id: Uuid,
    state: tauri::State<'_, AppState>,
) -> Result {
    Ok(core::disconnect_from_database(connection_id, &state).await?)
}

#[tauri::command]
pub async fn submit_query(
    connection_id: Uuid,
    query: &str,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<usize>> {
    Ok(core::submit_query(connection_id, query, &state).await?)
}

#[tauri::command]
pub async fn wait_until_renderable(
    query_id: usize,
    state: tauri::State<'_, AppState>,
) -> Result<QuerySnapshot> {
    Ok(core::wait_until_renderable(query_id, &state).await?)
}

#[tauri::command]
pub async fn fetch_page(
    query_id: usize,
    page_index: usize,
    state: tauri::State<'_, AppState>,
) -> Result<Option<Box<RawValue>>> {
    Ok(core::fetch_page(query_id, page_index, &state).await?)
}

#[tauri::command]
pub async fn get_query_status(
    query_id: usize,
    state: tauri::State<'_, AppState>,
) -> Result<QueryStatus> {
    Ok(core::get_query_status(query_id, &state).await?)
}

#[tauri::command]
pub async fn get_page_count(query_id: usize, state: tauri::State<'_, AppState>) -> Result<usize> {
    Ok(core::get_page_count(query_id, &state).await?)
}

#[tauri::command]
pub async fn get_connections(state: tauri::State<'_, AppState>) -> Result<Vec<ConnectionInfo>> {
    Ok(core::get_connections(&state).await?)
}

#[tauri::command]
pub async fn remove_connection(connection_id: Uuid, state: tauri::State<'_, AppState>) -> Result {
    Ok(core::remove_connection(connection_id, &state).await?)
}

#[tauri::command]
pub async fn test_connection(
    config: ConnectionConfig,
    certificates: tauri::State<'_, Certificates>,
) -> Result<bool> {
    Ok(core::test_connection(config, &certificates).await?)
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
) -> Result {
    Ok(core::save_query_to_history(
        connection_id,
        query,
        duration_ms,
        status,
        row_count,
        error_message,
        &state,
    )
    .await?)
}

#[tauri::command]
pub async fn get_query_history(
    connection_id: String,
    limit: Option<u32>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<QueryHistoryEntry>> {
    Ok(core::get_query_history(connection_id, limit, &state).await?)
}

#[tauri::command]
pub async fn initialize_connections(state: tauri::State<'_, AppState>) -> Result {
    Ok(core::initialize_connections(&state).await?)
}

#[tauri::command]
pub async fn format_sql(query: &str) -> Result<String> {
    Ok(core::format_sql(query).await?)
}

#[tauri::command]
pub async fn is_query_read_only(
    connection_id: Uuid,
    query: &str,
    state: tauri::State<'_, AppState>,
) -> Result<bool> {
    Ok(core::is_query_read_only(connection_id, query, &state).await?)
}

#[tauri::command]
pub async fn get_database_schema(
    connection_id: Uuid,
    state: tauri::State<'_, AppState>,
) -> Result<Arc<DatabaseSchema>> {
    Ok(core::get_database_schema(connection_id, &state).await?)
}

#[tauri::command]
pub async fn save_script(
    name: String,
    content: String,
    connection_id: Option<Uuid>,
    description: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<i64> {
    Ok(core::save_script(name, content, connection_id, description, &state).await?)
}

#[tauri::command]
pub async fn update_script(
    id: i64,
    name: String,
    content: String,
    connection_id: Option<Uuid>,
    description: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result {
    Ok(core::update_script(id, name, content, connection_id, description, &state).await?)
}

#[tauri::command]
pub async fn get_scripts(
    connection_id: Option<Uuid>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SavedQuery>> {
    Ok(core::get_scripts(connection_id, &state).await?)
}

#[tauri::command]
pub async fn delete_script(id: i64, state: tauri::State<'_, AppState>) -> Result {
    Ok(core::delete_script(id, &state).await?)
}

#[tauri::command]
pub async fn save_session_state(session_data: &str, state: tauri::State<'_, AppState>) -> Result {
    Ok(core::save_session_state(session_data, &state).await?)
}

#[tauri::command]
pub async fn get_session_state(state: tauri::State<'_, AppState>) -> Result<Option<String>> {
    Ok(core::get_session_state(&state).await?)
}

#[tauri::command]
pub async fn export_page(
    query_id: usize,
    page_index: usize,
    state: tauri::State<'_, AppState>,
) -> Result<String> {
    Ok(core::export_page(query_id, page_index, &state).await?)
}
