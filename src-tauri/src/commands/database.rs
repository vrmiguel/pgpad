use crate::{tls::load_certificates, AppState};

use super::errors::Error;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio_postgres_rustls::MakeRustlsConnect;
use tokio_postgres::{tls::MakeTlsConnect, Client, Connection, NoTls, Socket};
use uuid::Uuid;



#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub name: String,
    pub connection_string: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub id: String,
    pub name: String,
    pub connection_string: String,
    pub connected: bool,
}

#[derive(Debug)]
pub struct DatabaseConnection {
    info: ConnectionInfo,
    client: Option<Client>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub row_count: usize,
    pub duration_ms: u64,
}

async fn connect(connection_string: &str) -> Result<Client, Error> {
    use tokio_postgres::config::SslMode;

    let config: tokio_postgres::Config = connection_string.parse()?;

    let client = match config.get_ssl_mode() {
        SslMode::Require | SslMode::Prefer => {
            let certificate_store = load_certificates().await;
            let rustls_config = rustls::ClientConfig::builder()
                .with_root_certificates(certificate_store)
                .with_no_client_auth();
            let tls = tokio_postgres_rustls::MakeRustlsConnect::new(rustls_config);
            let (client, conn) = config.connect(tls).await?;

            tokio::spawn(check_connection::<MakeRustlsConnect>(conn));

            client
        },
        // Mostly SslMode::Disable, but the enum was marked as non_exhaustive
        _other => {
            let (client, conn) = config.connect(NoTls).await?;

            tokio::spawn(check_connection::<NoTls>(conn));  

            client
        }
    };

    Ok(client)
}

async fn check_connection<T>(conn: Connection<Socket, T::Stream>)
where
    T: MakeTlsConnect<Socket>,
{
    match conn.await {
        Ok(()) => println!("Connected successfully"),
        Err(err) => eprintln!("Failed to connect to Postgres: {err}"),
    }
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
pub async fn add_connection(
    config: ConnectionConfig,
    state: tauri::State<'_, AppState>
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
    state: tauri::State<'_, AppState>
) -> Result<bool, Error> {
    if let Some(mut connection_entry) = state.connections.get_mut(&connection_id) {
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
    } else {
        Err(Error::from("Connection not found"))
    }
}

#[tauri::command]
pub async fn disconnect_from_database(
    connection_id: String,
    state: tauri::State<'_, AppState>
) -> Result<(), Error> {
    if let Some(mut connection_entry) = state.connections.get_mut(&connection_id) {
        let connection = connection_entry.value_mut();
        connection.client = None;
        connection.info.connected = false;
        Ok(())
    } else {
        Err(Error::from("Connection not found"))
    }
}

#[tauri::command]
pub async fn execute_query(
    connection_id: String,
    query: String,
    state: tauri::State<'_, AppState>
) -> Result<QueryResult, Error> {
    if let Some(connection_entry) = state.connections.get(&connection_id) {
        let connection = connection_entry.value();
        
        if let Some(client) = &connection.client {
            let start = std::time::Instant::now();
            
            match client.query(&query, &[]).await {
                Ok(rows) => {
                    let duration = start.elapsed().as_millis() as u64;
                    
                    let columns = if rows.is_empty() {
                        Vec::new()
                    } else {
                        rows[0].columns().iter().map(|col| col.name().to_string()).collect()
                    };
                    
                    let mut result_rows = Vec::new();
                    for row in rows.iter() {
                        let mut result_row = Vec::new();
                        for i in 0..row.len() {
                            let value: Option<String> = row.try_get(i).unwrap_or(None);
                            result_row.push(
                                value.map(serde_json::Value::String)
                                    .unwrap_or(serde_json::Value::Null)
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
                    Err(Error::from(format!("Query failed: {}", e)))
                }
            }
        } else {
            Err(Error::from("Not connected to database"))
        }
    } else {
        Err(Error::from("Connection not found"))
    }
}

#[tauri::command]
pub async fn get_connections(state: tauri::State<'_, AppState>) -> Result<Vec<ConnectionInfo>, Error> {
    let result: Vec<ConnectionInfo> = state.connections
        .iter()
        .map(|entry| entry.value().info.clone())
        .collect();
    Ok(result)
}

#[tauri::command]
pub async fn remove_connection(
    connection_id: String,
    state: tauri::State<'_, AppState>
) -> Result<(), Error> {
    state.connections.remove(&connection_id);
    Ok(())
} 