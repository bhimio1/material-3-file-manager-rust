//! Right-hand details pane: preview and metadata of the selection.

use super::icons::{entry_icon, kind_color};
use super::widgets::{self, paint_icon, ButtonKind};
use super::Action;
use crate::app::App;
use crate::fs::entry::{format_size, format_time};
use crate::fs::places;
use crate::fs::{scanner, FileEntry, FileKind};
use crate::thumbs::Thumb;
use egui::{vec2, CornerRadius, Frame, Id, Margin, Rect, RichText, Sense, Ui, Vec2};
use egui_material_icons::icons::*;
use std::sync::Arc;

const TEXT_PREVIEW_BYTES: usize = 12 * 1024;

fn meta_row(ui: &mut Ui, label: &str, value: &str) {
    let p = widgets::palette(ui);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            vec2(84.0, 18.0),
            egui::Layout::left_to_right(egui::Align::Min),
            |ui| {
                ui.set_min_width(84.0);
                ui.label(RichText::new(label).size(12.5).color(p.on_surface_variant));
            },
        );
        ui.add(egui::Label::new(RichText::new(value).size(13.0)).wrap());
    });
}

/// Cached per (path, mtime) in egui memory so we read files once.
fn cached<T: Clone + Send + Sync + 'static>(
    ui: &Ui,
    key: impl std::hash::Hash + std::fmt::Debug,
    f: impl FnOnce() -> T,
) -> T {
    let id = Id::unique(("details-cache", key));
    if let Some(v) = ui.data(|d| d.get_temp::<Arc<T>>(id)) {
        return (*v).clone();
    }
    let v = f();
    ui.data_mut(|d| d.insert_temp(id, Arc::new(v.clone())));
    v
}

pub fn show(ui: &mut Ui, app: &mut App) {
    let p = app.palette.clone();
    let mut actions = Vec::new();
    Frame::new()
        .fill(p.surface_container_low)
        .corner_radius(CornerRadius::same(24))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let tab = app.tab();
                    let selected: Vec<FileEntry> =
                        tab.selected_entries().into_iter().cloned().collect();
                    match selected.as_slice() {
                        [] => folder_summary(ui, app, &mut actions),
                        [one] => single(ui, app, one, &mut actions),
                        many => multiple(ui, app, many),
                    }
                });
        });
    app.actions.extend(actions);
}

fn preview_box(ui: &mut Ui, app: &mut App, entry: &FileEntry) {
    let p = app.palette.clone();
    let w = ui.available_width();
    let h = (w * 0.75).clamp(140.0, 260.0);
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(16), p.surface_container);
    if entry.kind.has_thumbnail() {
        let px = rect.width() * ui.ctx().pixels_per_point();
        match app.thumbs.get(&entry.path, px, entry.modified) {
            Thumb::Ready(tex) => {
                let size = tex.size_vec2();
                let area = rect.shrink(8.0);
                let scale = (area.width() / size.x).min(area.height() / size.y).min(4.0);
                let r = Rect::from_center_size(area.center(), size * scale);
                egui::Image::new(&tex)
                    .corner_radius(CornerRadius::same(10))
                    .paint_at(ui, r);
                return;
            }
            Thumb::Loading => {
                egui::Spinner::new()
                    .size(24.0)
                    .color(p.primary)
                    .paint_at(ui, Rect::from_center_size(rect.center(), Vec2::splat(24.0)));
                return;
            }
            Thumb::Unavailable => {}
        }
    }
    paint_icon(
        ui,
        entry_icon(entry),
        rect.center(),
        h * 0.5,
        kind_color(entry.kind, &p),
    );
}

