use std::any::Any;

use eframe::egui;

fn tree_row_hover_fill(fill: egui::Color32) -> egui::Color32 {
    fill.gamma_multiply(0.5)
}

pub(crate) fn dnd_drag_source_with_grab_offset<Payload, R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    payload: Payload,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R>
where
    Payload: Any + Send + Sync,
{
    if ui.ctx().is_being_dragged(id) {
        egui::DragAndDrop::set_payload(ui.ctx(), payload);

        let layer_id = egui::LayerId::new(egui::Order::Tooltip, id);
        let response = ui.scope_builder(egui::UiBuilder::new().layer_id(layer_id), add_contents);

        let pointer_positions = ui.ctx().input(|input| {
            input
                .pointer
                .interact_pos()
                .zip(input.pointer.press_origin())
        });
        if let Some((pointer_pos, press_origin)) = pointer_positions {
            ui.ctx().transform_layer_shapes(
                layer_id,
                egui::emath::TSTransform::from_translation(drag_preview_translation(
                    pointer_pos,
                    press_origin,
                )),
            );
        }

        response
    } else {
        let egui::InnerResponse { inner, response } = ui.scope(add_contents);
        let drag_response = ui.interact(response.rect, id, egui::Sense::drag());
        drag_response.dnd_set_drag_payload(payload);
        egui::InnerResponse::new(inner, drag_response | response)
    }
}

fn drag_preview_translation(pointer_pos: egui::Pos2, press_origin: egui::Pos2) -> egui::Vec2 {
    pointer_pos - press_origin
}

pub(crate) fn tree_row<R>(
    ui: &mut egui::Ui,
    depth: usize,
    sense: egui::Sense,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    tree_row_impl(ui, None, depth, sense, add_contents)
}

pub(crate) fn tree_row_with_id<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    depth: usize,
    sense: egui::Sense,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    tree_row_impl(ui, Some(id), depth, sense, add_contents)
}

fn tree_row_impl<R>(
    ui: &mut egui::Ui,
    interaction_id: Option<egui::Id>,
    depth: usize,
    sense: egui::Sense,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let indent = ui.spacing().indent;
    let row_size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
    let (row_id, row_rect) = ui.allocate_space(row_size);
    let background = ui.painter().add(egui::Shape::Noop);

    if ui.visuals().indent_has_left_vline {
        let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        let spacing = ui.spacing().item_spacing.y * 0.5;
        for level in 0..depth {
            let x = row_rect.left() + (level as f32 + 0.5) * indent;
            ui.painter().line_segment(
                [
                    egui::pos2(x, row_rect.top() - spacing),
                    egui::pos2(x, row_rect.bottom() + spacing),
                ],
                stroke,
            );
        }
    }

    let mut content_rect = row_rect;
    content_rect.min.x += depth as f32 * indent;
    let mut row_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(content_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let inner = add_contents(&mut row_ui);

    let mut response = ui.interact(row_rect, interaction_id.unwrap_or(row_id), sense);
    if sense.interactive() {
        response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    }

    if response.hovered() || response.has_focus() {
        let visuals = ui.style().interact(&response);
        ui.painter().set(
            background,
            egui::Shape::rect_filled(
                response.rect,
                visuals.corner_radius,
                tree_row_hover_fill(visuals.weak_bg_fill),
            ),
        );
    }

    egui::InnerResponse::new(inner, response)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn show_draggable_row(ui: &mut egui::Ui) -> egui::Response {
        let drag_id = egui::Id::new("draggable_tree_row");
        dnd_drag_source_with_grab_offset(ui, drag_id, "payload", |ui| {
            tree_row_with_id(ui, drag_id, 0, egui::Sense::click_and_drag(), |ui| {
                ui.label("Row");
            })
            .response
        })
        .inner
        .on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    #[test]
    fn draggable_row_keeps_hover_cursor_and_click_response() {
        let ctx = egui::Context::default();
        let mut row_rect = egui::Rect::NOTHING;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            row_rect = show_draggable_row(ui).rect;
        })
        .drop_without_applying_deltas();

        let pointer_pos = row_rect.center();
        let mut hovered = false;
        let hover_output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::PointerMoved(pointer_pos)],
                ..Default::default()
            },
            |ui| hovered = show_draggable_row(ui).hovered(),
        );
        assert!(hovered);
        assert_eq!(
            hover_output.platform_output.cursor_icon,
            egui::CursorIcon::PointingHand
        );
        hover_output.drop_without_applying_deltas();

        ctx.run_ui(
            egui::RawInput {
                time: Some(0.1),
                events: vec![egui::Event::PointerButton {
                    pos: pointer_pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| {
                show_draggable_row(ui);
            },
        )
        .drop_without_applying_deltas();

        let mut clicked = false;
        ctx.run_ui(
            egui::RawInput {
                time: Some(0.2),
                events: vec![egui::Event::PointerButton {
                    pos: pointer_pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| clicked = show_draggable_row(ui).clicked(),
        )
        .drop_without_applying_deltas();

        assert!(clicked);
    }

    #[test]
    fn drag_preview_keeps_the_original_grab_point_under_the_pointer() {
        let original_rect =
            egui::Rect::from_min_size(egui::pos2(20.0, 30.0), egui::vec2(180.0, 20.0));
        let press_origin = original_rect.left_top() + egui::vec2(137.0, 7.0);
        let pointer_pos = egui::pos2(320.0, 210.0);

        let moved_rect =
            original_rect.translate(drag_preview_translation(pointer_pos, press_origin));
        let original_grab_offset = press_origin - original_rect.min;

        assert_eq!(moved_rect.min + original_grab_offset, pointer_pos);
    }

    #[test]
    fn tree_row_hover_background_is_half_transparent() {
        let fill = tree_row_hover_fill(egui::Color32::from_rgb(80, 120, 160));

        assert_eq!(fill.a(), 128);
    }
}
