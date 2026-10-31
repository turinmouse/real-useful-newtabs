use eframe::egui;
#[cfg(target_arch = "wasm32")]
use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use super::{BookmarkAction, BookmarkNode, BookmarksTool, SharedBookmarks};

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_name = __newtabs_load_bookmarks)]
    async fn load_chrome_bookmarks() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = __newtabs_rename_bookmark)]
    async fn rename_chrome_bookmark(id: &str, title: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = __newtabs_delete_bookmark)]
    async fn delete_chrome_bookmark(id: &str, is_folder: bool) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = __newtabs_move_bookmark)]
    async fn move_chrome_bookmark(
        id: &str,
        parent_id: &str,
        index: u32,
    ) -> Result<JsValue, JsValue>;
}

#[derive(Clone, Default)]
pub enum BookmarkState {
    #[default]
    Idle,
    Loading,
    Ready(Vec<BookmarkNode>),
    Error(String),
}

impl BookmarksTool {
    pub(crate) fn can_drop_before(
        is_searching: bool,
        parent_id: Option<&str>,
        payload: Option<&str>,
        target_id: &str,
    ) -> bool {
        !is_searching && parent_id.is_some() && payload.is_some_and(|id| id != target_id)
    }

    /// Render an insertion indicator before a sibling without affecting layout.
    pub(crate) fn drop_slot(
        ui: &mut egui::Ui,
        parent_id: Option<&str>,
        target_id: &str,
        is_searching: bool,
        pending_action: &mut Option<BookmarkAction>,
    ) -> bool {
        let payload = egui::DragAndDrop::payload::<String>(ui.ctx());
        if !Self::can_drop_before(
            is_searching,
            parent_id,
            payload.as_deref().map(String::as_str),
            target_id,
        ) {
            return false;
        }

        let cursor = ui.cursor();
        let slot_rect = egui::Rect::from_min_size(
            egui::pos2(cursor.left(), cursor.top() - 3.0),
            egui::vec2(ui.available_width(), 6.0),
        );
        let response = ui.interact(
            slot_rect,
            ui.make_persistent_id(("bookmarks_drop_slot", target_id)),
            egui::Sense::hover(),
        );
        if !response.contains_pointer() {
            return false;
        }

        let y = cursor.top();
        let stroke = ui.visuals().selection.stroke;
        let color = if stroke.color == egui::Color32::TRANSPARENT {
            ui.visuals().selection.bg_fill
        } else {
            stroke.color
        };
        ui.painter().line_segment(
            [egui::pos2(cursor.left(), y), egui::pos2(cursor.right(), y)],
            egui::Stroke::new(2.0, color),
        );

        if let Some(payload) = response.dnd_release_payload::<String>() {
            *pending_action = Some(BookmarkAction::MoveBefore {
                id: payload.as_ref().clone(),
                target_id: target_id.to_owned(),
            });
        }
        ui.ctx().request_repaint();
        true
    }

