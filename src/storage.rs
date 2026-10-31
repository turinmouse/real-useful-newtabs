//! Persistence for globally synchronized app configuration and tab-local layouts.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

#[cfg(target_arch = "wasm32")]
use eframe::egui;

use crate::canvas::{BookmarkFolderWindowState, Canvas, CanvasLayout, ShellLayout, WindowLayout};
use crate::state::AppState;

pub const APP_STATE_KEY: &str = "app_state";
pub const SHELL_LAYOUT_KEY: &str = "layout:shell";
pub const CANVAS_LAYOUT_PREFIX: &str = "layout:canvas:";
pub const LEGACY_APP_KEY: &str = eframe::APP_KEY;

pub fn canvas_layout_key(canvas_id: usize) -> String {
    format!("{CANVAS_LAYOUT_PREFIX}{canvas_id}")
}

#[derive(Debug)]
pub struct LoadedState {
    pub app_state: AppState,
    pub canvas_layouts: HashMap<usize, CanvasLayout>,
    pub shell_layout: ShellLayout,
    pub needs_migration: bool,
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast as _, closure::Closure, prelude::*};

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = __newtabs_get_storage_value)]
    fn get_web_storage_value(key: &str) -> JsValue;

    #[wasm_bindgen(catch, js_name = __newtabs_save_app_state)]
    async fn save_web_app_state(
        app_ron: String,
        tab_group_enhancement_enabled: bool,
        keep_newtab_rightmost: bool,
        reuse_newtab: bool,
    ) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = __newtabs_queue_layout)]
    fn queue_web_layout(key: &str, layout_ron: &str);

    #[wasm_bindgen(js_name = __newtabs_remove_layout)]
    fn remove_web_layout(key: &str);

    #[wasm_bindgen(js_name = __newtabs_subscribe_app_state)]
    fn subscribe_web_app_state(callback: &js_sys::Function);
}

fn stored_value(storage: Option<&dyn eframe::Storage>, key: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = storage;
        get_web_storage_value(key).as_string()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        storage.and_then(|storage| storage.get_string(key))
    }
}

pub fn load_initial_state(storage: Option<&dyn eframe::Storage>) -> LoadedState {
    if let Some(serialized) = stored_value(storage, APP_STATE_KEY)
        && let Some(mut app_state) = deserialize_app_state(&serialized)
    {
        app_state.sanitize();
        let canvas_layouts = app_state
            .canvases
            .iter()
            .map(|canvas| {
                let layout = stored_value(storage, &canvas_layout_key(canvas.id))
                    .and_then(|value| ron::from_str(&value).ok())
                    .unwrap_or_default();
                (canvas.id, layout)
            })
            .collect();
        let shell_layout = stored_value(storage, SHELL_LAYOUT_KEY)
            .and_then(|value| ron::from_str(&value).ok())
            .unwrap_or_default();
        return LoadedState {
            app_state,
            canvas_layouts,
            shell_layout,
            needs_migration: false,
        };
    }

    let legacy = stored_value(storage, LEGACY_APP_KEY)
        .and_then(|value| ron::from_str::<LegacyAppState>(&value).ok())
        .unwrap_or_default();
    migrate_legacy_state(legacy)
}

pub fn deserialize_app_state(serialized: &str) -> Option<AppState> {
    ron::from_str(serialized).ok().map(|mut state: AppState| {
        state.sanitize();
        state
    })
}

#[cfg(target_arch = "wasm32")]
pub fn persist_app_state(state: &AppState) -> bool {
    let Ok(serialized) = ron::ser::to_string(state) else {
        return false;
    };
    let tab_group_enhancement_enabled = state.tab_group_enhancement_enabled;
    let keep_newtab_rightmost = state.keep_newtab_rightmost;
    let reuse_newtab = state.reuse_newtab;
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(error) = save_web_app_state(
            serialized,
            tab_group_enhancement_enabled,
            keep_newtab_rightmost,
            reuse_newtab,
        )
        .await
        {
            log::error!("Failed to save real-useful-newtabs AppState: {error:?}");
        }
    });
    true
}

#[cfg(not(target_arch = "wasm32"))]
pub fn persist_app_state(_state: &AppState) -> bool {
    false
}

#[cfg(target_arch = "wasm32")]
pub fn subscribe_app_state(
    pending: std::sync::Arc<std::sync::Mutex<Option<AppState>>>,
    ctx: egui::Context,
) -> Closure<dyn FnMut(String)> {
    let callback = Closure::wrap(Box::new(move |serialized: String| {
        if let Some(state) = deserialize_app_state(&serialized) {
            *pending.lock().expect("pending AppState lock poisoned") = Some(state);
            ctx.request_repaint();
        }
    }) as Box<dyn FnMut(String)>);
    subscribe_web_app_state(callback.as_ref().unchecked_ref());
    callback
}

pub fn queue_canvas_layout(canvas_id: usize, layout: &CanvasLayout) -> bool {
    queue_layout(&canvas_layout_key(canvas_id), layout)
}

pub fn queue_shell_layout(layout: &ShellLayout) -> bool {
    queue_layout(SHELL_LAYOUT_KEY, layout)
}

fn queue_layout<T: Serialize>(key: &str, layout: &T) -> bool {
    let Ok(serialized) = ron::ser::to_string(layout) else {
        return false;
    };
    #[cfg(target_arch = "wasm32")]
    queue_web_layout(key, &serialized);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (key, serialized);
    true
}

