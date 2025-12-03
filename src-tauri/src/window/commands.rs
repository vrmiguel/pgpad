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
pub async fn open_sqlcipher_db(app: tauri::AppHandle) -> Result<Option<String>, Error> {
    let chosen_file = run_dialog(app, || {
        AsyncFileDialog::new()
            .set_title("Open a SQLCipher database file")
            .add_filter("SQLCipher database", &["db", "sqlite", "sqlite3"])
            .pick_file()
    })
    .await?
    .map(|file| file.path().to_string_lossy().to_string());

    Ok(chosen_file)
}

#[tauri::command]
pub async fn open_duckdb_db(app: tauri::AppHandle) -> Result<Option<String>, Error> {
    let chosen_file = run_dialog(app, || {
        AsyncFileDialog::new()
            .set_title("Pick a DuckDB database file")
            .add_filter("DuckDB database", &["duckdb", "db"])
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

#[tauri::command]
pub async fn save_duckdb_db(app: tauri::AppHandle) -> Result<Option<String>, Error> {
    let chosen_file = run_dialog(app, || {
        AsyncFileDialog::new()
            .set_title("Create a new DuckDB database file")
            .add_filter("DuckDB database", &["duckdb", "db"])
            .save_file()
    })
    .await?
    .map(|file| file.path().to_string_lossy().to_string());

    Ok(chosen_file)
}
#[tauri::command]
pub async fn pick_ca_cert(app: tauri::AppHandle) -> Result<Option<String>, Error> {
    let chosen_file = run_dialog(app, || {
        AsyncFileDialog::new()
            .set_title("Pick a certificate file")
            .add_filter("Certificate files", &["pem", "crt", "cer", "ca-bundle"])
            .pick_file()
    })
    .await?
    .map(|file| file.path().to_string_lossy().to_string());

    Ok(chosen_file)
}

#[tauri::command]
pub async fn pick_wallet_dir(app: tauri::AppHandle) -> Result<Option<String>, Error> {
    let chosen_dir = run_dialog(app, || {
        AsyncFileDialog::new()
            .set_title("Pick Oracle wallet directory (TNS_ADMIN)")
            .pick_folder()
    })
    .await?
    .map(|file| file.path().to_string_lossy().to_string());

    Ok(chosen_dir)
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

#[tauri::command]
pub async fn is_sqlcipher_encrypted(file_path: String) -> Result<bool, Error> {
    let mut header = [0u8; 16];
    let fp = file_path;
    let res = tokio::task::spawn_blocking(move || {
        use std::io::Read;
        let path = std::path::Path::new(&fp);
        let mut f = std::fs::File::open(path)?;
        let _ = f.read(&mut header)?;
        let magic = b"SQLite format 3\x00";
        Ok::<bool, std::io::Error>(&header != magic)
    })
    .await
    .map_err(|e| Error::Any(anyhow::anyhow!(e.to_string())))?;
    match res {
        Ok(b) => Ok(b),
        Err(e) => Err(Error::Any(anyhow::anyhow!(e.to_string()))),
    }
}
