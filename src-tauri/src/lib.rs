mod error;
mod postgres;
mod storage;

use dashmap::DashMap;

use crate::{postgres::types::DatabaseConnection, storage::Storage};
pub use error::{Error, Result};

#[derive(Debug)]
pub struct AppState {
    pub connections: DashMap<String, DatabaseConnection>,
    pub storage: Storage,
}

impl AppState {
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_dir().expect("Failed to get data directory");
        let db_path = data_dir.join("pgpad").join("pgpad.db");

        dbg!(&db_path);

        let storage = Storage::new(db_path)?;

        Ok(Self {
            connections: DashMap::new(),
            storage,
        })
    }
}

#[allow(clippy::missing_panics_doc)]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = match AppState::new() {
        Ok(app_state) => app_state,
        Err(e) => {
            eprintln!("Error initializing app state: {}", e);
            std::process::exit(1);
        }
    };

    tauri::Builder::default()
        .manage(app_state)
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
            postgres::commands::remove_connection,
            postgres::commands::initialize_connections,
            postgres::commands::save_query_to_history,
            postgres::commands::get_query_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