    pub fn label(node: &BookmarkNode) -> &str {
        if node.title.trim().is_empty() {
            "Untitled"
        } else {
            &node.title
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load(bookmarks: &SharedBookmarks, ctx: &egui::Context) {
        *bookmarks.lock().expect("bookmark state lock poisoned") = BookmarkState::Loading;

        let bookmarks = Arc::clone(bookmarks);
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let state = match load_chrome_bookmarks().await {
                Ok(value) => match serde_wasm_bindgen::from_value::<Vec<BookmarkNode>>(value) {
                    Ok(nodes) => BookmarkState::Ready(nodes),
                    Err(error) => BookmarkState::Error(format!("Invalid bookmark data: {error}")),
                },
                Err(error) => BookmarkState::Error(
                    error
                        .as_string()
                        .unwrap_or_else(|| "Unable to load Chrome bookmarks".to_owned()),
                ),
            };

            *bookmarks.lock().expect("bookmark state lock poisoned") = state;
            ctx.request_repaint();
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(bookmarks: &SharedBookmarks, _ctx: &egui::Context) {
        *bookmarks.lock().expect("bookmark state lock poisoned") = BookmarkState::Error(
            "Chrome bookmarks are available only inside the installed extension".to_owned(),
        );
    }

    pub fn rename(bookmarks: &SharedBookmarks, id: &str, new_title: &str, ctx: &egui::Context) {
        let id_str = id.to_owned();
        let title_str = new_title.to_owned();

        // Immediately update in-memory state
        {
            let mut guard = bookmarks.lock().expect("bookmark state lock poisoned");
            if let BookmarkState::Ready(roots) = &mut *guard {
                BookmarkNode::rename_node(roots, &id_str, &title_str);
            }
        }
        ctx.request_repaint();

        #[cfg(target_arch = "wasm32")]
        {
            let bookmarks = Arc::clone(bookmarks);
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(err) = rename_chrome_bookmark(&id_str, &title_str).await {
                    log::error!("Failed to rename Chrome bookmark: {err:?}");
                    Self::load(&bookmarks, &ctx);
                }
            });
        }
    }
    pub fn delete(bookmarks: &SharedBookmarks, id: &str, _is_folder: bool, ctx: &egui::Context) {
        let id_str = id.to_owned();

        // Immediately update in-memory state
        {
            let mut guard = bookmarks.lock().expect("bookmark state lock poisoned");
            if let BookmarkState::Ready(roots) = &mut *guard {
                BookmarkNode::delete_node(roots, &id_str);
            }
        }
        ctx.request_repaint();

        #[cfg(target_arch = "wasm32")]
        {
            let bookmarks = Arc::clone(bookmarks);
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(err) = delete_chrome_bookmark(&id_str, _is_folder).await {
                    log::error!("Failed to delete Chrome bookmark: {err:?}");
                    Self::load(&bookmarks, &ctx);
                }
            });
        }
    }

    pub(crate) fn move_into_bookmark(
        bookmarks: &SharedBookmarks,
        id: &str,
        folder_id: &str,
        ctx: &egui::Context,
    ) {
        let id_str = id.to_owned();
        let folder_id_str = folder_id.to_owned();
        let destination = {
            let mut guard = bookmarks.lock().expect("bookmark state lock poisoned");
            if let BookmarkState::Ready(roots) = &mut *guard {
                BookmarkNode::move_into(roots, &id_str, &folder_id_str)
            } else {
                None
            }
        };
        if let Some((parent_id, index)) = destination {
            Self::finish_move(bookmarks, id_str, parent_id, index, ctx);
        }
    }

    pub(crate) fn move_before_bookmark(
        bookmarks: &SharedBookmarks,
        id: &str,
        target_id: &str,
        ctx: &egui::Context,
    ) {
        let id_str = id.to_owned();
        let target_id_str = target_id.to_owned();
        let destination = {
            let mut guard = bookmarks.lock().expect("bookmark state lock poisoned");
            if let BookmarkState::Ready(roots) = &mut *guard {
                BookmarkNode::move_before(roots, &id_str, &target_id_str)
            } else {
                None
            }
        };
        if let Some((parent_id, index)) = destination {
            Self::finish_move(bookmarks, id_str, parent_id, index, ctx);
        }
    }

    fn finish_move(
        _bookmarks: &SharedBookmarks,
        _id: String,
        _parent_id: String,
        _index: usize,
        ctx: &egui::Context,
    ) {
        ctx.request_repaint();

        #[cfg(target_arch = "wasm32")]
        {
            let bookmarks = Arc::clone(_bookmarks);
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(err) = move_chrome_bookmark(&_id, &_parent_id, _index as u32).await {
                    log::error!("Failed to move Chrome bookmark: {err:?}");
                    Self::load(&bookmarks, &ctx);
                }
            });
        }
    }
}
