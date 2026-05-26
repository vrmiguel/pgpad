use pgpad_native::app::App;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 860.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("PgPad Native"),
        ..Default::default()
    };

    eframe::run_native(
        "PgPad Native",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::{wasm_bindgen::JsCast as _, web_sys};

    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");
        let canvas = document
            .get_element_by_id("pgpad-native-canvas")
            .expect("Missing pgpad-native-canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("pgpad-native-canvas was not a canvas");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(App::new(cc)))),
            )
            .await;

        if let Some(loading) = document.get_element_by_id("loading") {
            match start_result {
                Ok(()) => loading.remove(),
                Err(err) => {
                    loading.set_inner_html("The app crashed. See the developer console.");
                    panic!("Failed to start eframe: {err:?}");
                }
            }
        }
    });
}
