use serde::{Deserialize, Serialize};

use crate::canvas::Canvas;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum ThemePreference {
    #[default]
    Dark,
    Light,
}

impl ThemePreference {
    pub fn egui_theme(self) -> eframe::egui::Theme {
        match self {
            Self::Dark => eframe::egui::Theme::Dark,
            Self::Light => eframe::egui::Theme::Light,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum NewTabFocusPreference {
    #[default]
    KeepCurrent,
    SwitchToNewTab,
}

/// Project configuration shared live across all open new-tab pages.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct AppState {
    pub canvases: Vec<Canvas>,
    pub next_canvas_id: usize,
    pub new_tab_focus: NewTabFocusPreference,
    pub tab_group_enhancement_enabled: bool,
    pub keep_newtab_rightmost: bool,
    pub reuse_newtab: bool,
    pub theme: ThemePreference,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            canvases: vec![Canvas::new(1, "Canvas 1".to_owned())],
            next_canvas_id: 2,
            new_tab_focus: NewTabFocusPreference::KeepCurrent,
            tab_group_enhancement_enabled: false,
            keep_newtab_rightmost: false,
            reuse_newtab: false,
            theme: ThemePreference::Dark,
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_canvas(&mut self) -> usize {
        let id = self.next_canvas_id;
        let sequence = (1..)
            .find(|number| {
                let candidate = format!("Canvas {number}");
                !self.canvases.iter().any(|canvas| canvas.name == candidate)
            })
            .unwrap_or(self.canvases.len() + 1);
        self.next_canvas_id += 1;
        self.canvases
            .push(Canvas::new(id, format!("Canvas {sequence}")));
        id
    }

    pub fn close_canvas(&mut self, index: usize) -> bool {
        if self.canvases.len() <= 1 || index >= self.canvases.len() {
            return false;
        }
        self.canvases.remove(index);
        true
    }

    pub fn rename_canvas(&mut self, index: usize, new_name: &str) {
        if let Some(canvas) = self.canvases.get_mut(index) {
            let trimmed = new_name.trim();
            canvas.name = if trimmed.is_empty() {
                format!("Canvas {}", canvas.id)
            } else {
                trimmed.to_owned()
            };
        }
    }

    pub fn contains_canvas(&self, id: usize) -> bool {
        self.canvases.iter().any(|canvas| canvas.id == id)
    }

    pub fn sanitize(&mut self) {
        if self.canvases.is_empty() {
            self.canvases.push(Canvas::new(1, "Canvas 1".to_owned()));
            self.next_canvas_id = 2;
            return;
        }

        let max_id = self
            .canvases
            .iter()
            .map(|canvas| canvas.id)
            .max()
            .unwrap_or(0);
        if self.next_canvas_id <= max_id {
            self.next_canvas_id = max_id + 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_contains_only_global_project_configuration() {
        let state = AppState::default();
        assert_eq!(state.canvases.len(), 1);
        assert_eq!(state.canvases[0].name, "Canvas 1");
        assert_eq!(state.new_tab_focus, NewTabFocusPreference::KeepCurrent);
        assert!(!state.tab_group_enhancement_enabled);
        assert!(!state.keep_newtab_rightmost);
        assert!(!state.reuse_newtab);
        assert_eq!(state.theme, ThemePreference::Dark);
    }

    #[test]
    fn older_state_defaults_to_keeping_the_current_tab() {
        let state: AppState =
            ron::from_str("(canvases:[(id:1,name:\"Canvas 1\")],next_canvas_id:2,theme:Dark)")
                .unwrap();

        assert_eq!(state.new_tab_focus, NewTabFocusPreference::KeepCurrent);
        assert!(!state.keep_newtab_rightmost);
        assert!(!state.reuse_newtab);
    }

    #[test]
    fn newtab_position_preference_survives_serialization() {
        let state = AppState {
            keep_newtab_rightmost: true,
            reuse_newtab: true,
            ..AppState::default()
        };
        let saved = ron::ser::to_string(&state).unwrap();
        let restored: AppState = ron::from_str(&saved).unwrap();
        assert!(restored.keep_newtab_rightmost);
        assert!(restored.reuse_newtab);
    }

    #[test]
    fn legacy_tab_group_match_level_is_ignored() {
        let state: AppState = ron::from_str(
            "(canvases:[(id:1,name:\"Canvas 1\")],next_canvas_id:2,tab_group_url_match_level:Domain,theme:Dark)",
        )
        .unwrap();

        assert_eq!(state, AppState::default());
    }

    #[test]
    fn new_tab_focus_preference_round_trips() {
        let state = AppState {
            new_tab_focus: NewTabFocusPreference::SwitchToNewTab,
            ..Default::default()
        };

        let serialized = ron::ser::to_string(&state).unwrap();
        assert_eq!(ron::from_str::<AppState>(&serialized).unwrap(), state);
    }

    #[test]
    fn add_canvas_increments_id_without_selecting_it_globally() {
        let mut state = AppState::default();
        let new_id = state.add_canvas();
        assert_eq!(new_id, 2);
        assert_eq!(state.next_canvas_id, 3);
        assert_eq!(state.canvases[1].name, "Canvas 2");
    }

    #[test]
    fn close_last_canvas_is_prevented() {
        let mut state = AppState::default();
        assert!(!state.close_canvas(0));
    }

    #[test]
    fn sanitize_repairs_empty_and_invalid_id_state() {
        let mut state = AppState {
            canvases: vec![Canvas::new(4, "Work".to_owned())],
            next_canvas_id: 1,
            ..Default::default()
        };
        state.sanitize();
        assert_eq!(state.next_canvas_id, 5);

        state.canvases.clear();
        state.sanitize();
        assert_eq!(state, AppState::default());
    }
}
