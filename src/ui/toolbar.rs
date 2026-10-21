//! Navigation buttons, breadcrumb/location bar, search and view controls.

use super::widgets::{self, filter_chip, icon_button, menu_item, paint_icon, segmented};
use super::{Action, DragPaths};
use crate::app::App;
use crate::config::{SortKey, ViewMode};
use crate::fs::places;
use crate::fs::scanner;
use crate::tab::Location;
use egui::{
    pos2, vec2, Color32, CornerRadius, FontId, Id, Key, Modifiers, Rect, RichText, Sense, Ui,
};
use material_3_ui::icons::icons::*;
use material_3_ui::sys::paths;
use std::path::{Path, PathBuf};

const BAR_H: f32 = 40.0;

pub fn show(ui: &mut Ui, app: &mut App) {
    let mut actions = Vec::new();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let tab = app.tab();
        if icon_button(ui, ICON_ARROW_BACK, "Back (Alt+←)", tab.can_go_back()).clicked() {
            actions.push(Action::Back);
        }
        if icon_button(
            ui,
            ICON_ARROW_FORWARD,
            "Forward (Alt+→)",
            tab.can_go_forward(),
        )
        .clicked()
        {
            actions.push(Action::Forward);
        }
        if icon_button(
            ui,
            ICON_ARROW_UPWARD,
            "Parent folder (Alt+↑)",
            tab.can_go_up(),
        )
        .clicked()
        {
            actions.push(Action::Up);
        }
        if icon_button(ui, ICON_REFRESH, "Reload (F5)", true).clicked() {
            actions.push(Action::Reload);
        }
        ui.add_space(6.0);

        // Right-hand controls are laid out first (right-to-left) so the
        // location bar can take whatever width is left.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            more_menu(ui, app, &mut actions);
            sort_menu(ui, app, &mut actions);
            let mut mode = app.config.ui.view_mode;
            if segmented(
                ui,
                &mut mode,
                &[
                    (ViewMode::List, ICON_VIEW_LIST, ""),
                    (ViewMode::Grid, ICON_GRID_VIEW, ""),
                ],
            ) {
                app.config.ui.view_mode = mode;
                actions.push(Action::ConfigChanged);
            }
            ui.add_space(4.0);
            search_field(ui, app, &mut actions);
            ui.add_space(4.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                location_bar(ui, app, &mut actions);
            });
        });
    });

    if app.tab().search.is_active() {
        search_options_row(ui, app);
    }
    app.actions.extend(actions);
}

// ------------------------------------------------------------- location bar

fn segments(path: &Path) -> Vec<(String, PathBuf)> {
    let home = places::home();
    let mut out = Vec::new();
    let cloud = places::cloud_root(path);
    let rest: &Path = if let Some((root, name)) = &cloud {
        out.push((name.clone(), root.clone()));
        path.strip_prefix(root).unwrap_or(Path::new(""))
    } else if let Ok(rel) = path.strip_prefix(&home) {
        out.push(("Home".to_string(), home.clone()));
        rel
    } else {
        out.push(("/".to_string(), PathBuf::from("/")));
        path.strip_prefix("/").unwrap_or(path)
    };
    let mut acc = out[0].1.clone();
    for comp in rest.components() {
        acc.push(comp);
        out.push((comp.as_os_str().to_string_lossy().into_owned(), acc.clone()));
    }
    out
}

