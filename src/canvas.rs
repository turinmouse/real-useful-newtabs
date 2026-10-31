use std::collections::{BTreeSet, HashSet};

use eframe::egui;
use serde::{Deserialize, Serialize};

/// Project-level metadata for a canvas workspace.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Canvas {
    pub id: usize,
    pub name: String,
}

impl Canvas {
    pub fn new(id: usize, name: String) -> Self {
        Self { id, name }
    }
}

/// Persistent visibility for a tiled tool window.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct WindowLayout {
    pub open: bool,
}

impl WindowLayout {
    pub fn apply_layout<'a>(&self, window: egui::Window<'a>, rect: egui::Rect) -> egui::Window<'a> {
        window
            .default_open(true)
            .fixed_pos(rect.min)
            .fixed_size(rect.size())
            // Fixed sizing alone still lets egui expand to fit oversized content.
            .scroll([true, true])
            .constrain(false)
            .movable(false)
            .resizable(false)
            .collapsible(false)
    }
}

/// Deserialize-only defaults for bookmark folders expanded by older versions.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct BookmarkTreeLayout {
    pub initialized: bool,
    pub expanded_folder_ids: BTreeSet<String>,
}

impl BookmarkTreeLayout {
    pub fn initialize(&mut self, expanded_folder_ids: impl IntoIterator<Item = String>) {
        if self.initialized {
            return;
        }

        self.expanded_folder_ids.extend(expanded_folder_ids);
        self.initialized = true;
    }

    pub fn is_expanded(&self, folder_id: &str) -> bool {
        self.expanded_folder_ids.contains(folder_id)
    }
}

/// Persistent layout for a bookmarks window rooted at a folder.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct BookmarkFolderWindowState {
    pub folder_id: String,
    pub window: WindowLayout,
    #[serde(skip_serializing)]
    pub tree: BookmarkTreeLayout,
    #[serde(skip)]
    pub renaming: Option<(String, String)>,
}

impl Default for BookmarkFolderWindowState {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl PartialEq for BookmarkFolderWindowState {
    fn eq(&self, other: &Self) -> bool {
        self.folder_id == other.folder_id && self.window == other.window
    }
}

impl Eq for BookmarkFolderWindowState {}

impl BookmarkFolderWindowState {
    pub fn new(folder_id: String) -> Self {
        Self {
            folder_id,
            window: WindowLayout { open: true },
            tree: BookmarkTreeLayout::default(),
            renaming: None,
        }
    }

    pub fn open_or_reuse(windows: &mut Vec<Self>, folder_id: String) {
        if let Some(window) = windows
            .iter_mut()
            .find(|window| window.folder_id == folder_id)
        {
            window.window.open = true;
        } else {
            windows.push(Self::new(folder_id));
        }
    }
}

const TILE_WEIGHT_TOTAL: u32 = 10_000;
pub const TILE_GAP: f32 = 4.0;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub enum CanvasWindowId {
    // Retained only to deserialize old layouts; reconciliation removes these windows.
    #[serde(rename = "Search")]
    LegacySearch,
    Bookmarks,
    #[serde(rename = "Devices")]
    LegacyDevices,
    TabGroups,
    Settings,
    BookmarkFolder(String),
}

impl CanvasWindowId {
    pub fn title(&self) -> &str {
        match self {
            Self::LegacySearch => "Search",
            Self::Bookmarks => "Bookmarks",
            Self::LegacyDevices => "Devices",
            Self::TabGroups => "Tab Groups",
            Self::Settings => "Settings",
            Self::BookmarkFolder(_) => "Bookmarks folder",
        }
    }

