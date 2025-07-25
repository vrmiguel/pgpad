mod error;
mod postgres;


use dashmap::DashMap;

use crate::postgres::types::DatabaseConnection;

#[derive(Debug)]
pub struct AppState {
    pub connections: DashMap<String, DatabaseConnection>,
}

impl AppState {
    pub fn new() -> Self {
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
