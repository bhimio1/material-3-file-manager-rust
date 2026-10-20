//! Start page: greeting, storage, pinned and recent folders.

use super::widgets::{self, clickable_card, menu_item, paint_icon, truncated_galley, ButtonKind};
use super::{Action, DragPaths};
use crate::app::App;
use crate::fs::entry::format_size;
use crate::fs::places;
use crate::tab::Location;
use chrono::Timelike;
use egui::{pos2, vec2, Align2, CornerRadius, FontId, Id, Rect, RichText, Sense, Ui};
use egui_material_icons::icons::*;
use std::path::{Path, PathBuf};

fn greeting() -> &'static str {
    match chrono::Local::now().hour() {
        5..=11 => "Good morning",
        12..=16 => "Good afternoon",
        17..=20 => "Good evening",
        _ => "Good night",
    }
}

fn section_title(ui: &mut Ui, title: &str, trailing: impl FnOnce(&mut Ui)) {
    let p = widgets::palette(ui);
    ui.add_space(24.0);
    ui.horizontal(|ui| {
        ui.label(widgets::title_text(title, 16.0).color(p.on_surface));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), trailing);
    });
    ui.add_space(8.0);
}

/// Lay out fixed-height cards in responsive columns.
fn card_grid(
    ui: &mut Ui,
    count: usize,
    min_w: f32,
    h: f32,
    mut card: impl FnMut(&mut Ui, usize, Rect),
) {
    let gap = 12.0;
    let width = ui.available_width();
    let cols = (((width + gap) / (min_w + gap)).floor() as usize).max(1);
    let w = (width - gap * (cols as f32 - 1.0)) / cols as f32;
    let rows = count.div_ceil(cols);
    let (area, _) = ui.allocate_exact_size(
        vec2(width, rows as f32 * (h + gap) - gap.min(rows as f32 * gap)),
        Sense::hover(),
    );
    for i in 0..count {
        let (r, c) = (i / cols, i % cols);
        let rect = Rect::from_min_size(
            pos2(
                area.left() + c as f32 * (w + gap),
                area.top() + r as f32 * (h + gap),
            ),
            vec2(w, h),
        );
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        card(&mut child, i, rect);
    }
}

fn folder_card(ui: &mut Ui, rect: Rect, path: &Path, pinned: bool, actions: &mut Vec<Action>) {
    let p = widgets::palette(ui);
    let id = Id::unique(("dash-card", path, pinned));
    let (_, resp) = clickable_card(ui, rect.size(), p.surface_container, id);
    let exists = path.is_dir();
    let drop = resp.dnd_hover_payload::<DragPaths>().is_some();
    if drop {
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(16),
            egui::Stroke::new(2.0, p.primary),
            egui::StrokeKind::Inside,
        );
    }
    let icon_c = pos2(rect.left() + 32.0, rect.center().y);
    ui.painter().circle_filled(
        icon_c,
        20.0,
        if exists {
            p.primary_container
        } else {
            p.error_container
        },
    );
    let icon = if exists {
        places::folder_icon(path)
    } else {
        ICON_FOLDER_OFF
    };
    paint_icon(
        ui,
        icon,
        icon_c,
        22.0,
        if exists {
            p.on_primary_container
        } else {
            p.on_error_container
        },
    );
    let text_w = rect.width() - 72.0;
    let name = truncated_galley(
        ui,
        &places::display_name(path),
        widgets::title_font(14.5),
        p.on_surface,
        text_w,
    );
    let sub = truncated_galley(
        ui,
        &places::tilde(path),
        FontId::proportional(12.0),
        p.on_surface_variant,
        text_w,
    );
    ui.painter().galley(
        pos2(rect.left() + 62.0, rect.center().y - name.size().y),
        name,
        p.on_surface,
    );
    ui.painter().galley(
        pos2(rect.left() + 62.0, rect.center().y + 2.0),
        sub,
        p.on_surface_variant,
    );
    if resp.clicked() {
        actions.push(Action::Navigate(Location::Folder(path.to_path_buf())));
    }
    if resp.middle_clicked() {
        actions.push(Action::OpenInNewTab(Location::Folder(path.to_path_buf())));
    }
    if let Some(payload) = resp.dnd_release_payload::<DragPaths>() {
        actions.push(Action::Drop {
            paths: payload.0.clone(),
            target: path.to_path_buf(),
            copy: ui.input(|i| i.modifiers.command),
        });
    }
    let path = path.to_path_buf();
    resp.on_hover_text(path.to_string_lossy())
        .context_menu(|ui| {
            if menu_item(ui, ICON_OPEN_IN_NEW, "Open in new tab", "").clicked() {
                actions.push(Action::OpenInNewTab(Location::Folder(path.clone())));
            }
            if pinned {
                if menu_item(ui, ICON_KEEP_OFF, "Unpin", "").clicked() {
                    actions.push(Action::Unpin(path.clone()));
                }
            } else if menu_item(ui, ICON_KEEP, "Pin", "").clicked() {
                actions.push(Action::Pin(path.clone()));
            }
        });
}

