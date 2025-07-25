mod error;
mod postgres;
mod storage;

use dashmap::DashMap;

use crate::{postgres::types::DatabaseConnection, storage::Storage};
pub use error::{Error, Result};

#[derive(Debug)]
pub struct AppState {
    pub connections: DashMap<String, DatabaseConnection>,
}

impl AppState {
    pub fn new() -> Self {
        let data_dir = dirs::data_dir().expect("Failed to get data directory");
        let db_path = data_dir.join("pgpad").join("pgpad.db");

        Storage::new(db_path);

        Self {
            connections: DashMap::new(),
        }
    }
}

#[allow(clippy::missing_panics_doc)]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            postgres::commands::test_connection,
            postgres::commands::add_connection,
            postgres::commands::connect_to_database,
            postgres::commands::disconnect_from_database,
            postgres::commands::execute_query,
            postgres::commands::get_connections,
            postgres::commands::remove_connection
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
