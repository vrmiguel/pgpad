use std::{sync::Arc, time::Duration};

use tauri::{Emitter, EventTarget, Manager};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{database::postgres::connect::ConnectionCheck, AppState};

type ConnectionId = Uuid;

#[derive(Clone)]
pub struct ConnectionMonitor {
    app: tauri::AppHandle,
    connections: Arc<RwLock<Vec<(ConnectionId, ConnectionCheck)>>>,
}

impl ConnectionMonitor {
    pub fn new(app: tauri::AppHandle) -> Self {
        let monitor = Self {
            app,
            connections: Arc::new(RwLock::new(Vec::new())),
        };

        let polling_monitor = monitor.clone();
        tauri::async_runtime::spawn(async move { polling_monitor.poll().await });

        monitor
    }

    #[allow(dead_code)]
    pub fn oracle_ping_once(
        conn: std::sync::Arc<std::sync::Mutex<oracle::Connection>>,
    ) -> bool {
        match conn.lock() {
            Ok(c) => c.execute("SELECT 1 FROM DUAL", &[]).is_ok(),
            Err(_) => false,
        }
    }

    #[allow(dead_code)]
    pub fn sqlite_ping_once(
        conn: std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>,
    ) -> bool {
        match conn.lock() {
            Ok(c) => c.prepare("SELECT 1").and_then(|mut s| s.query([]).map(|_| ())).is_ok(),
            Err(_) => false,
        }
    }

    #[allow(dead_code)]
    pub fn duckdb_ping_once(
        conn: std::sync::Arc<std::sync::Mutex<duckdb::Connection>>,
    ) -> bool {
        match conn.lock() {
            Ok(c) => {
                match c.prepare("SELECT 1") {
                    Ok(mut stmt) => stmt.query([]).is_ok(),
                    Err(_) => false,
                }
            }
            Err(_) => false,
        }
    }

    pub async fn add_connection(&self, connection_id: Uuid, conn_check: ConnectionCheck) {
        log::info!("Adding connection {connection_id} to ConnectionMonitor");
        self.connections
            .write()
            .await
            .push((connection_id, conn_check));
    }

    pub async fn spawn_oracle_ping(
        &self,
        connection_id: Uuid,
        conn: std::sync::Arc<std::sync::Mutex<oracle::Connection>>,
    ) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                let ok = tauri::async_runtime::spawn_blocking({
                    let conn = conn.clone();
                    move || match conn.lock() {
                        Ok(c) => c.execute("SELECT 1 FROM DUAL", &[]).is_ok(),
                        Err(_) => false,
                    }
                })
                .await
                .unwrap_or(false);

