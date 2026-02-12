mod credentials;
mod database;
mod error;
mod init;
mod storage;
mod utils;
mod window;

use std::sync::Arc;

use dashmap::DashMap;
use tauri::Manager;
use uuid::Uuid;

use crate::{
    database::{
        stmt_manager::StatementManager,
        types::{DatabaseConnection, DatabaseSchema},
        ConnectionMonitor,
    },
    storage::Storage,
};
pub use error::{Error, Result};

#[derive(Debug)]
pub struct AppState {
    pub connections: DashMap<Uuid, DatabaseConnection>,
    pub schemas: DashMap<Uuid, Arc<DatabaseSchema>>,
    /// SQLite database for application data
    pub storage: Storage,
    pub stmt_manager: StatementManager,
}

impl AppState {
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_dir().expect("Failed to get data directory");
        let db_path = data_dir.join("pgpad").join("pgpad.db");

        let storage = Storage::new(db_path)?;

        Ok(Self {
            connections: DashMap::new(),
            schemas: DashMap::new(),
            storage,
            stmt_manager: StatementManager::new(),
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
                env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(
                    "trace,tokio_postgres=info,tao=info,sqlparser=info,rustls=info",
                ))
                .init();
            }

            init::build_window(app)?;
            init::build_menu(app)?;

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
            database::commands::submit_query,
            database::commands::is_query_read_only,
            database::commands::wait_until_renderable,
            database::commands::fetch_page,
            database::commands::get_query_status,
            database::commands::get_page_count,
            database::commands::get_columns,
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
            database::commands::format_sql,
            window::commands::minimize_window,
            window::commands::maximize_window,
            window::commands::close_window,
            window::commands::open_sqlite_db,
            window::commands::save_sqlite_db,
            window::commands::pick_ca_cert,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
