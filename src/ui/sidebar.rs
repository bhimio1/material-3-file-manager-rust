//! Navigation drawer: dashboard, places, pinned folders, drives, settings.

use super::widgets::{self, menu_item, nav_item, section_header};
use super::{Action, DragPaths};
use crate::app::App;
use crate::fs::entry::format_size;
use crate::fs::places;
use crate::tab::Location;
use egui::{pos2, vec2, CornerRadius, Rect, Response, RichText, Sense, Ui};
use egui_material_icons::icons::*;
use std::path::{Path, PathBuf};

/// Accept files dropped from the file view onto a sidebar destination.
fn drop_target(ui: &Ui, response: &Response, target: &Path, actions: &mut Vec<Action>) {
    if let Some(payload) = response.dnd_release_payload::<DragPaths>() {
        actions.push(Action::Drop {
            paths: payload.0.clone(),
            target: target.to_path_buf(),
            copy: ui.input(|i| i.modifiers.command),
        });
    }
}

fn folder_menu(response: &Response, path: &Path, pinned: bool, actions: &mut Vec<Action>) {
    response.context_menu(|ui| {
        if menu_item(ui, ICON_OPEN_IN_NEW, "Open in new tab", "").clicked() {
            actions.push(Action::OpenInNewTab(Location::Folder(path.to_path_buf())));
        }
        if menu_item(ui, ICON_TERMINAL, "Open in terminal", "").clicked() {
            actions.push(Action::OpenInTerminal(path.to_path_buf()));
        }
        ui.separator();
        if pinned {
            if menu_item(ui, ICON_KEEP_OFF, "Unpin", "").clicked() {
                actions.push(Action::Unpin(path.to_path_buf()));
            }
        } else if menu_item(ui, ICON_KEEP, "Pin", "").clicked() {
            actions.push(Action::Pin(path.to_path_buf()));
        }
        if menu_item(ui, ICON_INFO, "Properties", "").clicked() {
            actions.push(Action::Properties(path.to_path_buf()));
        }
    });
}

fn folder_item(
    ui: &mut Ui,
    icon: egui_material_icons::MaterialIcon,
    label: &str,
    path: &Path,
    current: Option<&Path>,
    pinned: bool,
    actions: &mut Vec<Action>,
) -> Response {
    let selected = current == Some(path);
    let response = nav_item(ui, icon, label, selected, None).on_hover_text(path.to_string_lossy());
    if response.clicked() {
        actions.push(Action::Navigate(Location::Folder(path.to_path_buf())));
    }
    if response.middle_clicked() {
        actions.push(Action::OpenInNewTab(Location::Folder(path.to_path_buf())));
    }
    drop_target(ui, &response, path, actions);
    folder_menu(&response, path, pinned, actions);
    response
}

