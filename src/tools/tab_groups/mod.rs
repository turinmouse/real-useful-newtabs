mod model;
mod sync;
mod ui;

pub use model::{SavedTabGroup, SavedTabPage, SavedTabRule};
pub use sync::TabGroupState;

use std::sync::{Arc, Mutex};

pub(crate) type SharedTabGroups = Arc<Mutex<TabGroupState>>;

pub struct TabGroupsTool;
