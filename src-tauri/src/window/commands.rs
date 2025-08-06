use anyhow::Context;
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
