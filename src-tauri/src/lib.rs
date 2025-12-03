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
    pub storage: Arc<Storage>,
    pub stmt_manager: StatementManager,
}

impl AppState {
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_dir().expect("Failed to get data directory");
        let db_path = data_dir.join("pgpad").join("pgpad.db");

        let storage = Arc::new(Storage::new(db_path)?);

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
            let default = if cfg!(debug_assertions) {
                "trace,winit=error,tao=error,wry=error,tauri_runtime_wry=error,tokio_postgres=info,sqlparser=info,rustls=info"
            } else {
                "info,winit=error,tao=error,wry=error,tauri_runtime_wry=error,tokio_postgres=warn,sqlparser=warn,rustls=warn"
            };
            let _ = tracing_log::LogTracer::init();
            if cfg!(debug_assertions) {
                let subscriber = tracing_subscriber::fmt()
                    .with_env_filter(tracing_subscriber::EnvFilter::new(default))
                    .finish();
                let _ = tracing::subscriber::set_global_default(subscriber);
            } else {
                let subscriber = tracing_subscriber::fmt()
                    .json()
                    .with_env_filter(tracing_subscriber::EnvFilter::new(default))
                    .finish();
                let _ = tracing::subscriber::set_global_default(subscriber);
            }

            init::build_window(app)?;
            init::build_menu(app)?;

            let handle = app.handle();
            let monitor = ConnectionMonitor::new(handle.clone());
            handle.manage(monitor);
            // Apply storage cipher settings from app settings at startup
            if let Some(state) = app.try_state::<AppState>() {
                let _ = state.storage.apply_storage_cipher_settings_current();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            database::commands::test_connection,
            database::commands::add_connection,
            database::commands::update_connection,
            database::commands::connect_to_database,
            database::commands::disconnect_from_database,
            database::commands::submit_query,
            database::commands::wait_until_renderable,
            database::commands::fetch_page,
            database::commands::get_query_status,
            database::commands::get_page_count,
            database::commands::get_columns,
            database::commands::cancel_query,
            database::commands::get_connections,
            database::commands::remove_connection,
            database::commands::initialize_connections,
            database::commands::save_query_to_history,
            database::commands::get_query_history,
            database::commands::get_database_schema,
            database::commands::set_reconnect_settings,
            database::commands::get_reconnect_settings,
            database::commands::set_variant_settings,
            database::commands::get_variant_settings,
            database::commands::get_mssql_check_constraints,
            database::commands::get_mssql_unique_index_included_columns,
            database::commands::get_postgres_indexes,
            database::commands::get_postgres_index_columns,
            database::commands::get_postgres_constraints,
            database::commands::get_postgres_check_constraints,
            database::commands::get_postgres_triggers,
            database::commands::get_postgres_routines,
            database::commands::get_postgres_views,
            database::commands::get_postgres_view_definitions,
            database::commands::get_postgres_foreign_keys,
            database::commands::get_sqlite_indexes,
            database::commands::get_sqlite_index_columns,
            database::commands::get_sqlite_constraints,
            database::commands::get_sqlite_triggers,
            database::commands::get_sqlite_routines,
            database::commands::get_sqlite_views,
            database::commands::get_sqlite_view_definitions,
            database::commands::get_sqlite_foreign_keys,
            database::commands::get_duckdb_indexes,
            database::commands::get_duckdb_index_columns,
            database::commands::get_duckdb_constraints,
            database::commands::get_duckdb_routines,
            database::commands::get_duckdb_views,
            database::commands::get_duckdb_view_definitions,
            database::commands::get_duckdb_foreign_keys,
            database::commands::get_oracle_settings,
            database::commands::set_oracle_settings,
            database::commands::get_oracle_indexes,
            database::commands::get_mssql_indexes,
            database::commands::get_mssql_constraints,
            database::commands::get_mssql_triggers,
            database::commands::get_mssql_routines,
            database::commands::get_mssql_views,
            database::commands::get_mssql_index_columns,
            database::commands::get_mssql_trigger_events,
            database::commands::get_mssql_routine_parameters,
            database::commands::get_mssql_foreign_keys,
            database::commands::get_mssql_view_definitions,
            database::commands::cancel_mssql,
            database::commands::cancel_and_reconnect_mssql,
            database::commands::get_mssql_variant_base_type,
            database::commands::get_mssql_unique_index_included_columns,
            database::commands::save_script,
            database::commands::update_script,
            database::commands::get_scripts,
            database::commands::delete_script,
            database::commands::save_session_state,
            database::commands::get_session_state,
            database::commands::apply_storage_cipher_settings,
            database::commands::storage_maintenance,
            database::commands::rotate_storage_key,
            database::commands::get_encryption_settings,
            database::commands::set_encryption_settings,
            window::commands::minimize_window,
            window::commands::maximize_window,
            window::commands::close_window,
            window::commands::open_sqlite_db,
            window::commands::is_sqlcipher_encrypted,
            window::commands::open_sqlcipher_db,
            window::commands::save_sqlite_db,
            window::commands::open_duckdb_db,
            window::commands::save_duckdb_db,
            window::commands::pick_ca_cert,
            window::commands::pick_wallet_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
