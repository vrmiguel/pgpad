use tauri::WebviewWindowBuilder;

fn init_script() -> String {
    format!(
        r#"
        console.log(`I am in the init script, window.location.origin: ${{window.location.origin}}`);
        if (window.location.origin === 'http://localhost:1420') {{
            window.__PGPAD_INTERNAL__ = {{ platform: "{}" }};
            console.log("window.__PGPAD_INTERNAL__: ", window.__PGPAD_INTERNAL__);
        }}
    "#,
        std::env::consts::OS
    )
}

pub fn build_window(app: &tauri::App) -> tauri::Result<()> {
    let cfg = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .expect("main window config missing");

    WebviewWindowBuilder::from_config(app.handle(), cfg)?
        .initialization_script(init_script())
        .prevent_overflow()
        .build()?;
    Ok(())
}
