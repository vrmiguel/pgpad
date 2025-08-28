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

    pub async fn add_connection(&self, connection_id: Uuid, conn_check: ConnectionCheck) {
        log::info!("Adding connection {connection_id} to ConnectionMonitor");
        self.connections
            .write()
            .await
            .push((connection_id, conn_check));
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
