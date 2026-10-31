use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use eframe::egui;

use super::tree_row;
use crate::navigation;
use crate::state::NewTabFocusPreference;

#[cfg(any(target_arch = "wasm32", test))]
const FAVICON_REQUEST_SIZE: u32 = 32;
const FAVICON_DISPLAY_SIZE: f32 = 13.0;

enum FaviconEntry {
    Loading,
    #[cfg(target_arch = "wasm32")]
    Ready(egui::TextureHandle),
    Unavailable,
}

#[derive(Clone, Default)]
pub(crate) struct FaviconStore {
    entries: Arc<Mutex<HashMap<String, FaviconEntry>>>,
}

impl FaviconStore {
    pub(crate) fn link_to(
        &self,
        ui: &mut egui::Ui,
        page_url: &str,
        depth: usize,
        label: impl Into<egui::WidgetText>,
        new_tab_focus: NewTabFocusPreference,
    ) -> egui::Response {
        let response = tree_row(ui, depth, egui::Sense::click(), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            self.show(ui, page_url);
            ui.add(egui::Label::new(label).sense(egui::Sense::hover()));
        })
        .response;

        navigation::open_from_response(ui, &response, page_url, new_tab_focus);

        response
    }

    pub(crate) fn show(&self, ui: &mut egui::Ui, page_url: &str) -> egui::Response {
        let texture = self.texture(page_url, ui.ctx());
        let size = egui::vec2(FAVICON_DISPLAY_SIZE, FAVICON_DISPLAY_SIZE);

        if let Some(texture) = texture {
            ui.add(egui::Image::new((texture.id(), size)).fit_to_exact_size(size))
        } else {
            ui.add_sized(size, egui::Label::new("󰌹"))
        }
    }

    pub(crate) fn retry_unavailable(&self) {
        self.entries
            .lock()
            .expect("favicon cache lock poisoned")
            .retain(|_, entry| !matches!(entry, FaviconEntry::Unavailable));
    }

    fn texture(&self, page_url: &str, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if !is_supported_page_url(page_url) {
            return None;
        }

        {
            let entries = self.entries.lock().expect("favicon cache lock poisoned");
            match entries.get(page_url) {
                #[cfg(target_arch = "wasm32")]
                Some(FaviconEntry::Ready(texture)) => return Some(texture.clone()),
                Some(FaviconEntry::Loading | FaviconEntry::Unavailable) => return None,
                None => {}
            }
        }

        if self.begin_load(page_url) {
            self.load(page_url.to_owned(), ctx.clone());
        }
        None
    }

    fn begin_load(&self, page_url: &str) -> bool {
        let mut entries = self.entries.lock().expect("favicon cache lock poisoned");
        if entries.contains_key(page_url) {
            return false;
        }
        entries.insert(page_url.to_owned(), FaviconEntry::Loading);
        true
    }

    #[cfg(target_arch = "wasm32")]
    fn load(&self, page_url: String, ctx: egui::Context) {
        let entries = Arc::clone(&self.entries);
        wasm_bindgen_futures::spawn_local(async move {
            let entry = match load_chrome_favicon(&page_url, FAVICON_REQUEST_SIZE).await {
                Ok(value) => match favicon_image(value) {
                    Ok(image) => FaviconEntry::Ready(ctx.load_texture(
                        format!("favicon:{page_url}"),
                        image,
                        egui::TextureOptions::LINEAR,
                    )),
                    Err(message) => {
                        log::warn!("Unable to decode favicon for {page_url}: {message}");
                        FaviconEntry::Unavailable
                    }
                },
                Err(error) => {
                    log::warn!("Unable to load favicon for {page_url}: {error:?}");
                    FaviconEntry::Unavailable
                }
            };

            entries
                .lock()
                .expect("favicon cache lock poisoned")
                .insert(page_url, entry);
            ctx.request_repaint();
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load(&self, page_url: String, ctx: egui::Context) {
        self.entries
            .lock()
            .expect("favicon cache lock poisoned")
            .insert(page_url, FaviconEntry::Unavailable);
        ctx.request_repaint();
    }
}

fn is_supported_page_url(page_url: &str) -> bool {
    page_url.starts_with("https://") || page_url.starts_with("http://")
}

#[cfg(any(target_arch = "wasm32", test))]
fn valid_pixel_buffer(width: u32, height: u32, pixel_count: usize) -> bool {
    width > 0
        && height > 0
        && width <= FAVICON_REQUEST_SIZE
        && height <= FAVICON_REQUEST_SIZE
        && width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4))
            .is_some_and(|expected| expected as usize == pixel_count)
}

#[cfg(target_arch = "wasm32")]
fn favicon_image(value: wasm_bindgen::JsValue) -> Result<egui::ColorImage, String> {
    use wasm_bindgen::JsValue;

    let property = |name: &str| {
        js_sys::Reflect::get(&value, &JsValue::from_str(name))
            .map_err(|_| format!("missing {name}"))
    };
    let width = property("width")?
        .as_f64()
        .filter(|value| value.fract() == 0.0 && *value >= 0.0)
        .ok_or_else(|| "invalid width".to_owned())? as u32;
    let height = property("height")?
        .as_f64()
        .filter(|value| value.fract() == 0.0 && *value >= 0.0)
        .ok_or_else(|| "invalid height".to_owned())? as u32;
    let pixels = js_sys::Uint8ClampedArray::new(&property("pixels")?).to_vec();

    if !valid_pixel_buffer(width, height, pixels.len()) {
        return Err("invalid RGBA pixel buffer".to_owned());
    }

    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [width as usize, height as usize],
        &pixels,
    ))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(catch, js_name = __newtabs_load_favicon)]
    async fn load_chrome_favicon(
        page_url: &str,
        size: u32,
    ) -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_http_pages_request_favicons() {
        assert!(is_supported_page_url("https://example.com/page"));
        assert!(is_supported_page_url("http://example.com"));
        assert!(!is_supported_page_url("chrome://settings"));
        assert!(!is_supported_page_url("file:///tmp/page.html"));
        assert!(!is_supported_page_url("not a URL"));
    }

    #[test]
    fn repeated_url_only_begins_one_load() {
        let store = FaviconStore::default();

        assert!(store.begin_load("https://example.com"));
        assert!(!store.begin_load("https://example.com"));
    }

    #[test]
    fn indented_link_rows_fill_the_root_width() {
        let ctx = egui::Context::default();
        let store = FaviconStore::default();
        let mut available_width = 0.0;
        let mut row_width = 0.0;

        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            available_width = ui.available_width();
            row_width = store
                .link_to(
                    ui,
                    "https://example.com",
                    1,
                    "Example",
                    NewTabFocusPreference::KeepCurrent,
                )
                .rect
                .width();
        });
        output.textures_delta.clear();

        assert_eq!(row_width, available_width);
    }

    #[test]
    fn rgba_buffer_must_match_bounded_dimensions() {
        assert!(valid_pixel_buffer(32, 32, 32 * 32 * 4));
        assert!(!valid_pixel_buffer(0, 32, 0));
        assert!(!valid_pixel_buffer(33, 32, 33 * 32 * 4));
        assert!(!valid_pixel_buffer(16, 16, 16 * 16 * 3));
    }
}
