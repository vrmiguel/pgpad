use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use anyhow::Context;
use serde_json::value::RawValue;
use uuid::Uuid;

use crate::{
    credentials,
    database::{
        self,
        postgres::{self, connect::connect},
        sqlite,
        types::{
            Connection, ConnectionConfig, ConnectionInfo, ConnectionRuntime, Database,
            DatabaseSchema, QuerySnapshot, QueryStatus, RuntimeClient,
        },
        Certificates, ConnectionMonitor,
    },
    error::Error,
    storage::{QueryHistoryEntry, SavedQuery},
    AppState,
};

pub async fn add_connection(
    name: String,
    config: ConnectionConfig,
    permissions: crate::database::types::Permissions,
    state: &AppState,
) -> Result<ConnectionInfo, Error> {
    let id = Uuid::new_v4();

    let (config, password) = credentials::extract_sensitive_data(config)?;

    // It's expected that add_connection receives config with the password included,
    // as checked by the form in the UI. This call saves it in the keyring.
    if let Some(password) = password {
        credentials::store_sensitive_data(&id, &password)?;
    }

    let connection = Connection::new(id, name, config, permissions);
    let info = connection.to_connection_info();

    state.storage.save_connection(&info)?;
    state.connections.insert(id, connection);

    Ok(info)
}

pub async fn update_connection(
    conn_id: Uuid,
    name: String,
    config: ConnectionConfig,
    permissions: crate::database::types::Permissions,
    state: &AppState,
) -> Result<ConnectionInfo, Error> {
    let (config, password) = credentials::extract_sensitive_data(config)?;
    if let Some(password) = password {
        credentials::store_sensitive_data(&conn_id, &password)?;
    }

    if let Some(mut connection_entry) = state.connections.get_mut(&conn_id) {
        let connection = connection_entry.value_mut();

        let config_changed = match (&connection.config, &config) {
            (
                ConnectionConfig::Postgres {
                    connection_string: old,
                    ca_cert_path: old_cert,
                },
                ConnectionConfig::Postgres {
                    connection_string: new,
                    ca_cert_path: new_cert,
                },
            ) => old != new || old_cert != new_cert,
            (
                ConnectionConfig::SQLite { db_path: old },
                ConnectionConfig::SQLite { db_path: new },
            ) => old != new,
            _ => true,
        };

        if config_changed {
            connection.runtime = ConnectionRuntime::Disconnected;
        }

        connection.name = name;
        connection.permissions = permissions;
        connection.config = config;
    }

    let updated_info = state
        .connections
        .get(&conn_id)
        .map(|conn| conn.to_connection_info())
        .with_context(|| format!("Connection not found: {}", conn_id))?;

    state.storage.update_connection(&updated_info)?;

    Ok(updated_info)
}

