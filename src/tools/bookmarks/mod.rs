mod model;
mod sync;
mod ui;

#[cfg(test)]
mod tests;

pub use model::BookmarkNode;
pub use sync::BookmarkState;

use std::sync::{Arc, Mutex};

pub(crate) type SharedBookmarks = Arc<Mutex<BookmarkState>>;

pub struct BookmarksTool;

pub(crate) struct BookmarkWindowParams<'a> {
    pub(crate) roots: Vec<BookmarkNode>,
    pub(crate) state: &'a BookmarkState,
    pub(crate) window_layout: &'a mut crate::canvas::WindowLayout,
    pub(crate) tree_layout: &'a mut crate::canvas::BookmarkTreeLayout,
    pub(crate) bookmarks: &'a SharedBookmarks,
    pub(crate) favicons: &'a crate::tools::FaviconStore,
    pub(crate) new_tab_focus: crate::state::NewTabFocusPreference,
    pub(crate) renaming: &'a mut Option<(String, String)>,
    pub(crate) rect: egui::Rect,
}

pub(crate) enum BookmarkAction {
    Rename { id: String, new_title: String },
    Delete { id: String, is_folder: bool },
    OpenInNewWindow { id: String },
    MoveBefore { id: String, target_id: String },
    MoveInto { id: String, folder_id: String },
}
