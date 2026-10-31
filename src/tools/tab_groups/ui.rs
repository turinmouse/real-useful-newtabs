use eframe::egui;

use super::{SavedTabGroup, SharedTabGroups, TabGroupState, TabGroupsTool};
use crate::canvas::WindowLayout;
use crate::state::NewTabFocusPreference;
use crate::tools::{FaviconStore, tree_row};

const TAB_GROUP_COLORS: [&str; 9] = [
    "grey", "blue", "red", "yellow", "green", "pink", "purple", "cyan", "orange",
];

enum TabGroupAction {
    Rename {
        id: String,
        title: String,
    },
    SetColor {
        id: String,
        color: String,
    },
    Delete {
        id: String,
    },
    AddRule {
        group_id: String,
        pattern: String,
    },
    UpdateRule {
        group_id: String,
        rule_id: String,
        pattern: String,
    },
    DeleteRule {
        group_id: String,
        rule_id: String,
    },
    DeletePage {
        group_id: String,
        url: String,
    },
}

impl TabGroupsTool {
    pub fn navigation_item(
        ui: &mut egui::Ui,
        is_open: bool,
        groups: &SharedTabGroups,
        open_handler: impl FnOnce(),
    ) {
        if ui
            .selectable_label(is_open, "󰓩  Tab Groups")
            .on_hover_text("Open saved tab groups")
            .clicked()
        {
            open_handler();
            if matches!(
                *groups.lock().expect("tab group state lock poisoned"),
                TabGroupState::Idle | TabGroupState::Error(_)
            ) {
                Self::load(groups, ui.ctx());
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn show_window(
        ui: &mut egui::Ui,
        canvas_id: usize,
        window_layout: &mut WindowLayout,
        groups: &SharedTabGroups,
        favicons: &FaviconStore,
        enabled: bool,
        new_tab_focus: NewTabFocusPreference,
        renaming: &mut Option<(String, String)>,
        rule_editing: &mut Option<(String, Option<String>, String)>,
        rect: egui::Rect,
    ) {
        if !window_layout.open {
            return;
        }
        if matches!(
            *groups.lock().expect("tab group state lock poisoned"),
            TabGroupState::Idle
        ) {
            Self::load(groups, ui.ctx());
        }

        let state = groups
            .lock()
            .expect("tab group state lock poisoned")
            .clone();
        let mut open = window_layout.open;
        let mut action = None;
        let mut retry = false;
        let window_id = egui::Id::new(("tiled_tab_groups", 1_u8, canvas_id));
        let search_id = egui::Id::new(("tab_groups_search", window_id));
        let mut search_query: String = ui
            .ctx()
            .data_mut(|data| data.get_temp(search_id))
            .unwrap_or_default();
        let window = egui::Window::new(egui::RichText::new("Tab Groups").size(14.0))
            .id(window_id)
            .open(&mut open);
        window_layout.apply_layout(window, rect).show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut search_query)
                    .hint_text("Search tab group URLs…")
                    .desired_width(f32::INFINITY),
            );
            ui.separator();

            if !enabled {
                ui.label(
                    egui::RichText::new("Enable Tab group enhancement in Settings")
                        .weak()
                        .italics(),
                );
                ui.separator();
            }

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| match &state {
                    TabGroupState::Idle => {
                        ui.label(egui::RichText::new("Open Tab Groups to load data").weak());
                    }
                    TabGroupState::Loading => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Loading saved tab groups…");
                        });
                    }
                    TabGroupState::Ready(saved) if saved.is_empty() => {
                        ui.label(egui::RichText::new("No saved tab groups").weak());
                    }
                    TabGroupState::Ready(saved) => {
                        let is_searching = !search_query.trim().is_empty();
                        let filtered = SavedTabGroup::filter_by_address(saved, &search_query);
                        let visible = if is_searching {
                            filtered.as_slice()
                        } else {
                            saved.as_slice()
                        };
                        if visible.is_empty() {
                            ui.label(
                                egui::RichText::new("No matching tab group records found").weak(),
                            );
                        }
                        for group in visible {
                            Self::group_node(
                                ui,
                                group,
                                is_searching,
                                favicons,
                                enabled,
                                new_tab_focus,
                                renaming,
                                rule_editing,
                                &mut action,
                            );
                        }
                    }
                    TabGroupState::Error(message) => {
                        ui.colored_label(ui.visuals().error_fg_color, message);
                        retry = ui.button("Retry").clicked();
                    }
                });
        });
        window_layout.open = open;
        ui.ctx()
            .data_mut(|data| data.insert_temp(search_id, search_query));

        match action {
            Some(TabGroupAction::Rename { id, title }) => {
                Self::rename(groups, &id, &title, ui.ctx());
            }
            Some(TabGroupAction::SetColor { id, color }) => {
                Self::set_color(groups, &id, &color, ui.ctx());
            }
            Some(TabGroupAction::Delete { id }) => Self::delete(groups, &id, ui.ctx()),
            Some(TabGroupAction::AddRule { group_id, pattern }) => {
                Self::add_rule(groups, &group_id, &pattern, ui.ctx());
            }
            Some(TabGroupAction::UpdateRule {
                group_id,
                rule_id,
                pattern,
            }) => Self::update_rule(groups, &group_id, &rule_id, &pattern, ui.ctx()),
            Some(TabGroupAction::DeleteRule { group_id, rule_id }) => {
                Self::delete_rule(groups, &group_id, &rule_id, ui.ctx());
            }
            Some(TabGroupAction::DeletePage { group_id, url }) => {
                Self::delete_page(groups, &group_id, &url, ui.ctx());
            }
            None => {}
        }
        if retry {
            Self::load(groups, ui.ctx());
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn group_node(
        ui: &mut egui::Ui,
        group: &SavedTabGroup,
        is_searching: bool,
        favicons: &FaviconStore,
        enabled: bool,
        new_tab_focus: NewTabFocusPreference,
        renaming: &mut Option<(String, String)>,
        rule_editing: &mut Option<(String, Option<String>, String)>,
        action: &mut Option<TabGroupAction>,
    ) {
        let is_renaming = renaming.as_ref().is_some_and(|(id, _)| id == &group.id);
        let state_id = if is_searching {
            ui.make_persistent_id(("saved_tab_group_search", &group.id))
        } else {
            ui.make_persistent_id(("saved_tab_group", &group.id))
        };
        let mut collapsing = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            state_id,
            is_searching,
        );
        if is_searching {
            collapsing.set_open(true);
        }
        let disclosure_icon = if collapsing.is_open() { "▼" } else { "▶" };

        if is_renaming {
            tree_row(ui, 0, egui::Sense::hover(), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(disclosure_icon);
                ui.label(egui::RichText::new("●").color(group_color(&group.color)));
                if let Some((_, title)) = renaming {
                    let edit = ui.add(egui::TextEdit::singleline(title).desired_width(180.0));
                    let cancelled = ui.input(|input| input.key_pressed(egui::Key::Escape));
                    let committed = !cancelled
                        && (edit.lost_focus()
                            || ui.input(|input| input.key_pressed(egui::Key::Enter)));
                    if cancelled || committed {
                        if committed {
                            *action = Some(TabGroupAction::Rename {
                                id: group.id.clone(),
                                title: title.trim().to_owned(),
                            });
                        }
                        *renaming = None;
                    } else if !edit.has_focus() {
                        edit.request_focus();
                    }
                }
            });
        } else {
            let row_response = tree_row(ui, 0, egui::Sense::click(), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(disclosure_icon);
                ui.label(egui::RichText::new("●").color(group_color(&group.color)));
                let count = if is_searching {
                    format!(
                        "{} match{}",
                        group.custom_rules.len() + group.pages.len(),
                        if group.custom_rules.len() + group.pages.len() == 1 {
                            ""
                        } else {
                            "es"
                        }
                    )
                } else {
                    group.pages.len().to_string()
                };
                ui.add(
                    egui::Label::new(format!("{} ({count})", group.label()))
                        .sense(egui::Sense::hover()),
                );
            })
            .response
            .on_hover_text(if is_searching {
                format!(
                    "{} matching record{}",
                    group.custom_rules.len() + group.pages.len(),
                    if group.custom_rules.len() + group.pages.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                )
            } else {
                format!(
                    "{} saved page{}",
                    group.pages.len(),
                    if group.pages.len() == 1 { "" } else { "s" }
                )
            });

            if row_response.clicked() {
                collapsing.toggle(ui);
            }

            if enabled {
                row_response.context_menu(|ui| {
                    if ui.button("Rename").clicked() {
                        *renaming = Some((group.id.clone(), group.title.clone()));
                        ui.close();
                    }
                    if ui.button("Add custom rule").clicked() {
                        *rule_editing = Some((group.id.clone(), None, String::new()));
                        ui.close();
                    }
                    ui.menu_button("Color", |ui| {
                        for color in TAB_GROUP_COLORS {
                            if ui
                                .button(
                                    egui::RichText::new(format!("●  {}", color_label(color)))
                                        .color(group_color(color)),
                                )
                                .clicked()
                            {
                                *action = Some(TabGroupAction::SetColor {
                                    id: group.id.clone(),
                                    color: color.to_owned(),
                                });
                                ui.close();
                            }
                        }
                    });
                    if ui.button("Delete saved group").clicked() {
                        *action = Some(TabGroupAction::Delete {
                            id: group.id.clone(),
                        });
                        ui.close();
                    }
                });
            }
        }

        if rule_editing
            .as_ref()
            .is_some_and(|(group_id, _, _)| group_id == &group.id)
        {
            collapsing.set_open(true);
        }

        collapsing.show_body_unindented(ui, |ui| {
            Self::custom_rule_rows(ui, group, enabled, rule_editing, action);
            if group.pages.is_empty() {
                tree_row(ui, 1, egui::Sense::hover(), |ui| {
                    ui.label(egui::RichText::new("No saved pages").weak().italics());
                });
            } else {
                for page in &group.pages {
                    let response = favicons
                        .link_to(ui, &page.url, 1, page.label(), new_tab_focus)
                        .on_hover_text(&page.url);
                    if enabled {
                        response.context_menu(|ui| {
                            if ui.button("Delete").clicked() {
                                *action = Some(TabGroupAction::DeletePage {
                                    group_id: group.id.clone(),
                                    url: page.url.clone(),
                                });
                                ui.close();
                            }
                        });
                    }
                }
            }
        });
    }

    fn custom_rule_rows(
        ui: &mut egui::Ui,
        group: &SavedTabGroup,
        enabled: bool,
        editing: &mut Option<(String, Option<String>, String)>,
        action: &mut Option<TabGroupAction>,
    ) {
        let editing_rule_id = enabled
            .then(|| {
                editing.as_ref().and_then(|(group_id, rule_id, _)| {
                    (group_id == &group.id).then(|| rule_id.clone()).flatten()
                })
            })
            .flatten();
        let adding = enabled
            && editing
                .as_ref()
                .is_some_and(|(group_id, rule_id, _)| group_id == &group.id && rule_id.is_none());

        if adding {
            Self::custom_rule_editor(ui, group, editing, action);
        }

        for rule in &group.custom_rules {
            if editing_rule_id.as_ref() == Some(&rule.id) {
                Self::custom_rule_editor(ui, group, editing, action);
                continue;
            }

            let response = Self::custom_rule_row(ui, &rule.pattern)
                .response
                .on_hover_text("Custom URL rule");
            if enabled {
                response.context_menu(|ui| {
                    if ui.button("Edit").clicked() {
                        *editing = Some((
                            group.id.clone(),
                            Some(rule.id.clone()),
                            rule.pattern.clone(),
                        ));
                        ui.close();
                    }
                    if ui.button("Delete").clicked() {
                        *action = Some(TabGroupAction::DeleteRule {
                            group_id: group.id.clone(),
                            rule_id: rule.id.clone(),
                        });
                        ui.close();
                    }
                });
            }
        }
    }

    fn custom_rule_row(ui: &mut egui::Ui, pattern: &str) -> egui::InnerResponse<egui::Response> {
        tree_row(ui, 1, egui::Sense::click(), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(egui::RichText::new("RULE:").weak());
            ui.add(
                egui::Label::new(pattern)
                    .truncate()
                    .sense(egui::Sense::hover()),
            )
        })
    }

    fn custom_rule_editor(
        ui: &mut egui::Ui,
        group: &SavedTabGroup,
        editing: &mut Option<(String, Option<String>, String)>,
        action: &mut Option<TabGroupAction>,
    ) {
        tree_row(ui, 1, egui::Sense::hover(), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(egui::RichText::new("RULE:").weak());
            if let Some((_, rule_id, pattern)) = editing {
                let available_width = ui.available_width();
                let edit = ui
                    .add(egui::TextEdit::singleline(pattern).desired_width(available_width))
                    .on_hover_text(
                        "Use * for any characters, ? for one character, and \\ to escape",
                    );
                let cancelled = ui.input(|input| input.key_pressed(egui::Key::Escape));
                let requested_commit =
                    edit.lost_focus() || ui.input(|input| input.key_pressed(egui::Key::Enter));
                let normalized = pattern.trim();
                let committed = !cancelled
                    && requested_commit
                    && !normalized.is_empty()
                    && !has_dangling_escape(normalized);
                if cancelled || committed {
                    if committed {
                        *action = Some(match rule_id {
                            Some(rule_id) => TabGroupAction::UpdateRule {
                                group_id: group.id.clone(),
                                rule_id: rule_id.clone(),
                                pattern: normalized.to_owned(),
                            },
                            None => TabGroupAction::AddRule {
                                group_id: group.id.clone(),
                                pattern: normalized.to_owned(),
                            },
                        });
                    }
                    *editing = None;
                } else if !edit.has_focus() {
                    edit.request_focus();
                }
            }
        });
    }
}

