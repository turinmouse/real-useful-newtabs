use std::collections::HashMap;

use eframe::egui;

use super::{
    BookmarkAction, BookmarkNode, BookmarkState, BookmarkWindowParams, BookmarksTool,
    SharedBookmarks,
};
use crate::canvas::{BookmarkFolderWindowState, BookmarkTreeLayout, CanvasLayout, CanvasWindowId};
use crate::navigation;
use crate::state::NewTabFocusPreference;
use crate::tools::{FaviconStore, dnd_drag_source_with_grab_offset, tree_row, tree_row_with_id};

impl BookmarksTool {
    fn folder_button(
        ui: &mut egui::Ui,
        depth: usize,
        text: egui::RichText,
        drag_id: Option<egui::Id>,
    ) -> egui::Response {
        let contents =
            |ui: &mut egui::Ui| ui.add(egui::Label::new(text).sense(egui::Sense::hover()));
        if let Some(drag_id) = drag_id {
            tree_row_with_id(ui, drag_id, depth, egui::Sense::click_and_drag(), contents).response
        } else {
            tree_row(ui, depth, egui::Sense::click(), contents).response
        }
    }

    fn link_button(
        ui: &mut egui::Ui,
        node: &BookmarkNode,
        url: &str,
        depth: usize,
        favicons: &FaviconStore,
        drag_id: Option<egui::Id>,
    ) -> egui::Response {
        let contents = |ui: &mut egui::Ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            favicons.show(ui, url);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(Self::label(node)).color(ui.visuals().hyperlink_color),
                )
                .sense(egui::Sense::hover()),
            );
        };
        if let Some(drag_id) = drag_id {
            tree_row_with_id(ui, drag_id, depth, egui::Sense::click_and_drag(), contents).response
        } else {
            tree_row(ui, depth, egui::Sense::click(), contents).response
        }
    }

    pub fn navigation_item(
        ui: &mut egui::Ui,
        is_open: bool,
        bookmarks: &SharedBookmarks,
        open_handler: impl FnOnce(),
    ) {
        if ui
            .selectable_label(is_open, "★  Bookmarks")
            .on_hover_text("Open bookmarks")
            .clicked()
        {
            open_handler();

            let should_load = matches!(
                *bookmarks.lock().expect("bookmark state lock poisoned"),
                BookmarkState::Idle | BookmarkState::Error(_)
            );

            if should_load {
                Self::load(bookmarks, ui.ctx());
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn bookmark_node(
        ui: &mut egui::Ui,
        node: &BookmarkNode,
        depth: usize,
        is_searching: bool,
        parent_id: Option<&str>,
        tree_layout: &mut BookmarkTreeLayout,
        favicons: &FaviconStore,
        new_tab_focus: NewTabFocusPreference,
        renaming: &mut Option<(String, String)>,
        pending_action: &mut Option<BookmarkAction>,
    ) {
        Self::drop_slot(ui, parent_id, &node.id, is_searching, pending_action);

        let is_editing = renaming
            .as_ref()
            .map(|(id, _)| id == &node.id)
            .unwrap_or(false);

        if let Some(url) = &node.url {
            if is_editing {
                tree_row(ui, depth, egui::Sense::hover(), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    favicons.show(ui, url);
                    if let Some((_, edit_text)) = renaming {
                        let response =
                            ui.add(egui::TextEdit::singleline(edit_text).desired_width(180.0));
                        let cancelled = ui.input(|input| input.key_pressed(egui::Key::Escape));
                        let committed = !cancelled
                            && (response.lost_focus()
                                || ui.input(|input| input.key_pressed(egui::Key::Enter)));
                        if cancelled || committed {
                            let new_title = edit_text.trim().to_owned();
                            if committed && !new_title.is_empty() && new_title != node.title {
                                *pending_action = Some(BookmarkAction::Rename {
                                    id: node.id.clone(),
                                    new_title,
                                });
                            }
                            *renaming = None;
                        } else if !response.has_focus() {
                            response.request_focus();
                        }
                    }
                });
            } else {
                let response = if is_searching {
                    Self::link_button(ui, node, url, depth, favicons, None)
                } else {
                    let drag_id = ui.make_persistent_id(("bookmarks_drag", &node.id));
                    dnd_drag_source_with_grab_offset(ui, drag_id, node.id.clone(), |ui| {
                        Self::link_button(ui, node, url, depth, favicons, Some(drag_id))
                    })
                    .inner
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                }
                .on_hover_text(url);
                if response.clicked() {
                    navigation::open_from_response(ui, &response, url, new_tab_focus);
                }

                response.context_menu(|ui| {
                    if ui.button("Rename").clicked() {
                        *renaming = Some((node.id.clone(), node.title.clone()));
                        ui.close();
                    }
                    if ui.button("Delete").clicked() {
                        *pending_action = Some(BookmarkAction::Delete {
                            id: node.id.clone(),
                            is_folder: false,
                        });
                        ui.close();
                    }
                });
            }
            return;
        }

        let id = if is_searching {
            ui.make_persistent_id(("bookmark_search_folder", &node.id))
        } else {
            ui.make_persistent_id(&node.id)
        };
        let persisted_open = tree_layout.is_expanded(&node.id);
        let mut collapsing = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            id,
            is_searching || persisted_open,
        );
        let is_open = collapsing.is_open();
        let icon = if is_open { "" } else { "" };

        if is_editing {
            tree_row(ui, depth, egui::Sense::hover(), |ui| {
                ui.label(icon);
                if let Some((_, edit_text)) = renaming {
                    let response =
                        ui.add(egui::TextEdit::singleline(edit_text).desired_width(180.0));
                    let cancelled = ui.input(|input| input.key_pressed(egui::Key::Escape));
                    let committed = !cancelled
                        && (response.lost_focus()
                            || ui.input(|input| input.key_pressed(egui::Key::Enter)));
                    if cancelled || committed {
                        let new_title = edit_text.trim().to_owned();
                        if committed && !new_title.is_empty() && new_title != node.title {
                            *pending_action = Some(BookmarkAction::Rename {
                                id: node.id.clone(),
                                new_title,
                            });
                        }
                        *renaming = None;
                    } else if !response.has_focus() {
                        response.request_focus();
                    }
                }
            });
        } else {
            let folder_text = egui::RichText::new(format!("{icon}  {}", Self::label(node)))
                .color(ui.visuals().text_color());
            let header_response = if is_searching {
                Self::folder_button(ui, depth, folder_text, None)
            } else {
                let drag_id = ui.make_persistent_id(("bookmarks_drag", &node.id));
                dnd_drag_source_with_grab_offset(ui, drag_id, node.id.clone(), |ui| {
                    Self::folder_button(ui, depth, folder_text, Some(drag_id))
                })
                .inner
                .on_hover_cursor(egui::CursorIcon::PointingHand)
            }
            .on_hover_text(format!(
                "{} item{}",
                node.children.len(),
                if node.children.len() == 1 { "" } else { "s" }
            ));

            if !is_searching && header_response.clicked() {
                collapsing.toggle(ui);
            }

            if !is_searching
                && let Some(payload) = header_response.dnd_release_payload::<String>()
                && payload.as_str() != node.id
            {
                *pending_action = Some(BookmarkAction::MoveInto {
                    id: payload.as_ref().clone(),
                    folder_id: node.id.clone(),
                });
            }

            header_response.context_menu(|ui| {
                if ui.button("Rename").clicked() {
                    *renaming = Some((node.id.clone(), node.title.clone()));
                    ui.close();
                }
                if ui.button("Open in new window").clicked() {
                    *pending_action = Some(BookmarkAction::OpenInNewWindow {
                        id: node.id.clone(),
                    });
                    ui.close();
                }
                if ui.button("Delete").clicked() {
                    *pending_action = Some(BookmarkAction::Delete {
                        id: node.id.clone(),
                        is_folder: true,
                    });
                    ui.close();
                }
            });

            collapsing.show_body_unindented(ui, |ui| {
                if node.children.is_empty() {
                    tree_row(ui, depth + 1, egui::Sense::hover(), |ui| {
                        ui.label(egui::RichText::new("Empty").weak().italics());
                    });
                } else {
                    for child in &node.children {
                        Self::bookmark_node(
                            ui,
                            child,
                            depth + 1,
                            is_searching,
                            Some(&node.id),
                            tree_layout,
                            favicons,
                            new_tab_focus,
                            renaming,
                            pending_action,
                        );
                    }
                }
            });
        }
    }

    pub(crate) fn default_expanded_folder_ids(roots: &[BookmarkNode]) -> Vec<String> {
        let mut folder_ids = Vec::new();

        for root in roots {
            let visible_roots = if root.title.is_empty() && root.url.is_none() {
                root.children.as_slice()
            } else {
                std::slice::from_ref(root)
            };
            folder_ids.extend(
                visible_roots
                    .iter()
                    .filter(|node| node.url.is_none())
                    .map(|node| node.id.clone()),
            );
        }

        folder_ids
    }

    #[allow(clippy::too_many_arguments)]
    fn bookmarks_tree(
        ui: &mut egui::Ui,
        roots: &[BookmarkNode],
        is_searching: bool,
        tree_layout: &mut BookmarkTreeLayout,
        favicons: &FaviconStore,
        new_tab_focus: NewTabFocusPreference,
        renaming: &mut Option<(String, String)>,
        pending_action: &mut Option<BookmarkAction>,
    ) {
        for root in roots {
            if root.title.is_empty() && root.url.is_none() {
                for child in &root.children {
                    Self::bookmark_node(
                        ui,
                        child,
                        0,
                        is_searching,
                        Some(&root.id),
                        tree_layout,
                        favicons,
                        new_tab_focus,
                        renaming,
                        pending_action,
                    );
                }
            } else {
                Self::bookmark_node(
                    ui,
                    root,
                    0,
                    is_searching,
                    None,
                    tree_layout,
                    favicons,
                    new_tab_focus,
                    renaming,
                    pending_action,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn show_window(
        ui: &mut egui::Ui,
        canvas_id: usize,
        layout: &mut CanvasLayout,
        bookmarks: &SharedBookmarks,
        favicons: &FaviconStore,
        new_tab_focus: NewTabFocusPreference,
        main_renaming: &mut Option<(String, String)>,
        tiled_rects: &HashMap<CanvasWindowId, egui::Rect>,
    ) {
        if !layout.bookmarks.open
            && !layout
                .bookmark_folder_windows
                .iter()
                .any(|window| window.window.open)
        {
            return;
        }

        let should_load = matches!(
            *bookmarks.lock().expect("bookmark state lock poisoned"),
            BookmarkState::Idle
        );
        if should_load {
            Self::load(bookmarks, ui.ctx());
        }

        let state = bookmarks
            .lock()
            .expect("bookmark state lock poisoned")
            .clone();
        let roots = match &state {
            BookmarkState::Ready(roots) => roots.clone(),
            _ => Vec::new(),
        };
        let mut actions = Vec::new();
        if layout.bookmarks.open
            && let Some(rect) = tiled_rects.get(&CanvasWindowId::Bookmarks).copied()
        {
            actions.extend(Self::show_window_instance(
                ui,
                egui::Id::new(("tiled_bookmarks", 1_u8, canvas_id)),
                "Bookmarks".to_owned(),
                BookmarkWindowParams {
                    roots,
                    state: &state,
                    window_layout: &mut layout.bookmarks,
                    tree_layout: &mut layout.bookmarks_tree,
                    bookmarks,
                    favicons,
                    new_tab_focus,
                    renaming: main_renaming,
                    rect,
                },
            ));
        }

        for window in layout
            .bookmark_folder_windows
            .iter_mut()
            .filter(|window| window.window.open)
        {
            let window_key = CanvasWindowId::BookmarkFolder(window.folder_id.clone());
            let Some(rect) = tiled_rects.get(&window_key).copied() else {
                continue;
            };
            let state = bookmarks
                .lock()
                .expect("bookmark state lock poisoned")
                .clone();
            let (folder_title, roots) = match &state {
                BookmarkState::Ready(roots) => {
                    let Some(folder) = BookmarkNode::find_node(roots, &window.folder_id) else {
                        window.window.open = false;
                        continue;
                    };
                    (Self::label(folder).to_owned(), vec![folder.clone()])
                }
                _ => (window.folder_id.clone(), Vec::new()),
            };

            actions.extend(Self::show_window_instance(
                ui,
                egui::Id::new(("tiled_bookmarks_folder", 1_u8, canvas_id, &window.folder_id)),
                format!("Bookmarks: {folder_title}"),
                BookmarkWindowParams {
                    roots,
                    state: &state,
                    window_layout: &mut window.window,
                    tree_layout: &mut window.tree,
                    bookmarks,
                    favicons,
                    new_tab_focus,
                    renaming: &mut window.renaming,
                    rect,
                },
            ));
        }

        for action in actions {
            match action {
                BookmarkAction::Rename { id, new_title } => {
                    Self::rename(bookmarks, &id, &new_title, ui.ctx());
                }
                BookmarkAction::Delete { id, is_folder } => {
                    Self::delete(bookmarks, &id, is_folder, ui.ctx());
                }
                BookmarkAction::OpenInNewWindow { id } => {
                    BookmarkFolderWindowState::open_or_reuse(
                        &mut layout.bookmark_folder_windows,
                        id,
                    );
                }
                BookmarkAction::MoveBefore { id, target_id } => {
                    Self::move_before_bookmark(bookmarks, &id, &target_id, ui.ctx());
                }
                BookmarkAction::MoveInto { id, folder_id } => {
                    Self::move_into_bookmark(bookmarks, &id, &folder_id, ui.ctx());
                }
            }
        }
    }

    fn show_window_instance(
        ui: &mut egui::Ui,
        window_id: egui::Id,
        title: String,
        params: BookmarkWindowParams<'_>,
    ) -> Vec<BookmarkAction> {
        let BookmarkWindowParams {
            roots,
            state,
            window_layout,
            tree_layout,
            bookmarks,
            favicons,
            new_tab_focus,
            renaming,
            rect,
        } = params;
        let mut open = window_layout.open;
        let mut retry = false;
        let search_id = egui::Id::new(("bookmarks_search", window_id));
        let mut search_query: String = ui
            .ctx()
            .data_mut(|d| d.get_temp(search_id))
            .unwrap_or_default();
        let mut pending_action: Option<BookmarkAction> = None;

        if matches!(state, BookmarkState::Ready(_)) {
            tree_layout.initialize(Self::default_expanded_folder_ids(&roots));
        }

        let window = egui::Window::new(egui::RichText::new(title).size(14.0))
            .id(window_id)
            .open(&mut open);
        window_layout.apply_layout(window, rect).show(ui, |ui| {
            egui::Frame::NONE
                .inner_margin(egui::Margin {
                    left: 8,
                    right: 0,
                    top: 0,
                    bottom: 0,
                })
                .show(ui, |ui| {
                    let toolbar_size =
                        egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
                    ui.allocate_ui_with_layout(
                        toolbar_size,
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            if ui
                                .add_enabled(
                                    !matches!(state, BookmarkState::Loading),
                                    egui::Button::new("↻"),
                                )
                                .on_hover_text("Reload bookmarks")
                                .clicked()
                            {
                                retry = true;
                            }
                            ui.scope(|ui| {
                                ui.visuals_mut().widgets.hovered.bg_stroke = egui::Stroke::NONE;
                                ui.visuals_mut().widgets.active.bg_stroke = egui::Stroke::NONE;
                                ui.visuals_mut().widgets.open.bg_stroke = egui::Stroke::NONE;
                                ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::NONE;
                                ui.visuals_mut().widgets.noninteractive.bg_stroke =
                                    egui::Stroke::NONE;
                                ui.visuals_mut().selection.stroke = egui::Stroke::NONE;
                                ui.visuals_mut().extreme_bg_color = ui.visuals().window_fill;

                                let search_edit = egui::TextEdit::singleline(&mut search_query)
                                    .hint_text("Search bookmarks…")
                                    .frame(egui::Frame::NONE)
                                    .desired_width(f32::INFINITY);

                                ui.add(search_edit);
                            });
                        },
                    );
                });

            ui.separator();

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Frame::NONE.inner_margin(egui::Margin::same(8)).show(
                        ui,
                        |ui| match state {
                            BookmarkState::Idle => {
                                ui.label(egui::RichText::new("Open Bookmarks to load data").weak());
                            }
                            BookmarkState::Loading => {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label("Loading Chrome bookmarks…");
                                });
                            }
                            BookmarkState::Ready(all_roots) if all_roots.is_empty() => {
                                ui.label(egui::RichText::new("No bookmarks found").weak());
                            }
                            BookmarkState::Ready(_) => {
                                let is_searching = !search_query.trim().is_empty();
                                let filtered = BookmarkNode::filter_tree(&roots, &search_query);
                                if filtered.is_empty() {
                                    ui.label(
                                        egui::RichText::new("No matching bookmarks found").weak(),
                                    );
                                } else {
                                    Self::bookmarks_tree(
                                        ui,
                                        &filtered,
                                        is_searching,
                                        tree_layout,
                                        favicons,
                                        new_tab_focus,
                                        renaming,
                                        &mut pending_action,
                                    );
                                }
                            }
                            BookmarkState::Error(message) => {
                                ui.colored_label(ui.visuals().error_fg_color, message);
                                ui.add_space(6.0);
                                retry = ui.button("Retry").clicked();
                            }
                        },
                    );
                });
        });

        window_layout.open = open;
        ui.ctx()
            .data_mut(|d| d.insert_temp(search_id, search_query));

        if retry {
            favicons.retry_unavailable();
            Self::load(bookmarks, ui.ctx());
        }

        pending_action.into_iter().collect()
    }
}