fn location_bar(ui: &mut Ui, app: &mut App, actions: &mut Vec<Action>) {
    let p = app.palette.clone();
    let Some(folder) = app.tab().folder().map(Path::to_path_buf) else {
        return;
    };
    let width = ui.available_width().max(120.0);
    let (rect, bg_resp) = ui.allocate_exact_size(vec2(width, BAR_H), Sense::click());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(20), p.surface_container_high);

    if app.tab().path_edit.is_some() {
        path_editor(ui, app, rect, actions);
        return;
    }
    if bg_resp.clicked() {
        actions.push(Action::EditPath);
    }
    let bg_resp = bg_resp.on_hover_cursor(egui::CursorIcon::Text);
    let _ = bg_resp;

    let segs = segments(&folder);
    let font = FontId::proportional(14.0);
    let seg_widths: Vec<f32> = segs
        .iter()
        .enumerate()
        .map(|(i, (label, _))| {
            let text_w = ui
                .painter()
                .layout_no_wrap(label.clone(), font.clone(), Color32::WHITE)
                .size()
                .x
                .min(220.0);
            text_w + 20.0 + if i == 0 { 22.0 } else { 0.0 }
        })
        .collect();
    let chevron_w = 18.0;
    let avail = rect.width() - 52.0;
    // Hide leading segments (after the first) until the rest fits.
    let mut first_shown = 1;
    let total = |from: usize| -> f32 {
        seg_widths[0]
            + seg_widths[from..].iter().sum::<f32>()
            + chevron_w * (segs.len() - from) as f32
            + if from > 1 { 40.0 } else { 0.0 }
    };
    while first_shown < segs.len().saturating_sub(1) && total(first_shown) > avail {
        first_shown += 1;
    }

    let mut x = rect.left() + 6.0;
    let cy = rect.center().y;
    let draw_segment = |ui: &mut Ui, i: usize, x: &mut f32, actions: &mut Vec<Action>| {
        let (label, path) = &segs[i];
        let w = seg_widths[i];
        let r = Rect::from_min_size(pos2(*x, cy - 15.0), vec2(w, 30.0));
        *x += w;
        let resp = ui.interact(r, Id::unique(("crumb", i, path)), Sense::click());
        let last = i + 1 == segs.len();
        let fg = if last {
            p.on_surface
        } else {
            p.on_surface_variant
        };
        let drop_hover = resp.dnd_hover_payload::<DragPaths>().is_some();
        if resp.hovered() || drop_hover {
            ui.painter().rect_filled(
                r,
                CornerRadius::same(15),
                if drop_hover {
                    p.primary_container
                } else {
                    p.on_surface.gamma_multiply(0.08)
                },
            );
        }
        let mut tx = r.left() + 10.0;
        if i == 0 {
            let icon = if label == "Home" {
                ICON_HOME
            } else if path != Path::new("/") {
                ICON_CLOUD
            } else {
                ICON_HARD_DRIVE
            };
            paint_icon(ui, icon, pos2(tx + 9.0, cy), 18.0, fg);
            tx += 22.0;
        }
        let galley = widgets::truncated_galley(ui, label, font.clone(), fg, 220.0);
        ui.painter()
            .galley(pos2(tx, cy - galley.size().y / 2.0), galley, fg);
        if resp.clicked() {
            actions.push(Action::Navigate(Location::Folder(path.clone())));
        }
        if resp.middle_clicked() {
            actions.push(Action::OpenInNewTab(Location::Folder(path.clone())));
        }
        if let Some(payload) = resp.dnd_release_payload::<DragPaths>() {
            actions.push(Action::Drop {
                paths: payload.0.clone(),
                target: path.clone(),
                copy: ui.input(|i| i.modifiers.command),
            });
        }
        let path = path.clone();
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
            .context_menu(|ui| {
                if menu_item(ui, Some(ICON_OPEN_IN_NEW), "Open in new tab", "", false) {
                    actions.push(Action::OpenInNewTab(Location::Folder(path.clone())));
                }
                if menu_item(ui, Some(ICON_CONTENT_COPY), "Copy path", "", false) {
                    actions.push(Action::CopyPathText(vec![path.clone()]));
                }
                if menu_item(ui, Some(ICON_TERMINAL), "Open in terminal", "", false) {
                    actions.push(Action::OpenInTerminal(path.clone()));
                }
            });
    };
    let chevron = |ui: &Ui, x: &mut f32| {
        paint_icon(
            ui,
            ICON_CHEVRON_RIGHT,
            pos2(*x + chevron_w / 2.0, cy),
            16.0,
            p.on_surface_variant,
        );
        *x += chevron_w;
    };

    draw_segment(ui, 0, &mut x, actions);
    if first_shown > 1 {
        chevron(ui, &mut x);
        let r = Rect::from_min_size(pos2(x, cy - 15.0), vec2(36.0, 30.0));
        x += 36.0;
        let more = ui.interact(r, Id::unique("crumb-more"), Sense::click());
        if more.hovered() {
            ui.painter()
                .rect_filled(r, CornerRadius::same(15), p.on_surface.gamma_multiply(0.08));
        }
        paint_icon(ui, ICON_MORE_HORIZ, r.center(), 18.0, p.on_surface_variant);
        egui::Popup::menu(&more).show(|ui| {
            for (label, path) in &segs[1..first_shown] {
                if menu_item(ui, Some(ICON_FOLDER), label, "", false) {
                    actions.push(Action::Navigate(Location::Folder(path.clone())));
                }
            }
        });
    }
    for i in first_shown..segs.len() {
        chevron(ui, &mut x);
        draw_segment(ui, i, &mut x, actions);
    }

    // Copy-path affordance at the right end.
    let copy_rect = Rect::from_center_size(pos2(rect.right() - 22.0, cy), vec2(32.0, 32.0));
    let copy = ui.interact(copy_rect, Id::unique("crumb-edit"), Sense::click());
    if copy.hovered() {
        ui.painter()
            .circle_filled(copy_rect.center(), 16.0, p.on_surface.gamma_multiply(0.08));
    }
    paint_icon(
        ui,
        ICON_EDIT,
        copy_rect.center(),
        18.0,
        p.on_surface_variant,
    );
    if copy.on_hover_text("Type a location (Ctrl+L)").clicked() {
        actions.push(Action::EditPath);
    }
}

