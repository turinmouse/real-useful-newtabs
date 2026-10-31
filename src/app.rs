use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::canvas::{
    CanvasLayout, CanvasWindowId, ShellLayout, SplitAxis, TileDivider, TileDropRegion,
    TilePlacement,
};
use crate::state::{AppState, ThemePreference};
use crate::storage;
use crate::tools::{
    BookmarkState, BookmarksTool, FaviconStore, SettingsTool, TabGroupState, TabGroupsTool,
};

#[derive(Clone, Debug, Default)]
struct CanvasRuntimeState {
    bookmarks_renaming: Option<(String, String)>,
    tab_group_renaming: Option<(String, String)>,
    tab_group_rule_editing: Option<(String, Option<String>, String)>,
    focused_window: Option<CanvasWindowId>,
    dragging_window: Option<CanvasWindowId>,
    drop_target: Option<(CanvasWindowId, TileDropRegion)>,
    divider_drag: Option<DividerDragState>,
}

#[derive(Clone, Debug)]
struct DividerDragState {
    path: Vec<usize>,
    index: usize,
}

pub struct NewTabsApp {
    state: AppState,
    last_saved_state: AppState,
    active_canvas_id: usize,
    canvas_layouts: HashMap<usize, CanvasLayout>,
    last_queued_layouts: HashMap<usize, CanvasLayout>,
    shell_layout: ShellLayout,
    last_queued_shell_layout: ShellLayout,
    runtime: HashMap<usize, CanvasRuntimeState>,
    renaming_canvas: Option<usize>,
    bookmarks: Arc<Mutex<BookmarkState>>,
    tab_groups: Arc<Mutex<TabGroupState>>,
    favicons: FaviconStore,
    pending_remote_state: Arc<Mutex<Option<AppState>>>,
    deleted_layouts: HashSet<usize>,
    #[cfg(target_arch = "wasm32")]
    force_app_persist: bool,
    #[cfg(target_arch = "wasm32")]
    _app_state_listener: wasm_bindgen::closure::Closure<dyn FnMut(String)>,
    #[cfg(target_arch = "wasm32")]
    _tab_groups_listener: wasm_bindgen::closure::Closure<dyn FnMut(wasm_bindgen::JsValue)>,
}

impl Default for NewTabsApp {
    fn default() -> Self {
        let loaded = storage::LoadedState {
            app_state: AppState::default(),
            canvas_layouts: HashMap::from([(1, CanvasLayout::default())]),
            shell_layout: ShellLayout {
                active_canvas_id: Some(1),
                ..Default::default()
            },
            needs_migration: false,
        };
        Self::from_loaded(loaded, None)
    }
}

