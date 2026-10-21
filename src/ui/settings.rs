//! Settings page (opened as a tab).

use super::widgets::{self, segmented, switch, ButtonKind};
use super::{Action, ToastKind};
use crate::app::App;
use crate::config::{default_file_categories, default_pinned_folders, Config, ViewMode};
use crate::matugen;
use crate::tab::Location;
use egui::{vec2, CornerRadius, Frame, Margin, RichText, Ui};
use material_3_ui::common::{card, note, page, row};
use material_3_ui::format::file_size;
use material_3_ui::icons::icons::*;
use material_3_ui::prefs;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct SettingsState {
    /// Comma-separated extension lists being edited, per category.
    categories: Option<BTreeMap<String, String>>,
    new_category: String,
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
    page(
        ui,
        ICON_SETTINGS,
        "Settings",
        "Changes apply immediately and are saved to config.toml.",
        |ui| {
            let config = &mut app.config;
            let mut changed = false;

            card(ui, "Browsing", |ui| {
                row(
                    ui,
                    "Show hidden files",
                    "Files and folders whose names start with a dot. (Ctrl+H)",
                    |ui| {
                        changed |= switch(ui, &mut config.ui.show_hidden).changed();
                    },
                );
                ui.separator();
                row(
                    ui,
                    "Default view",
                    "How folders are displayed. (Ctrl+1 / Ctrl+2)",
                    |ui| {
                        changed |= segmented(
                            ui,
                            &mut config.ui.view_mode,
                            &[
                                (ViewMode::Grid, ICON_GRID_VIEW, "Grid"),
                                (ViewMode::List, ICON_VIEW_LIST, "List"),
                            ],
                        );
                    },
                );
                ui.separator();
                row(
                    ui,
                    "Icon size",
                    "Grid tile size. Ctrl+scroll in a folder to adjust quickly.",
                    |ui| {
                        changed |= ui
                            .add(
                                egui::Slider::new(&mut config.ui.icon_size, 32..=160).suffix(" px"),
                            )
                            .changed();
                    },
                );
                ui.separator();
                row(
                    ui,
                    "Group by type",
                    "Show folders, images, documents… under separate headings. (Ctrl+G)",
                    |ui| {
                        changed |= switch(ui, &mut config.group_files_by_type).changed();
                    },
                );
                ui.separator();
                row(
                    ui,
                    "Folders first",
                    "Keep folders above files regardless of sort order.",
                    |ui| {
                        changed |= switch(ui, &mut config.ui.folders_first).changed();
                    },
                );
                ui.separator();
                row(
                    ui,
                    "Image thumbnails",
                    "Show previews for pictures in the grid, list and details pane.",
                    |ui| {
                        changed |= switch(ui, &mut config.ui.show_thumbnails).changed();
                    },
                );
            });

            card(ui, "Applications", |ui| {
                row(
                    ui,
                    "Terminal",
                    "Command used by “Open in terminal”. Started in the folder.",
                    |ui| {
                        command_field(ui, config, "terminal", "kitty", &mut actions);
                    },
                );
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
                                        material_3_ui::sys::paths::tilde(&drive.mount_point),
                                        file_size(q.free),
                                        file_size(q.total)
                                    ),
                                    None => format!("Mounted at {}", material_3_ui::sys::paths::tilde(&drive.mount_point)),
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

            // Applied with the rest of the config (Action::ConfigChanged).
            changed |= prefs::appearance_card(ui, &mut config.ui.appearance, &app.theme);

            card(ui, "Matugen", |ui| {
                note(ui, "Generate the palette from your wallpaper with matugen. Export the template, add the snippet to matugen's config, and the theme updates live whenever matugen runs.");
                let snippet = matugen::config_snippet();
                Frame::new()
                    .fill(p.surface_container_highest)
                    .corner_radius(CornerRadius::same(12))
                    .inner_margin(Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new(&snippet).monospace().size(12.0));
                    });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if widgets::button(
                        ui,
                        ButtonKind::Tonal,
                        Some(ICON_DOWNLOAD),
                        "Export template",
                    )
                    .clicked()
                    {
                        match matugen::write_template() {
                            Ok(path) => actions.push(Action::Toast(
                                format!("Template saved to {}", path.display()),
                                ToastKind::Success,
                            )),
                            Err(e) => actions.push(Action::Toast(
                                format!("Couldn't write template: {e}"),
                                ToastKind::Error,
                            )),
                        }
                    }
                    if widgets::button(
                        ui,
                        ButtonKind::Text,
                        Some(ICON_CONTENT_COPY),
                        "Copy snippet",
                    )
                    .clicked()
                    {
                        ui.ctx().copy_text(snippet.clone());
                        actions.push(Action::Toast("Snippet copied".into(), ToastKind::Info));
                    }
                    if widgets::button(ui, ButtonKind::Text, Some(ICON_REFRESH), "Reload theme")
                        .clicked()
                    {
                        actions.push(Action::ReloadTheme);
                    }
                });
                ui.add_space(8.0);
            });

            card(ui, "File types", |ui| {
                note(
                    ui,
                    "Categories used by “Group by type”. Extensions are comma separated.",
                );
                let mut reset_categories = false;
                let edits = app.settings.categories.get_or_insert_with(|| {
                    config
                        .file_categories
                        .iter()
                        .map(|(k, v)| {
                            (
                                k.clone(),
                                v.iter()
                                    .map(|e| e.trim_start_matches('.'))
                                    .collect::<Vec<_>>()
                                    .join(", "),
                            )
                        })
                        .collect()
                });
                let mut remove = None;
                for (name, exts) in edits.iter_mut() {
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            vec2(110.0, 24.0),
                            egui::Label::new(RichText::new(name.as_str()).size(14.0)),
                        );
                        let resp = ui.add(
                            egui::TextEdit::singleline(exts)
                                .desired_width(ui.available_width() - 44.0),
                        );
                        if resp.lost_focus() {
                            config.file_categories.insert(
                                name.clone(),
                                exts.split(',')
                                    .map(|e| e.trim().trim_start_matches('.').to_lowercase())
                                    .filter(|e| !e.is_empty())
                                    .map(|e| format!(".{e}"))
                                    .collect(),
                            );
                            changed = true;
                        }
                        if widgets::icon_button_sized(
                            ui,
                            ICON_DELETE,
                            "Remove category",
                            true,
                            30.0,
                            false,
                        )
                        .clicked()
                        {
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
                    ui.add(
                        egui::TextEdit::singleline(s)
                            .hint_text("New category name")
                            .desired_width(180.0),
                    );
                    let name = s.trim().to_string();
                    let valid = !name.is_empty()
                        && !config.file_categories.contains_key(&name)
                        && name != "Folders"
                        && name != "Other";
                    if widgets::button_enabled(ui, ButtonKind::Tonal, Some(ICON_ADD), "Add", valid)
                        .clicked()
                    {
                        config.file_categories.insert(name.clone(), Vec::new());
                        edits.insert(name, String::new());
                        s.clear();
                        changed = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        reset_categories = widgets::button(
                            ui,
                            ButtonKind::Text,
                            Some(ICON_RESTART_ALT),
                            "Reset to defaults",
                        )
                        .clicked();
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
                    if widgets::button(ui, ButtonKind::Text, Some(ICON_FOLDER_OPEN), "Show")
                        .clicked()
                    {
                        if let Some(dir) = path.parent() {
                            actions.push(Action::OpenInNewTab(Location::Folder(dir.to_path_buf())));
                        }
                    }
                    if widgets::button(ui, ButtonKind::Tonal, Some(ICON_EDIT), "Edit").clicked() {
                        actions.push(Action::OpenInEditor(path.clone()));
                    }
                });
                ui.separator();
                row(
                    ui,
                    "Pinned folders",
                    "Restore the default pins (Home, Downloads, Documents).",
                    |ui| {
                        if widgets::button(
                            ui,
                            ButtonKind::Outlined,
                            Some(ICON_RESTART_ALT),
                            "Reset",
                        )
                        .clicked()
                        {
                            config.pinned_folders = default_pinned_folders();
                            changed = true;
                            actions.push(Action::Toast(
                                "Pinned folders reset".into(),
                                ToastKind::Info,
                            ));
                        }
                    },
                );
                ui.separator();
                row(
                    ui,
                    "Recent folders",
                    &format!("{} remembered", config.recent_folders.len()),
                    |ui| {
                        if widgets::button(ui, ButtonKind::Outlined, Some(ICON_HISTORY), "Clear")
                            .clicked()
                        {
                            actions.push(Action::ClearRecent);
                        }
                    },
                );
                ui.separator();
                row(ui, "File chooser portal", "Run `m3fm --portal` to serve Open/Save dialogs for other apps via xdg-desktop-portal.", |_| {});
                ui.separator();
                row(
                    ui,
                    "Keyboard shortcuts",
                    "Everything you can do from the keyboard.",
                    |ui| {
                        if widgets::button(ui, ButtonKind::Tonal, Some(ICON_KEYBOARD), "Show")
                            .clicked()
                        {
                            actions.push(Action::ShowShortcuts);
                        }
                    },
                );
            });

            ui.add_space(24.0);
            ui.label(
                RichText::new(format!(
                    "Material 3 File Manager {}",
                    env!("CARGO_PKG_VERSION")
                ))
                .size(12.0)
                .color(p.on_surface_variant),
            );

            if changed {
                actions.push(Action::ConfigChanged);
            }
        },
    );
    app.actions.extend(actions);
}