fn path_editor(ui: &mut Ui, app: &mut App, rect: Rect, actions: &mut Vec<Action>) {
    let p = app.palette.clone();
    let id = Id::unique("path-editor");
    let tab_pressed = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab));
    let Some(text) = app.tab_mut().path_edit.as_mut() else {
        return;
    };
    if tab_pressed {
        let matches = scanner::complete_dir(text);
        if let [only] = matches.as_slice() {
            *text = format!("{}/", only.to_string_lossy());
        } else if let Some(first) = matches.first() {
            // Extend to the longest common prefix.
            let mut prefix = first.to_string_lossy().into_owned();
            for m in &matches[1..] {
                let s = m.to_string_lossy();
                while !s.starts_with(&prefix) {
                    prefix.pop();
                }
            }
            if prefix.len() > text.len() {
                *text = prefix;
            }
        }
        // Keep the caret at the end of the completed text.
        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id) {
            let end = egui::text::CCursor::new(text.chars().count());
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(end)));
            state.store(ui.ctx(), id);
        }
    }
    let inner = Rect::from_min_max(
        pos2(rect.left() + 16.0, rect.center().y - 10.0),
        pos2(rect.right() - 16.0, rect.center().y + 10.0),
    );
    let output = egui::TextEdit::singleline(text)
        .id(id)
        .frame(egui::Frame::NONE)
        .font(FontId::proportional(14.0))
        .hint_text("Type a path, Tab to complete")
        .desired_width(inner.width())
        .lock_focus(true)
        .show(
            &mut ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            ),
        );
    let resp = output.response;
    if !resp.has_focus() && !resp.lost_focus() {
        resp.request_focus();
        let mut state = output.state;
        let end = egui::text::CCursor::new(text.chars().count());
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(end)));
        state.store(ui.ctx(), id);
    }
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(20),
        egui::Stroke::new(2.0, p.primary),
        egui::StrokeKind::Inside,
    );

    let suggestions = if text.len() > 1 {
        scanner::complete_dir(text)
    } else {
        Vec::new()
    };
    let enter = ui.input(|i| i.key_pressed(Key::Enter));
    let escape = ui.input(|i| i.key_pressed(Key::Escape));
    let mut close = false;
    if escape || (resp.lost_focus() && !enter) {
        close = true;
    } else if enter {
        let typed = expand_tilde(text.trim());
        if typed.is_dir() {
            actions.push(Action::Navigate(Location::Folder(typed)));
            close = true;
        } else if typed.is_file() {
            actions.push(Action::Open(typed));
            close = true;
        } else {
            actions.push(Action::Toast(
                format!("“{}” doesn't exist", typed.display()),
                super::ToastKind::Error,
            ));
            resp.request_focus();
        }
    }

    if !suggestions.is_empty() && !close {
        egui::Area::new(Id::unique("path-suggestions"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.left_bottom() + vec2(12.0, 4.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(rect.width() - 24.0);
                    for s in &suggestions {
                        let label = s.to_string_lossy();
                        if menu_item(ui, Some(ICON_FOLDER), &label, "", false) {
                            actions.push(Action::Navigate(Location::Folder(s.clone())));
                            close = true;
                        }
                    }
                    ui.label(
                        RichText::new("Tab to complete · Enter to go · Esc to cancel")
                            .size(11.5)
                            .color(p.on_surface_variant),
                    );
                });
            });
    }
    if close {
        app.tab_mut().path_edit = None;
    }
}

/// Expand `~` and normalise away trailing slashes and `.` components.
fn expand_tilde(s: &str) -> PathBuf {
    paths::expand(s).components().collect()
}

// ------------------------------------------------------------------- search

