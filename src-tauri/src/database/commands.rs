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
        oracle,
        postgres::{self, connect::connect},
        sqlite,
        types::{
            ConnectionInfo, Database, DatabaseConnection, DatabaseInfo, DatabaseSchema,
            OracleSettings, QueryStatus, StatementInfo,
        },
        Certificates, ConnectionMonitor,
    },
    error::Error,
    storage::{QueryHistoryEntry, SavedQuery},
    AppState,
};

#[tauri::command]
pub async fn add_connection(
    name: String,
    database_info: DatabaseInfo,
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionInfo, Error> {
    let id = Uuid::new_v4();

    let (database_info, password) = credentials::extract_sensitive_data(database_info)?;

    // It's expected that add_connection receives database_info with the password included,
    // as checked by the form in the UI. This call saves it in the keyring.
    if let Some(password) = password {
        credentials::store_sensitive_data(&id, &password)?;
    }

    let connection = DatabaseConnection::new(id, name, database_info);
    let info = connection.to_connection_info();

    state.storage.save_connection(&info)?;
    state.connections.insert(id, connection);

    Ok(info)
}

#[tauri::command]
pub async fn update_connection(
    conn_id: Uuid,
    name: String,
    database_info: DatabaseInfo,
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionInfo, Error> {
    let (database_info, password) = credentials::extract_sensitive_data(database_info)?;
    if let Some(password) = password {
        credentials::store_sensitive_data(&conn_id, &password)?;
    }

    if let Some(mut connection_entry) = state.connections.get_mut(&conn_id) {
        let connection = connection_entry.value_mut();

        let config_changed = match (&connection.database, &database_info) {
            (
                Database::Postgres {
                    connection_string: old,
                    ca_cert_path: old_cert,
                    ..
                },
                DatabaseInfo::Postgres {
                    connection_string: new,
                    ca_cert_path: new_cert,
                },
            ) => old != new || old_cert != new_cert,
            (Database::SQLite { db_path: old, .. }, DatabaseInfo::SQLite { db_path: new }) => {
                old != new
            }
            (Database::DuckDB { db_path: old, .. }, DatabaseInfo::DuckDB { db_path: new }) => {
                old != new
            }
            (
                Database::Mssql {
                    connection_string: old,
                    ca_cert_path: old_cert,
                    ..
                },
                DatabaseInfo::Mssql {
                    connection_string: new,
                    ca_cert_path: new_cert,
                },
            ) => old != new || old_cert != new_cert,
            _ => true,
        };

        if config_changed {
            match &mut connection.database {
                Database::Postgres { client, .. } => *client = None,
                Database::SQLite {
                    connection: conn, ..
                } => *conn = None,
                Database::DuckDB {
                    connection: conn, ..
                } => *conn = None,
                Database::Oracle {
                    connection: conn, ..
                } => *conn = None,
                Database::Mssql {
                    connection: conn, ..
                } => *conn = None,
            }
            connection.connected = false;
        }

        connection.name = name;
        connection.database = match database_info {
            DatabaseInfo::Postgres {
                connection_string,
                ca_cert_path,
            } => Database::Postgres {
                connection_string,
                ca_cert_path,
                client: None,
            },
            DatabaseInfo::SQLite { db_path } => Database::SQLite {
                db_path,
                connection: None,
            },
            DatabaseInfo::DuckDB { db_path } => Database::DuckDB {
                db_path,
                connection: None,
            },
            DatabaseInfo::Oracle {
                connection_string,
                wallet_path,
                tns_alias,
            } => Database::Oracle {
                connection_string,
                wallet_path,
                tns_alias,
                connection: None,
            },
            DatabaseInfo::Mssql {
                connection_string,
                ca_cert_path,
            } => Database::Mssql {
                connection_string,
                ca_cert_path,
                connection: None,
            },
        };
    }

    let updated_info = state
        .connections
        .get(&conn_id)
        .map(|conn| conn.to_connection_info())
        .with_context(|| format!("Connection not found: {}", conn_id))?;

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
            let connection = DatabaseConnection::new(
                stored_connection.id,
                stored_connection.name.clone(),
                stored_connection.database_type.clone(),
            );
            state.connections.insert(connection_id, connection);
        }
    }

    let mut connection_entry = state
        .connections
        .get_mut(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;

    let connection = connection_entry.value_mut();

    match &mut connection.database {
        Database::Postgres {
            connection_string,
            ca_cert_path,
            client,
        } => {
            let mut config: tokio_postgres::Config =
                connection_string.parse().with_context(|| {
                    format!("Failed to parse connection string: {}", connection_string)
                })?;
            if config.get_password().is_none() {
                credentials::get_password(&connection_id)?.map(|pw| config.password(pw));
            }

            match connect(&config, &certificates, ca_cert_path.as_deref()).await {
                Ok((pg_client, conn_check)) => {
                    *client = Some(Arc::new(pg_client));
                    connection.connected = true;

                    if let Err(e) = state.storage.update_last_connected(&connection_id) {
                        log::warn!("Failed to update last connected timestamp: {}", e);
                    }

                    monitor.add_connection(connection_id, conn_check).await;

                    Ok(true)
                }
                Err(e) => {
                    log::error!("Failed to connect to Postgres: {}", e);
                    connection.connected = false;
                    Ok(false)
                }
            }
        }
        Database::SQLite {
            db_path,
            connection: sqlite_conn,
        } => match rusqlite::Connection::open(&db_path) {
            Ok(conn) => {
                *sqlite_conn = Some(Arc::new(Mutex::new(conn)));
                connection.connected = true;

                if let Err(e) = state.storage.update_last_connected(&connection_id) {
                    log::warn!("Failed to update last connected timestamp: {}", e);
                }

                log::info!("Successfully connected to SQLite database: {}", db_path);
                monitor
                    .spawn_sqlite_ping(connection_id, sqlite_conn.as_ref().unwrap().clone())
                    .await;
                Ok(true)
            }
            Err(e) => {
                log::error!("Failed to connect to SQLite database {}: {}", db_path, e);
                connection.connected = false;
                Ok(false)
            }
        },
        Database::DuckDB {
            db_path,
            connection: duck_conn,
        } => match duckdb::Connection::open(&db_path) {
            Ok(conn) => {
                *duck_conn = Some(Arc::new(Mutex::new(conn)));
                connection.connected = true;

                if let Err(e) = state.storage.update_last_connected(&connection_id) {
                    log::warn!("Failed to update last connected timestamp: {}", e);
                }

                log::info!("Successfully connected to DuckDB database: {}", db_path);
                monitor
                    .spawn_duckdb_ping(connection_id, duck_conn.as_ref().unwrap().clone())
                    .await;
                Ok(true)
            }
            Err(e) => {
                log::error!("Failed to connect to DuckDB database {}: {}", db_path, e);
                connection.connected = false;
                Ok(false)
            }
        },
        Database::Mssql {
            connection_string,
            ca_cert_path,
            connection: mssql_conn,
        } => {
            let password = credentials::get_password(&connection_id)?.unwrap_or_default();
            match crate::database::mssql::connect::connect(
                connection_string,
                &certificates,
                ca_cert_path.as_deref(),
                Some(password),
            )
            .await
            {
                Ok(client) => {
                    *mssql_conn = Some(Arc::new(Mutex::new(client)));
                    connection.connected = true;

                    if let Err(e) = state.storage.update_last_connected(&connection_id) {
                        log::warn!("Failed to update last connected timestamp: {}", e);
                    }

                    log::info!("Successfully connected to MSSQL database");
                    monitor
                        .spawn_mssql_ping(connection_id, mssql_conn.as_ref().unwrap().clone())
                        .await;
                    Ok(true)
                }
                Err(e) => {
                    log::error!("Failed to connect to MSSQL database: {}", e);
                    connection.connected = false;
                    Ok(false)
                }
            }
        }
        Database::Oracle {
            connection_string,
            wallet_path,
            tns_alias,
            connection: ora_conn,
        } => {
            let url = url::Url::parse(connection_string).with_context(|| {
                format!("Failed to parse connection string: {}", connection_string)
            })?;
            let user = url.username().to_string();
            let password = credentials::get_password(&connection_id)?.unwrap_or_default();
            let host = url.host_str().unwrap_or("localhost");
            let port = url.port().unwrap_or(1521);
            let service = url.path().trim_start_matches('/');
            let prev_tns = std::env::var("TNS_ADMIN").ok();
            if let Some(path) = wallet_path.as_deref() {
                std::env::set_var("TNS_ADMIN", path);
            }
            let scheme = url.scheme();
            let connect_str = if wallet_path.is_some() {
                if let Some(alias) = tns_alias.as_deref() {
                    alias.to_string()
                } else {
                    format!("//{}:{}/{}", host, port, service)
                }
            } else if scheme.eq_ignore_ascii_case("tcps") {
                format!("tcps://{}:{}/{}", host, port, service)
            } else {
                format!("//{}:{}/{}", host, port, service)
            };

            let connect_res = oracle::connect::connect(&user, &password, &connect_str);
            match &prev_tns {
                Some(v) => std::env::set_var("TNS_ADMIN", v),
                None => std::env::remove_var("TNS_ADMIN"),
            }
            match connect_res {
                Ok(conn) => {
                    *ora_conn = Some(Arc::new(Mutex::new(conn)));
                    connection.connected = true;

                    if let Err(e) = state.storage.update_last_connected(&connection_id) {
                        log::warn!("Failed to update last connected timestamp: {}", e);
                    }

                    log::info!("Successfully connected to Oracle database: {}", connect_str);
                    monitor
                        .spawn_oracle_ping(connection_id, ora_conn.as_ref().unwrap().clone())
                        .await;
                    Ok(true)
                }
                Err(e) => {
                    let msg = crate::database::oracle::execute::map_oracle_error(&e.to_string());
                    log::error!(
                        "Failed to connect to Oracle database {}: {}",
                        connect_str,
                        msg
                    );
                    connection.connected = false;
                    Ok(false)
                }
            }
        }
    }
}

#[tauri::command]
#[allow(dead_code)]
pub async fn oracle_ping_now(
    connection_id: Uuid,
    _monitor: tauri::State<'_, ConnectionMonitor>,
    state: tauri::State<'_, AppState>,
) -> Result<bool, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::Oracle {
            connection: Some(conn),
            ..
        } => Ok(ConnectionMonitor::oracle_ping_once(conn.clone())),
        _ => Err(Error::Any(anyhow::anyhow!("Oracle connection not active"))),
    }
}