                if !ok {
                    // Resolve per-connection settings from storage with fallback
                    let mut retries = 0u32;
                    let mut backoff_ms = 1000u64;
                    if let Some(state_manager) = app.try_state::<crate::AppState>() {
                        let key = format!("oracle_settings:{}", connection_id);
                        if let Ok(Some(s)) = state_manager.storage.get_setting(&key) {
                            if let Ok(set) = serde_json::from_str::<crate::database::types::OracleSettings>(&s) {
                                if let Some(r) = set.reconnect_max_retries { retries = r; }
                                if let Some(b) = set.reconnect_backoff_ms { backoff_ms = b; }
                            }
                        } else if let Ok(Some(s)) = state_manager.storage.get_setting("oracle_settings") {
                            if let Ok(set) = serde_json::from_str::<crate::database::types::OracleSettings>(&s) {
                                if let Some(r) = set.reconnect_max_retries { retries = r; }
                                if let Some(b) = set.reconnect_backoff_ms { backoff_ms = b; }
                            }
                        } else {
                            // final env fallback
                            retries = std::env::var("ORACLE_RECONNECT_MAX_RETRIES").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
                            backoff_ms = std::env::var("ORACLE_RECONNECT_BACKOFF_MS").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(1000);
                        }
                    }
                    let mut reconnected = false;
                    while retries > 0 {
                        retries -= 1;
                        if let Some(state_manager) = app.try_state::<crate::AppState>() {
                            if let Some(mut connection) = state_manager.connections.get_mut(&connection_id) {
                                let conn_mut = connection.value_mut();
                                if let crate::database::types::Database::Oracle { connection: ora_conn, connection_string, wallet_path, tns_alias } = &mut conn_mut.database {
                                    let url = match url::Url::parse(connection_string) { Ok(u) => u, Err(_) => break };
                                    let user = url.username().to_string();
                                    let password = match crate::credentials::get_password(&connection_id) { Ok(pw) => pw.unwrap_or_default(), Err(_) => String::new() };
                                    let host = url.host_str().unwrap_or("localhost");
                                    let port = url.port().unwrap_or(1521);
                                    let service = url.path().trim_start_matches('/');
                                    let prev_tns = std::env::var("TNS_ADMIN").ok();
                                    if let Some(path) = wallet_path.as_deref() { std::env::set_var("TNS_ADMIN", path); }
                                    let scheme = url.scheme();
                                    let connect_str = if wallet_path.is_some() {
                                        if let Some(alias) = tns_alias.as_deref() { alias.to_string() } else { format!("//{}:{}/{}", host, port, service) }
                                    } else if scheme.eq_ignore_ascii_case("tcps") {
                                        format!("tcps://{}:{}/{}", host, port, service)
                                    } else {
                                        format!("//{}:{}/{}", host, port, service)
                                    };
                                    let connect_res = crate::database::oracle::connect::connect(&user, &password, &connect_str);
                                    match &prev_tns { Some(v) => std::env::set_var("TNS_ADMIN", v), None => std::env::remove_var("TNS_ADMIN") }
                                    if let Ok(newc) = connect_res {
                                        *ora_conn = Some(std::sync::Arc::new(std::sync::Mutex::new(newc)));
                                        conn_mut.connected = true;
                                        reconnected = true;
                                    }
                                }
                            }
                        }
                        if reconnected { break; }
                        tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                        backoff_ms = backoff_ms.saturating_mul(2);
                    }
                    if reconnected {
                        let _ = app.emit_to(EventTarget::App, "connection-reconnected", connection_id);
                        continue;
                    }
                    if let Some(state_manager) = app.try_state::<crate::AppState>() {
                        if let Some(mut connection) = state_manager.connections.get_mut(&connection_id) {
                            let conn_mut = connection.value_mut();
                            conn_mut.connected = false;
                            if let crate::database::types::Database::Oracle { connection: ora_conn, .. } = &mut conn_mut.database {
                                *ora_conn = None;
                            }
                        }
                    }
                    let _ = app.emit_to(EventTarget::App, "end-of-connection", connection_id);
                    break;
                }

                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        });
    }

    pub async fn spawn_duckdb_ping(
        &self,
        connection_id: Uuid,
        conn: std::sync::Arc<std::sync::Mutex<duckdb::Connection>>,
    ) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                let ok = tauri::async_runtime::spawn_blocking({
                    let conn = conn.clone();
                    move || match conn.lock() {
                        Ok(c) => match c.prepare("SELECT 1") {
                            Ok(mut stmt) => stmt.query([]).is_ok(),
                            Err(_) => false,
                        },
                        Err(_) => false,
                    }
                })
                .await
                .unwrap_or(false);

                if !ok {
                    if let Some(state_manager) = app.try_state::<crate::AppState>() {
                        if let Some(mut connection) = state_manager.connections.get_mut(&connection_id) {
                            let conn_mut = connection.value_mut();
                            conn_mut.connected = false;
                            if let crate::database::types::Database::DuckDB { connection: duck_conn, .. } = &mut conn_mut.database {
                                *duck_conn = None;
                            }
                        }
                    }
                    let _ = app.emit_to(EventTarget::App, "end-of-connection", connection_id);
                    break;
                }

                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        });
    }

    pub async fn spawn_sqlite_ping(
        &self,
        connection_id: Uuid,
        conn: std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>,
    ) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                let ok = tauri::async_runtime::spawn_blocking({
                    let conn = conn.clone();
                    move || match conn.lock() {
                        Ok(c) => c.prepare("SELECT 1").and_then(|mut s| s.query([]).map(|_| ())).is_ok(),
                        Err(_) => false,
                    }
                })
                .await
                .unwrap_or(false);

                if !ok {
                    if let Some(state_manager) = app.try_state::<crate::AppState>() {
                        if let Some(mut connection) = state_manager.connections.get_mut(&connection_id) {
                            let conn_mut = connection.value_mut();
                            conn_mut.connected = false;
                            if let crate::database::types::Database::SQLite { connection: sqlite_conn, .. } = &mut conn_mut.database {
                                *sqlite_conn = None;
                            }
                        }
                    }
                    let _ = app.emit_to(EventTarget::App, "end-of-connection", connection_id);
                    break;
                }

                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        });
    }

    fn persist_disconnect(&self, connection_id: Uuid) {
        let Some(state_manager) = self.app.try_state::<AppState>() else {
            log::error!("No state manager found!");
            return;
        };
        let Some(mut connection) = state_manager.connections.get_mut(&connection_id) else {
            log::error!("Connection {connection_id} not found!");
            return;
        };
        connection.connected = false;
        match &mut connection.database {
            crate::database::types::Database::Postgres { client, .. } => *client = None,
            crate::database::types::Database::SQLite {
                connection: sqlite_conn,
                ..
            } => *sqlite_conn = None,
            crate::database::types::Database::DuckDB {
                connection: duck_conn,
                ..
            } => *duck_conn = None,
            crate::database::types::Database::Oracle {
                connection: ora_conn,
                ..
            } => *ora_conn = None,
        }
    }

    fn notify_disconnect(&self, connection_id: Uuid) {
        self.persist_disconnect(connection_id);
        match self
            .app
            .emit_to(EventTarget::App, "end-of-connection", connection_id)
        {
            Ok(()) => {
                log::info!("End-of-connection event emitted for connection {connection_id}");
            }
            Err(e) => {
                log::error!("Error emitting end-of-connection event: {e}");
            }
        }
    }

    async fn get_dropped_connections(
        &self,
        mut dropped_conns: Vec<ConnectionId>,
    ) -> Vec<ConnectionId> {
        let connections = self.connections.read().await;

        for (connection_id, conn_check) in &*connections {
            if conn_check.inner().is_finished() {
                dropped_conns.push(*connection_id);
            }
        }

        dropped_conns
    }

    async fn poll(self) {
        let mut dropped_conns = Vec::new();

        loop {
            dropped_conns = self.get_dropped_connections(dropped_conns).await;

            if !dropped_conns.is_empty() {
                for connection_id in &dropped_conns {
                    self.notify_disconnect(*connection_id);
                }

                self.connections
                    .write()
                    .await
                    .retain(|(id, _)| !dropped_conns.contains(id));

                dropped_conns.clear();
            }

            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}
