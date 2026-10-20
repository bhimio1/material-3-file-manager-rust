//! Settings page (opened as a tab).

use super::widgets::{self, segmented, switch, ButtonKind};
use super::{Action, ToastKind};
use crate::app::App;
use crate::config::{
    default_file_categories, default_pinned_folders, ColorScheme, Config, ThemeSource, ViewMode,
};
use crate::tab::Location;
use crate::theme::matugen;
use crate::theme::palette::{parse_hex, to_hex};
use egui::{vec2, Color32, CornerRadius, Frame, Margin, RichText, Ui};
use egui_material_icons::icons::*;
use std::collections::BTreeMap;

const SEED_PRESETS: &[&str] = &[
    "#4285F4", "#6750A4", "#B3261E", "#E8710A", "#1E8E3E", "#00897B", "#C2185B", "#795548",
    "#607D8B",
];

#[derive(Default)]
pub struct SettingsState {
    /// Comma-separated extension lists being edited, per category.
    categories: Option<BTreeMap<String, String>>,
    new_category: String,
    seed_text: Option<String>,
}

fn card(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    let p = widgets::palette(ui);
    ui.add_space(20.0);
    ui.label(widgets::title_text(title, 14.0).color(p.primary));
    ui.add_space(6.0);
    Frame::new()
        .fill(p.surface_container)
        .corner_radius(CornerRadius::same(20))
        .inner_margin(Margin::symmetric(20, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

/// A settings row: title and description on the left, control on the right.
fn row(ui: &mut Ui, title: &str, desc: &str, control: impl FnOnce(&mut Ui)) {
    let p = widgets::palette(ui);
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_max_width((ui.available_width() - 260.0).max(200.0));
            ui.label(RichText::new(title).size(14.5).color(p.on_surface));
            if !desc.is_empty() {
                ui.add(
                    egui::Label::new(RichText::new(desc).size(12.5).color(p.on_surface_variant))
                        .wrap(),
                );
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), control);
    });
    ui.add_space(8.0);
}

fn command_field(
    ui: &mut Ui,
    config: &mut Config,
    key: &str,
    hint: &str,
    actions: &mut Vec<Action>,
) {
    let mut value = config.commands.get(key).cloned().unwrap_or_default();
    let resp = ui.add(
        egui::TextEdit::singleline(&mut value)
            .hint_text(hint)
            .desired_width(220.0),
    );
    if resp.changed() {
        config.commands.insert(key.to_string(), value);
    }
    if resp.lost_focus() {
        actions.push(Action::ConfigChanged);
    }
}

