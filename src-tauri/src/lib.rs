mod credentials;
mod database;
mod error;
mod init;
mod storage;
mod utils;
mod window;

use dashmap::DashMap;
use tauri::Manager;
use uuid::Uuid;

use crate::{
    database::{types::DatabaseConnection, ConnectionMonitor},
    storage::Storage,
};
pub use error::{Error, Result};

#[derive(Debug)]
pub struct AppState {
    pub connections: DashMap<Uuid, DatabaseConnection>,
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
    let certificates = database::Certificates::new();

    tauri::Builder::default()
        .manage(app_state)
        .manage(certificates)
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            init::build_window(app)?;

            let handle = app.handle();
            let monitor = ConnectionMonitor::new(handle.clone());
            handle.manage(monitor);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            database::commands::test_connection,
            database::commands::add_connection,
            database::commands::update_connection,
            database::commands::connect_to_database,
            database::commands::disconnect_from_database,
            database::commands::execute_query_stream,
            database::commands::get_connections,
            database::commands::remove_connection,
            database::commands::initialize_connections,
            database::commands::save_query_to_history,
            database::commands::get_query_history,
            database::commands::get_database_schema,
            database::commands::save_script,
            database::commands::update_script,
            database::commands::get_scripts,
            database::commands::delete_script,
            database::commands::save_session_state,
            database::commands::get_session_state,
            window::commands::minimize_window,
            window::commands::maximize_window,
            window::commands::close_window,
            window::commands::open_file_dialog,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