#[tauri::command]
#[allow(dead_code)]
pub async fn oracle_reconnect(
    connection_id: Uuid,
    state: tauri::State<'_, AppState>,
    monitor: tauri::State<'_, ConnectionMonitor>,
) -> Result<bool, Error> {
    let mut connection_entry = state
        .connections
        .get_mut(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value_mut();
    match &mut connection.database {
        Database::Oracle {
            connection: ora_conn,
            connection_string,
            wallet_path,
            tns_alias,
        } => {
            *ora_conn = None;
            let url = url::Url::parse(connection_string).with_context(|| {
                format!("Failed to parse connection string: {}", connection_string)
            })?;
            let user = url.username().to_string();
            let password = credentials::get_password(&connection_id)?.unwrap_or_default();
            let host = url.host_str().unwrap_or("localhost");
            let port = url.port().unwrap_or(1521);
            let service = url.path().trim_start_matches('/');
            let prev_tns = std::env::var("TNS_ADMIN").ok();
            if let Some(path) = wallet_path.as_deref() {
                std::env::set_var("TNS_ADMIN", path);
            }
            let scheme = url.scheme();
            let connect_str = if wallet_path.is_some() {
                if let Some(alias) = tns_alias.as_deref() {
                    alias.to_string()
                } else {
                    format!("//{}:{}/{}", host, port, service)
                }
            } else if scheme.eq_ignore_ascii_case("tcps") {
                format!("tcps://{}:{}/{}", host, port, service)
            } else {
                format!("//{}:{}/{}", host, port, service)
            };

            let connect_res = oracle::connect::connect(&user, &password, &connect_str);
            match &prev_tns {
                Some(v) => std::env::set_var("TNS_ADMIN", v),
                None => std::env::remove_var("TNS_ADMIN"),
            }
            match connect_res {
                Ok(conn) => {
                    *ora_conn = Some(Arc::new(Mutex::new(conn)));
                    connection.connected = true;
                    monitor
                        .spawn_oracle_ping(connection_id, ora_conn.as_ref().unwrap().clone())
                        .await;
                    Ok(true)
                }
                Err(e) => {
                    log::error!(
                        "Failed to reconnect to Oracle database {}: {}",
                        connect_str,
                        e
                    );
                    connection.connected = false;
                    Ok(false)
                }
            }
        }
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not Oracle"))),
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

    match &mut connection.database {
        Database::Postgres { client, .. } => *client = None,
        Database::Mssql {
            connection: mssql_conn,
            ..
        } => *mssql_conn = None,
        Database::SQLite {
            connection: sqlite_conn,
            ..
        } => *sqlite_conn = None,
        Database::DuckDB {
            connection: duck_conn,
            ..
        } => *duck_conn = None,
        Database::Oracle {
            connection: ora_conn,
            ..
        } => *ora_conn = None,
    }
    connection.connected = false;
    Ok(())
}

#[tauri::command]
pub async fn submit_query(
    connection_id: Uuid,
    query: &str,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<usize>, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;

    let connection = connection_entry.value();

    let client = connection.get_client()?;
    let settings = {
        let key = oracle_settings_key(Some(connection_id));
        match state.storage.get_setting(&key)? {
            Some(s) => serde_json::from_str::<OracleSettings>(&s)
                .unwrap_or_else(|_| default_oracle_settings()),
            None => match state.storage.get_setting("oracle_settings")? {
                Some(s) => serde_json::from_str::<OracleSettings>(&s)
                    .unwrap_or_else(|_| default_oracle_settings()),
                None => default_oracle_settings(),
            },
        }
    };
    let query_ids = state
        .stmt_manager
        .submit_query_with_settings(client, query, Some(settings))?;

    Ok(query_ids)
}

#[allow(dead_code)]
#[tauri::command]
pub async fn submit_query_with_params(
    connection_id: Uuid,
    query: &str,
    params: serde_json::Map<String, serde_json::Value>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<usize>, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;

    let connection = connection_entry.value();
    let client = connection.get_client()?;
    let settings = {
        let key = oracle_settings_key(Some(connection_id));
        match state.storage.get_setting(&key)? {
            Some(s) => serde_json::from_str::<OracleSettings>(&s)
                .unwrap_or_else(|_| default_oracle_settings()),
            None => match state.storage.get_setting("oracle_settings")? {
                Some(s) => serde_json::from_str::<OracleSettings>(&s)
                    .unwrap_or_else(|_| default_oracle_settings()),
                None => default_oracle_settings(),
            },
        }
    };

    match client {
        crate::database::types::DatabaseClient::Oracle { .. } => state
            .stmt_manager
            .submit_query_with_params_settings(client, query, params, Some(settings)),
        _ => state
            .stmt_manager
            .submit_query_with_settings(client, query, Some(settings)),
    }
}

#[tauri::command]
pub async fn wait_until_renderable(
    query_id: usize,
    state: tauri::State<'_, AppState>,
) -> Result<StatementInfo, Error> {
    let now = Instant::now();
    let renderable = state.stmt_manager.get_renderable(query_id)?;
    renderable.wait().await;
    let info = state.stmt_manager.fetch_query(query_id)?;
    let elapsed = now.elapsed();
    log::info!("Wait until renderable took {}ms", elapsed.as_millis());
    Ok(info)
}

#[tauri::command]
pub async fn fetch_page(
    query_id: usize,
    page_index: usize,
    state: tauri::State<'_, AppState>,
) -> Result<Option<Box<RawValue>>, Error> {
    let now = Instant::now();
    let page = state.stmt_manager.fetch_page(query_id, page_index)?;
    let elapsed = now.elapsed();
    log::info!("Took {}us to get page {page_index}", elapsed.as_micros());

    Ok(page)
}

#[tauri::command]
pub async fn get_query_status(
    query_id: usize,
    state: tauri::State<'_, AppState>,
) -> Result<QueryStatus, Error> {
    state.stmt_manager.get_query_status(query_id)
}

#[tauri::command]
pub async fn get_page_count(
    query_id: usize,
    state: tauri::State<'_, AppState>,
) -> Result<usize, Error> {
    state.stmt_manager.get_page_count(query_id)
}

#[tauri::command]
pub async fn get_columns(
    query_id: usize,
    state: tauri::State<'_, AppState>,
) -> Result<Option<Box<RawValue>>, Error> {
    state.stmt_manager.get_columns(query_id)
}

#[tauri::command]
pub async fn cancel_query(query_id: usize, state: tauri::State<'_, AppState>) -> Result<(), Error> {
    state.stmt_manager.cancel_query(query_id)
}

#[tauri::command]
pub async fn get_connections(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ConnectionInfo>, Error> {
    let mut stored_connections = state.storage.get_connections()?;

    for connection in &mut stored_connections {
        if let Some(runtime_connection) = state.connections.get(&connection.id) {
            connection.connected = runtime_connection.connected;
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

#[tauri::command]
pub async fn test_connection(
    // It's expected that test_connection receives database_info with the password included
    database_info: DatabaseInfo,
    certificates: tauri::State<'_, Certificates>,
) -> Result<bool, Error> {
    match database_info {
        DatabaseInfo::Postgres {
            connection_string,
            ca_cert_path,
        } => {
            let config: tokio_postgres::Config = connection_string.parse().with_context(|| {
                format!("Failed to parse connection string: {}", connection_string)
            })?;
            log::info!("Testing Postgres connection: {config:?}");
            match connect(&config, &certificates, ca_cert_path.as_deref()).await {
                Ok(_) => Ok(true),
                Err(e) => {
                    log::error!("Postgres connection test failed: {}", e);
                    Ok(false)
                }
            }
        }
        DatabaseInfo::SQLite { db_path } => match rusqlite::Connection::open(db_path) {
            Ok(_) => Ok(true),
            Err(e) => {
                log::error!("SQLite connection test failed: {}", e);
                Ok(false)
            }
        },
        DatabaseInfo::DuckDB { db_path } => match duckdb::Connection::open(db_path) {
            Ok(_) => Ok(true),
            Err(e) => {
                log::error!("DuckDB connection test failed: {}", e);
                Ok(false)
            }
        },
        DatabaseInfo::Mssql {
            connection_string,
            ca_cert_path,
        } => {
            log::info!("Testing MSSQL connection");
            let password = url::Url::parse(&connection_string)
                .ok()
                .and_then(|u| u.password().map(ToOwned::to_owned));
            match crate::database::mssql::connect::connect(
                &connection_string,
                &certificates,
                ca_cert_path.as_deref(),
                password,
            )
            .await
            {
                Ok(_) => Ok(true),
                Err(e) => {
                    log::error!("MSSQL connection test failed: {}", e);
                    Ok(false)
                }
            }
        }
        DatabaseInfo::Oracle {
            connection_string,
            wallet_path,
            tns_alias,
        } => {
            let url = url::Url::parse(&connection_string).with_context(|| {
                format!("Failed to parse connection string: {}", connection_string)
            })?;
            let user = url.username().to_string();
            let host = url.host_str().unwrap_or("localhost");
            let port = url.port().unwrap_or(1521);
            let service = url.path().trim_start_matches('/');
            let prev_tns = std::env::var("TNS_ADMIN").ok();
            if let Some(path) = wallet_path.as_deref() {
                std::env::set_var("TNS_ADMIN", path);
            }
            let connect_str = if wallet_path.is_some() {
                if let Some(alias) = tns_alias.as_deref() {
                    alias.to_string()
                } else {
                    format!("//{}:{}/{}", host, port, service)
                }
            } else {
                format!("//{}:{}/{}", host, port, service)
            };

            log::info!("Testing Oracle connection: {}", connect_str);
            let connect_res =
                oracle::connect::connect(&user, url.password().unwrap_or(""), &connect_str);
            match &prev_tns {
                Some(v) => std::env::set_var("TNS_ADMIN", v),
                None => std::env::remove_var("TNS_ADMIN"),
            }
            match connect_res {
                Ok(_) => Ok(true),
                Err(e) => {
                    let msg = crate::database::oracle::execute::map_oracle_error(&e.to_string());
                    log::error!("Oracle connection test failed: {}", msg);
                    Ok(false)
                }
            }
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
    state
        .storage
        .get_query_history(&connection_id, limit.map(|l| l as i64))
}

#[tauri::command]
pub async fn initialize_connections(state: tauri::State<'_, AppState>) -> Result<(), Error> {
    let stored_connections = state.storage.get_connections()?;

    for stored_connection in stored_connections {
        let connection = DatabaseConnection::new(
            stored_connection.id,
            stored_connection.name,
            stored_connection.database_type,
        );
        state.connections.insert(connection.id, connection);
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
) -> Result<Arc<DatabaseSchema>, Error> {
    if let Some(schema) = state.schemas.get(&connection_id) {
        return Ok(schema.clone());
    }

    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;

    let connection = connection_entry.value();

    let schema = match &connection.database {
        Database::Postgres {
            client: Some(client),
            ..
        } => postgres::schema::get_database_schema(client).await?,
        Database::Postgres { client: None, .. } => {
            return Err(Error::Any(anyhow::anyhow!(
                "Postgres connection not active"
            )))
        }
        Database::SQLite {
            connection: Some(conn),
            ..
        } => sqlite::schema::get_database_schema(Arc::clone(conn)).await?,
        Database::SQLite {
            connection: None, ..
        } => return Err(Error::Any(anyhow::anyhow!("SQLite connection not active"))),
        Database::DuckDB {
            connection: Some(conn),
            ..
        } => crate::database::duckdb::schema::get_database_schema(Arc::clone(conn)).await?,
        Database::DuckDB {
            connection: None, ..
        } => return Err(Error::Any(anyhow::anyhow!("DuckDB connection not active"))),
        Database::Oracle {
            connection: Some(conn),
            ..
        } => oracle::schema::get_database_schema(Arc::clone(conn)).await?,
        Database::Mssql {
            connection: Some(conn),
            ..
        } => {
            let result = tauri::async_runtime::spawn_blocking({
                let conn = Arc::clone(conn);
                move || match conn.lock() {
                    Ok(mut client) => tauri::async_runtime::block_on(async {
                        crate::database::mssql::schema::get_database_schema(&mut client).await
                    }),
                    Err(_) => Err(Error::Any(anyhow::anyhow!(
                        "MSSQL connection mutex poisoned"
                    ))),
                }
            })
            .await
            .unwrap_or_else(|e| Err(Error::Any(anyhow::anyhow!(format!("Join error: {}", e)))))?;
            result
        }
        Database::Oracle {
            connection: None, ..
        } => return Err(Error::Any(anyhow::anyhow!("Oracle connection not active"))),
        Database::Mssql {
            connection: None, ..
        } => return Err(Error::Any(anyhow::anyhow!("MSSQL connection not active"))),
    };

    let schema = Arc::new(schema);
    state.schemas.insert(connection_id, schema.clone());

    Ok(schema)
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ReconnectSettings {
    max_retries: u32,
    backoff_ms: u64,
}

fn reconnect_settings_key(connection_id: Option<Uuid>) -> String {
    match connection_id {
        Some(id) => format!("reconnect_settings:{}", id),
        None => "reconnect_settings".into(),
    }
}

#[tauri::command]
pub async fn set_reconnect_settings(
    connection_id: Option<Uuid>,
    max_retries: u32,
    backoff_ms: u64,
    state: tauri::State<'_, AppState>,
) -> Result<(), Error> {
    let settings = ReconnectSettings {
        max_retries,
        backoff_ms,
    };
    let s =
        serde_json::to_string(&settings).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
    let key = reconnect_settings_key(connection_id);
    state.storage.set_setting(&key, &s)?;
    Ok(())
}

#[tauri::command]
pub async fn get_reconnect_settings(
    connection_id: Option<Uuid>,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, Error> {
    let key = reconnect_settings_key(connection_id);
    state.storage.get_setting(&key)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct VariantSettings {
    enrich_base_type: bool,
}

fn variant_settings_key(connection_id: Uuid) -> String {
    format!("variant_settings:{}", connection_id)
}

#[tauri::command]
pub async fn set_variant_settings(
    connection_id: Uuid,
    enrich_base_type: bool,
    state: tauri::State<'_, AppState>,
) -> Result<(), Error> {
    let settings = VariantSettings { enrich_base_type };
    let s =
        serde_json::to_string(&settings).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
    let key = variant_settings_key(connection_id);
    state.storage.set_setting(&key, &s)?;
    Ok(())
}

#[tauri::command]
pub async fn get_variant_settings(
    connection_id: Uuid,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, Error> {
    let key = variant_settings_key(connection_id);
    state.storage.get_setting(&key)
}

#[tauri::command]
pub async fn get_mssql_check_constraints(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_mssql_unique_index_included_columns(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_postgres_indexes(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_postgres_index_columns(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_postgres_constraints(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_postgres_check_constraints(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_postgres_triggers(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_postgres_routines(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_postgres_views(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_postgres_view_definitions(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("{}".into())
}

#[tauri::command]
pub async fn get_postgres_foreign_keys(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_sqlite_indexes(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_sqlite_index_columns(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_sqlite_constraints(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_sqlite_triggers(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_sqlite_routines(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_sqlite_views(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_sqlite_view_definitions(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("{}".into())
}

#[tauri::command]
pub async fn get_sqlite_foreign_keys(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_duckdb_indexes(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_duckdb_index_columns(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_duckdb_constraints(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_duckdb_routines(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_duckdb_views(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_duckdb_view_definitions(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("{}".into())
}

#[tauri::command]
pub async fn get_duckdb_foreign_keys(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_mssql_indexes(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_mssql_constraints(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_mssql_triggers(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_mssql_routines(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_mssql_views(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_mssql_index_columns(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_mssql_trigger_events(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_mssql_routine_parameters(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_mssql_foreign_keys(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("[]".into())
}

#[tauri::command]
pub async fn get_mssql_view_definitions(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    Ok("{}".into())
}

#[tauri::command]
pub async fn cancel_mssql(
    _connection_id: Uuid,
    _state: tauri::State<'_, AppState>,
) -> Result<(), Error> {
    Ok(())
}

#[tauri::command]
pub async fn cancel_and_reconnect_mssql(
    _connection_id: Uuid,
    _state: tauri::State<'_, AppState>,
) -> Result<(), Error> {
    Ok(())
}

#[tauri::command]
pub async fn get_mssql_variant_base_type(
    _connection_id: Uuid,
    value: String,
    _state: tauri::State<'_, AppState>,
) -> Result<Option<String>, Error> {
    let _ = value;
    Ok(None)
}

fn apply_oracle_settings_env(s: &OracleSettings) {
    if let Some(v) = &s.raw_format {
        std::env::set_var("ORACLE_RAW_FORMAT", v);
    }
    if let Some(v) = s.raw_chunk_size {
        std::env::set_var("ORACLE_RAW_CHUNK_SIZE", v.to_string());
    }
    if let Some(v) = &s.blob_stream {
        std::env::set_var("ORACLE_BLOB_STREAM", v);
    }
    if let Some(v) = s.blob_chunk_size {
        std::env::set_var("ORACLE_BLOB_CHUNK_SIZE", v.to_string());
    }
    if let Some(v) = s.allow_db_link_ping {
        std::env::set_var(
            "ORACLE_ALLOW_DB_LINK_PING",
            if v { "true" } else { "false" },
        );
    }
    if let Some(v) = &s.xplan_format {
        std::env::set_var("ORACLE_XPLAN_FORMAT", v);
    }
    if let Some(v) = s.reconnect_max_retries {
        std::env::set_var("ORACLE_RECONNECT_MAX_RETRIES", v.to_string());
    }
    if let Some(v) = s.reconnect_backoff_ms {
        std::env::set_var("ORACLE_RECONNECT_BACKOFF_MS", v.to_string());
    }
    if let Some(v) = s.stmt_cache_size {
        std::env::set_var("ORACLE_STMT_CACHE_SIZE", v.to_string());
    }
    if let Some(v) = s.money_as_string {
        std::env::set_var("PGPAD_MONEY_AS_STRING", if v { "true" } else { "false" });
    }
    if let Some(v) = s.money_decimals {
        std::env::set_var("PGPAD_MONEY_DECIMALS", v.to_string());
    }
}

fn default_oracle_settings() -> OracleSettings {
    OracleSettings {
        raw_format: Some(std::env::var("ORACLE_RAW_FORMAT").unwrap_or_else(|_| "preview".into())),
        raw_chunk_size: Some(
            std::env::var("ORACLE_RAW_CHUNK_SIZE")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(16),
        ),
        blob_stream: Some(std::env::var("ORACLE_BLOB_STREAM").unwrap_or_else(|_| "len".into())),
        blob_chunk_size: Some(
            std::env::var("ORACLE_BLOB_CHUNK_SIZE")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(4096),
        ),
        allow_db_link_ping: Some(
            std::env::var("ORACLE_ALLOW_DB_LINK_PING")
                .ok()
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
        ),
        xplan_format: Some(
            std::env::var("ORACLE_XPLAN_FORMAT").unwrap_or_else(|_| "TYPICAL".into()),
        ),
        xplan_mode: Some(String::from("display")),
        // Optional mode: DISPLAY (plan table) or DISPLAY_CURSOR (last cursor). Default DISPLAY
        // `xplan_mode` lives in settings only; env fallback not required
        reconnect_max_retries: Some(
            std::env::var("ORACLE_RECONNECT_MAX_RETRIES")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0),
        ),
        reconnect_backoff_ms: Some(
            std::env::var("ORACLE_RECONNECT_BACKOFF_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(1000),
        ),
        stmt_cache_size: Some(
            std::env::var("ORACLE_STMT_CACHE_SIZE")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(64),
        ),
        batch_size: Some(
            std::env::var("PGPAD_BATCH_SIZE")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|&n| n > 0)
                .unwrap_or(50),
        ),
        bytes_format: Some(std::env::var("PGPAD_BYTES_FORMAT").unwrap_or_else(|_| "len".into())),
        bytes_chunk_size: Some(
            std::env::var("PGPAD_BYTES_CHUNK_SIZE")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|&n| n > 0)
                .unwrap_or(4096),
        ),
        timestamp_tz_mode: Some(
            std::env::var("PGPAD_TIMESTAMP_TZ_MODE").unwrap_or_else(|_| "utc".into()),
        ),
        numeric_string_policy: Some(
            std::env::var("PGPAD_NUMERIC_STRING_POLICY")
                .unwrap_or_else(|_| "precision_threshold".into()),
        ),
        numeric_precision_threshold: Some(
            std::env::var("PGPAD_NUMERIC_PRECISION_THRESHOLD")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|&n| n > 0)
                .unwrap_or(18),
        ),
        json_detection: Some(
            std::env::var("PGPAD_JSON_DETECTION").unwrap_or_else(|_| "auto".into()),
        ),
        json_min_length: Some(
            std::env::var("PGPAD_JSON_MIN_LENGTH")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(1),
        ),
        money_as_string: Some(
            std::env::var("PGPAD_MONEY_AS_STRING")
                .ok()
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(true),
        ),
        money_decimals: Some(
            std::env::var("PGPAD_MONEY_DECIMALS")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(4),
        ),
    }
}

fn oracle_settings_key(connection_id: Option<uuid::Uuid>) -> String {
    match connection_id {
        Some(id) => format!("oracle_settings:{}", id),
        None => "oracle_settings".into(),
    }
}

#[allow(dead_code)]
#[tauri::command]
pub async fn get_oracle_settings(
    connection_id: Option<uuid::Uuid>,
    state: tauri::State<'_, AppState>,
) -> Result<OracleSettings, Error> {
    // Try per-connection first, fall back to global, then defaults
    let conn_key = oracle_settings_key(connection_id);
    if let Some(s) = state.storage.get_setting(&conn_key)? {
        let set: OracleSettings =
            serde_json::from_str(&s).unwrap_or_else(|_| default_oracle_settings());
        return Ok(set);
    }
    if let Some(s) = state.storage.get_setting("oracle_settings")? {
        let set: OracleSettings =
            serde_json::from_str(&s).unwrap_or_else(|_| default_oracle_settings());
        return Ok(set);
    }
    Ok(default_oracle_settings())
}

#[allow(dead_code)]
#[tauri::command]
pub async fn set_oracle_settings(
    connection_id: Option<uuid::Uuid>,
    settings: OracleSettings,
    state: tauri::State<'_, AppState>,
) -> Result<(), Error> {
    apply_oracle_settings_env(&settings);
    let s =
        serde_json::to_string(&settings).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
    let key = oracle_settings_key(connection_id);
    state.storage.set_setting(&key, &s)?;
    Ok(())
}