pub fn remove_canvas_layout(canvas_id: usize) {
    #[cfg(target_arch = "wasm32")]
    remove_web_layout(&canvas_layout_key(canvas_id));
    #[cfg(not(target_arch = "wasm32"))]
    let _ = canvas_id;
}

pub fn save_native_state(
    storage: &mut dyn eframe::Storage,
    app_state: &AppState,
    canvas_layouts: &HashMap<usize, CanvasLayout>,
    shell_layout: &ShellLayout,
    deleted_layouts: &HashSet<usize>,
) {
    eframe::set_value(storage, APP_STATE_KEY, app_state);
    eframe::set_value(storage, SHELL_LAYOUT_KEY, shell_layout);
    for (canvas_id, layout) in canvas_layouts {
        eframe::set_value(storage, &canvas_layout_key(*canvas_id), layout);
    }
    for canvas_id in deleted_layouts {
        storage.remove_string(&canvas_layout_key(*canvas_id));
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct LegacyAppState {
    canvases: Vec<LegacyCanvas>,
    active_canvas: usize,
    next_canvas_id: usize,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct LegacyCanvas {
    id: usize,
    name: String,
    bookmarks_open: bool,
    bookmark_folder_windows: Vec<LegacyFolderWindow>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct LegacyFolderWindow {
    folder_id: String,
    open: bool,
}

fn migrate_legacy_state(legacy: LegacyAppState) -> LoadedState {
    let active_id = legacy
        .canvases
        .get(legacy.active_canvas)
        .map(|canvas| canvas.id);
    let mut layouts = HashMap::new();
    let canvases = legacy
        .canvases
        .into_iter()
        .map(|legacy_canvas| {
            let layout = CanvasLayout {
                bookmarks: WindowLayout {
                    open: legacy_canvas.bookmarks_open,
                },
                bookmark_folder_windows: legacy_canvas
                    .bookmark_folder_windows
                    .into_iter()
                    .map(|folder| BookmarkFolderWindowState {
                        folder_id: folder.folder_id,
                        window: WindowLayout { open: folder.open },
                        tree: Default::default(),
                        renaming: None,
                    })
                    .collect(),
                ..Default::default()
            };
            layouts.insert(legacy_canvas.id, layout);
            Canvas::new(legacy_canvas.id, legacy_canvas.name)
        })
        .collect();

    let mut app_state = AppState {
        canvases,
        next_canvas_id: legacy.next_canvas_id,
        ..Default::default()
    };
    app_state.sanitize();
    for canvas in &app_state.canvases {
        layouts.entry(canvas.id).or_default();
    }
    LoadedState {
        app_state,
        canvas_layouts: layouts,
        shell_layout: ShellLayout {
            active_canvas_id: active_id,
            ..Default::default()
        },
        needs_migration: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_and_layout_round_trip_independently() {
        let app_state = AppState::default();
        let mut layout = CanvasLayout::default();
        layout.bookmarks.open = true;

        let app_ron = ron::ser::to_string(&app_state).unwrap();
        let layout_ron = ron::ser::to_string(&layout).unwrap();
        assert!(!app_ron.contains("bookmarks"));
        assert!(!layout_ron.contains("position"));
        assert!(!layout_ron.contains("collapsed"));
        assert!(!layout_ron.contains("vertical_scroll_offset"));
        assert!(!layout_ron.contains("bookmarks_tree"));
        assert_eq!(ron::from_str::<AppState>(&app_ron).unwrap(), app_state);
        let restored = ron::from_str::<CanvasLayout>(&layout_ron).unwrap();
        assert!(restored.bookmarks.open);
    }

    #[test]
    fn legacy_state_migrates_window_state_out_of_app_state() {
        let legacy: LegacyAppState = ron::from_str(
            "(canvases:[(id:7,name:\"Work\",bookmarks_open:true,devices_open:false,windows_pinned:true,bookmark_folder_windows:[(folder_id:\"42\",open:true)])],active_canvas:0,next_canvas_id:8,settings:(canvas_vertical_scroll:true),search_window_pos:Some((10,20)),search_window_size:Some((300,100)))",
        )
        .unwrap();
        let loaded = migrate_legacy_state(legacy);

        assert_eq!(loaded.app_state.canvases[0].name, "Work");
        let layout = &loaded.canvas_layouts[&7];
        assert!(layout.bookmarks.open);
        assert_eq!(layout.bookmark_folder_windows[0].folder_id, "42");
    }

    #[test]
    fn current_layout_ignores_legacy_floating_defaults() {
        let layout: CanvasLayout = ron::from_str(
            "(bookmarks:(open:true,position:Some((12,24)),size:Some((360,420)),collapsed:true),bookmarks_tree:(initialized:true,expanded_folder_ids:[\"toolbar\"]),vertical_scroll_offset:80)",
        )
        .unwrap();

        assert!(layout.bookmarks.open);
        assert!(layout.bookmarks_tree.is_expanded("toolbar"));

        let serialized = ron::ser::to_string(&layout).unwrap();
        assert!(!serialized.contains("position"));
        assert!(!serialized.contains("size"));
        assert!(!serialized.contains("collapsed"));
        assert!(!serialized.contains("bookmarks_tree"));
        assert!(!serialized.contains("vertical_scroll_offset"));
    }

    #[test]
    fn invalid_app_state_is_rejected() {
        assert!(deserialize_app_state("not RON").is_none());
    }
}