fn search_field(ui: &mut Ui, app: &mut App, actions: &mut Vec<Action>) {
    let p = app.palette.clone();
    let id = Id::unique("search-field").with(app.active);
    let focused = ui.memory(|m| m.has_focus(id));
    let active = app.tab().search.is_active();
    let target_w = if focused || active { 300.0 } else { 220.0 };
    let w = ui
        .ctx()
        .animate_value_with_time(Id::unique("search-w"), target_w, 0.12);
    let (rect, _) = ui.allocate_exact_size(vec2(w, BAR_H), Sense::hover());
    let fill = if focused {
        p.surface_container_highest
    } else {
        p.surface_container_high
    };
    ui.painter().rect_filled(rect, CornerRadius::same(20), fill);
    if focused {
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(20),
            egui::Stroke::new(2.0, p.primary),
            egui::StrokeKind::Inside,
        );
    }
    paint_icon(
        ui,
        ICON_SEARCH,
        pos2(rect.left() + 22.0, rect.center().y),
        20.0,
        p.on_surface_variant,
    );

    let tab = app.tab_mut();
    let mut query = tab.search.query.clone();
    let hint = format!("Search {}", tab.title());
    let clear_w = if query.is_empty() { 0.0 } else { 32.0 };
    let inner = Rect::from_min_max(
        pos2(rect.left() + 40.0, rect.center().y - 10.0),
        pos2(rect.right() - 10.0 - clear_w, rect.center().y + 10.0),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let resp = child.add(
        egui::TextEdit::singleline(&mut query)
            .id(id)
            .frame(egui::Frame::NONE)
            .hint_text(hint)
            .font(FontId::proportional(14.0))
            .desired_width(inner.width()),
    );
    if tab.focus_search {
        tab.focus_search = false;
        resp.request_focus();
    }
    if resp.changed() {
        tab.set_query(query.clone());
    }
    if resp.has_focus() {
        let (esc, down) = ui.input(|i| (i.key_pressed(Key::Escape), i.key_pressed(Key::ArrowDown)));
        if esc {
            tab.clear_search();
            resp.surrender_focus();
        } else if down {
            resp.surrender_focus();
            tab.move_cursor(1, false);
        }
    }
    if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
        tab.move_cursor(0, false);
    }
    if !query.is_empty() {
        let r =
            Rect::from_center_size(pos2(rect.right() - 24.0, rect.center().y), vec2(28.0, 28.0));
        let clear = ui.interact(r, id.with("clear"), Sense::click());
        if clear.hovered() {
            ui.painter()
                .circle_filled(r.center(), 14.0, p.on_surface.gamma_multiply(0.08));
        }
        paint_icon(ui, ICON_CLOSE, r.center(), 18.0, p.on_surface_variant);
        if clear.on_hover_text("Clear search (Esc)").clicked() {
            tab.clear_search();
        }
    }
    let _ = actions;
}

fn app_cloud_folder(folder: &Path, mounts: &[PathBuf]) -> bool {
    mounts.iter().any(|m| folder.starts_with(m))
}

fn search_options_row(ui: &mut Ui, app: &mut App) {
    let p = app.palette.clone();
    let cloud_mounts: Vec<PathBuf> = app
        .config
        .cloud_drives
        .iter()
        .map(|d| d.mount_point.clone())
        .collect();
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(156.0);
        let tab = app.tab_mut();
        let (mut recursive, mut content) = (tab.search.recursive, tab.search.content);
        if filter_chip(
            ui,
            "Include subfolders",
            recursive,
            Some(ICON_SUBDIRECTORY_ARROW_RIGHT),
        )
        .clicked()
        {
            recursive = !recursive;
        }
        let on_cloud = tab
            .folder()
            .map(|f| app_cloud_folder(f, &cloud_mounts))
            .unwrap_or(false);
        let chip = ui
            .add_enabled_ui(!on_cloud, |ui| {
                filter_chip(
                    ui,
                    "Search file contents",
                    content && !on_cloud,
                    Some(ICON_DESCRIPTION),
                )
            })
            .inner;
        if on_cloud {
            chip.on_disabled_hover_text(
                "Not available on cloud drives — it would download every file",
            );
        } else if chip.clicked() {
            content = !content;
        }
        if (recursive, content) != (tab.search.recursive, tab.search.content) {
            tab.set_search_options(recursive, content);
        }
        ui.add_space(8.0);
        let n = tab.view.len();
        let running = tab.search.is_running();
        if running {
            ui.add(egui::Spinner::new().size(16.0).color(p.primary));
        }
        let capped = tab.search.deep.as_ref().map(|d| d.capped).unwrap_or(false);
        let status = match (running, n, capped) {
            (true, _, _) => format!("Searching… {n} found"),
            (false, 0, _) => "No matches".to_string(),
            (false, n, true) => format!("First {n} results"),
            (false, 1, _) => "1 result".to_string(),
            (false, n, _) => format!("{n} results"),
        };
        ui.label(RichText::new(status).size(13.0).color(p.on_surface_variant));
    });
}

