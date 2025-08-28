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
pub async fn open_file_dialog(app: tauri::AppHandle) -> Result<Option<String>, Error> {
    log::info!("Opening file dialog");

    // rfd recommends running the dialog on the main thread, for compatibility reasons.
    let dialog_future = {
        let (tx, rx) = tokio::sync::oneshot::channel();

        app.run_on_main_thread(move || {
            let dialog = AsyncFileDialog::new();
            let handle = dialog.pick_file();
            tauri::async_runtime::spawn(async move {
                let result = handle.await;
                let _ = tx.send(result);
            });
        })?;

        rx
    };

    let chosen_file = dialog_future
        .await
        .map_err(|_| anyhow::anyhow!("Failed to receive file dialog result"))?;
    let chosen_path = chosen_file.map(|file| file.path().to_string_lossy().to_string());

    log::info!("Chosen path: {:?}", chosen_path);

    Ok(chosen_path)
}
