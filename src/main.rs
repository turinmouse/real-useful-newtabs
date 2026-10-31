#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("real-useful-newtabs")
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "real-useful-newtabs",
        native_options,
        Box::new(|cc| Ok(Box::new(real_useful_newtabs::NewTabsApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("browser window is unavailable")
            .document()
            .expect("browser document is unavailable");
        let canvas = document
            .get_element_by_id("newtabs_canvas")
            .expect("#newtabs_canvas is missing")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#newtabs_canvas is not a canvas element");

        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(real_useful_newtabs::NewTabsApp::new(cc)))),
            )
            .await;

        if let Some(status) = document.get_element_by_id("loading_status") {
            match result {
                Ok(()) => status.remove(),
                Err(error) => {
                    status.set_text_content(Some(
                        "real-useful-newtabs failed to start. Check the console.",
                    ));
                    panic!("failed to start real-useful-newtabs: {error:?}");
                }
            }
        }
    });
}