pub fn show(ui: &mut Ui, app: &mut App) {
    let p = app.palette.clone();
    let mut actions = Vec::new();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin::symmetric(32, 28)).show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(1100.0));
            ui.label(RichText::new(greeting()).size(32.0).color(p.on_surface));
            ui.label(
                RichText::new(chrono::Local::now().format("%A, %B %-d").to_string())
                    .size(15.0)
                    .color(p.on_surface_variant),
            );

            // Quick jump chips.
            ui.add_space(16.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                for place in &app.places {
                    if widgets::filter_chip(ui, place.name, false, Some(place.icon)).clicked() {
                        actions.push(Action::Navigate(Location::Folder(place.path.clone())));
                    }
                }
            });

            section_title(ui, "Storage", |_| {});
            // Local drives, then cloud drives (or an invitation to connect one).
            let mut cards: Vec<StorageCard> = app
                .drives
                .iter()
                .map(|d| StorageCard {
                    id: d.mount.to_string_lossy().into_owned(),
                    icon: d.icon(),
                    name: d.name.clone(),
                    usage: Some((d.total.saturating_sub(d.available), d.total)),
                    detail: None,
                    hover: d.mount.to_string_lossy().into_owned(),
                    action: Action::Navigate(Location::Folder(d.mount.clone())),
                })
                .collect();
            for d in &app.config.cloud_drives {
                let mounted = app.cloud.mounted.contains(&d.remote);
                let quota = app.cloud.quota.get(&d.remote).filter(|_| mounted);
                let detail = match (app.cloud.busy.get(&d.remote), mounted, quota) {
                    (Some(b), _, _) => Some(b.to_string()),
                    (None, false, _) => Some("Not mounted — click to mount".into()),
                    (None, true, None) => Some("Mounted".into()),
                    (None, true, Some(_)) => None,
                };
                cards.push(StorageCard {
                    id: d.remote.clone(),
                    icon: if mounted { ICON_CLOUD } else { ICON_CLOUD_OFF },
                    name: d.name.clone(),
                    usage: quota.map(|q| (q.used, q.total)),
                    detail,
                    hover: places::tilde(&d.mount_point),
                    action: Action::CloudOpen(d.remote.clone()),
                });
            }
            if app.config.cloud_drives.is_empty() && matches!(app.cloud.rclone, Some(Some(_))) {
                cards.push(StorageCard {
                    id: "connect-gdrive".into(),
                    icon: ICON_ADD_TO_DRIVE,
                    name: "Google Drive".into(),
                    usage: None,
                    detail: Some("Connect your Drive to browse it here".into()),
                    hover: "Sign in with Google".into(),
                    action: Action::CloudConnect,
                });
            }
            card_grid(ui, cards.len(), 260.0, 96.0, |ui, i, rect| {
                if let Some(action) = storage_card(ui, rect, &cards[i]) {
                    actions.push(action);
                }
            });

            section_title(ui, "Pinned", |ui| {
                if widgets::button(ui, ButtonKind::Text, Some(ICON_ADD), "Add").clicked() {
                    actions.push(Action::PickFolderToPin);
                }
            });
            let pinned: Vec<PathBuf> = app.config.pinned_folders.clone();
            if pinned.is_empty() {
                ui.label(RichText::new("Pin folders you use often — right-click a folder and choose “Pin to sidebar”.").color(p.on_surface_variant));
            }
            card_grid(ui, pinned.len(), 240.0, 64.0, |ui, i, rect| folder_card(ui, rect, &pinned[i], true, &mut actions));

            let recent: Vec<PathBuf> = app.config.recent_folders.iter().cloned().collect();
            section_title(ui, "Recent", |ui| {
                if !recent.is_empty() && widgets::button(ui, ButtonKind::Text, Some(ICON_HISTORY), "Clear").clicked() {
                    actions.push(Action::ClearRecent);
                }
            });
            if recent.is_empty() {
                ui.label(RichText::new("Folders you visit will show up here.").color(p.on_surface_variant));
            }
            card_grid(ui, recent.len(), 240.0, 64.0, |ui, i, rect| folder_card(ui, rect, &recent[i], false, &mut actions));
            ui.add_space(24.0);
        });
    });
    app.actions.extend(actions);
}

