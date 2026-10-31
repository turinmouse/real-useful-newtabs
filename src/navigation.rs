use eframe::egui;

use crate::state::NewTabFocusPreference;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NavigationTarget {
    CurrentTab,
    NewTab { active: bool },
}

pub(crate) fn target_for_click(
    command_clicked: bool,
    preference: NewTabFocusPreference,
) -> NavigationTarget {
    if command_clicked {
        NavigationTarget::NewTab {
            active: preference == NewTabFocusPreference::SwitchToNewTab,
        }
    } else {
        NavigationTarget::CurrentTab
    }
}

pub(crate) fn open_from_response(
    ui: &egui::Ui,
    response: &egui::Response,
    url: &str,
    preference: NewTabFocusPreference,
) {
    if response.clicked() {
        let command_clicked = ui.input(|input| input.modifiers.command);
        open_url(ui.ctx(), url, target_for_click(command_clicked, preference));
    }
}

pub(crate) fn open_url(ctx: &egui::Context, url: &str, target: NavigationTarget) {
    #[cfg(target_arch = "wasm32")]
    {
        let (new_tab, active) = match target {
            NavigationTarget::CurrentTab => (false, false),
            NavigationTarget::NewTab { active } => (true, active),
        };
        open_web_url(url, new_tab, active);
        let _ = ctx;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let open_url = match target {
            NavigationTarget::CurrentTab => egui::OpenUrl::same_tab(url),
            NavigationTarget::NewTab { .. } => egui::OpenUrl::new_tab(url),
        };
        ctx.open_url(open_url);
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = __newtabs_open_url)]
    fn open_web_url(url: &str, new_tab: bool, active: bool);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_click_reuses_the_current_tab() {
        assert_eq!(
            target_for_click(false, NewTabFocusPreference::SwitchToNewTab),
            NavigationTarget::CurrentTab
        );
    }

    #[test]
    fn modified_click_opens_a_background_tab_by_default() {
        assert_eq!(
            target_for_click(true, NewTabFocusPreference::KeepCurrent),
            NavigationTarget::NewTab { active: false }
        );
    }

    #[test]
    fn modified_click_can_activate_the_new_tab() {
        assert_eq!(
            target_for_click(true, NewTabFocusPreference::SwitchToNewTab),
            NavigationTarget::NewTab { active: true }
        );
    }
}