fn single(ui: &mut Ui, app: &mut App, entry: &FileEntry, actions: &mut Vec<Action>) {
    let p = app.palette.clone();
    preview_box(ui, app, entry);
    ui.add_space(12.0);
    ui.add(egui::Label::new(widgets::title_text(&entry.name, 17.0).color(p.on_surface)).wrap());
    ui.label(
        RichText::new(entry.type_label())
            .size(13.0)
            .color(p.on_surface_variant),
    );
    ui.add_space(10.0);

    ui.horizontal_wrapped(|ui| {
        if widgets::button(
            ui,
            ButtonKind::Filled,
            Some(if entry.is_dir {
                ICON_FOLDER_OPEN
            } else {
                ICON_OPEN_IN_NEW
            }),
            "Open",
        )
        .clicked()
        {
            actions.push(Action::Open(entry.path.clone()));
        }
        if !entry.is_dir && widgets::icon_button(ui, ICON_APPS, "Open with…", true).clicked() {
            actions.push(Action::OpenWith(entry.path.clone()));
        }
        if widgets::icon_button(ui, ICON_LINK, "Copy path", true).clicked() {
            actions.push(Action::CopyPathText(vec![entry.path.clone()]));
        }
        if widgets::icon_button(ui, ICON_INFO, "Properties", true).clicked() {
            actions.push(Action::Properties(entry.path.clone()));
        }
    });
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(6.0);

    if !entry.is_dir {
        meta_row(ui, "Size", &format_size(entry.size));
    } else {
        let count = cached(ui, ("count", &entry.path, entry.modified), || {
            std::fs::read_dir(&entry.path).map(|rd| rd.count()).ok()
        });
        meta_row(
            ui,
            "Contains",
            &count
                .map(|n| crate::fs::entry::plural(n as u64, "item"))
                .unwrap_or_else(|| "—".into()),
        );
    }
    meta_row(ui, "Modified", &format_time(entry.modified));
    if entry.kind == FileKind::Image {
        let dims = cached(ui, ("dims", &entry.path, entry.modified), || {
            image::image_dimensions(&entry.path).ok()
        });
        if let Some((w, h)) = dims {
            meta_row(ui, "Dimensions", &format!("{w} × {h}"));
        }
    }
    if let Some(parent) = entry.path.parent() {
        meta_row(ui, "Location", &places::tilde(parent));
    }
    if entry.is_symlink {
        if let Ok(target) = std::fs::read_link(&entry.path) {
            meta_row(ui, "Link to", &target.to_string_lossy());
        }
    }

    if matches!(entry.kind, FileKind::Text | FileKind::Code)
        || (entry.kind == FileKind::Other && entry.size < 256 * 1024)
    {
        let preview = cached(ui, ("text", &entry.path, entry.modified), || {
            scanner::read_text_preview(&entry.path, TEXT_PREVIEW_BYTES)
        });
        if let Some(text) = preview.filter(|t| !t.trim().is_empty()) {
            ui.add_space(12.0);
            ui.label(
                RichText::new("Preview")
                    .size(12.5)
                    .color(p.on_surface_variant),
            );
            ui.add_space(4.0);
            Frame::new()
                .fill(p.surface_container)
                .corner_radius(CornerRadius::same(12))
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let shown: String = text.lines().take(60).collect::<Vec<_>>().join("\n");
                    ui.add(
                        egui::Label::new(
                            RichText::new(shown)
                                .monospace()
                                .size(11.5)
                                .color(p.on_surface_variant),
                        )
                        .wrap(),
                    );
                });
        }
    }
}

fn multiple(ui: &mut Ui, app: &mut App, entries: &[FileEntry]) {
    let p = app.palette.clone();
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 140.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(16), p.surface_container);
    // A small fan of the first few icons.
    for (i, e) in entries.iter().take(3).enumerate().rev() {
        let offset = vec2((i as f32 - 1.0) * 26.0, (i as f32 - 1.0).abs() * 6.0);
        let c = rect.center() + offset;
        ui.painter()
            .circle_filled(c, 30.0, p.surface_container_high);
        paint_icon(ui, entry_icon(e), c, 34.0, kind_color(e.kind, &p));
    }
    ui.add_space(12.0);
    ui.label(
        RichText::new(format!("{} items selected", entries.len()))
            .size(17.0)
            .color(p.on_surface),
    );
    let files: Vec<&FileEntry> = entries.iter().filter(|e| !e.is_dir).collect();
    let folders = entries.len() - files.len();
    let total: u64 = files.iter().map(|e| e.size).sum();
    ui.add_space(8.0);
    meta_row(
        ui,
        "Files",
        &format!("{} ({})", files.len(), format_size(total)),
    );
    if folders > 0 {
        meta_row(ui, "Folders", &folders.to_string());
    }
}

fn folder_summary(ui: &mut Ui, app: &mut App, actions: &mut Vec<Action>) {
    let p = app.palette.clone();
    let Some(folder) = app.tab().folder().map(|f| f.to_path_buf()) else {
        return;
    };
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 140.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(16), p.surface_container);
    paint_icon(
        ui,
        places::folder_icon(&folder),
        rect.center(),
        72.0,
        p.primary,
    );
    ui.add_space(12.0);
    ui.label(
        RichText::new(app.tab().title())
            .size(17.0)
            .color(p.on_surface),
    );
    ui.label(
        RichText::new("No selection")
            .size(13.0)
            .color(p.on_surface_variant),
    );
    ui.add_space(10.0);
    let tab = app.tab();
    let shown = tab.view.len();
    let hidden = tab.entries.iter().filter(|e| e.hidden).count();
    meta_row(ui, "Items", &shown.to_string());
    if hidden > 0 && !app.config.ui.show_hidden {
        meta_row(ui, "Hidden", &format!("{hidden} (Ctrl+H to show)"));
    }
    let total: u64 = tab
        .entries
        .iter()
        .filter(|e| !e.is_dir)
        .map(|e| e.size)
        .sum();
    meta_row(ui, "Files size", &format_size(total));
    meta_row(ui, "Location", &places::tilde(&folder));
    if let Some(drive) = places::drive_for(&app.drives, &folder) {
        ui.add_space(10.0);
        ui.label(
            RichText::new(format!(
                "{} — {} free",
                drive.name,
                format_size(drive.available)
            ))
            .size(12.5)
            .color(p.on_surface_variant),
        );
        ui.add_space(4.0);
        widgets::linear_progress(ui, Some(drive.used_fraction()), ui.available_width());
    }
    ui.add_space(14.0);
    if widgets::button(ui, ButtonKind::Tonal, Some(ICON_INFO), "Folder properties").clicked() {
        actions.push(Action::Properties(folder));
    }
}