impl NewTabsApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let loaded = storage::load_initial_state(cc.storage);
        crate::theme::apply(&cc.egui_ctx);
        Self::from_loaded(loaded, Some(&cc.egui_ctx))
    }

    fn from_loaded(loaded: storage::LoadedState, ctx: Option<&egui::Context>) -> Self {
        let fallback_canvas_id = loaded
            .app_state
            .canvases
            .first()
            .map_or(1, |canvas| canvas.id);
        let active_canvas_id = loaded
            .shell_layout
            .active_canvas_id
            .filter(|canvas_id| loaded.app_state.contains_canvas(*canvas_id))
            .unwrap_or(fallback_canvas_id);
        let mut shell_layout = loaded.shell_layout.clone();
        shell_layout.active_canvas_id = Some(active_canvas_id);
        let pending_remote_state = Arc::new(Mutex::new(None));
        let tab_groups = Arc::new(Mutex::new(TabGroupState::Idle));
        if let Some(ctx) = ctx {
            Self::apply_theme(ctx, loaded.app_state.theme);
        }

        #[cfg(target_arch = "wasm32")]
        let listener = storage::subscribe_app_state(
            Arc::clone(&pending_remote_state),
            ctx.expect("WASM app requires an egui context").clone(),
        );
        #[cfg(target_arch = "wasm32")]
        let tab_groups_listener = TabGroupsTool::subscribe(
            Arc::clone(&tab_groups),
            ctx.expect("WASM app requires an egui context").clone(),
        );

        let last_queued_layouts = if loaded.needs_migration {
            HashMap::new()
        } else {
            loaded.canvas_layouts.clone()
        };
        let last_queued_shell_layout = if loaded.needs_migration {
            ShellLayout {
                right_navigation_width: 0,
                active_canvas_id: None,
            }
        } else {
            loaded.shell_layout.clone()
        };

        Self {
            last_saved_state: loaded.app_state.clone(),
            state: loaded.app_state,
            active_canvas_id,
            last_queued_layouts,
            canvas_layouts: loaded.canvas_layouts,
            last_queued_shell_layout,
            shell_layout,
            runtime: HashMap::new(),
            renaming_canvas: None,
            bookmarks: Arc::default(),
            tab_groups,
            favicons: FaviconStore::default(),
            pending_remote_state,
            deleted_layouts: HashSet::new(),
            #[cfg(target_arch = "wasm32")]
            // Also publishes the tab-group worker configuration after an extension upgrade,
            // even when the serialized AppState itself did not need migration.
            force_app_persist: true,
            #[cfg(target_arch = "wasm32")]
            _app_state_listener: listener,
            #[cfg(target_arch = "wasm32")]
            _tab_groups_listener: tab_groups_listener,
        }
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }

    pub fn active_canvas_id(&self) -> usize {
        self.active_canvas_id
    }

    pub fn active_bookmarks_open(&self) -> bool {
        self.canvas_layouts
            .get(&self.active_canvas_id)
            .is_some_and(|layout| layout.bookmarks.open)
    }

    fn apply_theme(ctx: &egui::Context, preference: ThemePreference) {
        let theme = preference.egui_theme();
        ctx.set_theme(theme);
        crate::theme::sync_browser_color_scheme(theme);
    }

    fn apply_pending_remote_state(&mut self, ctx: &egui::Context) {
        let pending = self
            .pending_remote_state
            .lock()
            .expect("pending AppState lock poisoned")
            .take();
        let Some(state) = pending else {
            return;
        };

        self.last_saved_state = state.clone();
        if state == self.state {
            return;
        }

        let valid_ids: HashSet<_> = state.canvases.iter().map(|canvas| canvas.id).collect();
        self.canvas_layouts
            .retain(|canvas_id, _| valid_ids.contains(canvas_id));
        self.last_queued_layouts
            .retain(|canvas_id, _| valid_ids.contains(canvas_id));
        self.runtime
            .retain(|canvas_id, _| valid_ids.contains(canvas_id));
        for canvas_id in &valid_ids {
            self.canvas_layouts.entry(*canvas_id).or_default();
            self.last_queued_layouts.entry(*canvas_id).or_default();
        }
        if !valid_ids.contains(&self.active_canvas_id) {
            self.active_canvas_id = state.canvases.first().map_or(1, |canvas| canvas.id);
            self.shell_layout.active_canvas_id = Some(self.active_canvas_id);
        }
        Self::apply_theme(ctx, state.theme);
        self.state = state;
        self.renaming_canvas = None;
    }

    fn top_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            let is_dark = self.state.theme == ThemePreference::Dark;
            let icon = if is_dark { "☀" } else { "🌙" };
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(icon))
                        .frame(false)
                        .min_size(egui::vec2(26.0, 22.0)),
                )
                .on_hover_text("Toggle theme")
                .clicked()
            {
                self.state.theme = if is_dark {
                    ThemePreference::Light
                } else {
                    ThemePreference::Dark
                };
                Self::apply_theme(ui.ctx(), self.state.theme);
            }

            ui.separator();
            let mut close_index = None;
            let canvas_count = self.state.canvases.len();
            for index in 0..canvas_count {
                let canvas_id = self.state.canvases[index].id;
                let is_active = canvas_id == self.active_canvas_id;
                if self.renaming_canvas == Some(index) {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.state.canvases[index].name)
                            .desired_width(80.0),
                    );
                    if response.lost_focus() {
                        let name = self.state.canvases[index].name.clone();
                        self.state.rename_canvas(index, &name);
                        self.renaming_canvas = None;
                    } else if !response.has_focus() {
                        response.request_focus();
                    }
                    continue;
                }

                let tab = ui.selectable_label(is_active, &self.state.canvases[index].name);
                if tab.clicked() {
                    self.active_canvas_id = canvas_id;
                    self.shell_layout.active_canvas_id = Some(canvas_id);
                }
                tab.context_menu(|ui| {
                    if ui.button("Rename").clicked() {
                        self.renaming_canvas = Some(index);
                        ui.close();
                    }
                    if canvas_count > 1 && ui.button("Close").clicked() {
                        close_index = Some(index);
                        ui.close();
                    }
                });
            }

            if ui
                .add(egui::Button::new("+").frame(false))
                .on_hover_text("New canvas")
                .clicked()
            {
                let canvas_id = self.state.add_canvas();
                self.active_canvas_id = canvas_id;
                self.shell_layout.active_canvas_id = Some(canvas_id);
                self.canvas_layouts.entry(canvas_id).or_default();
            }

            if let Some(index) = close_index {
                let removed_id = self.state.canvases[index].id;
                if self.state.close_canvas(index) {
                    self.canvas_layouts.remove(&removed_id);
                    self.last_queued_layouts.remove(&removed_id);
                    self.runtime.remove(&removed_id);
                    self.deleted_layouts.insert(removed_id);
                    storage::remove_canvas_layout(removed_id);
                    if self.active_canvas_id == removed_id {
                        self.active_canvas_id =
                            self.state.canvases[index.min(self.state.canvases.len() - 1)].id;
                        self.shell_layout.active_canvas_id = Some(self.active_canvas_id);
                    }
                }
                self.renaming_canvas = None;
            }
        });
    }

    fn tools_navigation(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("real-useful-newtabs")
                    .heading()
                    .strong(),
            );
            ui.add_space(8.0);
        });
        ui.separator();
        ui.add_space(8.0);
        ui.label(egui::RichText::new("TOOLS").small().weak());
        ui.add_space(3.0);

        let canvas_id = self.active_canvas_id;
        let mut requested_focus = None;
        {
            let layout = self.canvas_layouts.entry(canvas_id).or_default();
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                BookmarksTool::navigation_item(ui, layout.bookmarks.open, &self.bookmarks, || {
                    layout.bookmarks.open = true;
                    requested_focus = Some(CanvasWindowId::Bookmarks);
                });
                TabGroupsTool::navigation_item(
                    ui,
                    layout.tab_groups.open,
                    &self.tab_groups,
                    || {
                        layout.tab_groups.open = true;
                        requested_focus = Some(CanvasWindowId::TabGroups);
                    },
                );
            });

            ui.with_layout(
                egui::Layout::bottom_up(egui::Align::LEFT).with_cross_justify(true),
                |ui| {
                    ui.add_space(8.0);
                    SettingsTool::navigation_item(ui, layout.settings.open, || {
                        layout.settings.open = true;
                        requested_focus = Some(CanvasWindowId::Settings);
                    });
                    ui.separator();
                },
            );
        }
        if let Some(window_id) = requested_focus {
            self.runtime.entry(canvas_id).or_default().focused_window = Some(window_id);
        }
    }

    fn open_window_ids(layout: &CanvasLayout) -> Vec<CanvasWindowId> {
        let mut windows = Vec::new();
        if layout.bookmarks.open {
            windows.push(CanvasWindowId::Bookmarks);
        }
        if layout.tab_groups.open {
            windows.push(CanvasWindowId::TabGroups);
        }
        if layout.settings.open {
            windows.push(CanvasWindowId::Settings);
        }
        windows.extend(
            layout
                .bookmark_folder_windows
                .iter()
                .filter(|window| window.window.open)
                .map(|window| CanvasWindowId::BookmarkFolder(window.folder_id.clone())),
        );
        windows
    }

    fn tiled_canvas(&mut self, ui: &mut egui::Ui, canvas_rect: egui::Rect) {
        let canvas_id = self.active_canvas_id;
        let root_rect = canvas_rect.shrink(4.0);
        let open_windows = Self::open_window_ids(
            self.canvas_layouts
                .get(&canvas_id)
                .expect("active canvas layout must exist"),
        );
        {
            let layout = self.canvas_layouts.entry(canvas_id).or_default();
            let runtime = self.runtime.entry(canvas_id).or_default();
            layout
                .tiling
                .reconcile(&open_windows, &mut runtime.focused_window);
        }

        let (placements, dividers) = {
            let tiling = &self
                .canvas_layouts
                .get(&canvas_id)
                .expect("active canvas layout must exist")
                .tiling;
            (tiling.placements(root_rect), tiling.dividers(root_rect))
        };
        let tiled_rects: HashMap<_, _> = placements
            .iter()
            .map(|placement| (placement.window_id.clone(), placement.rect))
            .collect();
        let new_tab_focus = self.state.new_tab_focus;

        {
            let layout = self.canvas_layouts.entry(canvas_id).or_default();
            let runtime = self.runtime.entry(canvas_id).or_default();
            BookmarksTool::show_window(
                ui,
                canvas_id,
                layout,
                &self.bookmarks,
                &self.favicons,
                new_tab_focus,
                &mut runtime.bookmarks_renaming,
                &tiled_rects,
            );
            if let Some(rect) = tiled_rects.get(&CanvasWindowId::Settings).copied() {
                SettingsTool::show_window(
                    ui.ctx(),
                    canvas_id,
                    &mut layout.settings,
                    &mut self.state,
                    rect,
                );
            }
            if let Some(rect) = tiled_rects.get(&CanvasWindowId::TabGroups).copied() {
                TabGroupsTool::show_window(
                    ui,
                    canvas_id,
                    &mut layout.tab_groups,
                    &self.tab_groups,
                    &self.favicons,
                    self.state.tab_group_enhancement_enabled,
                    new_tab_focus,
                    &mut runtime.tab_group_renaming,
                    &mut runtime.tab_group_rule_editing,
                    rect,
                );
            }
        }

        self.tiling_interactions(ui, &placements, &dividers);
    }

    fn tiling_interactions(
        &mut self,
        ui: &mut egui::Ui,
        placements: &[TilePlacement],
        dividers: &[TileDivider],
    ) {
        let canvas_id = self.active_canvas_id;
        let pointer_pos = ui.ctx().input(|input| input.pointer.interact_pos());
        if ui.ctx().input(|input| input.pointer.primary_pressed())
            && let Some(pointer_pos) = pointer_pos
            && let Some(placement) = placements
                .iter()
                .find(|placement| placement.rect.contains(pointer_pos))
        {
            self.runtime.entry(canvas_id).or_default().focused_window =
                Some(placement.window_id.clone());
        }

        let mut drag_started = None;
        let mut drag_stopped = false;
        for placement in placements {
            let handle_width = (placement.rect.width() - 34.0).max(0.0);
            let handle_height = placement.rect.height().clamp(0.0, 30.0);
            if handle_width <= 0.0 || handle_height <= 0.0 {
                continue;
            }
            let response = egui::Area::new(egui::Id::new((
                "tiling_title_handle",
                canvas_id,
                &placement.window_id,
            )))
            .order(egui::Order::Foreground)
            .fixed_pos(placement.rect.min)
            .show(ui.ctx(), |ui| {
                ui.allocate_exact_size(
                    egui::vec2(handle_width, handle_height),
                    egui::Sense::click_and_drag(),
                )
                .1
            })
            .inner;
            let response = if response.dragged() {
                response.on_hover_cursor(egui::CursorIcon::Grabbing)
            } else {
                response.on_hover_cursor(egui::CursorIcon::Grab)
            };
            if response.clicked() {
                self.runtime.entry(canvas_id).or_default().focused_window =
                    Some(placement.window_id.clone());
            }
            if response.drag_started() {
                drag_started = Some(placement.window_id.clone());
            }
            if response.drag_stopped() {
                drag_stopped = true;
            }
        }

        if let Some(window_id) = drag_started {
            let runtime = self.runtime.entry(canvas_id).or_default();
            runtime.dragging_window = Some(window_id);
            runtime.drop_target = None;
        }

        let dragging = self
            .runtime
            .get(&canvas_id)
            .and_then(|runtime| runtime.dragging_window.clone());
        if let Some(source) = dragging.as_ref() {
            let drop_target = pointer_pos.and_then(|pointer_pos| {
                placements
                    .iter()
                    .find(|placement| {
                        placement.window_id != *source && placement.rect.contains(pointer_pos)
                    })
                    .map(|placement| {
                        (
                            placement.window_id.clone(),
                            tile_drop_region(placement.rect, pointer_pos),
                        )
                    })
            });
            self.runtime.entry(canvas_id).or_default().drop_target = drop_target;
            ui.ctx().request_repaint();
        }

        if drag_stopped {
            let (source, target) = {
                let runtime = self.runtime.entry(canvas_id).or_default();
                (runtime.dragging_window.take(), runtime.drop_target.take())
            };
            if let (Some(source), Some((target, region))) = (source, target)
                && self
                    .canvas_layouts
                    .entry(canvas_id)
                    .or_default()
                    .tiling
                    .move_window(&source, &target, region)
            {
                self.runtime.entry(canvas_id).or_default().focused_window = Some(source);
            }
        }

        self.tiling_dividers(ui, dividers);
        self.paint_tiling_overlay(ui, placements, dividers);
    }

    fn tiling_dividers(&mut self, ui: &mut egui::Ui, dividers: &[TileDivider]) {
        let canvas_id = self.active_canvas_id;
        let mut resize_command = None;
        for divider in dividers {
            let hit_rect = divider.rect.expand(2.0);
            let response = egui::Area::new(egui::Id::new((
                "tiling_divider",
                canvas_id,
                divider.index,
                &divider.path,
            )))
            .order(egui::Order::Foreground)
            .fixed_pos(hit_rect.min)
            .show(ui.ctx(), |ui| {
                ui.allocate_exact_size(hit_rect.size(), egui::Sense::drag())
                    .1
            })
            .inner
            .on_hover_cursor(match divider.axis {
                SplitAxis::Horizontal => egui::CursorIcon::ResizeHorizontal,
                SplitAxis::Vertical => egui::CursorIcon::ResizeVertical,
            });

            let drag_delta = match divider.axis {
                SplitAxis::Horizontal => response.drag_delta().x,
                SplitAxis::Vertical => response.drag_delta().y,
            };
            if response.drag_started() {
                self.runtime.entry(canvas_id).or_default().divider_drag = Some(DividerDragState {
                    path: divider.path.clone(),
                    index: divider.index,
                });
            }
            if response.dragged() {
                let runtime = self.runtime.entry(canvas_id).or_default();
                if let Some(state) = runtime.divider_drag.as_mut()
                    && state.path == divider.path
                    && state.index == divider.index
                    && drag_delta != 0.0
                {
                    resize_command = Some((
                        divider.path.clone(),
                        divider.index,
                        drag_delta,
                        divider.container_span,
                        divider.axis,
                    ));
                }
            }
            if response.drag_stopped() {
                self.runtime.entry(canvas_id).or_default().divider_drag = None;
            }
        }

        if let Some((path, index, delta, span, _axis)) = resize_command {
            self.canvas_layouts
                .entry(canvas_id)
                .or_default()
                .tiling
                .resize_divider(&path, index, delta, span);
        }
    }

    fn paint_tiling_overlay(
        &self,
        ui: &egui::Ui,
        placements: &[TilePlacement],
        dividers: &[TileDivider],
    ) {
        let canvas_id = self.active_canvas_id;
        let runtime = self.runtime.get(&canvas_id);
        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new(("tiling_overlay", canvas_id)),
        ));

        for divider in dividers {
            let (start, end) = match divider.axis {
                SplitAxis::Horizontal => (
                    egui::pos2(divider.rect.center().x, divider.rect.top()),
                    egui::pos2(divider.rect.center().x, divider.rect.bottom()),
                ),
                SplitAxis::Vertical => (
                    egui::pos2(divider.rect.left(), divider.rect.center().y),
                    egui::pos2(divider.rect.right(), divider.rect.center().y),
                ),
            };
            painter.line_segment([start, end], ui.visuals().widgets.noninteractive.bg_stroke);
        }

        if let Some((target, region)) = runtime.and_then(|runtime| runtime.drop_target.as_ref())
            && let Some(placement) = placements
                .iter()
                .find(|placement| &placement.window_id == target)
        {
            let preview = tile_drop_preview(placement.rect, *region).shrink(2.0);
            painter.rect_filled(
                preview,
                egui::CornerRadius::same(3),
                ui.visuals().selection.bg_fill.linear_multiply(0.55),
            );
            painter.rect_stroke(
                preview,
                egui::CornerRadius::same(3),
                ui.visuals().selection.stroke,
                egui::StrokeKind::Inside,
            );
        }

        if let Some(dragging) = runtime.and_then(|runtime| runtime.dragging_window.as_ref())
            && let Some(pointer_pos) = ui.ctx().input(|input| input.pointer.interact_pos())
        {
            let ghost = egui::Rect::from_min_size(
                pointer_pos + egui::vec2(12.0, 12.0),
                egui::vec2(164.0, 28.0),
            );
            painter.rect_filled(ghost, egui::CornerRadius::same(4), ui.visuals().window_fill);
            painter.rect_stroke(
                ghost,
                egui::CornerRadius::same(4),
                ui.visuals().widgets.active.bg_stroke,
                egui::StrokeKind::Inside,
            );
            painter.text(
                ghost.left_center() + egui::vec2(8.0, 0.0),
                egui::Align2::LEFT_CENTER,
                dragging.title(),
                egui::FontId::proportional(14.0),
                ui.visuals().text_color(),
            );
        }
    }

    fn queue_changed_layouts(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            for (canvas_id, layout) in &self.canvas_layouts {
                if self.last_queued_layouts.get(canvas_id) != Some(layout)
                    && storage::queue_canvas_layout(*canvas_id, layout)
                {
                    self.last_queued_layouts.insert(*canvas_id, layout.clone());
                }
            }
            if self.shell_layout != self.last_queued_shell_layout
                && storage::queue_shell_layout(&self.shell_layout)
            {
                self.last_queued_shell_layout = self.shell_layout.clone();
            }
        }
    }

    fn persist_changed_app_state(&mut self) {
        #[cfg(target_arch = "wasm32")]
        if (self.force_app_persist || self.state != self.last_saved_state)
            && storage::persist_app_state(&self.state)
        {
            self.last_saved_state = self.state.clone();
            self.force_app_persist = false;
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn has_unsaved_native_changes(&self) -> bool {
        self.state != self.last_saved_state
            || self.canvas_layouts != self.last_queued_layouts
            || self.shell_layout != self.last_queued_shell_layout
            || !self.deleted_layouts.is_empty()
    }
}

fn tile_drop_region(rect: egui::Rect, pointer: egui::Pos2) -> TileDropRegion {
    let horizontal_threshold = (rect.width() * 0.25).min(64.0);
    let vertical_threshold = (rect.height() * 0.25).min(64.0);
    let candidates = [
        (
            pointer.x - rect.left(),
            horizontal_threshold,
            TileDropRegion::Left,
        ),
        (
            rect.right() - pointer.x,
            horizontal_threshold,
            TileDropRegion::Right,
        ),
        (
            pointer.y - rect.top(),
            vertical_threshold,
            TileDropRegion::Top,
        ),
        (
            rect.bottom() - pointer.y,
            vertical_threshold,
            TileDropRegion::Bottom,
        ),
    ];
    candidates
        .into_iter()
        .filter(|(distance, threshold, _)| *distance <= *threshold)
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map_or(TileDropRegion::Center, |(_, _, region)| region)
}

fn tile_drop_preview(rect: egui::Rect, region: TileDropRegion) -> egui::Rect {
    match region {
        TileDropRegion::Left => {
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.center().x, rect.bottom()))
        }
        TileDropRegion::Right => {
            egui::Rect::from_min_max(egui::pos2(rect.center().x, rect.top()), rect.max)
        }
        TileDropRegion::Top => {
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.center().y))
        }
        TileDropRegion::Bottom => {
            egui::Rect::from_min_max(egui::pos2(rect.left(), rect.center().y), rect.max)
        }
        TileDropRegion::Center => rect.shrink2(egui::vec2(
            (rect.width() * 0.16).min(32.0),
            (rect.height() * 0.16).min(32.0),
        )),
    }
}

