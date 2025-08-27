use tauri::WebviewWindowBuilder;

fn init_script() -> String {
    format!(
        r#"
        console.log(`I am in the init script, window.location.origin: ${{window.location.origin}}`);
        if (window.location.origin === '{}') {{
            window.__PGPAD_INTERNAL__ = {{ platform: "{}" }};
            console.log("window.__PGPAD_INTERNAL__: ", window.__PGPAD_INTERNAL__);
        }}
    "#,
        if cfg!(debug_assertions) {
            "http://localhost:1420"
        } else {
            "tauri://localhost"
        },
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

    let mut window_builder = WebviewWindowBuilder::from_config(app.handle(), cfg)?
        .initialization_script(init_script())
        .prevent_overflow();

    #[cfg(target_os = "macos")]
    {
        use tauri::{utils::config::WindowEffectsConfig, window::Effect, LogicalPosition};
        window_builder = window_builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .decorations(true)
            .effects(WindowEffectsConfig {
                effects: vec![Effect::WindowBackground],
                state: None,
                radius: Some(12.0),
                color: None,
            })
            .traffic_light_position(tauri::Position::Logical(LogicalPosition::new(16.0, 23.0)))
            .hidden_title(true);
    }

    window_builder.build()?;

    Ok(())
}
