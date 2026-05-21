mod credentials;
pub mod database;
mod error;
pub mod storage;
mod utils;

use std::{path::PathBuf, sync::Arc};

use dashmap::DashMap;
use uuid::Uuid;

use crate::{
    database::{
        stmt_manager::StatementManager,
        types::{Connection, ConnectionRuntime, DatabaseSchema},
    },
    storage::Storage,
};
pub use database::{Certificates, ConnectionMonitor};
pub use error::{Error, Result};
pub use storage::{QueryHistoryEntry, SavedQuery};

#[derive(Debug)]
pub struct AppState {
    pub connections: DashMap<Uuid, Connection>,
    pub schemas: DashMap<Uuid, Arc<DatabaseSchema>>,
    /// SQLite database for application data
    pub storage: Storage,
    pub stmt_manager: StatementManager,
}

impl AppState {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let storage = Storage::new(db_path.into())?;

        Ok(Self {
            connections: DashMap::new(),
            schemas: DashMap::new(),
            storage,
            stmt_manager: StatementManager::new(),
        })
    }

    pub fn mark_disconnected(&self, connection_id: Uuid) -> bool {
        let Some(mut connection) = self.connections.get_mut(&connection_id) else {
            return false;
        };

        connection.runtime = ConnectionRuntime::Disconnected;
        true
    }
}