impl eframe::App for NewTabsApp {
    fn save(&mut self, storage_backend: &mut dyn eframe::Storage) {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = storage_backend;
            self.persist_changed_app_state();
            self.queue_changed_layouts();
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if !self.has_unsaved_native_changes() {
                return;
            }
            storage::save_native_state(
                storage_backend,
                &self.state,
                &self.canvas_layouts,
                &self.shell_layout,
                &self.deleted_layouts,
            );
            self.last_saved_state = self.state.clone();
            self.last_queued_layouts = self.canvas_layouts.clone();
            self.last_queued_shell_layout = self.shell_layout.clone();
            self.deleted_layouts.clear();
        }
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(1)
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.apply_pending_remote_state(ui.ctx());

        egui::Panel::top("top_tabs")
            .resizable(false)
            .exact_size(26.0)
            .frame(egui::Frame::side_top_panel(ui.style()).fill(ui.visuals().panel_fill))
            .show(ui, |ui| self.top_tabs(ui));

        egui::Panel::right("right_navigation")
            .resizable(true)
            .default_size(self.shell_layout.right_navigation_width as f32)
            .size_range(165.0..=270.0)
            .show(ui, |ui| self.tools_navigation(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(ui.style())
                    .fill(ui.visuals().panel_fill)
                    .inner_margin(0.0),
            )
            .show(ui, |ui| {
                let visible_canvas_rect = ui.available_rect_before_wrap();
                self.tiled_canvas(ui, visible_canvas_rect);
            });

        self.persist_changed_app_state();
        self.queue_changed_layouts();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn removed_tools_in_saved_layout_preserve_remaining_windows() {
        let mut layout: CanvasLayout = ron::from_str(
            r#"(bookmarks:(open:true),search:(open:true),devices:(open:true),tiling:(root:Some(Split(axis:Horizontal,children:[Leaf(Search),Leaf(Bookmarks),Leaf(Devices)],weights:[3000,4000,3000]))))"#,
        ).unwrap();
        let mut focused = Some(CanvasWindowId::LegacySearch);
        let open = NewTabsApp::open_window_ids(&layout);
        layout.tiling.reconcile(&open, &mut focused);
        assert_eq!(
            layout.tiling.root,
            Some(crate::canvas::TileNode::Leaf(CanvasWindowId::Bookmarks))
        );
        assert_eq!(focused, Some(CanvasWindowId::Bookmarks));
        let saved = ron::ser::to_string(&layout).unwrap();
        assert!(!saved.contains("Search"));
        assert!(!saved.contains("Devices"));
        let restored: CanvasLayout = ron::from_str(&saved).unwrap();
        assert_eq!(restored, layout);
    }

    #[derive(Default)]
    struct MockStorage {
        values: HashMap<String, String>,
        writes: usize,
    }

    impl eframe::Storage for MockStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.values.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.values.insert(key.to_owned(), value);
            self.writes += 1;
        }

