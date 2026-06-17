mod database_commands;
mod error;
mod init;
mod window;

use pgpad_core::{AppState, Certificates, ConnectionMonitor};
use tauri::{Emitter, EventTarget, Manager};
use tokio::sync::mpsc;
use uuid::Uuid;

fn app_db_path() -> std::path::PathBuf {
    dirs::data_dir()
        .expect("Failed to get data directory")
        .join("pgpad")
        .join("pgpad.db")
}

fn app_state() -> AppState {
    match AppState::new(app_db_path()) {
        Ok(app_state) => app_state,
        Err(e) => {
            eprintln!("Error initializing app state: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_dropped_connections(
    handle: tauri::AppHandle,
    mut dropped_connections: mpsc::UnboundedReceiver<Uuid>,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(connection_id) = dropped_connections.recv().await {
            let Some(state) = handle.try_state::<AppState>() else {
                log::error!("No state manager found!");
                continue;
            };

            if !state.mark_disconnected(connection_id) {
                log::error!("Connection {connection_id} not found!");
                continue;
            }

            if let Err(e) = handle.emit_to(EventTarget::App, "end-of-connection", connection_id) {
                log::error!("Error emitting end-of-connection event: {e}");
                continue;
            }

            log::info!("End-of-connection event emitted for connection {connection_id}");
        }
    });
}

fn handle_query_events(handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let Some(state) = handle.try_state::<AppState>() else {
            log::error!("No state manager found!");
            return;
        };

        let mut query_events_receiver = state.stmt_manager.query_events_receiver();

        loop {
            match query_events_receiver.recv().await {
                Ok(event) => {
                    if let Err(e) = handle.emit_to(EventTarget::App, "query-event", event) {
                        log::error!("Error emitting query event: {e}");
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    log::warn!("Query event listener lagged and skipped {skipped} events");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

#[allow(clippy::missing_panics_doc)]
pub fn builder() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .manage(app_state())
        .manage(Certificates::new())
        .setup(|app| {
            if cfg!(debug_assertions) {
                env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(
                    "trace,tokio_postgres=info,tao=info,sqlparser=info,rustls=info",
                ))
                .init();
            }

            init::build_window(app)?;
            init::build_menu(app)?;

            let certificates = app.state::<Certificates>().inner().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = certificates.read().await {
                    log::warn!("Failed to preload certificates: {e}");
                }
            });

            let handle = app.handle();
            let (connection_monitor, dropped_connections) = ConnectionMonitor::new();
            handle_dropped_connections(handle.clone(), dropped_connections);
            handle_query_events(handle.clone());
            handle.manage(connection_monitor);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            database_commands::test_connection,
            database_commands::add_connection,
            database_commands::update_connection,
            database_commands::connect_to_database,
            database_commands::disconnect_from_database,
            database_commands::submit_query,
            database_commands::is_query_read_only,
            database_commands::wait_until_renderable,
            database_commands::fetch_page,
            database_commands::get_query_status,
            database_commands::get_page_count,
            database_commands::get_connections,
            database_commands::remove_connection,
            database_commands::initialize_connections,
            database_commands::save_query_to_history,
            database_commands::get_query_history,
            database_commands::get_database_schema,
            database_commands::save_script,
            database_commands::update_script,
            database_commands::get_scripts,
            database_commands::delete_script,
            database_commands::save_session_state,
            database_commands::get_session_state,
            database_commands::format_sql,
            database_commands::export_page,
            window::commands::minimize_window,
            window::commands::maximize_window,
            window::commands::close_window,
            window::commands::open_sqlite_db,
            window::commands::save_sqlite_db,
            window::commands::pick_ca_cert,
        ])
}

#[allow(clippy::missing_panics_doc)]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    builder()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
