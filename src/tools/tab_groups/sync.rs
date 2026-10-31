use eframe::egui;
#[cfg(target_arch = "wasm32")]
use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast as _, closure::Closure, prelude::*};

use super::{SavedTabGroup, SharedTabGroups, TabGroupsTool};

#[derive(Clone, Default)]
pub enum TabGroupState {
    #[default]
    Idle,
    Loading,
    Ready(Vec<SavedTabGroup>),
    Error(String),
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_name = __newtabs_load_tab_groups)]
    async fn load_saved_tab_groups() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = __newtabs_rename_tab_group)]
    async fn rename_saved_tab_group(id: &str, title: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = __newtabs_set_tab_group_color)]
    async fn set_saved_tab_group_color(id: &str, color: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = __newtabs_delete_tab_group)]
    async fn delete_saved_tab_group(id: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = __newtabs_add_tab_group_rule)]
    async fn add_saved_tab_group_rule(group_id: &str, pattern: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = __newtabs_update_tab_group_rule)]
    async fn update_saved_tab_group_rule(
        group_id: &str,
        rule_id: &str,
        pattern: &str,
    ) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = __newtabs_delete_tab_group_rule)]
    async fn delete_saved_tab_group_rule(group_id: &str, rule_id: &str)
    -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = __newtabs_delete_tab_group_page)]
    async fn delete_saved_tab_group_page(group_id: &str, url: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = __newtabs_subscribe_tab_groups)]
    fn subscribe_saved_tab_groups(callback: &js_sys::Function);
}

#[cfg(target_arch = "wasm32")]
fn decode_groups(value: wasm_bindgen::JsValue) -> TabGroupState {
    match serde_wasm_bindgen::from_value::<Vec<SavedTabGroup>>(value) {
        Ok(mut groups) => {
            SavedTabGroup::sort_contents(&mut groups);
            TabGroupState::Ready(groups)
        }
        Err(error) => TabGroupState::Error(format!("Invalid tab group data: {error}")),
    }
}

