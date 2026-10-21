//! The shared Material 3 widgets, plus the few extras the file manager needs.

pub use material_3_ui::widgets::*;

use super::DragPaths;
use egui::{Color32, CornerRadius, FontId, Response, Shape, Ui};
use material_3_ui::icons::MaterialIcon;
use std::sync::Arc;

/// A navigation drawer destination that also lights up while files are
/// dragged over it, since sidebar places are drop targets.
pub fn nav_item(
    ui: &mut Ui,
    icon: MaterialIcon,
    label: &str,
    selected: bool,
    trailing: Option<&str>,
) -> Response {
    let drop_layer = ui.painter().add(Shape::Noop);
    let response = material_3_ui::widgets::nav_item(ui, icon, label, selected, trailing);
    if !selected && !response.hovered() && response.dnd_hover_payload::<DragPaths>().is_some() {
        let fill = palette(ui).on_surface.gamma_multiply(0.08);
        ui.painter().set(
            drop_layer,
            Shape::rect_filled(response.rect, CornerRadius::same(20), fill),
        );
    }
    response
}

/// A menu entry that is greyed out and inert when `enabled` is false.
pub fn menu_item_enabled(
    ui: &mut Ui,
    enabled: bool,
    icon: MaterialIcon,
    text: &str,
    shortcut: &str,
) -> bool {
    ui.add_enabled_ui(enabled, |ui| {
        menu_item(ui, Some(icon), text, shortcut, false)
    })
    .inner
}

/// Multi-line centered galley limited to `rows` lines (grid tile labels).
pub fn wrapped_galley(
    ui: &Ui,
    text: &str,
    font: FontId,
    color: Color32,
    max_width: f32,
    rows: usize,
) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_string(),
        egui::TextFormat::simple(font, color),
    );
    job.wrap = egui::text::TextWrapping {
        max_width: max_width.max(10.0),
        max_rows: rows,
        break_anywhere: false,
        overflow_character: Some('…'),
    };
    job.halign = egui::Align::Center;
    ui.painter().layout_job(job)
}