struct StorageCard {
    id: String,
    icon: egui_material_icons::MaterialIcon,
    name: String,
    /// (used, total) bytes.
    usage: Option<(u64, u64)>,
    /// Shown instead of the usage line.
    detail: Option<String>,
    hover: String,
    action: Action,
}

fn storage_card(ui: &mut Ui, rect: Rect, card: &StorageCard) -> Option<Action> {
    let p = widgets::palette(ui);
    let (_, resp) = clickable_card(
        ui,
        rect.size(),
        p.surface_container,
        Id::unique(("storage", &card.id)),
    );
    paint_icon(
        ui,
        card.icon,
        pos2(rect.left() + 30.0, rect.top() + 30.0),
        24.0,
        p.primary,
    );
    ui.painter().text(
        pos2(rect.left() + 52.0, rect.top() + 30.0),
        Align2::LEFT_CENTER,
        &card.name,
        widgets::title_font(15.0),
        p.on_surface,
    );
    let small = FontId::proportional(12.5);
    if let (None, Some((used, total))) = (&card.detail, card.usage) {
        let fraction = if total == 0 {
            0.0
        } else {
            used as f32 / total as f32
        };
        ui.painter().text(
            pos2(rect.left() + 18.0, rect.top() + 58.0),
            Align2::LEFT_CENTER,
            format!("{} used of {}", format_size(used), format_size(total)),
            small.clone(),
            p.on_surface_variant,
        );
        ui.painter().text(
            pos2(rect.right() - 18.0, rect.top() + 58.0),
            Align2::RIGHT_CENTER,
            format!("{:.0}%", fraction * 100.0),
            small,
            p.on_surface_variant,
        );
        let bar = Rect::from_min_size(
            pos2(rect.left() + 18.0, rect.top() + 74.0),
            vec2(rect.width() - 36.0, 6.0),
        );
        ui.painter()
            .rect_filled(bar, CornerRadius::same(3), p.secondary_container);
        let fill = Rect::from_min_size(bar.min, vec2(bar.width() * fraction, bar.height()));
        ui.painter().rect_filled(
            fill,
            CornerRadius::same(3),
            if fraction > 0.9 { p.error } else { p.primary },
        );
    } else {
        let text = card.detail.clone().unwrap_or_default();
        ui.painter().text(
            pos2(rect.left() + 18.0, rect.top() + 62.0),
            Align2::LEFT_CENTER,
            text,
            small,
            p.on_surface_variant,
        );
    }
    resp.on_hover_text(&card.hover)
        .clicked()
        .then(|| card.action.clone())
}