fn has_dangling_escape(pattern: &str) -> bool {
    let mut escaping = false;
    for character in pattern.chars() {
        if escaping {
            escaping = false;
        } else if character == '\\' {
            escaping = true;
        }
    }
    escaping
}

fn color_label(color: &str) -> &str {
    match color {
        "grey" => "Grey",
        "blue" => "Blue",
        "red" => "Red",
        "yellow" => "Yellow",
        "green" => "Green",
        "pink" => "Pink",
        "purple" => "Purple",
        "cyan" => "Cyan",
        "orange" => "Orange",
        _ => "Grey",
    }
}

fn group_color(color: &str) -> egui::Color32 {
    match color {
        "blue" => egui::Color32::from_rgb(66, 133, 244),
        "red" => egui::Color32::from_rgb(234, 67, 53),
        "yellow" => egui::Color32::from_rgb(251, 188, 4),
        "green" => egui::Color32::from_rgb(52, 168, 83),
        "pink" => egui::Color32::from_rgb(255, 112, 181),
        "purple" => egui::Color32::from_rgb(162, 84, 246),
        "cyan" => egui::Color32::from_rgb(36, 189, 209),
        "orange" => egui::Color32::from_rgb(250, 123, 23),
        _ => egui::Color32::from_gray(140),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_group_colors_have_labels_and_visuals() {
        for color in TAB_GROUP_COLORS {
            assert_ne!(color_label(color), "");
            assert_ne!(group_color(color), egui::Color32::TRANSPARENT);
        }
    }

    #[test]
    fn custom_rule_validation_rejects_only_dangling_escapes() {
        assert!(!has_dangling_escape(r"https://*.example.com/\?q=foo"));
        assert!(has_dangling_escape(r"https://example.com/foo\"));
    }

    #[test]
    fn custom_rule_rows_fill_width_and_align_patterns() {
        let ctx = egui::Context::default();
        let mut available_width = 0.0;
        let mut row_widths = Vec::new();
        let mut pattern_starts = Vec::new();

        ctx.run_ui(egui::RawInput::default(), |ui| {
            available_width = ui.available_width();
            for pattern in ["*://short.example/*", "*://a-much-longer.example/path/*"] {
                let row = TabGroupsTool::custom_rule_row(ui, pattern);
                row_widths.push(row.response.rect.width());
                pattern_starts.push(row.inner.rect.left());
            }
        })
        .drop_without_applying_deltas();

        assert!(row_widths.iter().all(|width| *width == available_width));
        assert_eq!(pattern_starts[0], pattern_starts[1]);
    }
}
