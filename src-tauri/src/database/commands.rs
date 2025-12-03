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
use ::oracle::sql_type;

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
            (Database::SQLite { db_path: old, .. }, DatabaseInfo::SQLite { db_path: new, .. }) => {
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
                backend_pid: None,
            },
            DatabaseInfo::SQLite { db_path, .. } => Database::SQLite {
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
            backend_pid,
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

                    if let Some(cl) = client.as_ref() {
                        match cl.query_one("SELECT pg_backend_pid()", &[]).await {
                            Ok(row) => {
                                let pid: i32 = row.get(0);
                                *backend_pid = Some(pid);
                            }
                            Err(e) => log::warn!("Failed to get backend pid: {}", e),
                        }
                    }

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
                // Apply optional session settings
                let setup_res = (|| -> Result<(), Error> {
                    if let Some(pw) = credentials::get_password(&connection_id)? {
                        let secret = secrecy::SecretString::new(pw);
                        crate::utils::sqlite_cipher::apply_cipher_settings(&conn, &secret)
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        crate::utils::sqlite_cipher::verify_cipher_ok(&conn)
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    }
                    crate::utils::sqlite_cipher::apply_common_settings(&conn)
                        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    Ok(())
                })();
                if let Err(e) = setup_res {
                    log::error!("SQLite setup failed: {}", e);
                    connection.connected = false;
                    return Ok(false);
                }
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

#[allow(dead_code)]
fn is_valid_pg_identifier(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    if !(bytes[0].is_ascii_alphabetic() || bytes[0] == b'_') {
        return false;
    }
    for &b in &bytes[1..] {
        if !(b.is_ascii_alphanumeric() || b == b'_') {
            return false;
        }
    }
    true
}

#[allow(dead_code)]
#[tauri::command]
pub async fn listen_postgres(
    connection_id: Uuid,
    channel: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), Error> {
    if !is_valid_pg_identifier(&channel) {
        return Err(Error::Any(anyhow::anyhow!("Invalid channel identifier")));
    }
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::Postgres {
            client: Some(client),
            ..
        } => {
            let sql = format!("LISTEN {}", channel);
            client
                .batch_execute(&sql)
                .await
                .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
            Ok(())
        }
        Database::Postgres { client: None, .. } => Err(Error::Any(anyhow::anyhow!(
            "Postgres connection not active"
        ))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not Postgres"))),
    }
}

#[allow(dead_code)]
#[tauri::command]
pub async fn unlisten_postgres(
    connection_id: Uuid,
    channel: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::Postgres {
            client: Some(client),
            ..
        } => {
            let sql = if let Some(ch) = channel.as_ref() {
                if !is_valid_pg_identifier(ch) {
                    return Err(Error::Any(anyhow::anyhow!("Invalid channel identifier")));
                }
                format!("UNLISTEN {}", ch)
            } else {
                String::from("UNLISTEN *")
            };
            client
                .batch_execute(&sql)
                .await
                .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
            Ok(())
        }
        Database::Postgres { client: None, .. } => Err(Error::Any(anyhow::anyhow!(
            "Postgres connection not active"
        ))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not Postgres"))),
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

#[allow(dead_code)]
#[tauri::command]
pub async fn cancel_postgres(
    connection_id: Uuid,
    state: tauri::State<'_, AppState>,
    certificates: tauri::State<'_, Certificates>,
) -> Result<(), Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::Postgres {
            connection_string,
            ca_cert_path,
            backend_pid: Some(pid),
            ..
        } => {
            let mut cfg: tokio_postgres::Config = connection_string.parse().with_context(|| {
                format!("Failed to parse connection string: {}", connection_string)
            })?;
            if cfg.get_password().is_none() {
                if let Some(pw) = crate::credentials::get_password(&connection_id)? {
                    cfg.password(pw);
                }
            }
            match crate::database::postgres::connect::connect(
                &cfg,
                &certificates,
                ca_cert_path.as_deref(),
            )
            .await
            {
                Ok((client, _)) => {
                    let _ = client.execute("SELECT pg_cancel_backend($1)", &[pid]).await;
                    Ok(())
                }
                Err(e) => Err(Error::Any(anyhow::anyhow!(format!(
                    "Failed to connect for cancellation: {}",
                    e
                )))),
            }
        }
        Database::Postgres {
            backend_pid: None, ..
        } => Err(Error::Any(anyhow::anyhow!(
            "Postgres backend PID not recorded"
        ))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not Postgres"))),
    }
}

#[allow(dead_code)]
#[tauri::command]
pub async fn terminate_postgres(
    connection_id: Uuid,
    state: tauri::State<'_, AppState>,
    certificates: tauri::State<'_, Certificates>,
) -> Result<(), Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::Postgres {
            connection_string,
            ca_cert_path,
            backend_pid: Some(pid),
            ..
        } => {
            let mut cfg: tokio_postgres::Config = connection_string.parse().with_context(|| {
                format!("Failed to parse connection string: {}", connection_string)
            })?;
            if cfg.get_password().is_none() {
                if let Some(pw) = crate::credentials::get_password(&connection_id)? {
                    cfg.password(pw);
                }
            }
            match crate::database::postgres::connect::connect(
                &cfg,
                &certificates,
                ca_cert_path.as_deref(),
            )
            .await
            {
                Ok((client, _)) => {
                    let _ = client
                        .execute("SELECT pg_terminate_backend($1)", &[pid])
                        .await;
                    Ok(())
                }
                Err(e) => Err(Error::Any(anyhow::anyhow!(format!(
                    "Failed to connect for termination: {}",
                    e
                )))),
            }
        }
        Database::Postgres {
            backend_pid: None, ..
        } => Err(Error::Any(anyhow::anyhow!(
            "Postgres backend PID not recorded"
        ))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not Postgres"))),
    }
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
        DatabaseInfo::SQLite {
            db_path,
            passphrase,
        } => match rusqlite::Connection::open(db_path) {
            Ok(conn) => {
                if let Some(pw) = passphrase.as_ref() {
                    let _ = conn.pragma_update(None, "key", pw);
                    let _ = conn.pragma_update(None, "cipher_compatibility", 4);
                    let _ = conn.pragma_update(None, "cipher_page_size", 4096);
                    let _ = conn.execute("PRAGMA cipher_memory_security = ON", []);
                    let ck = conn
                        .pragma_query_value(None, "cipher_integrity_check", |row| {
                            row.get::<_, String>(0)
                        })
                        .unwrap_or_else(|_| String::from("error"));
                    if ck != "ok" {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
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

#[tauri::command]
pub async fn get_encryption_settings(state: tauri::State<'_, AppState>) -> Result<String, Error> {
    let enabled = state
        .storage
        .get_setting("encryption_enabled")?
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(true);
    let key_storage = state
        .storage
        .get_setting("encryption_key_storage")?
        .unwrap_or_else(|| String::from("keychain"));
    let payload = serde_json::json!({
        "enabled": enabled,
        "key_storage": key_storage
    });
    Ok(payload.to_string())
}

#[tauri::command]
pub async fn set_encryption_settings(
    enabled: bool,
    key_storage: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), Error> {
    state
        .storage
        .set_setting("encryption_enabled", if enabled { "1" } else { "0" })?;
    state
        .storage
        .set_setting("encryption_key_storage", &key_storage)?;
    Ok(())
}

#[tauri::command]
pub async fn get_oracle_indexes(
    connection_id: Uuid,
    table_name: Option<String>,
    index_name: Option<String>,
    page: Option<usize>,
    limit: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::Oracle {
            connection: Some(conn),
            ..
        } => {
            let mut indexes: Vec<serde_json::Value> = Vec::new();
            let mut total_count: i64 = 0;
            let page_num = page.unwrap_or(1);
            let per_page = limit.unwrap_or(50);
            let offset: i64 = ((page_num.max(1) - 1) * per_page) as i64;
            let mut idx_names: Vec<String> = Vec::new();
            if let Ok(client) = conn.lock() {
                let mut where_clauses: Vec<String> = Vec::new();
                let mut binds_count: Vec<&dyn sql_type::ToSql> = Vec::new();
                let mut binds_page: Vec<&dyn sql_type::ToSql>;
                let mut bind_values_count: Vec<String> = Vec::new();
                where_clauses.push("i.table_name NOT LIKE 'BIN$%'".into());
                if let Some(tn) = table_name.as_ref() {
                    where_clauses.push("UPPER(i.table_name) = :1".into());
                    let tn_upper = tn.to_uppercase();
                    bind_values_count.push(tn_upper);
                }
                if let Some(iname) = index_name.as_ref() {
                    let pos = if bind_values_count.is_empty() { 1 } else { 2 };
                    where_clauses.push(format!("UPPER(i.index_name) LIKE :{}", pos));
                    let like = iname.to_uppercase();
                    let likep = if like.contains('%') {
                        like
                    } else {
                        format!("%{}%", like)
                    };
                    bind_values_count.push(likep);
                }
                for v in &bind_values_count {
                    binds_count.push(v);
                }
                let where_sql = if where_clauses.is_empty() {
                    String::new()
                } else {
                    format!(" WHERE {}", where_clauses.join(" AND "))
                };
                let count_sql = format!("SELECT COUNT(*) FROM user_indexes i{}", where_sql);
                if let Ok(mut rows) = client.query(&count_sql, &binds_count[..]) {
                    if let Some(Ok(row)) = rows.next() {
                        total_count = row.get::<usize, i64>(0).unwrap_or(0);
                    }
                }
                binds_page = binds_count.clone();
                let order_sql = String::from(" ORDER BY i.table_name, i.index_name ");
                let page_sql = format!(
                    "SELECT * FROM (
                        SELECT i.index_name, i.table_name, i.index_type, i.uniqueness, i.status, TO_CHAR(i.created, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created, s.bytes AS size_bytes,
                               ROW_NUMBER() OVER (ORDER BY i.table_name, i.index_name) AS rn
                        FROM user_indexes i
                        JOIN user_segments s ON i.index_name = s.segment_name
                        {}{}
                    ) WHERE rn > :{} AND rn <= :{}",
                    where_sql,
                    order_sql,
                    binds_page.len() + 1,
                    binds_page.len() + 2
                );
                binds_page.push(&offset);
                let end = offset + per_page as i64;
                binds_page.push(&end);
                if let Ok(rows) = client.query(&page_sql, &binds_page[..]) {
                    for row in rows.flatten() {
                        let index_name: String = row.get(0).unwrap_or_default();
                        let table_name_v: String = row.get(1).unwrap_or_default();
                        let index_type: String = row.get(2).unwrap_or_default();
                        let uniqueness: String = row.get(3).unwrap_or_default();
                        let status: String = row.get(4).unwrap_or_default();
                        let created: String = row.get(5).unwrap_or_default();
                        let size_bytes: i64 = row.get(6).unwrap_or(0);
                        idx_names.push(index_name.clone());
                        let obj = serde_json::json!({
                            "index_name": index_name,
                            "table_name": table_name_v,
                            "index_type": index_type,
                            "uniqueness": uniqueness,
                            "status": status,
                            "created": created,
                            "size_bytes": size_bytes,
                            "column_names": []
                        });
                        indexes.push(obj);
                    }
                }
                if !idx_names.is_empty() {
                    let qcols = "SELECT index_name, column_name, column_position, descend FROM user_ind_columns WHERE index_name IN (:1) ORDER BY index_name, column_position";
                    for name in idx_names.iter() {
                        if let Ok(mut rows) = client.query(qcols, &[name]) {
                            let mut cols_map: Vec<String> = Vec::new();
                            while let Some(Ok(row)) = rows.next() {
                                let col_name: String = row.get(1).unwrap_or_default();
                                cols_map.push(col_name);
                            }
                            for it in indexes.iter_mut() {
                                if it.get("index_name").and_then(|v| v.as_str())
                                    == Some(name.as_str())
                                {
                                    if let Some(arr) = it.get_mut("column_names") {
                                        *arr = serde_json::Value::Array(
                                            cols_map
                                                .iter()
                                                .map(|c| serde_json::Value::String(c.clone()))
                                                .collect(),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                return Err(Error::Any(anyhow::anyhow!(
                    "Oracle connection mutex poisoned"
                )));
            }
            let result = serde_json::json!({
                "indexes": indexes,
                "total_count": total_count,
                "page": page_num
            });
            Ok(result.to_string())
        }
        _ => Err(Error::Any(anyhow::anyhow!("Oracle connection not active"))),
    }
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
    connection_id: Uuid,
    page: Option<usize>,
    page_size: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::Postgres {
            client: Some(client),
            ..
        } => {
            let page = page.unwrap_or(0);
            let page_size = page_size.unwrap_or(50);
            let count_sql = r#"
                SELECT COUNT(*)
                FROM pg_index ix
                JOIN pg_class i ON i.oid = ix.indexrelid
                JOIN pg_class t ON t.oid = ix.indrelid
                JOIN pg_namespace n ON n.oid = t.relnamespace
                WHERE t.relkind IN ('r','m','p')
            "#;
            let total_rows: i64 = client
                .query_one(count_sql, &[])
                .await
                .map(|r| r.get::<usize, i64>(0))
                .unwrap_or(0);
            let total_pages = if page_size == 0 {
                0
            } else {
                (total_rows + page_size as i64 - 1) / page_size as i64
            };
            let offset = (page as i64) * (page_size as i64);
            let list_sql = r#"
                SELECT n.nspname AS schema_name,
                       t.relname AS table_name,
                       i.relname AS index_name,
                       ix.indisunique,
                       ix.indisprimary
                FROM pg_index ix
                JOIN pg_class i ON i.oid = ix.indexrelid
                JOIN pg_class t ON t.oid = ix.indrelid
                JOIN pg_namespace n ON n.oid = t.relnamespace
                WHERE t.relkind IN ('r','m','p')
                ORDER BY n.nspname, t.relname, i.relname
                LIMIT $1 OFFSET $2
            "#;
            let rows = client
                .query(list_sql, &[&(page_size as i64), &offset])
                .await
                .unwrap_or_default();
            let mut data: Vec<serde_json::Value> = Vec::with_capacity(rows.len());
            for row in rows {
                let schema_name: &str = row.get(0);
                let table_name: &str = row.get(1);
                let index_name: &str = row.get(2);
                let is_unique: bool = row.get(3);
                let is_primary: bool = row.get(4);
                data.push(serde_json::json!({
                    "schema_name": schema_name,
                    "table_name": table_name,
                    "index_name": index_name,
                    "is_unique": is_unique,
                    "is_primary": is_primary
                }));
            }
            let payload = serde_json::json!({
                "data": data,
                "total_pages": total_pages,
                "current_page": page
            });
            Ok(payload.to_string())
        }
        Database::Postgres { client: None, .. } => Err(Error::Any(anyhow::anyhow!(
            "Postgres connection not active"
        ))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not Postgres"))),
    }
}

#[tauri::command]
pub async fn get_postgres_index_columns(
    connection_id: Uuid,
    page: Option<usize>,
    page_size: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::Postgres {
            client: Some(client),
            ..
        } => {
            let page = page.unwrap_or(0);
            let page_size = page_size.unwrap_or(100);
            let count_sql = r#"
                SELECT COUNT(*)
                FROM pg_index ix
                JOIN pg_class i ON i.oid = ix.indexrelid
                JOIN pg_class t ON t.oid = ix.indrelid
                JOIN pg_namespace n ON n.oid = t.relnamespace
                JOIN LATERAL unnest(ix.indkey) WITH ORDINALITY AS k(attnum, n) ON TRUE
                WHERE t.relkind IN ('r','m','p')
            "#;
            let total_rows: i64 = client
                .query_one(count_sql, &[])
                .await
                .map(|r| r.get::<usize, i64>(0))
                .unwrap_or(0);
            let total_pages = if page_size == 0 {
                0
            } else {
                (total_rows + page_size as i64 - 1) / page_size as i64
            };
            let offset = (page as i64) * (page_size as i64);
            let list_sql = r#"
                SELECT n.nspname AS schema_name,
                       t.relname AS table_name,
                       i.relname AS index_name,
                       a.attname AS column_name,
                       k.n AS column_position,
                       ((ix.indoption[k.n-1] & 1) = 1) AS is_desc
                FROM pg_index ix
                JOIN pg_class i ON i.oid = ix.indexrelid
                JOIN pg_class t ON t.oid = ix.indrelid
                JOIN pg_namespace n ON n.oid = t.relnamespace
                JOIN LATERAL unnest(ix.indkey) WITH ORDINALITY AS k(attnum, n) ON TRUE
                LEFT JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = k.attnum
                WHERE t.relkind IN ('r','m','p')
                ORDER BY n.nspname, t.relname, i.relname, k.n
                LIMIT $1 OFFSET $2
            "#;
            let rows = client
                .query(list_sql, &[&(page_size as i64), &offset])
                .await
                .unwrap_or_default();
            let mut data: Vec<serde_json::Value> = Vec::with_capacity(rows.len());
            for row in rows {
                let schema_name: &str = row.get(0);
                let table_name: &str = row.get(1);
                let index_name: &str = row.get(2);
                let column_name: Option<&str> = row.get(3);
                let column_position: i64 = row.get(4);
                let is_desc: bool = row.get(5);
                data.push(serde_json::json!({
                    "schema_name": schema_name,
                    "table_name": table_name,
                    "index_name": index_name,
                    "column_name": column_name.unwrap_or("") ,
                    "column_position": column_position,
                    "is_desc": is_desc
                }));
            }
            let payload = serde_json::json!({
                "data": data,
                "total_pages": total_pages,
                "current_page": page
            });
            Ok(payload.to_string())
        }
        Database::Postgres { client: None, .. } => Err(Error::Any(anyhow::anyhow!(
            "Postgres connection not active"
        ))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not Postgres"))),
    }
}

#[tauri::command]
pub async fn get_postgres_constraints(
    connection_id: Uuid,
    page: Option<usize>,
    page_size: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::Postgres {
            client: Some(client),
            ..
        } => {
            let page = page.unwrap_or(0);
            let page_size = page_size.unwrap_or(50);
            let count_sql = r#"
                SELECT COUNT(*)
                FROM pg_constraint c
                JOIN pg_class t ON t.oid = c.conrelid
                JOIN pg_namespace n ON n.oid = t.relnamespace
                WHERE t.relkind IN ('r','m','p')
            "#;
            let total_rows: i64 = client
                .query_one(count_sql, &[])
                .await
                .map(|r| r.get::<usize, i64>(0))
                .unwrap_or(0);
            let total_pages = if page_size == 0 {
                0
            } else {
                (total_rows + page_size as i64 - 1) / page_size as i64
            };
            let offset = (page as i64) * (page_size as i64);
            let list_sql = r#"
                SELECT n.nspname AS schema_name,
                       t.relname AS table_name,
                       c.conname AS constraint_name,
                       c.contype AS constraint_type
                FROM pg_constraint c
                JOIN pg_class t ON t.oid = c.conrelid
                JOIN pg_namespace n ON n.oid = t.relnamespace
                WHERE t.relkind IN ('r','m','p')
                ORDER BY n.nspname, t.relname, c.conname
                LIMIT $1 OFFSET $2
            "#;
            let rows = client
                .query(list_sql, &[&(page_size as i64), &offset])
                .await
                .unwrap_or_default();
            let mut data: Vec<serde_json::Value> = Vec::with_capacity(rows.len());
            for row in rows {
                let schema_name: &str = row.get(0);
                let table_name: &str = row.get(1);
                let constraint_name: &str = row.get(2);
                let constraint_type: &str = row.get(3); // p=PK u=Unique f=FK c=Check
                data.push(serde_json::json!({
                    "schema_name": schema_name,
                    "table_name": table_name,
                    "constraint_name": constraint_name,
                    "constraint_type": constraint_type
                }));
            }
            let payload = serde_json::json!({
                "data": data,
                "total_pages": total_pages,
                "current_page": page
            });
            Ok(payload.to_string())
        }
        Database::Postgres { client: None, .. } => Err(Error::Any(anyhow::anyhow!(
            "Postgres connection not active"
        ))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not Postgres"))),
    }
}

#[tauri::command]
pub async fn get_postgres_check_constraints(
    connection_id: Uuid,
    page: Option<usize>,
    page_size: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::Postgres {
            client: Some(client),
            ..
        } => {
            let page = page.unwrap_or(0);
            let page_size = page_size.unwrap_or(50);
            let count_sql = r#"
                SELECT COUNT(*)
                FROM pg_constraint c
                JOIN pg_class t ON t.oid = c.conrelid
                JOIN pg_namespace n ON n.oid = t.relnamespace
                WHERE t.relkind IN ('r','m','p') AND c.contype = 'c'
            "#;
            let total_rows: i64 = client
                .query_one(count_sql, &[])
                .await
                .map(|r| r.get::<usize, i64>(0))
                .unwrap_or(0);
            let total_pages = if page_size == 0 {
                0
            } else {
                (total_rows + page_size as i64 - 1) / page_size as i64
            };
            let offset = (page as i64) * (page_size as i64);
            let list_sql = r#"
                SELECT n.nspname AS schema_name,
                       t.relname AS table_name,
                       c.conname AS constraint_name,
                       pg_get_constraintdef(c.oid) AS definition
                FROM pg_constraint c
                JOIN pg_class t ON t.oid = c.conrelid
                JOIN pg_namespace n ON n.oid = t.relnamespace
                WHERE t.relkind IN ('r','m','p') AND c.contype = 'c'
                ORDER BY n.nspname, t.relname, c.conname
                LIMIT $1 OFFSET $2
            "#;
            let rows = client
                .query(list_sql, &[&(page_size as i64), &offset])
                .await
                .unwrap_or_default();
            let mut data: Vec<serde_json::Value> = Vec::with_capacity(rows.len());
            for row in rows {
                let schema_name: &str = row.get(0);
                let table_name: &str = row.get(1);
                let constraint_name: &str = row.get(2);
                let definition: &str = row.get(3);
                data.push(serde_json::json!({
                    "schema_name": schema_name,
                    "table_name": table_name,
                    "constraint_name": constraint_name,
                    "definition": definition
                }));
            }
            let payload = serde_json::json!({
                "data": data,
                "total_pages": total_pages,
                "current_page": page
            });
            Ok(payload.to_string())
        }
        Database::Postgres { client: None, .. } => Err(Error::Any(anyhow::anyhow!(
            "Postgres connection not active"
        ))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not Postgres"))),
    }
}

#[tauri::command]
pub async fn get_postgres_triggers(
    connection_id: Uuid,
    page: Option<usize>,
    page_size: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::Postgres {
            client: Some(client),
            ..
        } => {
            let page = page.unwrap_or(0);
            let page_size = page_size.unwrap_or(50);
            let count_sql = r#"
                SELECT COUNT(*)
                FROM pg_trigger tg
                JOIN pg_class t ON t.oid = tg.tgrelid
                JOIN pg_namespace n ON n.oid = t.relnamespace
                WHERE NOT tg.tgisinternal AND t.relkind IN ('r','m','p')
            "#;
            let total_rows: i64 = client
                .query_one(count_sql, &[])
                .await
                .map(|r| r.get::<usize, i64>(0))
                .unwrap_or(0);
            let total_pages = if page_size == 0 {
                0
            } else {
                (total_rows + page_size as i64 - 1) / page_size as i64
            };
            let offset = (page as i64) * (page_size as i64);
            let list_sql = r#"
                SELECT n.nspname AS schema_name,
                       t.relname AS table_name,
                       tg.tgname AS trigger_name,
                       pg_get_triggerdef(tg.oid) AS definition
                FROM pg_trigger tg
                JOIN pg_class t ON t.oid = tg.tgrelid
                JOIN pg_namespace n ON n.oid = t.relnamespace
                WHERE NOT tg.tgisinternal AND t.relkind IN ('r','m','p')
                ORDER BY n.nspname, t.relname, tg.tgname
                LIMIT $1 OFFSET $2
            "#;
            let rows = client
                .query(list_sql, &[&(page_size as i64), &offset])
                .await
                .unwrap_or_default();
            let mut data: Vec<serde_json::Value> = Vec::with_capacity(rows.len());
            for row in rows {
                let schema_name: &str = row.get(0);
                let table_name: &str = row.get(1);
                let trigger_name: &str = row.get(2);
                let definition: &str = row.get(3);
                data.push(serde_json::json!({
                    "schema_name": schema_name,
                    "table_name": table_name,
                    "trigger_name": trigger_name,
                    "definition": definition
                }));
            }
            let payload = serde_json::json!({
                "data": data,
                "total_pages": total_pages,
                "current_page": page
            });
            Ok(payload.to_string())
        }
        Database::Postgres { client: None, .. } => Err(Error::Any(anyhow::anyhow!(
            "Postgres connection not active"
        ))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not Postgres"))),
    }
}

#[tauri::command]
pub async fn get_postgres_routines(
    connection_id: Uuid,
    page: Option<usize>,
    page_size: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::Postgres {
            client: Some(client),
            ..
        } => {
            let page = page.unwrap_or(0);
            let page_size = page_size.unwrap_or(50);
            let count_sql = r#"
                SELECT COUNT(*)
                FROM information_schema.routines r
                WHERE r.specific_schema NOT IN ('pg_catalog','information_schema')
            "#;
            let total_rows: i64 = client
                .query_one(count_sql, &[])
                .await
                .map(|r| r.get::<usize, i64>(0))
                .unwrap_or(0);
            let total_pages = if page_size == 0 {
                0
            } else {
                (total_rows + page_size as i64 - 1) / page_size as i64
            };
            let offset = (page as i64) * (page_size as i64);
            let list_sql = r#"
                SELECT r.specific_schema AS schema_name,
                       r.routine_name,
                       r.routine_type,
                       r.external_language AS language,
                       r.data_type AS return_type
                FROM information_schema.routines r
                WHERE r.specific_schema NOT IN ('pg_catalog','information_schema')
                ORDER BY r.specific_schema, r.routine_name
                LIMIT $1 OFFSET $2
            "#;
            let rows = client
                .query(list_sql, &[&(page_size as i64), &offset])
                .await
                .unwrap_or_default();
            let mut data: Vec<serde_json::Value> = Vec::with_capacity(rows.len());
            for row in rows {
                let schema_name: &str = row.get(0);
                let routine_name: &str = row.get(1);
                let routine_type: &str = row.get(2);
                let language: Option<&str> = row.get(3);
                let return_type: &str = row.get(4);
                data.push(serde_json::json!({
                    "schema_name": schema_name,
                    "routine_name": routine_name,
                    "routine_type": routine_type,
                    "language": language.unwrap_or(""),
                    "return_type": return_type
                }));
            }
            let payload = serde_json::json!({
                "data": data,
                "total_pages": total_pages,
                "current_page": page
            });
            Ok(payload.to_string())
        }
        Database::Postgres { client: None, .. } => Err(Error::Any(anyhow::anyhow!(
            "Postgres connection not active"
        ))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not Postgres"))),
    }
}

#[tauri::command]
pub async fn get_postgres_views(
    connection_id: Uuid,
    page: Option<usize>,
    page_size: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::Postgres {
            client: Some(client),
            ..
        } => {
            let page = page.unwrap_or(0);
            let page_size = page_size.unwrap_or(50);
            let count_sql = r#"
                SELECT COUNT(*)
                FROM information_schema.views v
                WHERE v.table_schema NOT IN ('pg_catalog','information_schema')
            "#;
            let total_rows: i64 = client
                .query_one(count_sql, &[])
                .await
                .map(|r| r.get::<usize, i64>(0))
                .unwrap_or(0);
            let total_pages = if page_size == 0 {
                0
            } else {
                (total_rows + page_size as i64 - 1) / page_size as i64
            };
            let offset = (page as i64) * (page_size as i64);
            let list_sql = r#"
                SELECT v.table_schema, v.table_name
                FROM information_schema.views v
                WHERE v.table_schema NOT IN ('pg_catalog','information_schema')
                ORDER BY v.table_schema, v.table_name
                LIMIT $1 OFFSET $2
            "#;
            let rows = client
                .query(list_sql, &[&(page_size as i64), &offset])
                .await
                .unwrap_or_default();
            let mut data: Vec<serde_json::Value> = Vec::with_capacity(rows.len());
            for row in rows {
                let schema_name: &str = row.get(0);
                let view_name: &str = row.get(1);
                data.push(serde_json::json!({
                    "schema_name": schema_name,
                    "view_name": view_name
                }));
            }
            let payload = serde_json::json!({
                "data": data,
                "total_pages": total_pages,
                "current_page": page
            });
            Ok(payload.to_string())
        }
        Database::Postgres { client: None, .. } => Err(Error::Any(anyhow::anyhow!(
            "Postgres connection not active"
        ))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not Postgres"))),
    }
}

#[tauri::command]
pub async fn get_postgres_view_definitions(
    connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::Postgres {
            client: Some(client),
            ..
        } => {
            let sql = r#"
                SELECT n.nspname AS schema_name,
                       c.relname AS view_name,
                       pg_get_viewdef(c.oid) AS definition
                FROM pg_class c
                JOIN pg_namespace n ON n.oid = c.relnamespace
                WHERE c.relkind = 'v' AND n.nspname NOT IN ('pg_catalog','information_schema')
                ORDER BY n.nspname, c.relname
            "#;
            let rows = client.query(sql, &[]).await.unwrap_or_default();
            let mut map = serde_json::Map::new();
            for row in rows {
                let schema_name: &str = row.get(0);
                let view_name: &str = row.get(1);
                let definition: &str = row.get(2);
                map.insert(
                    format!("{}.{}", schema_name, view_name),
                    serde_json::Value::String(definition.to_string()),
                );
            }
            Ok(serde_json::Value::Object(map).to_string())
        }
        Database::Postgres { client: None, .. } => Err(Error::Any(anyhow::anyhow!(
            "Postgres connection not active"
        ))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not Postgres"))),
    }
}

#[tauri::command]
pub async fn get_postgres_foreign_keys(
    connection_id: Uuid,
    page: Option<usize>,
    page_size: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::Postgres {
            client: Some(client),
            ..
        } => {
            let page = page.unwrap_or(0);
            let page_size = page_size.unwrap_or(50);
            let count_sql = r#"
                SELECT COUNT(*)
                FROM pg_constraint c
                JOIN pg_class t ON t.oid = c.conrelid
                JOIN pg_namespace n ON n.oid = t.relnamespace
                WHERE t.relkind IN ('r','m','p') AND c.contype = 'f'
            "#;
            let total_rows: i64 = client
                .query_one(count_sql, &[])
                .await
                .map(|r| r.get::<usize, i64>(0))
                .unwrap_or(0);
            let total_pages = if page_size == 0 {
                0
            } else {
                (total_rows + page_size as i64 - 1) / page_size as i64
            };
            let offset = (page as i64) * (page_size as i64);
            let list_sql = r#"
                SELECT n.nspname AS schema_name,
                       t.relname AS table_name,
                       c.conname AS constraint_name,
                       pg_get_constraintdef(c.oid) AS definition
                FROM pg_constraint c
                JOIN pg_class t ON t.oid = c.conrelid
                JOIN pg_namespace n ON n.oid = t.relnamespace
                WHERE t.relkind IN ('r','m','p') AND c.contype = 'f'
                ORDER BY n.nspname, t.relname, c.conname
                LIMIT $1 OFFSET $2
            "#;
            let rows = client
                .query(list_sql, &[&(page_size as i64), &offset])
                .await
                .unwrap_or_default();
            let mut data: Vec<serde_json::Value> = Vec::with_capacity(rows.len());
            for row in rows {
                let schema_name: &str = row.get(0);
                let table_name: &str = row.get(1);
                let constraint_name: &str = row.get(2);
                let definition: &str = row.get(3);
                data.push(serde_json::json!({
                    "schema_name": schema_name,
                    "table_name": table_name,
                    "constraint_name": constraint_name,
                    "definition": definition
                }));
            }
            let payload = serde_json::json!({
                "data": data,
                "total_pages": total_pages,
                "current_page": page
            });
            Ok(payload.to_string())
        }
        Database::Postgres { client: None, .. } => Err(Error::Any(anyhow::anyhow!(
            "Postgres connection not active"
        ))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not Postgres"))),
    }
}

#[tauri::command]
pub async fn get_sqlite_indexes(
    connection_id: Uuid,
    page: Option<usize>,
    page_size: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::SQLite {
            connection: Some(conn),
            ..
        } => {
            let result = tauri::async_runtime::spawn_blocking({
                let conn = conn.clone();
                let page = page.unwrap_or(0);
                let page_size = page_size.unwrap_or(50);
                move || {
                    use rusqlite::Connection;
                    let mut data: Vec<serde_json::Value> = Vec::new();
                    let c: std::sync::MutexGuard<Connection> = conn.lock().map_err(|_| Error::Any(anyhow::anyhow!("SQLite connection mutex poisoned")))?;
                    let mut stmt = c
                        .prepare("SELECT name FROM sqlite_master WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' ORDER BY name")
                        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    let rows = stmt.query_map([], |row| row.get::<usize, String>(0)).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    let mut idx_items: Vec<(String, String, bool, bool)> = Vec::new();
                    for table_name in rows.flatten() {
                        let mut istmt = c
                            .prepare(&format!("PRAGMA index_list('{}')", table_name.replace("'", "''")))
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        let indices = istmt
                            .query_map([], |row| {
                                let name: String = row.get(1)?;
                                let unique: i64 = row.get(2)?;
                                let origin: String = row.get(3)?; // c/u/pk
                                Ok((name, unique == 1, origin == "pk"))
                            })
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        for item in indices.flatten() {
                            idx_items.push((table_name.clone(), item.0, item.1, item.2));
                        }
                    }
                    let total_rows = idx_items.len() as i64;
                    let start = (page as i64) * (page_size as i64);
                    let end = (start + page_size as i64).min(total_rows);
                    for i in start..end {
                        let (table_name, index_name, is_unique, is_primary) = idx_items[i as usize].clone();
                        data.push(serde_json::json!({
                            "schema_name": "main",
                            "table_name": table_name,
                            "index_name": index_name,
                            "is_unique": is_unique,
                            "is_primary": is_primary
                        }));
                    }
                    let total_pages = if page_size == 0 { 0 } else { (total_rows + page_size as i64 - 1) / page_size as i64 };
                    let payload = serde_json::json!({
                        "data": data,
                        "total_pages": total_pages,
                        "current_page": page
                    });
                    Ok::<String, Error>(payload.to_string())
                }
            }).await.unwrap_or_else(|e| Err(Error::Any(anyhow::anyhow!(format!("Join error: {}", e)))))?;
            Ok(result)
        }
        Database::SQLite {
            connection: None, ..
        } => Err(Error::Any(anyhow::anyhow!("SQLite connection not active"))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not SQLite"))),
    }
}

#[tauri::command]
pub async fn get_sqlite_index_columns(
    connection_id: Uuid,
    page: Option<usize>,
    page_size: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::SQLite {
            connection: Some(conn),
            ..
        } => {
            let result = tauri::async_runtime::spawn_blocking({
                let conn = conn.clone();
                let page = page.unwrap_or(0);
                let page_size = page_size.unwrap_or(100);
                move || {
                    use rusqlite::Connection;
                    let mut rows_all: Vec<serde_json::Value> = Vec::new();
                    let c: std::sync::MutexGuard<Connection> = conn.lock().map_err(|_| Error::Any(anyhow::anyhow!("SQLite connection mutex poisoned")))?;
                    let mut tstmt = c
                        .prepare("SELECT name FROM sqlite_master WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' ORDER BY name")
                        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    let tables = tstmt.query_map([], |row| row.get::<usize, String>(0)).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    for table_name in tables.flatten() {
                        let mut istmt = c
                            .prepare(&format!("PRAGMA index_list('{}')", table_name.replace("'", "''")))
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        let indices = istmt
                            .query_map([], |row| {
                                let name: String = row.get(1)?;
                                Ok(name)
                            })
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        for index_name in indices.flatten() {
                            let mut cstmt = c
                                .prepare(&format!("PRAGMA index_info('{}')", index_name.replace("'", "''")))
                                .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                            let cols = cstmt
                                .query_map([], |row| {
                                    let pos: i64 = row.get(0)?; // seqno
                                    let col_name: String = row.get(2)?; // name
                                    Ok((pos, col_name))
                                })
                                .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                            for (pos, col) in cols.flatten() {
                                rows_all.push(serde_json::json!({
                                    "schema_name": "main",
                                    "table_name": table_name,
                                    "index_name": index_name,
                                    "column_name": col,
                                    "column_position": pos,
                                    "is_desc": false
                                }));
                            }
                        }
                    }
                    let total_rows = rows_all.len() as i64;
                    let start = (page as i64) * (page_size as i64);
                    let end = (start + page_size as i64).min(total_rows);
                    let data = rows_all.into_iter().skip(start as usize).take((end - start) as usize).collect::<Vec<_>>();
                    let total_pages = if page_size == 0 { 0 } else { (total_rows + page_size as i64 - 1) / page_size as i64 };
                    let payload = serde_json::json!({
                        "data": data,
                        "total_pages": total_pages,
                        "current_page": page
                    });
                    Ok::<String, Error>(payload.to_string())
                }
            }).await.unwrap_or_else(|e| Err(Error::Any(anyhow::anyhow!(format!("Join error: {}", e)))))?;
            Ok(result)
        }
        Database::SQLite {
            connection: None, ..
        } => Err(Error::Any(anyhow::anyhow!("SQLite connection not active"))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not SQLite"))),
    }
}

#[tauri::command]
pub async fn get_sqlite_constraints(
    connection_id: Uuid,
    page: Option<usize>,
    page_size: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::SQLite {
            connection: Some(conn),
            ..
        } => {
            let result = tauri::async_runtime::spawn_blocking({
                let conn = conn.clone();
                let page = page.unwrap_or(0);
                let page_size = page_size.unwrap_or(100);
                move || {
                    use rusqlite::Connection;
                    let c: std::sync::MutexGuard<Connection> = conn
                        .lock()
                        .map_err(|_| Error::Any(anyhow::anyhow!("SQLite connection mutex poisoned")))?;
                    // Collect table names and CREATE SQL
                    let mut tstmt = c
                        .prepare("SELECT name, sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
                        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    let mut rows = tstmt
                        .query([])
                        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    let mut items: Vec<serde_json::Value> = Vec::new();
                    while let Some(row) = rows
                        .next()
                        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?
                    {
                        let table_name: String = row
                            .get(0)
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        let create_sql: String = row
                            .get(1)
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;

                        // PRIMARY KEY columns via PRAGMA table_info
                        let mut pinfo = c
                            .prepare(&format!("PRAGMA table_info('{}')", table_name.replace("'", "''")))
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        let cols = pinfo
                            .query_map([], |r| {
                                let name: String = r.get(1)?; // name
                                let pkpos: i64 = r.get(5)?;   // pk (0 if not part of PK)
                                Ok((name, pkpos))
                            })
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        let mut pk_cols: Vec<String> = Vec::new();
                        for (name, pkpos) in cols.flatten() {
                            if pkpos > 0 {
                                pk_cols.push(name);
                            }
                        }
                        if !pk_cols.is_empty() {
                            items.push(serde_json::json!({
                                "schema_name": "main",
                                "table_name": table_name,
                                "constraint_name": "PRIMARY KEY",
                                "constraint_type": "pk",
                                "columns": pk_cols,
                            }));
                        }

                        // UNIQUE constraints via PRAGMA index_list(origin='u')
                        let mut ilist = c
                            .prepare(&format!("PRAGMA index_list('{}')", table_name.replace("'", "''")))
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        let idxs = ilist
                            .query_map([], |r| {
                                let name: String = r.get(1)?;
                                let unique: i64 = r.get(2)?;
                                let origin: String = r.get(3)?; // 'c', 'u', 'pk'
                                Ok((name, unique == 1, origin))
                            })
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        for (iname, is_unique, origin) in idxs.flatten() {
                            if is_unique || origin == "u" {
                                let mut iinfo = c
                                    .prepare(&format!("PRAGMA index_info('{}')", iname.replace("'", "''")))
                                    .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                                let cols = iinfo
                                    .query_map([], |r| r.get::<usize, String>(2))
                                    .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                                let mut ucols: Vec<String> = Vec::new();
                                for col in cols.flatten() {
                                    ucols.push(col);
                                }
                                items.push(serde_json::json!({
                                    "schema_name": "main",
                                    "table_name": table_name,
                                    "constraint_name": iname,
                                    "constraint_type": "unique",
                                    "columns": ucols,
                                }));
                            }
                        }

                        // CHECK constraints: naive extraction of `CHECK(...)` fragments
                        let mut checks: Vec<String> = Vec::new();
                        let mut rest = create_sql.as_str();
                        while let Some(pos) = rest.to_uppercase().find("CHECK(") {
                            let orig_pos = rest.char_indices().nth(pos).map(|(i, _)| i).unwrap_or(pos);
                            let after = &rest[orig_pos + 6..];
                            // find matching closing parenthesis
                            let mut depth = 1i32;
                            let mut end_idx = 0usize;
                            for (i, ch) in after.char_indices() {
                                if ch == '(' { depth += 1; }
                                if ch == ')' {
                                    depth -= 1;
                                    if depth == 0 { end_idx = i; break; }
                                }
                            }
                            if end_idx == 0 { break; }
                            let expr = after[..end_idx].trim().to_string();
                            checks.push(expr);
                            rest = &after[end_idx+1..];
                        }
                        for expr in checks {
                            items.push(serde_json::json!({
                                "schema_name": "main",
                                "table_name": table_name,
                                "constraint_name": "CHECK",
                                "constraint_type": "check",
                                "definition": expr,
                            }));
                        }
                    }

                    let total_rows = items.len() as i64;
                    let start = (page as i64) * (page_size as i64);
                    let end = (start + page_size as i64).min(total_rows);
                    let data = items
                        .into_iter()
                        .skip(start as usize)
                        .take((end - start) as usize)
                        .collect::<Vec<_>>();
                    let total_pages = if page_size == 0 { 0 } else { (total_rows + page_size as i64 - 1) / page_size as i64 };
                    let payload = serde_json::json!({
                        "data": data,
                        "total_pages": total_pages,
                        "current_page": page
                    });
                    Ok::<String, Error>(payload.to_string())
                }
            })
            .await
            .unwrap_or_else(|e| Err(Error::Any(anyhow::anyhow!(format!("Join error: {}", e)))))?;
            Ok(result)
        }
        Database::SQLite {
            connection: None, ..
        } => Err(Error::Any(anyhow::anyhow!("SQLite connection not active"))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not SQLite"))),
    }
}

#[tauri::command]
pub async fn get_sqlite_triggers(
    connection_id: Uuid,
    page: Option<usize>,
    page_size: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::SQLite {
            connection: Some(conn),
            ..
        } => {
            let result = tauri::async_runtime::spawn_blocking({
                let conn = conn.clone();
                let page = page.unwrap_or(0);
                let page_size = page_size.unwrap_or(50);
                move || {
                    use rusqlite::Connection;
                    let mut data: Vec<serde_json::Value> = Vec::new();
                    let c: std::sync::MutexGuard<Connection> = conn.lock().map_err(|_| Error::Any(anyhow::anyhow!("SQLite connection mutex poisoned")))?;
                    let mut stmt = c
                        .prepare("SELECT name, tbl_name, sql FROM sqlite_master WHERE type='trigger' AND name NOT LIKE 'sqlite_%' ORDER BY name")
                        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    let mut rows = stmt.query([]).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    let mut all: Vec<(String, String, String)> = Vec::new();
                    while let Some(row) = rows.next().map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))? {
                        let name: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        let tbl: String = row.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        let sql: String = row.get(2).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        all.push((name, tbl, sql));
                    }
                    let total_rows = all.len() as i64;
                    let start = (page as i64) * (page_size as i64);
                    let end = (start + page_size as i64).min(total_rows);
                    for (name, tbl, sql) in all.into_iter().skip(start as usize).take((end - start) as usize) {
                        data.push(serde_json::json!({
                            "schema_name": "main",
                            "table_name": tbl,
                            "trigger_name": name,
                            "definition": sql,
                        }));
                    }
                    let total_pages = if page_size == 0 { 0 } else { (total_rows + page_size as i64 - 1) / page_size as i64 };
                    let payload = serde_json::json!({
                        "data": data,
                        "total_pages": total_pages,
                        "current_page": page
                    });
                    Ok::<String, Error>(payload.to_string())
                }
            }).await.unwrap_or_else(|e| Err(Error::Any(anyhow::anyhow!(format!("Join error: {}", e)))))?;
            Ok(result)
        }
        Database::SQLite {
            connection: None, ..
        } => Err(Error::Any(anyhow::anyhow!("SQLite connection not active"))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not SQLite"))),
    }
}

#[tauri::command]
pub async fn get_sqlite_routines(
    _connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    _state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    // SQLite doesn't have stored routines; return empty payload
    let payload = serde_json::json!({"data": [], "total_pages": 0, "current_page": 0});
    Ok(payload.to_string())
}

#[tauri::command]
pub async fn get_sqlite_views(
    connection_id: Uuid,
    page: Option<usize>,
    page_size: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::SQLite {
            connection: Some(conn),
            ..
        } => {
            let result = tauri::async_runtime::spawn_blocking({
                let conn = conn.clone();
                let page = page.unwrap_or(0);
                let page_size = page_size.unwrap_or(50);
                move || {
                    use rusqlite::Connection;
                    let mut data: Vec<serde_json::Value> = Vec::new();
                    let c: std::sync::MutexGuard<Connection> = conn.lock().map_err(|_| Error::Any(anyhow::anyhow!("SQLite connection mutex poisoned")))?;
                    let mut stmt = c
                        .prepare("SELECT name FROM sqlite_master WHERE type='view' AND name NOT LIKE 'sqlite_%' ORDER BY name")
                        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    let views = stmt.query_map([], |row| row.get::<usize, String>(0)).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    let all: Vec<String> = views.flatten().collect();
                    let total_rows = all.len() as i64;
                    let start = (page as i64) * (page_size as i64);
                    let end = (start + page_size as i64).min(total_rows);
                    for v in all.iter().skip(start as usize).take((end - start) as usize) {
                        data.push(serde_json::json!({"schema_name":"main","view_name":v}));
                    }
                    let total_pages = if page_size == 0 { 0 } else { (total_rows + page_size as i64 - 1) / page_size as i64 };
                    let payload = serde_json::json!({
                        "data": data,
                        "total_pages": total_pages,
                        "current_page": page
                    });
                    Ok::<String, Error>(payload.to_string())
                }
            }).await.unwrap_or_else(|e| Err(Error::Any(anyhow::anyhow!(format!("Join error: {}", e)))))?;
            Ok(result)
        }
        Database::SQLite {
            connection: None, ..
        } => Err(Error::Any(anyhow::anyhow!("SQLite connection not active"))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not SQLite"))),
    }
}

#[tauri::command]
pub async fn get_sqlite_view_definitions(
    connection_id: Uuid,
    _page: Option<usize>,
    _page_size: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::SQLite {
            connection: Some(conn),
            ..
        } => {
            let result = tauri::async_runtime::spawn_blocking({
                let conn = conn.clone();
                move || {
                    use rusqlite::Connection;
                    let mut map = serde_json::Map::new();
                    let c: std::sync::MutexGuard<Connection> = conn.lock().map_err(|_| Error::Any(anyhow::anyhow!("SQLite connection mutex poisoned")))?;
                    let mut stmt = c
                        .prepare("SELECT name, sql FROM sqlite_master WHERE type='view' AND name NOT LIKE 'sqlite_%' ORDER BY name")
                        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    let mut rows = stmt.query([]).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    while let Some(row) = rows.next().map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))? {
                        let name: String = row.get(0).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        let sql: String = row.get(1).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        map.insert(name, serde_json::Value::String(sql));
                    }
                    Ok::<String, Error>(serde_json::Value::Object(map).to_string())
                }
            }).await.unwrap_or_else(|e| Err(Error::Any(anyhow::anyhow!(format!("Join error: {}", e)))))?;
            Ok(result)
        }
        Database::SQLite {
            connection: None, ..
        } => Err(Error::Any(anyhow::anyhow!("SQLite connection not active"))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not SQLite"))),
    }
}

#[tauri::command]
pub async fn get_sqlite_foreign_keys(
    connection_id: Uuid,
    page: Option<usize>,
    page_size: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::SQLite {
            connection: Some(conn),
            ..
        } => {
            let result = tauri::async_runtime::spawn_blocking({
                let conn = conn.clone();
                let page = page.unwrap_or(0);
                let page_size = page_size.unwrap_or(50);
                move || {
                    use rusqlite::Connection;
                    let mut all: Vec<serde_json::Value> = Vec::new();
                    let c: std::sync::MutexGuard<Connection> = conn.lock().map_err(|_| Error::Any(anyhow::anyhow!("SQLite connection mutex poisoned")))?;
                    let mut tstmt = c
                        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
                        .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    let tables = tstmt.query_map([], |row| row.get::<usize, String>(0)).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                    for table_name in tables.flatten() {
                        let mut fkstmt = c
                            .prepare(&format!("PRAGMA foreign_key_list('{}')", table_name.replace("'", "''")))
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        let fks = fkstmt
                            .query_map([], |row| {
                                let id: i64 = row.get(0)?; // foreign key id
                                let seq: i64 = row.get(1)?; // sequence within fk
                                let ref_table: String = row.get(2)?;
                                let from_col: String = row.get(3)?;
                                let to_col: String = row.get(4)?;
                                let on_update: Option<String> = row.get::<usize, Option<String>>(5)?;
                                let on_delete: Option<String> = row.get::<usize, Option<String>>(6)?;
                                let match_opt: Option<String> = row.get::<usize, Option<String>>(7)?;
                                Ok((id, seq, ref_table, from_col, to_col, on_update, on_delete, match_opt))
                            })
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        for (id, seq, ref_table, from_col, to_col, on_update, on_delete, match_opt) in fks.flatten() {
                            all.push(serde_json::json!({
                                "schema_name": "main",
                                "table_name": table_name,
                                "fk_id": id,
                                "seq": seq,
                                "ref_table": ref_table,
                                "from_column": from_col,
                                "to_column": to_col,
                                "on_update": on_update.unwrap_or_default(),
                                "on_delete": on_delete.unwrap_or_default(),
                                "match": match_opt.unwrap_or_default()
                            }));
                        }
                    }
                    let total_rows = all.len() as i64;
                    let start = (page as i64) * (page_size as i64);
                    let end = (start + page_size as i64).min(total_rows);
                    let data = all.into_iter().skip(start as usize).take((end - start) as usize).collect::<Vec<_>>();
                    let total_pages = if page_size == 0 { 0 } else { (total_rows + page_size as i64 - 1) / page_size as i64 };
                    let payload = serde_json::json!({
                        "data": data,
                        "total_pages": total_pages,
                        "current_page": page
                    });
                    Ok::<String, Error>(payload.to_string())
                }
            }).await.unwrap_or_else(|e| Err(Error::Any(anyhow::anyhow!(format!("Join error: {}", e)))))?;
            Ok(result)
        }
        Database::SQLite {
            connection: None, ..
        } => Err(Error::Any(anyhow::anyhow!("SQLite connection not active"))),
        _ => Err(Error::Any(anyhow::anyhow!("Connection is not SQLite"))),
    }
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
    connection_id: Uuid,
    page: Option<usize>,
    page_size: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<String, Error> {
    let connection_entry = state
        .connections
        .get(&connection_id)
        .with_context(|| format!("Connection not found: {}", connection_id))?;
    let connection = connection_entry.value();
    match &connection.database {
        Database::Mssql {
            connection: Some(conn),
            ..
        } => {
            let result = tauri::async_runtime::spawn_blocking({
                let conn = conn.clone();
                let page = page.unwrap_or(0);
                let page_size = page_size.unwrap_or(50);
                move || match conn.lock() {
                    Ok(mut client) => tauri::async_runtime::block_on(async move {
                        use futures_util::TryStreamExt;
                        use tiberius::{Query, QueryItem};
                        let count_sql = "SELECT COUNT(*) FROM sys.indexes i JOIN sys.tables t ON i.object_id = t.object_id JOIN sys.schemas s ON t.schema_id = s.schema_id WHERE i.is_hypothetical = 0";
                        let mut count_stream = Query::new(count_sql)
                            .query(&mut client)
                            .await
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        let mut total_rows: i64 = 0;
                        if let Some(QueryItem::Row(r)) = count_stream
                            .try_next()
                            .await
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))? {
                            let v: Option<i64> = r.try_get(0).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                            total_rows = v.unwrap_or(0);
                        }
                        drop(count_stream);
                        let total_pages = if page_size == 0 { 0 } else { (total_rows + page_size as i64 - 1) / page_size as i64 };
                        let offset = (page as i64) * (page_size as i64);
                        let mut q = Query::new("SELECT s.name AS schema_name, t.name AS table_name, i.name AS index_name, i.is_unique, i.is_primary_key FROM sys.indexes i JOIN sys.tables t ON i.object_id = t.object_id JOIN sys.schemas s ON t.schema_id = s.schema_id WHERE i.is_hypothetical = 0 ORDER BY s.name, t.name, i.name OFFSET @P1 ROWS FETCH NEXT @P2 ROWS ONLY");
                        q.bind(offset);
                        q.bind(page_size as i64);
                        let mut stream = q
                            .query(&mut client)
                            .await
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                        let mut data: Vec<serde_json::Value> = Vec::new();
                        while let Some(item) = stream
                            .try_next()
                            .await
                            .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))? {
                            if let QueryItem::Row(row) = item {
                                let schema_name: Option<&str> = row.try_get(0).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                                let table_name: Option<&str> = row.try_get(1).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                                let index_name: Option<&str> = row.try_get(2).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                                let is_unique: Option<bool> = row.try_get(3).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                                let is_primary: Option<bool> = row.try_get(4).map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
                                data.push(serde_json::json!({
                                    "schema_name": schema_name.unwrap_or(""),
                                    "table_name": table_name.unwrap_or(""),
                                    "index_name": index_name.unwrap_or(""),
                                    "is_unique": is_unique.unwrap_or(false),
                                    "is_primary": is_primary.unwrap_or(false)
                                }));
                            }
                        }
                        let payload = serde_json::json!({
                            "data": data,
                            "total_pages": total_pages,
                            "current_page": page
                        });
                        Ok::<String, Error>(payload.to_string())
                    }),
                    Err(_) => Err(Error::Any(anyhow::anyhow!("MSSQL connection mutex poisoned"))),
                }
            })
            .await
            .unwrap_or_else(|e| Err(Error::Any(anyhow::anyhow!(format!("Join error: {}", e)))))?;
            Ok(result)
        }
        _ => Err(Error::Any(anyhow::anyhow!("MSSQL connection not active"))),
    }
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
// Notifications can be consumed by issuing LISTEN and polling via client in the UI loop if needed.