    fn minimum_size(&self) -> egui::Vec2 {
        match self {
            Self::Settings => egui::vec2(420.0, 72.0),
            _ => egui::vec2(160.0, 72.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum TileNode {
    Leaf(CanvasWindowId),
    Split {
        axis: SplitAxis,
        children: Vec<TileNode>,
        weights: Vec<u32>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileDropRegion {
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

#[derive(Clone, Debug)]
pub struct TilePlacement {
    pub window_id: CanvasWindowId,
    pub rect: egui::Rect,
}

#[derive(Clone, Debug)]
pub struct TileDivider {
    pub path: Vec<usize>,
    pub index: usize,
    pub axis: SplitAxis,
    pub rect: egui::Rect,
    pub container_span: f32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct TilingLayout {
    pub root: Option<TileNode>,
}

impl TilingLayout {
    pub fn contains(&self, window_id: &CanvasWindowId) -> bool {
        self.root
            .as_ref()
            .is_some_and(|root| node_contains(root, window_id))
    }

    pub fn first_window(&self) -> Option<&CanvasWindowId> {
        self.root.as_ref().and_then(first_leaf)
    }

    pub fn reconcile(
        &mut self,
        open_windows: &[CanvasWindowId],
        focused: &mut Option<CanvasWindowId>,
    ) {
        let mut seen = HashSet::new();
        let duplicates: HashSet<_> = self
            .window_ids()
            .into_iter()
            .filter(|window_id| !seen.insert(window_id.clone()))
            .collect();
        for window_id in duplicates {
            self.remove(&window_id);
        }

        let open: HashSet<_> = open_windows.iter().cloned().collect();
        let stale: Vec<_> = self
            .window_ids()
            .into_iter()
            .filter(|window_id| !open.contains(window_id))
            .collect();
        for window_id in stale {
            self.remove(&window_id);
        }

        if focused
            .as_ref()
            .is_some_and(|window_id| !self.contains(window_id))
        {
            *focused = None;
        }

        for window_id in open_windows {
            if !self.contains(window_id) {
                self.insert_after_focus(window_id.clone(), focused.as_ref());
                *focused = Some(window_id.clone());
            }
        }

        if focused.is_none() {
            *focused = self.first_window().cloned();
        }
    }

    pub fn insert_after_focus(
        &mut self,
        window_id: CanvasWindowId,
        focused: Option<&CanvasWindowId>,
    ) {
        if self.contains(&window_id) {
            return;
        }

        let Some(root) = self.root.as_mut() else {
            self.root = Some(TileNode::Leaf(window_id));
            return;
        };

        let target = focused
            .filter(|focused| node_contains(root, focused))
            .cloned()
            .or_else(|| last_leaf(root).cloned());
        let Some(target) = target else {
            self.root = Some(TileNode::Leaf(window_id));
            return;
        };

        if !insert_after_leaf(root, &target, window_id.clone()) {
            let old_root = self.root.take().expect("tiling root must exist");
            self.root = Some(TileNode::Split {
                axis: SplitAxis::Horizontal,
                children: vec![old_root, TileNode::Leaf(window_id)],
                weights: equal_weights(2),
            });
        }
    }

    pub fn remove(&mut self, window_id: &CanvasWindowId) -> bool {
        let Some(root) = self.root.take() else {
            return false;
        };
        let (root, removed) = remove_leaf(root, window_id);
        self.root = root;
        removed
    }

    pub fn move_window(
        &mut self,
        source: &CanvasWindowId,
        target: &CanvasWindowId,
        region: TileDropRegion,
    ) -> bool {
        if source == target || !self.contains(source) || !self.contains(target) {
            return false;
        }
        if region == TileDropRegion::Center {
            if let Some(root) = self.root.as_mut() {
                swap_leaves(root, source, target);
                return true;
            }
            return false;
        }

        let axis = match region {
            TileDropRegion::Left | TileDropRegion::Right => SplitAxis::Horizontal,
            TileDropRegion::Top | TileDropRegion::Bottom => SplitAxis::Vertical,
            TileDropRegion::Center => unreachable!(),
        };
        let before = matches!(region, TileDropRegion::Left | TileDropRegion::Top);
        self.remove(source);
        let inserted = self
            .root
            .as_mut()
            .is_some_and(|root| insert_at_edge(root, target, source.clone(), axis, before));
        debug_assert!(inserted, "drop target must remain after removing source");
        inserted
    }

    pub fn placements(&self, rect: egui::Rect) -> Vec<TilePlacement> {
        let mut placements = Vec::new();
        if let Some(root) = &self.root {
            collect_geometry(
                root,
                rect,
                &mut Vec::new(),
                &mut placements,
                &mut Vec::new(),
            );
        }
        placements
    }

    pub fn dividers(&self, rect: egui::Rect) -> Vec<TileDivider> {
        let mut dividers = Vec::new();
        if let Some(root) = &self.root {
            collect_geometry(root, rect, &mut Vec::new(), &mut Vec::new(), &mut dividers);
        }
        dividers
    }

    pub fn resize_divider(
        &mut self,
        path: &[usize],
        index: usize,
        delta_pixels: f32,
        container_span: f32,
    ) -> bool {
        let Some(root) = self.root.as_mut() else {
            return false;
        };
        let Some(TileNode::Split {
            axis,
            children,
            weights,
        }) = node_at_path_mut(root, path)
        else {
            return false;
        };
        if index + 1 >= children.len() || container_span <= 0.0 {
            return false;
        }

        *weights = normalized_weights(weights, children.len());
        let pair_total = weights[index] + weights[index + 1];
        let spans = constrained_child_spans(children, weights, *axis, container_span);
        let pair_span = spans[index] + spans[index + 1];
        if pair_span <= 0.0 {
            return false;
        }
        let before_minimum = minimum_span(&children[index], *axis);
        let after_minimum = minimum_span(&children[index + 1], *axis);
        let (minimum, maximum) = if before_minimum + after_minimum <= pair_span {
            (before_minimum, pair_span - after_minimum)
        } else {
            (1.0, (pair_span - 1.0).max(1.0))
        };
        let requested_span = (spans[index] + delta_pixels).clamp(minimum, maximum);
        let left = ((requested_span / pair_span) * pair_total as f32)
            .round()
            .clamp(1.0, pair_total.saturating_sub(1) as f32) as u32;
        if left == weights[index] {
            return false;
        }
        weights[index] = left;
        weights[index + 1] = pair_total - left;
        true
    }

    fn window_ids(&self) -> Vec<CanvasWindowId> {
        let mut ids = Vec::new();
        if let Some(root) = &self.root {
            collect_window_ids(root, &mut ids);
        }
        ids
    }
}

fn node_contains(node: &TileNode, window_id: &CanvasWindowId) -> bool {
    match node {
        TileNode::Leaf(id) => id == window_id,
        TileNode::Split { children, .. } => {
            children.iter().any(|child| node_contains(child, window_id))
        }
    }
}

fn first_leaf(node: &TileNode) -> Option<&CanvasWindowId> {
    match node {
        TileNode::Leaf(id) => Some(id),
        TileNode::Split { children, .. } => children.first().and_then(first_leaf),
    }
}

fn last_leaf(node: &TileNode) -> Option<&CanvasWindowId> {
    match node {
        TileNode::Leaf(id) => Some(id),
        TileNode::Split { children, .. } => children.last().and_then(last_leaf),
    }
}

fn collect_window_ids(node: &TileNode, ids: &mut Vec<CanvasWindowId>) {
    match node {
        TileNode::Leaf(id) => ids.push(id.clone()),
        TileNode::Split { children, .. } => {
            for child in children {
                collect_window_ids(child, ids);
            }
        }
    }
}

fn insert_after_leaf(
    node: &mut TileNode,
    target: &CanvasWindowId,
    window_id: CanvasWindowId,
) -> bool {
    let TileNode::Split {
        children, weights, ..
    } = node
    else {
        return false;
    };

    if let Some(index) = children
        .iter()
        .position(|child| matches!(child, TileNode::Leaf(id) if id == target))
    {
        insert_child(children, weights, index + 1, TileNode::Leaf(window_id));
        return true;
    }

    for child in children {
        if node_contains(child, target) {
            return insert_after_leaf(child, target, window_id);
        }
    }
    false
}

fn insert_at_edge(
    node: &mut TileNode,
    target: &CanvasWindowId,
    window_id: CanvasWindowId,
    axis: SplitAxis,
    before: bool,
) -> bool {
    match node {
        TileNode::Leaf(id) if id == target => {
            let target_leaf = TileNode::Leaf(id.clone());
            let new_leaf = TileNode::Leaf(window_id);
            let children = if before {
                vec![new_leaf, target_leaf]
            } else {
                vec![target_leaf, new_leaf]
            };
            *node = TileNode::Split {
                axis,
                children,
                weights: equal_weights(2),
            };
            true
        }
        TileNode::Leaf(_) => false,
        TileNode::Split {
            axis: node_axis,
            children,
            weights,
        } => {
            if *node_axis == axis
                && let Some(index) = children
                    .iter()
                    .position(|child| matches!(child, TileNode::Leaf(id) if id == target))
            {
                let insertion = if before { index } else { index + 1 };
                insert_child(children, weights, insertion, TileNode::Leaf(window_id));
                return true;
            }

            for child in children {
                if node_contains(child, target) {
                    return insert_at_edge(child, target, window_id, axis, before);
                }
            }
            false
        }
    }
}

fn insert_child(
    children: &mut Vec<TileNode>,
    weights: &mut Vec<u32>,
    index: usize,
    child: TileNode,
) {
    let new_share = TILE_WEIGHT_TOTAL / (children.len() as u32 + 1);
    let old_weights = normalized_weights(weights, children.len());
    *weights = scale_weights(&old_weights, TILE_WEIGHT_TOTAL - new_share);
    children.insert(index, child);
    weights.insert(index, new_share);
    *weights = normalized_weights(weights, children.len());
}

fn remove_leaf(node: TileNode, window_id: &CanvasWindowId) -> (Option<TileNode>, bool) {
    match node {
        TileNode::Leaf(id) => {
            if &id == window_id {
                (None, true)
            } else {
                (Some(TileNode::Leaf(id)), false)
            }
        }
        TileNode::Split {
            axis,
            children,
            weights,
        } => {
            let mut removed = false;
            let mut retained = Vec::with_capacity(children.len());
            let mut retained_weights = Vec::with_capacity(children.len());
            for (index, child) in children.into_iter().enumerate() {
                let (child, child_removed) = remove_leaf(child, window_id);
                removed |= child_removed;
                if let Some(child) = child {
                    retained.push(child);
                    retained_weights.push(weights.get(index).copied().unwrap_or(1));
                }
            }
            match retained.len() {
                0 => (None, removed),
                1 => (retained.pop(), removed),
                _ => {
                    let retained_len = retained.len();
                    (
                        Some(TileNode::Split {
                            axis,
                            children: retained,
                            weights: normalized_weights(&retained_weights, retained_len),
                        }),
                        removed,
                    )
                }
            }
        }
    }
}

fn swap_leaves(node: &mut TileNode, left: &CanvasWindowId, right: &CanvasWindowId) {
    match node {
        TileNode::Leaf(id) if id == left => *id = right.clone(),
        TileNode::Leaf(id) if id == right => *id = left.clone(),
        TileNode::Leaf(_) => {}
        TileNode::Split { children, .. } => {
            for child in children {
                swap_leaves(child, left, right);
            }
        }
    }
}

fn equal_weights(count: usize) -> Vec<u32> {
    if count == 0 {
        return Vec::new();
    }
    let base = TILE_WEIGHT_TOTAL / count as u32;
    let mut weights = vec![base; count];
    for weight in weights
        .iter_mut()
        .take((TILE_WEIGHT_TOTAL - base * count as u32) as usize)
    {
        *weight += 1;
    }
    weights
}

fn scale_weights(weights: &[u32], total: u32) -> Vec<u32> {
    let sum: u64 = weights.iter().map(|weight| *weight as u64).sum();
    if weights.is_empty() || sum == 0 {
        return Vec::new();
    }
    let mut scaled: Vec<u32> = weights
        .iter()
        .map(|weight| ((*weight as u64 * total as u64) / sum) as u32)
        .collect();
    let assigned: u32 = scaled.iter().sum();
    for weight in scaled.iter_mut().take((total - assigned) as usize) {
        *weight += 1;
    }
    scaled
}

fn normalized_weights(weights: &[u32], count: usize) -> Vec<u32> {
    if weights.len() != count || weights.iter().all(|weight| *weight == 0) {
        equal_weights(count)
    } else {
        scale_weights(weights, TILE_WEIGHT_TOTAL)
    }
}

fn collect_geometry(
    node: &TileNode,
    rect: egui::Rect,
    path: &mut Vec<usize>,
    placements: &mut Vec<TilePlacement>,
    dividers: &mut Vec<TileDivider>,
) {
    match node {
        TileNode::Leaf(window_id) => placements.push(TilePlacement {
            window_id: window_id.clone(),
            rect,
        }),
        TileNode::Split {
            axis,
            children,
            weights,
        } => {
            if children.is_empty() {
                return;
            }
            let weights = normalized_weights(weights, children.len());
            let total_span = match axis {
                SplitAxis::Horizontal => rect.width(),
                SplitAxis::Vertical => rect.height(),
            };
            let usable_span =
                (total_span - TILE_GAP * children.len().saturating_sub(1) as f32).max(0.0);
            let child_spans = constrained_child_spans(children, &weights, *axis, usable_span);
            let mut cursor = match axis {
                SplitAxis::Horizontal => rect.left(),
                SplitAxis::Vertical => rect.top(),
            };

            for (index, child) in children.iter().enumerate() {
                let child_span = if index + 1 == children.len() {
                    match axis {
                        SplitAxis::Horizontal => rect.right() - cursor,
                        SplitAxis::Vertical => rect.bottom() - cursor,
                    }
                } else {
                    child_spans[index]
                }
                .max(0.0);
                let child_rect = match axis {
                    SplitAxis::Horizontal => egui::Rect::from_min_max(
                        egui::pos2(cursor, rect.top()),
                        egui::pos2((cursor + child_span).min(rect.right()), rect.bottom()),
                    ),
                    SplitAxis::Vertical => egui::Rect::from_min_max(
                        egui::pos2(rect.left(), cursor),
                        egui::pos2(rect.right(), (cursor + child_span).min(rect.bottom())),
                    ),
                };
                path.push(index);
                collect_geometry(child, child_rect, path, placements, dividers);
                path.pop();
                cursor += child_span;

                if index + 1 < children.len() {
                    let divider_rect = match axis {
                        SplitAxis::Horizontal => egui::Rect::from_min_size(
                            egui::pos2(cursor, rect.top()),
                            egui::vec2(TILE_GAP, rect.height()),
                        ),
                        SplitAxis::Vertical => egui::Rect::from_min_size(
                            egui::pos2(rect.left(), cursor),
                            egui::vec2(rect.width(), TILE_GAP),
                        ),
                    };
                    dividers.push(TileDivider {
                        path: path.clone(),
                        index,
                        axis: *axis,
                        rect: divider_rect,
                        container_span: usable_span,
                    });
                    cursor += TILE_GAP;
                }
            }
        }
    }
}

fn minimum_span(node: &TileNode, axis: SplitAxis) -> f32 {
    match node {
        TileNode::Leaf(window_id) => match axis {
            SplitAxis::Horizontal => window_id.minimum_size().x,
            SplitAxis::Vertical => window_id.minimum_size().y,
        },
        TileNode::Split {
            axis: split_axis,
            children,
            ..
        } => {
            let child_minimums = children.iter().map(|child| minimum_span(child, axis));
            if *split_axis == axis {
                child_minimums.sum::<f32>() + TILE_GAP * children.len().saturating_sub(1) as f32
            } else {
                child_minimums.fold(0.0, f32::max)
            }
        }
    }
}

fn constrained_child_spans(
    children: &[TileNode],
    weights: &[u32],
    axis: SplitAxis,
    available_span: f32,
) -> Vec<f32> {
    if children.is_empty() {
        return Vec::new();
    }

    let minimums: Vec<_> = children
        .iter()
        .map(|child| minimum_span(child, axis))
        .collect();
    let minimum_total: f32 = minimums.iter().sum();
    if minimum_total >= available_span {
        if minimum_total == 0.0 {
            return vec![available_span / children.len() as f32; children.len()];
        }
        return minimums
            .into_iter()
            .map(|minimum| available_span * minimum / minimum_total)
            .collect();
    }

    let weights = normalized_weights(weights, children.len());
    let mut spans = vec![0.0; children.len()];
    let mut unresolved: Vec<_> = (0..children.len()).collect();
    let mut remaining_span = available_span;
    let mut remaining_weight: u32 = weights.iter().sum();

    loop {
        let constrained: Vec<_> = unresolved
            .iter()
            .copied()
            .filter(|index| {
                remaining_span * weights[*index] as f32 / (remaining_weight as f32)
                    < minimums[*index]
            })
            .collect();
        if constrained.is_empty() {
            for index in unresolved {
                spans[index] = remaining_span * weights[index] as f32 / remaining_weight as f32;
            }
            break;
        }

        for index in &constrained {
            spans[*index] = minimums[*index];
            remaining_span -= minimums[*index];
            remaining_weight -= weights[*index];
        }
        unresolved.retain(|index| !constrained.contains(index));
    }

    spans
}

fn node_at_path_mut<'a>(node: &'a mut TileNode, path: &[usize]) -> Option<&'a mut TileNode> {
    let Some((&index, rest)) = path.split_first() else {
        return Some(node);
    };
    match node {
        TileNode::Split { children, .. } => node_at_path_mut(children.get_mut(index)?, rest),
        TileNode::Leaf(_) => None,
    }
}

/// Persistent, tab-local-at-runtime layout for a single canvas.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct CanvasLayout {
    pub bookmarks: WindowLayout,
    #[serde(skip_serializing)]
    pub bookmarks_tree: BookmarkTreeLayout,
    pub tab_groups: WindowLayout,
    pub settings: WindowLayout,
    pub tiling: TilingLayout,
    pub bookmark_folder_windows: Vec<BookmarkFolderWindowState>,
}

/// Persistent layout for UI outside an individual canvas.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct ShellLayout {
    #[serde(skip_serializing)]
    pub right_navigation_width: i32,
    pub active_canvas_id: Option<usize>,
}

impl Default for ShellLayout {
    fn default() -> Self {
        Self {
            right_navigation_width: 210,
            active_canvas_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_canvas_contains_only_project_metadata() {
        let canvas = Canvas::new(1, "Test Canvas".to_owned());
        assert_eq!(canvas.id, 1);
        assert_eq!(canvas.name, "Test Canvas");
    }

    #[test]
    fn tiled_window_keeps_oversized_content_inside_its_bounds() {
        let ctx = egui::Context::default();
        let layout = WindowLayout { open: true };
        for width in [420.0, 160.0, 96.0, 240.0] {
            let bounds =
                egui::Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(width, 180.0));
            for _ in 0..3 {
                let mut measured = None;
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800.0, 600.0),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        let response = layout
                            .apply_layout(egui::Window::new("Overflow regression"), bounds)
                            .show(ctx, |ui| {
                                let clip = ui.clip_rect();
                                ui.allocate_space(egui::vec2(1200.0, 900.0));
                                clip
                            })
                            .unwrap();
                        measured = Some((response.response.rect, response.inner.unwrap()));
                    },
                );
                output.textures_delta.clear();
                let (window, clip) = measured.unwrap();
                assert!(
                    bounds.expand(1.0).contains_rect(window),
                    "window {window:?} exceeds {bounds:?}"
                );
                assert!(
                    bounds.expand(1.0).contains_rect(clip),
                    "clip {clip:?} exceeds {bounds:?}"
                );
            }
        }
    }

    #[test]
    fn folder_window_reuses_visibility_record() {
        let mut windows = Vec::new();
        BookmarkFolderWindowState::open_or_reuse(&mut windows, "1".to_owned());
        windows[0].window.open = false;
        BookmarkFolderWindowState::open_or_reuse(&mut windows, "1".to_owned());

        assert_eq!(windows.len(), 1);
        assert!(windows[0].window.open);
    }

    #[test]
    fn bookmark_tree_initializes_legacy_defaults_only_once() {
        let mut tree = BookmarkTreeLayout::default();
        tree.initialize(["toolbar".to_owned(), "other".to_owned()]);
        tree.initialize(["toolbar".to_owned(), "other".to_owned()]);

        assert!(tree.initialized);
        assert!(tree.is_expanded("toolbar"));
        assert!(tree.is_expanded("other"));
    }

    #[test]
    fn folder_window_equality_ignores_superficial_ui_state() {
        let mut left = BookmarkFolderWindowState::new("folder".to_owned());
        let mut right = left.clone();

        left.renaming = Some(("item".to_owned(), "Draft".to_owned()));
        assert_eq!(left, right);

        right.tree.initialize(["nested".to_owned()]);
        assert_eq!(left, right);
    }

    #[test]
    fn tiling_inserts_new_windows_after_focus() {
        let mut tiling = TilingLayout::default();
        let mut focused = None;
        tiling.reconcile(
            &[CanvasWindowId::TabGroups, CanvasWindowId::Bookmarks],
            &mut focused,
        );

        assert_eq!(focused, Some(CanvasWindowId::Bookmarks));
        assert_eq!(
            tiling.window_ids(),
            vec![CanvasWindowId::TabGroups, CanvasWindowId::Bookmarks]
        );
        assert!(matches!(
            tiling.root,
            Some(TileNode::Split {
                axis: SplitAxis::Horizontal,
                ..
            })
        ));
    }

    #[test]
    fn orthogonal_drop_wraps_target_and_center_drop_swaps() {
        let mut tiling = TilingLayout::default();
        let mut focused = None;
        tiling.reconcile(
            &[
                CanvasWindowId::TabGroups,
                CanvasWindowId::Bookmarks,
                CanvasWindowId::BookmarkFolder("test".to_owned()),
            ],
            &mut focused,
        );

        assert!(tiling.move_window(
            &CanvasWindowId::BookmarkFolder("test".to_owned()),
            &CanvasWindowId::TabGroups,
            TileDropRegion::Bottom,
        ));
        let placements = tiling.placements(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(900.0, 600.0),
        ));
        let folder = placements
            .iter()
            .find(|placement| {
                placement.window_id == CanvasWindowId::BookmarkFolder("test".to_owned())
            })
            .unwrap();
        let tab_groups = placements
            .iter()
            .find(|placement| placement.window_id == CanvasWindowId::TabGroups)
            .unwrap();
        assert!(folder.rect.top() > tab_groups.rect.top());

        assert!(tiling.move_window(
            &CanvasWindowId::TabGroups,
            &CanvasWindowId::Bookmarks,
            TileDropRegion::Center,
        ));
        assert_eq!(
            tiling.window_ids(),
            vec![
                CanvasWindowId::Bookmarks,
                CanvasWindowId::BookmarkFolder("test".to_owned()),
                CanvasWindowId::TabGroups,
            ]
        );
    }

    #[test]
    fn removing_window_collapses_single_child_splits() {
        let mut tiling = TilingLayout::default();
        let mut focused = None;
        tiling.reconcile(
            &[CanvasWindowId::TabGroups, CanvasWindowId::Bookmarks],
            &mut focused,
        );

        assert!(tiling.remove(&CanvasWindowId::TabGroups));
        assert_eq!(tiling.root, Some(TileNode::Leaf(CanvasWindowId::Bookmarks)));
        assert!(tiling.remove(&CanvasWindowId::Bookmarks));
        assert_eq!(tiling.root, None);
    }

    #[test]
    fn reconcile_prunes_closed_windows_and_avoids_duplicates() {
        let mut tiling = TilingLayout::default();
        let mut focused = None;
        tiling.reconcile(
            &[
                CanvasWindowId::TabGroups,
                CanvasWindowId::Bookmarks,
                CanvasWindowId::TabGroups,
            ],
            &mut focused,
        );
        tiling.reconcile(&[CanvasWindowId::Bookmarks], &mut focused);

        assert_eq!(tiling.window_ids(), vec![CanvasWindowId::Bookmarks]);
        assert_eq!(focused, Some(CanvasWindowId::Bookmarks));
    }

    #[test]
    fn divider_resize_preserves_total_weight() {
        let mut tiling = TilingLayout::default();
        let mut focused = None;
        tiling.reconcile(
            &[CanvasWindowId::TabGroups, CanvasWindowId::Bookmarks],
            &mut focused,
        );

        assert!(tiling.resize_divider(&[], 0, 100.0, 1000.0));
        let Some(TileNode::Split { weights, .. }) = tiling.root else {
            panic!("expected split root");
        };
        assert_eq!(weights.iter().sum::<u32>(), TILE_WEIGHT_TOTAL);
        assert!(weights[0] > weights[1]);
    }

    #[test]
    fn settings_window_keeps_its_minimum_width_when_space_allows() {
        let mut tiling = TilingLayout::default();
        let mut focused = None;
        tiling.reconcile(
            &[CanvasWindowId::Settings, CanvasWindowId::TabGroups],
            &mut focused,
        );
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(700.0, 300.0));

        let placements = tiling.placements(rect);
        let settings = placements
            .iter()
            .find(|placement| placement.window_id == CanvasWindowId::Settings)
            .unwrap();
        assert!(settings.rect.width() >= 420.0);
    }