// ---------------------------------------------------------------- menus

fn sort_menu(ui: &mut Ui, app: &mut App, actions: &mut Vec<Action>) {
    let resp = icon_button(ui, ICON_SORT, "Sort", true);
    egui::Popup::menu(&resp).show(|ui| {
        ui.label(
            RichText::new("Sort by")
                .size(12.0)
                .color(app.palette.on_surface_variant),
        );
        for key in SortKey::ALL {
            let selected = app.config.ui.sort_by == key;
            if menu_item(ui, None, key.label(), "", selected) {
                if selected {
                    app.config.ui.sort_ascending = !app.config.ui.sort_ascending;
                } else {
                    app.config.ui.sort_by = key;
                    app.config.ui.sort_ascending = key == SortKey::Name || key == SortKey::Type;
                }
                actions.push(Action::ConfigChanged);
            }
        }
        ui.separator();
        let asc = app.config.ui.sort_ascending;
        if menu_item(
            ui,
            Some(if asc {
                ICON_ARROW_UPWARD
            } else {
                ICON_ARROW_DOWNWARD
            }),
            if asc { "Ascending" } else { "Descending" },
            "",
            false,
        ) {
            app.config.ui.sort_ascending = !asc;
            actions.push(Action::ConfigChanged);
        }
        let ff = app.config.ui.folders_first;
        if menu_item(ui, None, "Folders first", "", ff) {
            app.config.ui.folders_first = !ff;
            actions.push(Action::ConfigChanged);
        }
    });
}

fn more_menu(ui: &mut Ui, app: &mut App, actions: &mut Vec<Action>) {
    let resp = icon_button(ui, ICON_MORE_VERT, "More", true);
    let folder = app.tab().folder().map(Path::to_path_buf);
    egui::Popup::menu(&resp).show(|ui| {
        if menu_item(
            ui,
            None,
            "Show hidden files",
            "Ctrl+H",
            app.config.ui.show_hidden,
        ) {
            app.config.ui.show_hidden = !app.config.ui.show_hidden;
            actions.push(Action::ConfigChanged);
        }
        if menu_item(
            ui,
            None,
            "Group by type",
            "Ctrl+G",
            app.config.group_files_by_type,
        ) {
            app.config.group_files_by_type = !app.config.group_files_by_type;
            actions.push(Action::ConfigChanged);
        }
        if menu_item(ui, None, "Details pane", "F9", app.show_details) {
            actions.push(Action::ToggleDetails);
        }
        if menu_item(
            ui,
            None,
            "Image thumbnails",
            "",
            app.config.ui.show_thumbnails,
        ) {
            app.config.ui.show_thumbnails = !app.config.ui.show_thumbnails;
            actions.push(Action::ConfigChanged);
        }
        ui.separator();
        if let Some(dir) = folder {
            if menu_item(
                ui,
                Some(ICON_CREATE_NEW_FOLDER),
                "New folder",
                "Ctrl+Shift+N",
                false,
            ) {
                actions.push(Action::NewFolder(dir.clone()));
            }
            if menu_item(ui, Some(ICON_SELECT_ALL), "Select all", "Ctrl+A", false) {
                actions.push(Action::SelectAll);
            }
            let pinned = app.config.is_pinned(&dir);
            if menu_item(
                ui,
                Some(if pinned { ICON_KEEP_OFF } else { ICON_KEEP }),
                if pinned { "Unpin folder" } else { "Pin folder" },
                "Ctrl+D",
                false,
            ) {
                actions.push(if pinned {
                    Action::Unpin(dir.clone())
                } else {
                    Action::Pin(dir.clone())
                });
            }
            if menu_item(ui, Some(ICON_TERMINAL), "Open in terminal", "", false) {
                actions.push(Action::OpenInTerminal(dir.clone()));
            }
            if menu_item(ui, Some(ICON_INFO), "Folder properties", "", false) {
                actions.push(Action::Properties(dir));
            }
            ui.separator();
        }
        if menu_item(ui, Some(ICON_KEYBOARD), "Keyboard shortcuts", "F1", false) {
            actions.push(Action::ShowShortcuts);
        }
        if menu_item(ui, Some(ICON_SETTINGS), "Settings", "Ctrl+,", false) {
            actions.push(Action::Navigate(Location::Settings));
        }
    });
}
