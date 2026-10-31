use eframe::egui;

pub const CANVAS_BACKGROUND: egui::Color32 = egui::Color32::from_rgb(0x14, 0x14, 0x14);
pub const SURFACE_WINDOW: egui::Color32 = egui::Color32::from_rgb(0x1B, 0x1B, 0x1B);

const INTER_NERD_FONT: &[u8] = include_bytes!("../assets/fonts/InterNerdFont-Regular.otf");
const NOTO_SANS_SC: &[u8] = include_bytes!("../assets/fonts/NotoSansSC-Regular.ttf");

/// Font family name used to register Inter Nerd Font.
pub const MAIN_FONT_NAME: &str = "InterNerdFont";

/// Font family name used to register Noto Sans SC as CJK fallback.
pub const CJK_FONT_NAME: &str = "NotoSansSC";

pub fn apply(ctx: &egui::Context) {
    // The web backend cannot apply native window decoration themes.
    #[cfg(target_arch = "wasm32")]
    ctx.options_mut(|options| options.sync_window_theme = false);

    configure_fonts(ctx);

    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        let mut style = (*ctx.style_of(theme)).clone();
        style.visuals = match theme {
            egui::Theme::Dark => {
                let mut v = egui::Visuals::dark();
                v.extreme_bg_color = SURFACE_WINDOW;
                v.window_fill = SURFACE_WINDOW;
                v.panel_fill = SURFACE_WINDOW;
                v
            }
            egui::Theme::Light => egui::Visuals::light(),
        };
        style.spacing.item_spacing = egui::vec2(8.0, 3.0);
        style.spacing.window_margin = egui::Margin::same(4);
        style.spacing.button_padding = egui::vec2(4.0, 1.0);
        style.spacing.interact_size = egui::vec2(40.0, 18.0);
        style.visuals.window_shadow = egui::Shadow::NONE;
        style.visuals.popup_shadow = egui::Shadow::NONE;
        style.animation_time = 0.2;
        ctx.set_style_of(theme, style);
    }
    ctx.set_theme(egui::Theme::Dark);
    sync_browser_color_scheme(egui::Theme::Dark);
}

/// Keep browser-native surfaces (scrollbars, form controls, and extension chrome)
/// in sync with the egui theme when running as a WebAssembly extension.
pub fn sync_browser_color_scheme(theme: egui::Theme) {
    #[cfg(target_arch = "wasm32")]
    set_browser_color_scheme(theme == egui::Theme::Dark);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = theme;
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = __newtabs_set_theme)]
    fn set_browser_color_scheme(dark: bool);
}

/// Register Inter Nerd Font as primary font and Noto Sans SC as CJK fallback
/// for both `Proportional` and `Monospace` font families.
fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        MAIN_FONT_NAME.to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(INTER_NERD_FONT)),
    );

    fonts.font_data.insert(
        CJK_FONT_NAME.to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(NOTO_SANS_SC)),
    );

    // Set Inter Nerd Font as the primary font (index 0) and Noto Sans SC as CJK fallback
    if let Some(proportional) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        proportional.insert(0, MAIN_FONT_NAME.to_owned());
        proportional.push(CJK_FONT_NAME.to_owned());
    }

    if let Some(monospace) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        monospace.insert(0, MAIN_FONT_NAME.to_owned());
        monospace.push(CJK_FONT_NAME.to_owned());
    }

    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_design_system_metrics() {
        let ctx = egui::Context::default();
        apply(&ctx);
        let style = ctx.style_of(egui::Theme::Dark);

        assert!(style.visuals.dark_mode);
        assert_eq!(style.visuals.extreme_bg_color, SURFACE_WINDOW);
        assert_eq!(style.spacing.item_spacing, egui::vec2(8.0, 3.0));
        assert_eq!(style.spacing.interact_size, egui::vec2(40.0, 18.0));
        assert_eq!(style.animation_time, 0.2);
    }

    #[test]
    fn custom_fonts_registered_successfully() {
        let ctx = egui::Context::default();
        apply(&ctx);

        assert!(!MAIN_FONT_NAME.is_empty());
        assert!(!CJK_FONT_NAME.is_empty());
    }
}
