#[cfg(target_os = "macos")]
use tauri::menu::AboutMetadata;
use tauri::menu::MenuItem;
use tauri::{
    menu::{Menu, PredefinedMenuItem, Submenu, WINDOW_SUBMENU_ID},
    AppHandle, Emitter, WebviewWindowBuilder,
};

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

    let window_builder = WebviewWindowBuilder::from_config(app.handle(), cfg)?
        .initialization_script(init_script())
        .prevent_overflow();

    #[cfg(target_os = "macos")]
    let window_builder = {
        use tauri::{utils::config::WindowEffectsConfig, window::Effect, LogicalPosition};

        window_builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .decorations(true)
            .effects(WindowEffectsConfig {
                effects: vec![Effect::WindowBackground],
                state: None,
                radius: Some(12.0),
                color: None,
            })
            .traffic_light_position(tauri::Position::Logical(LogicalPosition::new(16.0, 23.0)))
            .hidden_title(true)
    };

    window_builder.build()?;

    Ok(())
}

pub fn build_menu(app: &tauri::App) -> anyhow::Result<()> {
    let app_handle = app.handle();
    #[cfg(target_os = "macos")]
    let pkg_info = app_handle.package_info();
    #[cfg(target_os = "macos")]
    let about_metadata = {
        let config = app_handle.config();
        AboutMetadata {
            name: Some(pkg_info.name.clone()),
            version: Some(pkg_info.version.to_string()),
            copyright: config.bundle.copyright.clone(),
            authors: config.bundle.publisher.clone().map(|p| vec![p]),
            ..Default::default()
        }
    };

    let window_menu = Submenu::with_id_and_items(
        app_handle,
        WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app_handle, None)?,
            &PredefinedMenuItem::maximize(app_handle, None)?,
            #[cfg(target_os = "macos")]
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::close_window(app_handle, None)?,
        ],
    )?;

    let menu = Menu::with_items(
        app_handle,
        &[
            #[cfg(target_os = "macos")]
            &Submenu::with_items(
                app_handle,
                pkg_info.name.clone(),
                true,
                &[
                    &PredefinedMenuItem::about(app_handle, None, Some(about_metadata))?,
                    &PredefinedMenuItem::separator(app_handle)?,
                    &PredefinedMenuItem::services(app_handle, None)?,
                    &PredefinedMenuItem::separator(app_handle)?,
                    &PredefinedMenuItem::hide(app_handle, None)?,
                    &PredefinedMenuItem::hide_others(app_handle, None)?,
                    &PredefinedMenuItem::separator(app_handle)?,
                    &PredefinedMenuItem::quit(app_handle, None)?,
                ],
            )?,
            &Submenu::with_items(
                app_handle,
                "File",
                true,
                &[
                    &MenuItem::with_id(
                        app_handle,
                        "new_tab",
                        "New Tab",
                        true,
                        Some("CmdOrControl+N"),
                    )?,
                    &MenuItem::with_id(
                        app_handle,
                        "close_tab",
                        "Close Tab",
                        true,
                        Some("CmdOrControl+W"),
                    )?,
                ],
            )?,
            &Submenu::with_items(
                app_handle,
                "Edit",
                true,
                &[
                    &PredefinedMenuItem::undo(app_handle, None)?,
                    &PredefinedMenuItem::redo(app_handle, None)?,
                    &PredefinedMenuItem::separator(app_handle)?,
                    &PredefinedMenuItem::cut(app_handle, None)?,
                    &PredefinedMenuItem::copy(app_handle, None)?,
                    &PredefinedMenuItem::paste(app_handle, None)?,
                    &PredefinedMenuItem::select_all(app_handle, None)?,
                ],
            )?,
            #[cfg(target_os = "macos")]
            &Submenu::with_items(
                app_handle,
                "View",
                true,
                &[&PredefinedMenuItem::fullscreen(app_handle, None)?],
            )?,
            &window_menu,
        ],
    )?;

    app.set_menu(menu)?;
    app.on_menu_event(move |handle: &tauri::AppHandle, event| {
        let event = event.id().0.as_str();

        log::debug!("[on_menu_event][{event}] Event triggered");

        if let Err(err) = menu_event_handler(event, handle) {
            log::error!("[on_menu_event] [{event}] {:?}", err);
        }
    });

    Ok(())
}

fn menu_event_handler(event: &str, handle: &AppHandle) -> anyhow::Result<()> {
    match event {
        "new_tab" => {
            handle.emit("new_tab", ())?;
        }
        "close_tab" => {
            handle.emit("close_tab", ())?;
        }
        _ => {
            log::info!("Unexpected menu event: {}", event);
        }
    }
    Ok(())
}
