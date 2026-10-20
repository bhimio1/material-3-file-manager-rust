//! Tab strip above the toolbar.

use super::widgets::{self, icon_button, paint_icon, truncated_galley};
use super::{Action, DragPaths};
use crate::app::App;
use crate::fs::places;
use crate::tab::Location;
use egui::{pos2, vec2, CornerRadius, FontId, Rect, Sense, Ui};
use egui_material_icons::icons::*;

const TAB_W: f32 = 200.0;
const TAB_H: f32 = 36.0;

pub fn show(ui: &mut Ui, app: &mut App) {
    let p = app.palette.clone();
    let mut actions = Vec::new();
    let n = app.tabs.len();
    let available = ui.available_width() - 48.0;
    let tab_w = (available / n as f32).clamp(110.0, TAB_W);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        egui::ScrollArea::horizontal()
            .max_width(available)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (i, tab) in app.tabs.iter().enumerate() {
                        let active = i == app.active;
                        let (rect, response) =
                            ui.allocate_exact_size(vec2(tab_w, TAB_H), Sense::click());
                        let hovered = response.hovered();
                        let base = if active {
                            p.secondary_container
                        } else {
                            egui::Color32::TRANSPARENT
                        };
                        let fill = if active {
                            widgets::state_layer(base, p.on_secondary_container, &response)
                        } else if hovered || response.dnd_hover_payload::<DragPaths>().is_some() {
                            p.on_surface.gamma_multiply(0.08)
                        } else {
                            base
                        };
                        ui.painter().rect_filled(rect, CornerRadius::same(18), fill);
                        let fg = if active {
                            p.on_secondary_container
                        } else {
                            p.on_surface_variant
                        };
                        let icon = match &tab.location {
                            Location::Dashboard => ICON_SPACE_DASHBOARD,
                            Location::Settings => ICON_SETTINGS,
                            Location::Folder(path) => places::folder_icon(path),
                        };
                        paint_icon(
                            ui,
                            icon,
                            pos2(rect.left() + 20.0, rect.center().y),
                            18.0,
                            fg,
                        );
                        let show_close = n > 1 && (active || hovered);
                        let text_w = rect.width() - 38.0 - if show_close { 30.0 } else { 10.0 };
                        let loading = tab.loading || tab.search.is_running();
                        let title = truncated_galley(
                            ui,
                            &tab.title(),
                            if active {
                                widgets::title_font(13.5)
                            } else {
                                FontId::proportional(13.5)
                            },
                            fg,
                            text_w,
                        );
                        ui.painter().galley(
                            pos2(rect.left() + 36.0, rect.center().y - title.size().y / 2.0),
                            title,
                            fg,
                        );

                        let mut close_clicked = false;
                        if show_close {
                            let close_rect = Rect::from_center_size(
                                pos2(rect.right() - 18.0, rect.center().y),
                                vec2(24.0, 24.0),
                            );
                            let close = ui.interact(
                                close_rect,
                                ui.scope_id().with(("tab-close", i)),
                                Sense::click(),
                            );
                            if close.hovered() {
                                ui.painter().circle_filled(
                                    close_rect.center(),
                                    12.0,
                                    p.on_surface.gamma_multiply(0.1),
                                );
                            }
                            paint_icon(ui, ICON_CLOSE, close_rect.center(), 16.0, fg);
                            close_clicked = close.clicked();
                        } else if loading {
                            let r = Rect::from_center_size(
                                pos2(rect.right() - 18.0, rect.center().y),
                                vec2(14.0, 14.0),
                            );
                            egui::Spinner::new().size(14.0).color(fg).paint_at(ui, r);
                        }

                        let response = response.on_hover_text(match &tab.location {
                            Location::Folder(path) => path.to_string_lossy().into_owned(),
                            _ => tab.title(),
                        });
                        if close_clicked || response.middle_clicked() {
                            actions.push(Action::CloseTab(i));
                        } else if response.clicked() {
                            actions.push(Action::ActivateTab(i));
                        }
                        if let (Some(payload), Some(folder)) =
                            (response.dnd_release_payload::<DragPaths>(), tab.folder())
                        {
                            actions.push(Action::Drop {
                                paths: payload.0.clone(),
                                target: folder.to_path_buf(),
                                copy: ui.input(|i| i.modifiers.command),
                            });
                        }
                        response.context_menu(|ui| {
                            if widgets::menu_item(ui, ICON_TAB, "New tab", "Ctrl+T").clicked() {
                                actions.push(Action::NewTab);
                            }
                            if let Location::Folder(path) = &tab.location {
                                if widgets::menu_item(ui, ICON_CONTENT_COPY, "Duplicate tab", "")
                                    .clicked()
                                {
                                    actions.push(Action::ActivateTab(i));
                                    actions
                                        .push(Action::OpenInNewTab(Location::Folder(path.clone())));
                                }
                            }
                            ui.separator();
                            if widgets::menu_item_enabled(
                                ui,
                                n > 1,
                                ICON_CLOSE,
                                "Close tab",
                                "Ctrl+W",
                            )
                            .clicked()
                            {
                                actions.push(Action::CloseTab(i));
                            }
                            if widgets::menu_item_enabled(
                                ui,
                                n > 1,
                                ICON_TAB_CLOSE,
                                "Close other tabs",
                                "",
                            )
                            .clicked()
                            {
                                for j in (0..n).rev().filter(|j| *j != i) {
                                    actions.push(Action::CloseTab(j));
                                }
                            }
                        });
                    }
                });
            });
        if icon_button(ui, ICON_ADD, "New tab (Ctrl+T)", true).clicked() {
            actions.push(Action::NewTab);
        }
    });
    ui.add_space(2.0);
    app.actions.extend(actions);
}