pub async fn connect_to_database(
    connection_id: Uuid,
    state: &AppState,
    monitor: &ConnectionMonitor,
    certificates: &Certificates,
) -> Result<bool, Error> {
    if !state.connections.contains_key(&connection_id) {
        let stored_connections = state.storage.get_connections()?;
        if let Some(stored_connection) = stored_connections.iter().find(|c| c.id == connection_id) {
            let connection = Connection::new(
                stored_connection.id,
                stored_connection.name.clone(),
                stored_connection.config.clone(),
                stored_connection.permissions,
            );
            state.connections.insert(connection_id, connection);
        }
    }

    let mut connection_entry = state
        .connections
        .get_mut(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;

    let connection = connection_entry.value_mut();

    match &connection.config {
        ConnectionConfig::Postgres {
            connection_string,
            ca_cert_path,
        } => {
            let mut config: tokio_postgres::Config =
                connection_string.parse().with_context(|| {
                    format!("Failed to parse connection string: {}", connection_string)
                })?;
            if config.get_password().is_none() {
                credentials::get_password(&connection_id)?.map(|pw| config.password(pw));
            }

            match connect(
                &config,
                certificates,
                ca_cert_path.as_deref(),
                monitor.notifier(connection_id),
            )
            .await
            {
                Ok(pg_client) => {
                    connection.runtime = ConnectionRuntime::Connected(RuntimeClient::Postgres {
                        client: Arc::new(pg_client),
                    });

                    if let Err(e) = state.storage.update_last_connected(&connection_id) {
                        log::warn!("Failed to update last connected timestamp: {}", e);
                    }

                    Ok(true)
                }
                Err(e) => {
                    log::error!("Failed to connect to Postgres: {}", e);
                    connection.runtime = ConnectionRuntime::Disconnected;
                    Ok(false)
                }
            }
        }
        ConnectionConfig::SQLite { db_path } => match rusqlite::Connection::open(db_path) {
            Ok(conn) => {
                // Set busy timeout so concurrent writers wait instead of
                // immediately returning SQLITE_BUSY.
                if let Err(e) = conn.execute_batch("PRAGMA busy_timeout = 5000;") {
                    log::error!(
                        "Failed to set busy_timeout on SQLite database {}: {}",
                        db_path,
                        e
                    );
                }

                connection.runtime = ConnectionRuntime::Connected(RuntimeClient::SQLite {
                    connection: Arc::new(Mutex::new(conn)),
                });

                if let Err(e) = state.storage.update_last_connected(&connection_id) {
                    log::warn!("Failed to update last connected timestamp: {}", e);
                }

                log::info!("Successfully connected to SQLite database: {}", db_path);
                Ok(true)
            }
            Err(e) => {
                log::error!("Failed to connect to SQLite database {}: {}", db_path, e);
                connection.runtime = ConnectionRuntime::Disconnected;
                Ok(false)
            }
        },
    }
}

pub async fn disconnect_from_database(connection_id: Uuid, state: &AppState) -> Result<(), Error> {
    let mut connection_entry = state
        .connections
        .get_mut(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value_mut();

    connection.runtime = ConnectionRuntime::Disconnected;
    Ok(())
}

pub async fn submit_query(
    connection_id: Uuid,
    query: &str,
    state: &AppState,
) -> Result<Vec<usize>, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;

    let connection = connection_entry.value();

    let client = connection.get_client()?;
    let query_ids = state.stmt_manager.submit_query(client, query)?;

    Ok(query_ids)
}

pub async fn wait_until_renderable(
    query_id: usize,
    state: &AppState,
) -> Result<QuerySnapshot, Error> {
    let now = Instant::now();

    let info = state
        .stmt_manager
        .fetch_initial_renderable_state(query_id)
        .await?;
    let elapsed = now.elapsed();
    log::info!("wait_until_renderable took {}ms", elapsed.as_millis());
    Ok(info)
}

pub async fn fetch_page(
    query_id: usize,
    page_index: usize,
    state: &AppState,
) -> Result<Option<Box<RawValue>>, Error> {
    let now = Instant::now();
    let page = state.stmt_manager.fetch_page(query_id, page_index)?;
    let elapsed = now.elapsed();
    log::info!("Took {}us to get page {page_index}", elapsed.as_micros());

    Ok(page)
}

pub async fn get_query_status(query_id: usize, state: &AppState) -> Result<QueryStatus, Error> {
    state.stmt_manager.get_query_status(query_id)
}

pub async fn get_page_count(query_id: usize, state: &AppState) -> Result<usize, Error> {
    state.stmt_manager.get_page_count(query_id)
}

pub async fn get_connections(state: &AppState) -> Result<Vec<ConnectionInfo>, Error> {
    let mut stored_connections = state.storage.get_connections()?;

    for connection in &mut stored_connections {
        if let Some(runtime_connection) = state.connections.get(&connection.id) {
            connection.connected = runtime_connection.is_client_connected();
        } else {
            connection.connected = false;
        }
    }

    Ok(stored_connections)
}

pub async fn remove_connection(connection_id: Uuid, state: &AppState) -> Result<(), Error> {
    if let Err(e) = credentials::delete_password(&connection_id) {
        log::debug!(
            "Could not delete password from keyring (may not exist): {}",
            e
        );
    }

    state.storage.remove_connection(&connection_id)?;
    state.connections.remove(&connection_id);

    Ok(())
}

pub async fn test_connection(
    // It's expected that test_connection receives config with the password included
    config: ConnectionConfig,
    certificates: &Certificates,
) -> Result<bool, Error> {
    match config {
        ConnectionConfig::Postgres {
            connection_string,
            ca_cert_path,
        } => {
            let config: tokio_postgres::Config = connection_string.parse().with_context(|| {
                format!("Failed to parse connection string: {}", connection_string)
            })?;
            log::info!("Testing Postgres connection: {config:?}");
            match postgres::connect::test_connection(&config, certificates, ca_cert_path.as_deref())
                .await
            {
                Ok(()) => Ok(true),
                Err(e) => {
                    log::error!("Postgres connection test failed: {}", e);
                    Ok(false)
                }
            }
        }
        ConnectionConfig::SQLite { db_path } => match rusqlite::Connection::open(db_path) {
            Ok(_) => Ok(true),
            Err(e) => {
                log::error!("SQLite connection test failed: {}", e);
                Ok(false)
            }
        },
    }
}

pub async fn save_query_to_history(
    connection_id: String,
    query: String,
    duration_ms: Option<u64>,
    status: String,
    row_count: u64,
    error_message: Option<String>,
    state: &AppState,
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

pub async fn get_query_history(
    connection_id: String,
    limit: Option<u32>,
    state: &AppState,
) -> Result<Vec<QueryHistoryEntry>, Error> {
    state
        .storage
        .get_query_history(&connection_id, limit.map(|l| l as i64))
}

pub async fn initialize_connections(state: &AppState) -> Result<(), Error> {
    let stored_connections = state.storage.get_connections()?;

    for stored_connection in stored_connections {
        let connection = Connection::new(
            stored_connection.id,
            stored_connection.name,
            stored_connection.config,
            stored_connection.permissions,
        );
        state.connections.insert(connection.id, connection);
    }

    log::info!(
        "Initialized {} connections from storage",
        state.connections.len()
    );
    Ok(())
}

pub async fn format_sql(query: &str) -> Result<String, Error> {
    let formatted = sqlformat::format(query, &Default::default(), &Default::default());
    Ok(formatted)
}

pub async fn is_query_read_only(
    connection_id: Uuid,
    query: &str,
    state: &AppState,
) -> Result<bool, Error> {
    log::info!("Checking if {query} is read-only");
    let db = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?
        .config
        .kind();

    let stmts = match db {
        Database::Postgres => database::postgres::parser::parse_statements(query)?,
        Database::Sqlite => database::sqlite::parser::parse_statements(query)?,
    };

    Ok(stmts.into_iter().all(|stmt| stmt.is_read_only))
}

pub async fn get_database_schema(
    connection_id: Uuid,
    state: &AppState,
) -> Result<Arc<DatabaseSchema>, Error> {
    if let Some(schema) = state.schemas.get(&connection_id) {
        return Ok(schema.clone());
    }

    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;

    let connection = connection_entry.value();

    let schema = match &connection.runtime {
        ConnectionRuntime::Connected(RuntimeClient::Postgres { client }) => {
            postgres::schema::get_database_schema(client).await?
        }
        ConnectionRuntime::Connected(RuntimeClient::SQLite { connection }) => {
            sqlite::schema::get_database_schema(Arc::clone(connection)).await?
        }
        ConnectionRuntime::Disconnected => {
            return Err(Error::Any(anyhow::anyhow!("Connection not active")))
        }
    };

    let schema = Arc::new(schema);
    state.schemas.insert(connection_id, schema.clone());

    Ok(schema)
}

// Script management commands
pub async fn save_script(
    name: String,
    content: String,
    connection_id: Option<Uuid>,
    description: Option<String>,
    state: &AppState,
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

pub async fn update_script(
    id: i64,
    name: String,
    content: String,
    connection_id: Option<Uuid>,
    description: Option<String>,
    state: &AppState,
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

pub async fn get_scripts(
    connection_id: Option<Uuid>,
    state: &AppState,
) -> Result<Vec<SavedQuery>, Error> {
    let scripts = state.storage.get_saved_queries(connection_id.as_ref())?;
    Ok(scripts)
}

pub async fn delete_script(id: i64, state: &AppState) -> Result<(), Error> {
    state.storage.delete_saved_query(id)?;
    Ok(())
}

pub async fn save_session_state(session_data: &str, state: &AppState) -> Result<(), Error> {
    state.storage.set_setting("session_state", session_data)?;
    Ok(())
}

pub async fn get_session_state(state: &AppState) -> Result<Option<String>, Error> {
    let session_data = state.storage.get_setting("session_state")?;
    Ok(session_data)
}

/// Export page to CSV
pub async fn export_page(
    query_id: usize,
    page_index: usize,
    state: &AppState,
) -> Result<String, Error> {
    let now = Instant::now();
    let columns = state
        .stmt_manager
        .get_columns(query_id)?
        .ok_or_else(|| anyhow::anyhow!("No columns found yet"))?;

    let page = state
        .stmt_manager
        .fetch_page(query_id, page_index)?
        .ok_or_else(|| anyhow::anyhow!("No page {page_index} found yet"))?;

    let csv_export = database::export::export_to_csv(columns.get(), page.get())?;

    let elapsed = now.elapsed();

    log::info!(
        "Took {}us to export page {page_index} to CSV",
        elapsed.as_micros()
    );

    Ok(csv_export)
}