impl TabGroupsTool {
    #[cfg(target_arch = "wasm32")]
    pub fn subscribe(groups: SharedTabGroups, ctx: egui::Context) -> Closure<dyn FnMut(JsValue)> {
        let callback = Closure::wrap(Box::new(move |value: JsValue| {
            *groups.lock().expect("tab group state lock poisoned") = decode_groups(value);
            ctx.request_repaint();
        }) as Box<dyn FnMut(JsValue)>);
        subscribe_saved_tab_groups(callback.as_ref().unchecked_ref());
        callback
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load(groups: &SharedTabGroups, ctx: &egui::Context) {
        *groups.lock().expect("tab group state lock poisoned") = TabGroupState::Loading;
        let groups = Arc::clone(groups);
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let state = match load_saved_tab_groups().await {
                Ok(value) => decode_groups(value),
                Err(error) => TabGroupState::Error(
                    error
                        .as_string()
                        .unwrap_or_else(|| "Unable to load saved tab groups".to_owned()),
                ),
            };
            *groups.lock().expect("tab group state lock poisoned") = state;
            ctx.request_repaint();
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(groups: &SharedTabGroups, _ctx: &egui::Context) {
        *groups.lock().expect("tab group state lock poisoned") = TabGroupState::Error(
            "Tab groups are available only inside the installed extension".to_owned(),
        );
    }

    pub(crate) fn rename(groups: &SharedTabGroups, id: &str, title: &str, ctx: &egui::Context) {
        if let TabGroupState::Ready(saved) =
            &mut *groups.lock().expect("tab group state lock poisoned")
            && let Some(group) = saved.iter_mut().find(|group| group.id == id)
        {
            group.title = title.to_owned();
            SavedTabGroup::sort_contents(saved);
        }
        #[cfg(target_arch = "wasm32")]
        {
            let groups = Arc::clone(groups);
            let id = id.to_owned();
            let title = title.to_owned();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(error) = rename_saved_tab_group(&id, &title).await {
                    log::error!("Unable to rename saved tab group: {error:?}");
                    Self::load(&groups, &ctx);
                }
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (groups, id, title, ctx);
    }

    pub(crate) fn set_color(groups: &SharedTabGroups, id: &str, color: &str, ctx: &egui::Context) {
        if let TabGroupState::Ready(saved) =
            &mut *groups.lock().expect("tab group state lock poisoned")
            && let Some(group) = saved.iter_mut().find(|group| group.id == id)
        {
            group.color = color.to_owned();
        }
        #[cfg(target_arch = "wasm32")]
        {
            let groups = Arc::clone(groups);
            let id = id.to_owned();
            let color = color.to_owned();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(error) = set_saved_tab_group_color(&id, &color).await {
                    log::error!("Unable to update saved tab group color: {error:?}");
                    Self::load(&groups, &ctx);
                }
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (groups, id, color, ctx);
    }

    pub(crate) fn delete(groups: &SharedTabGroups, id: &str, ctx: &egui::Context) {
        if let TabGroupState::Ready(saved) =
            &mut *groups.lock().expect("tab group state lock poisoned")
        {
            saved.retain(|group| group.id != id);
        }

        #[cfg(target_arch = "wasm32")]
        {
            let groups = Arc::clone(groups);
            let id = id.to_owned();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(error) = delete_saved_tab_group(&id).await {
                    log::error!("Unable to delete saved tab group: {error:?}");
                    Self::load(&groups, &ctx);
                }
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (groups, id, ctx);
    }

    pub(crate) fn add_rule(
        groups: &SharedTabGroups,
        group_id: &str,
        pattern: &str,
        ctx: &egui::Context,
    ) {
        #[cfg(target_arch = "wasm32")]
        {
            let groups = Arc::clone(groups);
            let group_id = group_id.to_owned();
            let pattern = pattern.to_owned();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(error) = add_saved_tab_group_rule(&group_id, &pattern).await {
                    log::error!("Unable to add saved tab group rule: {error:?}");
                    Self::load(&groups, &ctx);
                }
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (groups, group_id, pattern, ctx);
    }

    pub(crate) fn update_rule(
        groups: &SharedTabGroups,
        group_id: &str,
        rule_id: &str,
        pattern: &str,
        ctx: &egui::Context,
    ) {
        #[cfg(target_arch = "wasm32")]
        {
            let groups = Arc::clone(groups);
            let group_id = group_id.to_owned();
            let rule_id = rule_id.to_owned();
            let pattern = pattern.to_owned();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(error) = update_saved_tab_group_rule(&group_id, &rule_id, &pattern).await
                {
                    log::error!("Unable to update saved tab group rule: {error:?}");
                    Self::load(&groups, &ctx);
                }
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (groups, group_id, rule_id, pattern, ctx);
    }

    pub(crate) fn delete_rule(
        groups: &SharedTabGroups,
        group_id: &str,
        rule_id: &str,
        ctx: &egui::Context,
    ) {
        #[cfg(target_arch = "wasm32")]
        {
            let groups = Arc::clone(groups);
            let group_id = group_id.to_owned();
            let rule_id = rule_id.to_owned();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(error) = delete_saved_tab_group_rule(&group_id, &rule_id).await {
                    log::error!("Unable to delete saved tab group rule: {error:?}");
                    Self::load(&groups, &ctx);
                }
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (groups, group_id, rule_id, ctx);
    }

    pub(crate) fn delete_page(
        groups: &SharedTabGroups,
        group_id: &str,
        url: &str,
        ctx: &egui::Context,
    ) {
        #[cfg(target_arch = "wasm32")]
        {
            let groups = Arc::clone(groups);
            let group_id = group_id.to_owned();
            let url = url.to_owned();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(error) = delete_saved_tab_group_page(&group_id, &url).await {
                    log::error!("Unable to delete saved tab group page: {error:?}");
                    Self::load(&groups, &ctx);
                }
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (groups, group_id, url, ctx);
    }
}