    #[test]
    fn divider_cannot_shrink_settings_below_its_minimum_width() {
        let mut tiling = TilingLayout::default();
        let mut focused = None;
        tiling.reconcile(
            &[CanvasWindowId::Settings, CanvasWindowId::TabGroups],
            &mut focused,
        );
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 300.0));
        let divider = tiling.dividers(rect).remove(0);

        assert!(tiling.resize_divider(
            &divider.path,
            divider.index,
            -400.0,
            divider.container_span,
        ));
        let placements = tiling.placements(rect);
        let settings = placements
            .iter()
            .find(|placement| placement.window_id == CanvasWindowId::Settings)
            .unwrap();
        assert!(settings.rect.width() >= 420.0);
    }

    #[test]
    fn legacy_canvas_layout_defaults_to_an_empty_tiling_tree() {
        let layout: CanvasLayout = ron::from_str("(bookmarks:(open:true))").unwrap();
        assert_eq!(layout.tiling, TilingLayout::default());
    }

    #[test]
    fn egui_memory_round_trip_preserves_widget_state() {
        let source = egui::Context::default();
        let collapse_id = egui::Id::new("persisted_collapse");
        let scroll_id = egui::Id::new("persisted_scroll");

        let mut collapse = egui::collapsing_header::CollapsingState::load_with_default_open(
            &source,
            collapse_id,
            false,
        );
        collapse.set_open(true);
        collapse.store(&source);

        let mut scroll = egui::scroll_area::State::default();
        scroll.offset.y = 42.5;
        scroll.store(&source, scroll_id);

        let serialized = source.memory(ron::ser::to_string).unwrap();
        let restored_memory = ron::from_str(&serialized).unwrap();
        let restored = egui::Context::default();
        restored.memory_mut(|memory| *memory = restored_memory);

        assert!(
            egui::collapsing_header::CollapsingState::load(&restored, collapse_id)
                .unwrap()
                .is_open()
        );
        assert_eq!(
            egui::scroll_area::State::load(&restored, scroll_id)
                .unwrap()
                .offset
                .y,
            42.5
        );
    }
}
