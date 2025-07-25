mod commands;
mod tls;

use commands::default::{read, write};
use commands::database::{
    test_connection, add_connection, connect_to_database, disconnect_from_database,
    execute_query, get_connections, remove_connection
};
use dashmap::DashMap;

use crate::commands::database::DatabaseConnection;

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
            read, 
            write,
            test_connection,
            add_connection,
            connect_to_database,
            disconnect_from_database,
            execute_query,
            get_connections,
            remove_connection
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
