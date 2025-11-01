use anyhow::Context;
use rfd::AsyncFileDialog;
use tauri::Manager;

use crate::Error;

#[tauri::command]
pub async fn minimize_window(app: tauri::AppHandle) -> Result<(), Error> {
    app.get_webview_window("main")
        .context("Failed to get main window")?
        .minimize()
        .context("Failed to minimize window")?;

    Ok(())
}

#[tauri::command]
pub async fn maximize_window(app: tauri::AppHandle) -> Result<(), Error> {
    app.get_webview_window("main")
        .context("Failed to get main window")?
        .maximize()
        .context("Failed to maximize window")?;

    Ok(())
}

#[tauri::command]
pub async fn close_window(app: tauri::AppHandle) -> Result<(), Error> {
    app.get_webview_window("main")
        .context("Failed to get main window")?
        .close()
        .context("Failed to close window")?;

    Ok(())
}

#[tauri::command]
pub async fn open_sqlite_db(app: tauri::AppHandle) -> Result<Option<String>, Error> {
    let chosen_file = run_dialog(app, || {
        AsyncFileDialog::new()
            .set_title("Pick a SQLite database file")
            .add_filter("SQLite database", &["db", "sqlite", "sqlite3"])
            .pick_file()
    })
    .await?
    .map(|file| file.path().to_string_lossy().to_string());

    Ok(chosen_file)
}

#[tauri::command]
pub async fn save_sqlite_db(app: tauri::AppHandle) -> Result<Option<String>, Error> {
    let chosen_file = run_dialog(app, || {
        AsyncFileDialog::new()
            .set_title("Create a new SQLite database file")
            .save_file()
    })
    .await?
    .map(|file| file.path().to_string_lossy().to_string());

    Ok(chosen_file)
}

async fn run_dialog<F, Fut, T>(app: tauri::AppHandle, make_future: F) -> Result<Option<T>, Error>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Option<T>> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel();

    app.run_on_main_thread(move || {
        // According to the rfd docs, we have to _spawn_ the dialog on the main thread,
        // but we can await it in any other thread.
        let fut = make_future();

        tauri::async_runtime::spawn(async move {
            let _ = tx.send(fut.await);
        });
    })?;

    rx.await
        .map_err(|_| Error::Any(anyhow::anyhow!("Failed to receive dialog result")))
}