pub fn show(ui: &mut Ui, app: &mut App) {
    let p = app.palette.clone();
    let mut actions: Vec<Action> = Vec::new();
    let effective_source = app.effective_theme_source();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        Frame::new().inner_margin(Margin::symmetric(32, 24)).show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(860.0));
            ui.horizontal(|ui| {
                widgets::icon(ui, ICON_SETTINGS, 30.0, p.primary);
                ui.label(RichText::new("Settings").size(28.0).color(p.on_surface));
            });
            ui.label(RichText::new("Changes apply immediately and are saved to config.toml.").color(p.on_surface_variant));

            let config = &mut app.config;
            let mut changed = false;

            card(ui, "Browsing", |ui| {
                row(ui, "Show hidden files", "Files and folders whose names start with a dot. (Ctrl+H)", |ui| {
                    changed |= switch(ui, &mut config.ui.show_hidden).changed();
                });
                ui.separator();
                row(ui, "Default view", "How folders are displayed. (Ctrl+1 / Ctrl+2)", |ui| {
                    changed |= segmented(ui, &mut config.ui.view_mode, &[(ViewMode::Grid, ICON_GRID_VIEW, "Grid"), (ViewMode::List, ICON_VIEW_LIST, "List")]);
                });
                ui.separator();
                row(ui, "Icon size", "Grid tile size. Ctrl+scroll in a folder to adjust quickly.", |ui| {
                    changed |= ui.add(egui::Slider::new(&mut config.ui.icon_size, 32..=160).suffix(" px")).changed();
                });
                ui.separator();
                row(ui, "Group by type", "Show folders, images, documents… under separate headings. (Ctrl+G)", |ui| {
                    changed |= switch(ui, &mut config.group_files_by_type).changed();
                });
                ui.separator();
                row(ui, "Folders first", "Keep folders above files regardless of sort order.", |ui| {
                    changed |= switch(ui, &mut config.ui.folders_first).changed();
                });
                ui.separator();
                row(ui, "Image thumbnails", "Show previews for pictures in the grid, list and details pane.", |ui| {
                    changed |= switch(ui, &mut config.ui.show_thumbnails).changed();
                });
            });

            card(ui, "Applications", |ui| {
                row(ui, "Terminal", "Command used by “Open in terminal”. Started in the folder.", |ui| {
                    command_field(ui, config, "terminal", "kitty", &mut actions);
                });
                ui.separator();
                row(ui, "Editor", "Command used by “Open in editor”.", |ui| {
                    command_field(ui, config, "editor", "code", &mut actions);
                });
                ui.separator();
                row(ui, "Open files with DMS", "Use `dms open` (DankMaterialShell) to open files, falling back to the default app.", |ui| {
                    changed |= switch(ui, &mut config.use_dms).changed();
                });
            });

            card(ui, "Cloud storage", |ui| {
                let cloud = &app.cloud;
                match &cloud.rclone {
                    None => row(ui, "rclone", "Checking whether rclone is installed…", |ui| {
                        ui.add(egui::Spinner::new().color(p.primary));
                    }),
                    Some(None) => row(
                        ui,
                        "rclone is required",
                        "Google Drive is mounted with rclone. Install it (and FUSE 3) from your package manager, then check again.",
                        |ui| {
                            if widgets::button(ui, ButtonKind::Tonal, Some(ICON_REFRESH), "Check again").clicked() {
                                actions.push(Action::CloudCheckRclone);
                            }
                            if widgets::button(ui, ButtonKind::Text, Some(ICON_OPEN_IN_NEW), "Install guide").clicked() {
                                actions.push(Action::OpenUrl(crate::fs::cloud::INSTALL_URL.into()));
                            }
                        },
                    ),
                    Some(Some(version)) => {
                        let mut mount_changed = false;
                        for drive in config.cloud_drives.iter_mut() {
                            let mounted = cloud.mounted.contains(&drive.remote);
                            let busy = cloud.busy.get(&drive.remote).copied();
                            let status = match (busy, mounted) {
                                (Some(b), _) => b.to_string(),
                                (None, true) => match cloud.quota.get(&drive.remote) {
                                    Some(q) => format!(
                                        "Mounted at {} · {} free of {}",
                                        crate::fs::places::tilde(&drive.mount_point),
                                        crate::fs::entry::format_size(q.free),
                                        crate::fs::entry::format_size(q.total)
                                    ),
                                    None => format!("Mounted at {}", crate::fs::places::tilde(&drive.mount_point)),
                                },
                                (None, false) => "Not mounted".to_string(),
                            };
                            row(ui, &format!("{} ({}:)", drive.name, drive.remote), &status, |ui| {
                                if widgets::button(ui, ButtonKind::Text, None, "Disconnect").clicked() {
                                    actions.push(Action::CloudDisconnect(drive.remote.clone()));
                                }
                                let (label, action) = if mounted {
                                    ("Unmount", Action::CloudUnmount(drive.remote.clone()))
                                } else {
                                    ("Mount", Action::CloudMount(drive.remote.clone()))
                                };
                                if widgets::button_enabled(ui, ButtonKind::Tonal, None, label, busy.is_none()).clicked() {
                                    actions.push(action);
                                }
                            });
                            row(ui, "Mount when the app starts", "Keeps working after the app closes, until you unmount or log out.", |ui| {
                                changed |= switch(ui, &mut drive.auto_mount).changed();
                            });
                            row(ui, "Mount folder", "Must be empty. Unmount to change it.", |ui| {
                                let mut text = drive.mount_point.to_string_lossy().into_owned();
                                let r = ui.add_enabled(!mounted, egui::TextEdit::singleline(&mut text).desired_width(240.0));
                                if r.changed() {
                                    drive.mount_point = std::path::PathBuf::from(text.trim());
                                }
                                mount_changed |= r.lost_focus();
                            });
                            ui.separator();
                        }
                        changed |= mount_changed;
                        let connecting = cloud.connect.is_some() || cloud.preparing_connect;
                        let (title, desc) = if config.cloud_drives.is_empty() {
                            ("Google Drive", format!("Sign in with Google and browse your Drive like any folder (rclone {version})."))
                        } else {
                            ("Another Google account", "Connect an additional Drive.".to_string())
                        };
                        row(ui, title, &desc, |ui| {
                            if widgets::button_enabled(ui, ButtonKind::Filled, Some(ICON_ADD_TO_DRIVE), "Connect", !connecting).clicked() {
                                actions.push(Action::CloudConnect);
                            }
                        });
                    }
                }
            });

            card(ui, "Appearance", |ui| {
                row(ui, "Color source", match effective_source {
                    ThemeSource::Matugen => "Using colors from matugen's theme.json (live-reloads when it changes).",
                    _ => "Using a palette generated from the seed color below.",
                }, |ui| {
                    changed |= segmented(ui, &mut config.ui.theme_source, &[
                        (ThemeSource::Auto, ICON_AUTO_AWESOME, "Auto"),
                        (ThemeSource::Matugen, ICON_WALLPAPER, "Matugen"),
                        (ThemeSource::Seed, ICON_PALETTE, "Seed"),
                    ]);
                });
                ui.separator();
                let seed_mode = effective_source == ThemeSource::Seed;
                ui.add_enabled_ui(seed_mode, |ui| {
                    row(ui, "Color scheme", if seed_mode { "Light, dark, or follow the system." } else { "matugen decides light or dark." }, |ui| {
                        changed |= segmented(ui, &mut config.ui.color_scheme, &[
                            (ColorScheme::System, ICON_BRIGHTNESS_AUTO, "System"),
                            (ColorScheme::Light, ICON_LIGHT_MODE, "Light"),
                            (ColorScheme::Dark, ICON_DARK_MODE, "Dark"),
                        ]);
                    });
                    ui.separator();
                    row(ui, "Seed color", "Every color in the interface is derived from this one.", |ui| {
                        let text = app.settings.seed_text.get_or_insert_with(|| config.ui.seed_color.clone());
                        let resp = ui.add(egui::TextEdit::singleline(text).desired_width(92.0).char_limit(7));
                        if resp.changed() {
                            if let Some(c) = parse_hex(text) {
                                config.ui.seed_color = to_hex(c);
                                changed = true;
                            }
                        }
                        let mut color = parse_hex(&config.ui.seed_color).unwrap_or(Color32::from_rgb(0x42, 0x85, 0xF4));
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            config.ui.seed_color = to_hex(color);
                            *text = config.ui.seed_color.clone();
                            changed = true;
                        }
                        for hex in SEED_PRESETS.iter().rev() {
                            let c = parse_hex(hex).unwrap();
                            let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), egui::Sense::click());
                            ui.painter().circle_filled(rect.center(), 11.0, c);
                            if config.ui.seed_color.eq_ignore_ascii_case(hex) {
                                ui.painter().circle_stroke(rect.center(), 12.5, egui::Stroke::new(2.0, p.on_surface));
                            }
                            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                config.ui.seed_color = hex.to_string();
                                *text = hex.to_string();
                                changed = true;
                            }
                        }
                    });
                });
                ui.separator();
                row(ui, "Filled icons", "Use filled Material Symbols instead of outlined ones.", |ui| {
                    changed |= switch(ui, &mut config.ui.filled_icons).changed();
                });
                ui.separator();
                row(ui, "Interface scale", "Size of everything in the window.", |ui| {
                    let r = ui.add(egui::Slider::new(&mut config.ui.ui_scale, 0.75..=1.75).step_by(0.05).custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
                    // Apply on release so the slider doesn't jump under the cursor.
                    if r.drag_stopped() || (r.changed() && !r.dragged()) {
                        changed = true;
                    }
                });
            });

            card(ui, "Matugen", |ui| {
                ui.add_space(8.0);
                ui.add(egui::Label::new(RichText::new("Generate the palette from your wallpaper with matugen. Export the template, add the snippet to matugen's config, and the theme updates live whenever matugen runs.").size(13.0).color(p.on_surface_variant)).wrap());
                ui.add_space(8.0);
                let snippet = matugen::config_snippet();
                Frame::new().fill(p.surface_container_highest).corner_radius(CornerRadius::same(12)).inner_margin(Margin::same(12)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(&snippet).monospace().size(12.0));
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if widgets::button(ui, ButtonKind::Tonal, Some(ICON_DOWNLOAD), "Export template").clicked() {
                        match matugen::write_template() {
                            Ok(path) => actions.push(Action::Toast(format!("Template saved to {}", path.display()), ToastKind::Success)),
                            Err(e) => actions.push(Action::Toast(format!("Couldn't write template: {e}"), ToastKind::Error)),
                        }
                    }
                    if widgets::button(ui, ButtonKind::Text, Some(ICON_CONTENT_COPY), "Copy snippet").clicked() {
                        ui.ctx().copy_text(snippet.clone());
                        actions.push(Action::Toast("Snippet copied".into(), ToastKind::Info));
                    }
                    if widgets::button(ui, ButtonKind::Text, Some(ICON_REFRESH), "Reload theme").clicked() {
                        actions.push(Action::ReloadTheme);
                    }
                });
                ui.add_space(8.0);
            });

            card(ui, "File types", |ui| {
                ui.add_space(8.0);
                ui.add(egui::Label::new(RichText::new("Categories used by “Group by type”. Extensions are comma separated.").size(13.0).color(p.on_surface_variant)).wrap());
                ui.add_space(4.0);
                let mut reset_categories = false;
                let edits = app.settings.categories.get_or_insert_with(|| {
                    config.file_categories.iter().map(|(k, v)| (k.clone(), v.iter().map(|e| e.trim_start_matches('.')).collect::<Vec<_>>().join(", "))).collect()
                });
                let mut remove = None;
                for (name, exts) in edits.iter_mut() {
                    ui.horizontal(|ui| {
                        ui.add_sized(vec2(110.0, 24.0), egui::Label::new(RichText::new(name.as_str()).size(14.0)));
                        let resp = ui.add(egui::TextEdit::singleline(exts).desired_width(ui.available_width() - 44.0));
                        if resp.lost_focus() {
                            config.file_categories.insert(
                                name.clone(),
                                exts.split(',').map(|e| e.trim().trim_start_matches('.').to_lowercase()).filter(|e| !e.is_empty()).map(|e| format!(".{e}")).collect(),
                            );
                            changed = true;
                        }
                        if widgets::icon_button_sized(ui, ICON_DELETE, "Remove category", true, 30.0, false).clicked() {
                            remove = Some(name.clone());
                        }
                    });
                }
                if let Some(name) = remove {
                    edits.remove(&name);
                    config.file_categories.remove(&name);
                    changed = true;
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let s = &mut app.settings.new_category;
                    ui.add(egui::TextEdit::singleline(s).hint_text("New category name").desired_width(180.0));
                    let name = s.trim().to_string();
                    let valid = !name.is_empty() && !config.file_categories.contains_key(&name) && name != "Folders" && name != "Other";
                    if widgets::button_enabled(ui, ButtonKind::Tonal, Some(ICON_ADD), "Add", valid).clicked() {
                        config.file_categories.insert(name.clone(), Vec::new());
                        edits.insert(name, String::new());
                        s.clear();
                        changed = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        reset_categories = widgets::button(ui, ButtonKind::Text, Some(ICON_RESTART_ALT), "Reset to defaults").clicked();
                    });
                });
                if reset_categories {
                    config.file_categories = default_file_categories();
                    app.settings.categories = None;
                    changed = true;
                }
                ui.add_space(8.0);
            });

            card(ui, "Advanced", |ui| {
                let path = Config::path().unwrap_or_default();
                row(ui, "Configuration file", &path.to_string_lossy(), |ui| {
                    if widgets::button(ui, ButtonKind::Text, Some(ICON_FOLDER_OPEN), "Show").clicked() {
                        if let Some(dir) = path.parent() {
                            actions.push(Action::OpenInNewTab(Location::Folder(dir.to_path_buf())));
                        }
                    }
                    if widgets::button(ui, ButtonKind::Tonal, Some(ICON_EDIT), "Edit").clicked() {
                        actions.push(Action::OpenInEditor(path.clone()));
                    }
                });
                ui.separator();
                row(ui, "Pinned folders", "Restore the default pins (Home, Downloads, Documents).", |ui| {
                    if widgets::button(ui, ButtonKind::Outlined, Some(ICON_RESTART_ALT), "Reset").clicked() {
                        config.pinned_folders = default_pinned_folders();
                        changed = true;
                        actions.push(Action::Toast("Pinned folders reset".into(), ToastKind::Info));
                    }
                });
                ui.separator();
                row(ui, "Recent folders", &format!("{} remembered", config.recent_folders.len()), |ui| {
                    if widgets::button(ui, ButtonKind::Outlined, Some(ICON_HISTORY), "Clear").clicked() {
                        actions.push(Action::ClearRecent);
                    }
                });
                ui.separator();
                row(ui, "File chooser portal", "Run `m3fm --portal` to serve Open/Save dialogs for other apps via xdg-desktop-portal.", |_| {});
                ui.separator();
                row(ui, "Keyboard shortcuts", "Everything you can do from the keyboard.", |ui| {
                    if widgets::button(ui, ButtonKind::Tonal, Some(ICON_KEYBOARD), "Show").clicked() {
                        actions.push(Action::ShowShortcuts);
                    }
                });
            });

            ui.add_space(24.0);
            ui.label(RichText::new(format!("Material 3 File Manager {}", env!("CARGO_PKG_VERSION"))).size(12.0).color(p.on_surface_variant));

            if changed {
                actions.push(Action::ConfigChanged);
            }
        });
    });
    app.actions.extend(actions);
}
