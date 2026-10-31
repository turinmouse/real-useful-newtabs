use eframe::egui;

use super::SettingsTool;
use crate::canvas::WindowLayout;
use crate::state::{AppState, NewTabFocusPreference};

impl SettingsTool {
    pub fn navigation_item(ui: &mut egui::Ui, is_open: bool, open_handler: impl FnOnce()) {
        if ui
            .selectable_label(is_open, "⚙  Settings")
            .on_hover_text("Open settings")
            .clicked()
        {
            open_handler();
        }
    }

    pub fn show_window(
        ctx: &egui::Context,
        canvas_id: usize,
        window_layout: &mut WindowLayout,
        state: &mut AppState,
        rect: egui::Rect,
    ) {
        if !window_layout.open {
            return;
        }
        let window_id = egui::Id::new(("tiled_settings", 1_u8, canvas_id));
        let mut open = window_layout.open;
        let window = egui::Window::new(egui::RichText::new("Settings").size(14.0))
            .id(window_id)
            .open(&mut open);
        window_layout.apply_layout(window, rect).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("After opening a new tab:");
                ui.radio_value(
                    &mut state.new_tab_focus,
                    NewTabFocusPreference::KeepCurrent,
                    "Keep current",
                );
                ui.radio_value(
                    &mut state.new_tab_focus,
                    NewTabFocusPreference::SwitchToNewTab,
                    "Switch to new",
                );
            });
            ui.checkbox(&mut state.keep_newtab_rightmost, "Keep new tab pages on the far right")
                .on_hover_text("Keeps unpinned new tab pages at the right of each window. New tab pages are removed from tab groups.");
            ui.checkbox(&mut state.reuse_newtab, "Reuse existing new tab page")
                .on_hover_text("Opening a new tab switches to the most recently used new tab page in this window. Also applies to the + button.");
            ui.separator();
            ui.checkbox(&mut state.tab_group_enhancement_enabled, "Tab group enhancement");
        });
        window_layout.open = open;
    }
}
