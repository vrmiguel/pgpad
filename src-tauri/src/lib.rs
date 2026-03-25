#[allow(clippy::missing_panics_doc)]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    pgpad_core::builder()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