pub fn show(ui: &mut Ui, app: &mut App) {
    let p = app.palette.clone();
    let mut actions = Vec::new();
    let current: Option<PathBuf> = app.tab().folder().map(Path::to_path_buf);
    let location = app.tab().location.clone();

    // Header + "New" extended FAB.
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        widgets::icon(ui, ICON_FOLDER_OPEN, 26.0, p.primary);
        ui.label(widgets::title_text("Files", 20.0).color(p.on_surface));
    });
    ui.add_space(14.0);

    let fab_size = vec2(ui.available_width().min(140.0), 52.0);
    let (rect, fab) = ui.allocate_exact_size(fab_size, Sense::click());
    let fab_fill = widgets::state_layer(p.primary_container, p.on_primary_container, &fab);
    ui.painter()
        .rect_filled(rect, CornerRadius::same(16), fab_fill);
    widgets::paint_icon(
        ui,
        ICON_ADD,
        pos2(rect.left() + 28.0, rect.center().y),
        24.0,
        p.on_primary_container,
    );
    ui.painter().text(
        pos2(rect.left() + 50.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "New",
        widgets::title_font(15.0),
        p.on_primary_container,
    );
    let fab = fab.on_hover_cursor(egui::CursorIcon::PointingHand);
    egui::Popup::menu(&fab).show(|ui| {
        let dir = current.clone();
        if widgets::menu_item_enabled(
            ui,
            dir.is_some(),
            ICON_CREATE_NEW_FOLDER,
            "New folder",
            "Ctrl+Shift+N",
        )
        .clicked()
        {
            actions.extend(dir.clone().map(Action::NewFolder));
        }
        if widgets::menu_item_enabled(ui, dir.is_some(), ICON_NOTE_ADD, "New empty file", "")
            .clicked()
        {
            actions.extend(dir.clone().map(Action::NewFile));
        }
        ui.separator();
        if menu_item(ui, ICON_TAB, "New tab", "Ctrl+T").clicked() {
            actions.push(Action::NewTab);
        }
    });
    ui.add_space(12.0);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(ui.available_height() - 52.0)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            if nav_item(
                ui,
                ICON_SPACE_DASHBOARD,
                "Dashboard",
                location == Location::Dashboard,
                None,
            )
            .clicked()
            {
                actions.push(Action::Navigate(Location::Dashboard));
            }

            section_header(ui, "Places");
            for place in app.places.clone() {
                folder_item(
                    ui,
                    place.icon,
                    place.name,
                    &place.path,
                    current.as_deref(),
                    app.config.is_pinned(&place.path),
                    &mut actions,
                );
            }

            section_header(ui, "Pinned");
            if app.config.pinned_folders.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new("Drag a folder here or press Ctrl+D")
                            .size(12.0)
                            .color(p.on_surface_variant),
                    );
                });
            }
            for path in app.config.pinned_folders.clone() {
                let name = places::display_name(&path);
                let icon = if path.exists() {
                    ICON_KEEP
                } else {
                    ICON_FOLDER_OFF
                };
                folder_item(
                    ui,
                    icon,
                    &name,
                    &path,
                    current.as_deref(),
                    true,
                    &mut actions,
                );
            }
            // Dropping a folder on the section pins it.
            let (zone, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
            let zone_resp = ui.interact(
                zone.expand2(vec2(0.0, 4.0)),
                ui.scope_id().with("pin-drop"),
                Sense::hover(),
            );
            if let Some(payload) = zone_resp.dnd_release_payload::<DragPaths>() {
                for path in payload.0.iter().filter(|p| p.is_dir()) {
                    actions.push(Action::Pin(path.clone()));
                }
            }
            if zone_resp.dnd_hover_payload::<DragPaths>().is_some() {
                ui.painter()
                    .rect_filled(zone, CornerRadius::same(3), p.primary);
            }

            section_header(ui, "Drives");
            for drive in app.drives.clone() {
                let selected = current.as_deref() == Some(drive.mount.as_path());
                let response = nav_item(ui, drive.icon(), &drive.name, selected, None)
                    .on_hover_text(format!(
                        "{}\n{} free of {} · {}",
                        drive.mount.display(),
                        format_size(drive.available),
                        format_size(drive.total),
                        drive.file_system
                    ));
                // Thin usage bar under the label.
                let r = response.rect;
                let bar = Rect::from_min_size(
                    pos2(r.left() + 50.0, r.bottom() - 9.0),
                    vec2((r.width() - 70.0).max(10.0), 3.0),
                );
                ui.painter()
                    .rect_filled(bar, CornerRadius::same(2), p.surface_container_highest);
                let used = Rect::from_min_size(
                    bar.min,
                    vec2(bar.width() * drive.used_fraction(), bar.height()),
                );
                let color = if drive.used_fraction() > 0.9 {
                    p.error
                } else {
                    p.primary
                };
                ui.painter().rect_filled(used, CornerRadius::same(2), color);
                if response.clicked() {
                    actions.push(Action::Navigate(Location::Folder(drive.mount.clone())));
                }
                drop_target(ui, &response, &drive.mount, &mut actions);
                folder_menu(
                    &response,
                    &drive.mount,
                    app.config.is_pinned(&drive.mount),
                    &mut actions,
                );
            }
        });

    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        if nav_item(
            ui,
            ICON_SETTINGS,
            "Settings",
            location == Location::Settings,
            Some("Ctrl+,"),
        )
        .clicked()
        {
            actions.push(Action::Navigate(Location::Settings));
        }
    });

    app.actions.extend(actions);
}