        fn remove_string(&mut self, key: &str) {
            self.values.remove(key);
            self.writes += 1;
        }

        fn flush(&mut self) {}
    }

    #[test]
    fn active_canvas_is_local_runtime_state() {
        let mut app = NewTabsApp::default();
        let second = app.state.add_canvas();
        app.active_canvas_id = second;

        assert_eq!(app.active_canvas_id(), second);
        let serialized = ron::ser::to_string(&app.state).unwrap();
        assert!(!serialized.contains("active_canvas"));
    }

    #[test]
    fn new_page_restores_active_canvas_from_shell_layout() {
        let mut state = AppState::default();
        let second = state.add_canvas();
        let loaded = storage::LoadedState {
            app_state: state,
            canvas_layouts: HashMap::from([
                (1, CanvasLayout::default()),
                (second, CanvasLayout::default()),
            ]),
            shell_layout: ShellLayout {
                active_canvas_id: Some(second),
                ..Default::default()
            },
            needs_migration: false,
        };

        let app = NewTabsApp::from_loaded(loaded, None);
        assert_eq!(app.active_canvas_id(), second);
    }

    #[test]
    fn remote_state_preserves_valid_local_selection() {
        let mut app = NewTabsApp::default();
        let second = app.state.add_canvas();
        app.active_canvas_id = second;
        let mut remote = app.state.clone();
        remote.canvases[0].name = "Renamed".to_owned();
        *app.pending_remote_state.lock().unwrap() = Some(remote);

        app.apply_pending_remote_state(&egui::Context::default());
        assert_eq!(app.active_canvas_id(), second);
        assert_eq!(app.state.canvases[0].name, "Renamed");
    }

    #[test]
    fn remote_deletion_falls_back_to_first_canvas() {
        let mut app = NewTabsApp::default();
        let second = app.state.add_canvas();
        app.active_canvas_id = second;
        let mut remote = app.state.clone();
        remote.canvases.retain(|canvas| canvas.id != second);
        *app.pending_remote_state.lock().unwrap() = Some(remote);

        app.apply_pending_remote_state(&egui::Context::default());
        assert_eq!(app.active_canvas_id(), 1);
    }

    #[test]
    fn native_save_skips_storage_when_nothing_changed() {
        use eframe::App as _;

        let mut app = NewTabsApp::default();
        let mut storage = MockStorage::default();
        app.save(&mut storage);
        assert_eq!(storage.writes, 0);

        app.canvas_layouts.get_mut(&1).unwrap().bookmarks.open = true;
        app.save(&mut storage);
        assert!(storage.writes > 0);

        let writes_after_change = storage.writes;
        app.save(&mut storage);
        assert_eq!(storage.writes, writes_after_change);
    }

    #[test]
    fn tiling_drop_regions_use_edges_and_center() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 300.0));

        assert_eq!(
            tile_drop_region(rect, egui::pos2(4.0, 150.0)),
            TileDropRegion::Left
        );
        assert_eq!(
            tile_drop_region(rect, egui::pos2(396.0, 150.0)),
            TileDropRegion::Right
        );
        assert_eq!(
            tile_drop_region(rect, egui::pos2(200.0, 4.0)),
            TileDropRegion::Top
        );
        assert_eq!(
            tile_drop_region(rect, egui::pos2(200.0, 296.0)),
            TileDropRegion::Bottom
        );
        assert_eq!(
            tile_drop_region(rect, rect.center()),
            TileDropRegion::Center
        );
    }
}
