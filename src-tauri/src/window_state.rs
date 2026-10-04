use pgpad_core::{storage::Storage, AppState};
use serde::{Deserialize, Serialize};
use tauri::{Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

const SETTING: &str = "window_state";

#[derive(Default, Deserialize, Serialize)]
struct WindowState {
    size: Option<(u32, u32)>,
    position: Option<(i32, i32)>,
    maximized: bool,
}

impl WindowState {
    fn load(storage: &Storage) -> anyhow::Result<Self> {
        let saved = storage.get_setting(SETTING)?;
        Ok(saved
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default())
    }
}

pub fn restore(window: &WebviewWindow) -> anyhow::Result<()> {
    let app_state = window.state::<AppState>();
    let saved = WindowState::load(&app_state.storage)?;

    if let Some((width, height)) = saved.size.filter(|(w, h)| *w > 0 && *h > 0) {
        window.set_size(PhysicalSize::new(width, height))?;
    }
    if let Some((x, y)) = saved.position {
        // Leave placement to the OS if the saved title bar is on a disconnected display.
        let reachable = window.available_monitors()?.iter().any(|monitor| {
            let area = monitor.work_area();
            let (x, y) = (i64::from(x), i64::from(y));
            x >= i64::from(area.position.x)
                && y >= i64::from(area.position.y)
                && x + 64 <= i64::from(area.position.x) + i64::from(area.size.width)
                && y + 32 <= i64::from(area.position.y) + i64::from(area.size.height)
        });
        if reachable {
            window.set_position(PhysicalPosition::new(x, y))?;
        }
    }
    if saved.maximized {
        window.maximize()?;
    }
    Ok(())
}

pub fn save(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(error) = save_window(&window) {
        log::warn!("Failed to save window state: {error}");
    }
}

fn save_window(window: &WebviewWindow) -> anyhow::Result<()> {
    let app_state = window.state::<AppState>();
    let mut saved = WindowState::load(&app_state.storage)?;

    // Keep the last saved normal bounds when closing in another window mode.
    if !window.is_minimized()? && !window.is_fullscreen()? {
        saved.maximized = window.is_maximized()?;
        if !saved.maximized {
            let size = window.inner_size()?;
            let position = window.outer_position()?;
            saved.size = Some((size.width, size.height));
            saved.position = Some((position.x, position.y));
        }
    }

    app_state
        .storage
        .set_setting(SETTING, &serde_json::to_string(&saved)?)?;
    Ok(())
}
