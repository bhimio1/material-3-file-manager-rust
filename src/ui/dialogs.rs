//! Modal dialogs.

use super::icons::{entry_icon, kind_color};
use super::widgets::{self, paint_icon, truncated_galley, ButtonKind};
use super::{Action, ToastKind};
use crate::app::{App, SHORTCUTS};
use crate::config::SortKey;
use crate::fs::apps::AppEntry;
use crate::fs::entry::{format_size, format_time_full, sort_entries};
use crate::fs::portal::{PickKind, PickerSpec, PortalResponse};
use crate::fs::scanner::{self, DirStats};
use crate::fs::{launch, ops, FileEntry, FileKind};
use crate::thumbs::Thumb;
use egui::{
    pos2, vec2, Color32, CornerRadius, FontId, Frame, Id, Key, Margin, Modal, Rect, RichText,
    Sense, Ui,
};
use egui_material_icons::icons::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

pub enum PickerPurpose {
    MoveTo(Vec<PathBuf>),
    CopyTo(Vec<PathBuf>),
    Pin,
    /// Answer for another application, via the file chooser portal.
    Portal(Option<flume::Sender<PortalResponse>>),
}

pub struct Picker {
    spec: PickerSpec,
    purpose: PickerPurpose,
    dir: PathBuf,
    entries: Vec<FileEntry>,
    error: Option<String>,
    selection: Vec<PathBuf>,
    filter: usize,
    name: String,
    confirm_overwrite: bool,
    show_hidden: bool,
    /// Listing in progress on a background thread (dir, result).
    loading: Option<flume::Receiver<(PathBuf, std::io::Result<Vec<FileEntry>>)>>,
    ctx: egui::Context,
}

impl Drop for Picker {
    fn drop(&mut self) {
        // Closing a portal dialog any way other than "accept" is a cancel.
        if let PickerPurpose::Portal(tx) = &mut self.purpose {
            if let Some(tx) = tx.take() {
                let _ = tx.send(PortalResponse::Cancelled);
            }
        }
    }
}

pub struct Properties {
    path: PathBuf,
    entry: Option<FileEntry>,
    mime: String,
    link_target: Option<PathBuf>,
    permissions: Option<String>,
    owner: Option<String>,
    accessed: Option<SystemTime>,
    created: Option<SystemTime>,
    dims: Option<(u32, u32)>,
    stats: Option<DirStats>,
    stats_rx: Option<flume::Receiver<DirStats>>,
    cancel: Arc<AtomicBool>,
}

impl Drop for Properties {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

pub struct OpenWith {
    path: PathBuf,
    mime: String,
    query: String,
    selected: usize,
    focus: bool,
    icons: HashMap<String, Option<PathBuf>>,
}

pub enum Dialog {
    NewName {
        dir: PathBuf,
        folder: bool,
        name: String,
        error: Option<String>,
        focus: bool,
    },
    ConfirmDelete {
        paths: Vec<PathBuf>,
    },
    Properties(Box<Properties>),
    OpenWith(Box<OpenWith>),
    Picker(Box<Picker>),
    Shortcuts,
    /// Google sign-in in progress (state lives in `app.cloud`).
    CloudConnect,
    ConfirmDisconnect {
        remote: String,
    },
}

impl Dialog {
    pub fn new_name(dir: PathBuf, folder: bool) -> Self {
        let base = if folder { "New Folder" } else { "Untitled" };
        let mut name = base.to_string();
        let mut n = 2;
        while dir.join(&name).exists() {
            name = format!("{base} {n}");
            n += 1;
        }
        Dialog::NewName {
            dir,
            folder,
            name,
            error: None,
            focus: true,
        }
    }

    pub fn open_with(path: PathBuf) -> Self {
        let mime = mime_guess::from_path(&path)
            .first_or_octet_stream()
            .essence_str()
            .to_string();
        Dialog::OpenWith(Box::new(OpenWith {
            path,
            mime,
            query: String::new(),
            selected: 0,
            focus: true,
            icons: HashMap::new(),
        }))
    }

    pub fn picker(
        spec: PickerSpec,
        purpose: PickerPurpose,
        start: PathBuf,
        ctx: &egui::Context,
    ) -> Self {
        let name = match &spec.kind {
            PickKind::Save { current_name } => current_name.clone(),
            _ => String::new(),
        };
        let mut picker = Picker {
            spec,
            purpose,
            dir: start.clone(),
            entries: Vec::new(),
            error: None,
            selection: Vec::new(),
            filter: 0,
            name,
            confirm_overwrite: false,
            show_hidden: false,
            loading: None,
            ctx: ctx.clone(),
        };
        picker.enter(start);
        Dialog::Picker(Box::new(picker))
    }

    pub fn folder_picker(
        title: &str,
        accept: &str,
        purpose: PickerPurpose,
        start: PathBuf,
        ctx: &egui::Context,
    ) -> Self {
        Self::picker(
            PickerSpec {
                title: title.into(),
                kind: PickKind::Folder { multiple: false },
                filters: Vec::new(),
                start_dir: None,
                accept_label: Some(accept.into()),
            },
            purpose,
            start,
            ctx,
        )
    }

    pub fn properties(path: PathBuf, ctx: &egui::Context) -> Self {
        let entry = FileEntry::from_path(path.clone());
        let link_meta = std::fs::symlink_metadata(&path).ok();
        let meta = std::fs::metadata(&path).ok().or(link_meta.clone());
        let is_dir = entry.as_ref().map(|e| e.is_dir).unwrap_or(false);
        let mime = if is_dir {
            "inode/directory".to_string()
        } else {
            mime_guess::from_path(&path)
                .first_or_octet_stream()
                .essence_str()
                .to_string()
        };
        let link_target = link_meta
            .filter(|m| m.file_type().is_symlink())
            .and_then(|_| std::fs::read_link(&path).ok());
        let (permissions, owner) = unix_details(meta.as_ref());
        let dims = entry
            .as_ref()
            .filter(|e| e.kind == FileKind::Image)
            .and_then(|_| image::image_dimensions(&path).ok());

        let cancel = Arc::new(AtomicBool::new(false));
        let stats_rx = is_dir.then(|| {
            let (tx, rx) = flume::bounded(1);
            let (p, c, ctx) = (path.clone(), cancel.clone(), ctx.clone());
            std::thread::spawn(move || {
                let _ = tx.send(scanner::dir_stats(&p, &c));
                ctx.request_repaint();
            });
            rx
        });
        Dialog::Properties(Box::new(Properties {
            accessed: meta.as_ref().and_then(|m| m.accessed().ok()),
            created: meta.as_ref().and_then(|m| m.created().ok()),
            path,
            entry,
            mime,
            link_target,
            permissions,
            owner,
            dims,
            stats: None,
            stats_rx,
            cancel,
        }))
    }
}

#[cfg(unix)]
fn unix_details(meta: Option<&std::fs::Metadata>) -> (Option<String>, Option<String>) {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let Some(meta) = meta else {
        return (None, None);
    };
    let mode = meta.permissions().mode() & 0o777;
    let mut s = String::new();
    for shift in [6, 3, 0] {
        let bits = (mode >> shift) & 7;
        s.push(if bits & 4 != 0 { 'r' } else { '-' });
        s.push(if bits & 2 != 0 { 'w' } else { '-' });
        s.push(if bits & 1 != 0 { 'x' } else { '-' });
    }
    let user_name = |uid: u32| -> String {
        std::fs::read_to_string("/etc/passwd")
            .ok()
            .and_then(|passwd| {
                passwd.lines().find_map(|l| {
                    let mut f = l.split(':');
                    let name = f.next()?;
                    let id: u32 = f.nth(1)?.parse().ok()?;
                    (id == uid).then(|| name.to_string())
                })
            })
            .unwrap_or_else(|| uid.to_string())
    };
    (
        Some(format!("{s}  ({mode:o})")),
        Some(user_name(meta.uid())),
    )
}

#[cfg(not(unix))]
fn unix_details(_meta: Option<&std::fs::Metadata>) -> (Option<String>, Option<String>) {
    (None, None)
}

fn modal_frame(p: &crate::theme::M3Palette) -> Frame {
    Frame::new()
        .fill(p.surface_container_high)
        .corner_radius(CornerRadius::same(28))
        .inner_margin(Margin::same(24))
        .shadow(egui::Shadow {
            offset: [0, 8],
            blur: 32,
            spread: 0,
            color: Color32::from_black_alpha(90),
        })
}

fn title(ui: &mut Ui, icon: egui_material_icons::MaterialIcon, text: &str) {
    let p = widgets::palette(ui);
    ui.vertical_centered(|ui| {
        widgets::icon(ui, icon, 26.0, p.secondary);
        ui.add_space(8.0);
        ui.label(RichText::new(text).size(22.0).color(p.on_surface));
    });
    ui.add_space(14.0);
}

/// A right-aligned row of dialog buttons. The fixed-height row keeps the
/// right-to-left layout from claiming the modal's remaining height.
fn right_buttons(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.add_space(18.0);
    let width = ui.available_width();
    ui.allocate_ui_with_layout(
        vec2(width, 36.0),
        egui::Layout::right_to_left(egui::Align::Center),
        add,
    );
}

pub fn show(ctx: &egui::Context, app: &mut App) {
    let Some(mut dialog) = app.dialog.take() else {
        return;
    };
    let p = app.palette.clone();
    let modal = Modal::new(Id::unique("dialog"))
        .backdrop_color(p.scrim.gamma_multiply(0.4))
        .frame(modal_frame(&p));
    let keep = match &mut dialog {
        Dialog::NewName {
            dir,
            folder,
            name,
            error,
            focus,
        } => {
            let mut keep = true;
            let r = modal.show(ctx, |ui| {
                ui.set_width(380.0);
                title(
                    ui,
                    if *folder {
                        ICON_CREATE_NEW_FOLDER
                    } else {
                        ICON_NOTE_ADD
                    },
                    if *folder { "New folder" } else { "New file" },
                );
                let resp = ui.add(
                    egui::TextEdit::singleline(name)
                        .desired_width(f32::INFINITY)
                        .font(FontId::proportional(15.0))
                        .margin(Margin::symmetric(12, 10)),
                );
                if *focus {
                    *focus = false;
                    resp.request_focus();
                    if let Some(mut s) = egui::TextEdit::load_state(ui.ctx(), resp.id) {
                        s.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                            egui::text::CCursor::new(0),
                            egui::text::CCursor::new(name.chars().count()),
                        )));
                        s.store(ui.ctx(), resp.id);
                    }
                }
                let live_error = ops::validate_name(name)
                    .err()
                    .map(String::from)
                    .or_else(|| {
                        dir.join(name.trim())
                            .exists()
                            .then(|| "Something with this name already exists".to_string())
                    });
                let shown = error.clone().or(live_error.clone());
                ui.add_space(4.0);
                ui.label(
                    RichText::new(shown.unwrap_or_else(|| format!("In {}", dir.display())))
                        .size(12.5)
                        .color(if error.is_some() || live_error.is_some() {
                            p.error
                        } else {
                            p.on_surface_variant
                        }),
                );
                let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                right_buttons(ui, |ui| {
                    let create = widgets::button_enabled(
                        ui,
                        ButtonKind::Filled,
                        None,
                        "Create",
                        live_error.is_none(),
                    )
                    .clicked();
                    if widgets::button(ui, ButtonKind::Text, None, "Cancel").clicked() {
                        keep = false;
                    }
                    if (create || enter) && live_error.is_none() {
                        let result = if *folder {
                            ops::create_folder(dir, name)
                        } else {
                            ops::create_file(dir, name)
                        };
                        match result {
                            Ok(path) => {
                                for tab in &mut app.tabs {
                                    if tab.folder() == Some(dir.as_path()) {
                                        tab.reload();
                                    }
                                }
                                app.tab_mut().select_when_listed = vec![path];
                                keep = false;
                            }
                            Err(e) => *error = Some(e.to_string()),
                        }
                    }
                });
            });
            keep && !r.should_close()
        }

        Dialog::ConfirmDelete { paths } => {
            let mut keep = true;
            let r = modal.show(ctx, |ui| {
                ui.set_width(400.0);
                let n = paths.len();
                title(
                    ui,
                    ICON_DELETE_FOREVER,
                    &if n == 1 {
                        "Delete permanently?".to_string()
                    } else {
                        format!("Delete {n} items permanently?")
                    },
                );
                for path in paths.iter().take(5) {
                    ui.horizontal(|ui| {
                        widgets::icon(
                            ui,
                            if path.is_dir() {
                                ICON_FOLDER
                            } else {
                                ICON_DRAFT
                            },
                            18.0,
                            p.on_surface_variant,
                        );
                        ui.label(
                            path.file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                        );
                    });
                }
                if n > 5 {
                    ui.label(
                        RichText::new(format!("…and {} more", n - 5)).color(p.on_surface_variant),
                    );
                }
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "This can't be undone. Use “Move to trash” if you might need them later.",
                    )
                    .size(13.0)
                    .color(p.on_surface_variant),
                );
                right_buttons(ui, |ui| {
                    if widgets::button(ui, ButtonKind::Danger, Some(ICON_DELETE_FOREVER), "Delete")
                        .clicked()
                    {
                        let job = ops::remove(paths.clone(), ops::JobKind::Delete, app.notify());
                        app.jobs.push(job);
                        keep = false;
                    }
                    if widgets::button(ui, ButtonKind::Text, None, "Cancel").clicked() {
                        keep = false;
                    }
                    if widgets::button(ui, ButtonKind::Tonal, Some(ICON_DELETE), "Move to trash")
                        .clicked()
                    {
                        app.actions.push(Action::Trash(paths.clone()));
                        keep = false;
                    }
                });
            });
            keep && !r.should_close()
        }

        Dialog::Properties(props) => {
            if let Some(rx) = &props.stats_rx {
                if let Ok(stats) = rx.try_recv() {
                    props.stats = Some(stats);
                    props.stats_rx = None;
                }
            }
            let mut keep = true;
            let r = modal.show(ctx, |ui| {
                ui.set_width(460.0);
                properties_ui(ui, app, props, &mut keep);
            });
            keep && !r.should_close()
        }

        Dialog::OpenWith(ow) => {
            let mut keep = true;
            let r = modal.show(ctx, |ui| {
                ui.set_width(440.0);
                open_with_ui(ui, app, ow, &mut keep);
            });
            keep && !r.should_close()
        }

        Dialog::Picker(picker) => {
            let mut keep = true;
            let r = modal.show(ctx, |ui| {
                ui.set_width(720.0);
                ui.set_height(520.0);
                picker_ui(ui, app, picker, &mut keep);
            });
            keep && !r.should_close()
        }

        Dialog::CloudConnect => {
            let auth_url = app
                .cloud
                .connect
                .as_ref()
                .and_then(|c| c.auth_url.lock().unwrap().clone());
            let waiting = app.cloud.connect.is_some() || app.cloud.preparing_connect;
            let mut cancel = false;
            let r = modal.show(ctx, |ui| {
                ui.set_width(420.0);
                title(ui, ICON_ADD_TO_DRIVE, "Connect Google Drive");
                ui.vertical_centered(|ui| {
                    ui.add(egui::Spinner::new().size(28.0).color(p.primary));
                    ui.add_space(10.0);
                    let text = if app.cloud.preparing_connect {
                        "Checking for an existing rclone setup…"
                    } else {
                        "Sign in with Google in the browser window that just opened, then allow access. This window updates when you're done."
                    };
                    ui.add(egui::Label::new(RichText::new(text).color(p.on_surface_variant)).wrap());
                    if let Some(url) = &auth_url {
                        ui.add_space(10.0);
                        ui.label(RichText::new("No browser window?").size(12.5).color(p.on_surface_variant));
                        if widgets::button(ui, ButtonKind::Text, Some(ICON_OPEN_IN_NEW), "Open sign-in page").clicked() {
                            app.actions.push(Action::OpenUrl(url.clone()));
                        }
                    }
                });
                right_buttons(ui, |ui| {
                    cancel = widgets::button(ui, ButtonKind::Text, None, "Cancel").clicked();
                });
            });
            if cancel || r.should_close() {
                app.actions.push(Action::CloudCancelConnect);
                false
            } else {
                // Closes itself once sign-in finished (or failed).
                waiting
            }
        }

        Dialog::ConfirmDisconnect { remote } => {
            let mut keep = true;
            let r = modal.show(ctx, |ui| {
                ui.set_width(400.0);
                title(ui, ICON_CLOUD_OFF, "Disconnect Google Drive?");
                ui.add(
                    egui::Label::new(
                        RichText::new("The drive is unmounted and this computer is signed out of it. Your files stay in Google Drive.")
                            .color(p.on_surface_variant),
                    )
                    .wrap(),
                );
                right_buttons(ui, |ui| {
                    if widgets::button(ui, ButtonKind::Danger, None, "Disconnect").clicked() {
                        app.actions.push(Action::CloudDisconnectConfirmed(remote.clone()));
                        keep = false;
                    }
                    if widgets::button(ui, ButtonKind::Text, None, "Cancel").clicked() {
                        keep = false;
                    }
                });
            });
            keep && !r.should_close()
        }

        Dialog::Shortcuts => {
            let r = modal.show(ctx, |ui| {
                ui.set_width(560.0);
                title(ui, ICON_KEYBOARD, "Keyboard shortcuts");
                egui::ScrollArea::vertical()
                    .max_height(460.0)
                    .show(ui, |ui| {
                        egui::Grid::new("shortcuts")
                            .num_columns(2)
                            .spacing(vec2(24.0, 8.0))
                            .striped(false)
                            .show(ui, |ui| {
                                for (keys, what) in SHORTCUTS {
                                    Frame::new()
                                        .fill(p.surface_container_highest)
                                        .corner_radius(CornerRadius::same(6))
                                        .inner_margin(Margin::symmetric(8, 3))
                                        .show(ui, |ui| {
                                            ui.label(RichText::new(*keys).monospace().size(12.5));
                                        });
                                    ui.label(*what);
                                    ui.end_row();
                                }
                            });
                    });
                let mut close = false;
                right_buttons(ui, |ui| {
                    close = widgets::button(ui, ButtonKind::Filled, None, "Done").clicked();
                });
                close
            });
            !r.inner && !r.should_close()
        }
    };
    if keep && app.dialog.is_none() {
        app.dialog = Some(dialog);
    }
}

// ----------------------------------------------------------------- properties

fn properties_ui(ui: &mut Ui, app: &mut App, props: &mut Properties, keep: &mut bool) {
    let p = app.palette.clone();
    let name = props
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| props.path.to_string_lossy().into_owned());
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::hover());
        ui.painter()
            .rect_filled(rect, CornerRadius::same(16), p.surface_container_highest);
        let mut drew = false;
        if let Some(e) = &props.entry {
            if e.kind.has_thumbnail() {
                if let Thumb::Ready(tex) = app.thumbs.get(&e.path, 128.0, e.modified) {
                    let s = tex.size_vec2();
                    let scale = (56.0 / s.x).min(56.0 / s.y);
                    egui::Image::new(&tex)
                        .corner_radius(CornerRadius::same(8))
                        .paint_at(ui, Rect::from_center_size(rect.center(), s * scale));
                    drew = true;
                }
            }
            if !drew {
                paint_icon(
                    ui,
                    entry_icon(e),
                    rect.center(),
                    40.0,
                    kind_color(e.kind, &p),
                );
            }
        }
        ui.vertical(|ui| {
            ui.add_space(6.0);
            ui.add(egui::Label::new(widgets::title_text(&name, 19.0).color(p.on_surface)).wrap());
            let kind = props
                .entry
                .as_ref()
                .map(|e| e.type_label())
                .unwrap_or_default();
            ui.label(
                RichText::new(format!("{kind} · {}", props.mime))
                    .size(12.5)
                    .color(p.on_surface_variant),
            );
        });
    });
    ui.add_space(14.0);
    egui::Grid::new("props")
        .num_columns(2)
        .spacing(vec2(18.0, 8.0))
        .show(ui, |ui| {
            let mut row = |label: &str, value: String| {
                ui.label(RichText::new(label).color(p.on_surface_variant));
                ui.add(egui::Label::new(value).wrap());
                ui.end_row();
            };
            let is_dir = props.entry.as_ref().map(|e| e.is_dir).unwrap_or(false);
            if is_dir {
                match props.stats {
                    Some(s) => {
                        row(
                            "Size",
                            format!("{} ({} bytes)", format_size(s.bytes), s.bytes),
                        );
                        row(
                            "Contents",
                            format!(
                                "{}, {}",
                                crate::fs::entry::plural(s.files, "file"),
                                crate::fs::entry::plural(s.dirs, "folder")
                            ),
                        );
                    }
                    None => row("Size", "Calculating…".into()),
                }
            } else if let Some(e) = &props.entry {
                row(
                    "Size",
                    format!("{} ({} bytes)", format_size(e.size), e.size),
                );
            }
            if let Some((w, h)) = props.dims {
                row("Dimensions", format!("{w} × {h} pixels"));
            }
            row(
                "Location",
                props
                    .path
                    .parent()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "/".into()),
            );
            if let Some(t) = &props.link_target {
                row("Link target", t.to_string_lossy().into_owned());
            }
            row(
                "Modified",
                format_time_full(props.entry.as_ref().and_then(|e| e.modified)),
            );
            if props.accessed.is_some() {
                row("Accessed", format_time_full(props.accessed));
            }
            if props.created.is_some() {
                row("Created", format_time_full(props.created));
            }
            if let Some(perm) = &props.permissions {
                row("Permissions", perm.clone());
            }
            if let Some(owner) = &props.owner {
                row("Owner", owner.clone());
            }
        });
    if props.stats_rx.is_some() {
        ui.add_space(6.0);
        widgets::linear_progress(ui, None, ui.available_width());
    }
    right_buttons(ui, |ui| {
        if widgets::button(ui, ButtonKind::Filled, None, "Close").clicked() {
            *keep = false;
        }
        if widgets::button(ui, ButtonKind::Text, Some(ICON_LINK), "Copy path").clicked() {
            app.actions
                .push(Action::CopyPathText(vec![props.path.clone()]));
        }
    });
}

// ------------------------------------------------------------------ open with

fn open_with_ui(ui: &mut Ui, app: &mut App, ow: &mut OpenWith, keep: &mut bool) {
    let p = app.palette.clone();
    let file_name = ow
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    title(ui, ICON_APPS, "Open with");
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(format!("“{file_name}” · {}", ow.mime))
                .size(12.5)
                .color(p.on_surface_variant),
        );
    });
    ui.add_space(10.0);
    let search = ui.add(
        egui::TextEdit::singleline(&mut ow.query)
            .hint_text("Search applications")
            .desired_width(f32::INFINITY)
            .margin(Margin::symmetric(12, 8)),
    );
    if ow.focus {
        ow.focus = false;
        search.request_focus();
    }
    if search.changed() {
        ow.selected = 0;
    }

    let Some(apps) = app.apps.clone() else {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.add(egui::Spinner::new().size(24.0).color(p.primary));
            ui.label("Finding applications…");
        });
        return;
    };
    let q = ow.query.trim().to_lowercase();
    let matches = |a: &AppEntry| {
        q.is_empty()
            || a.name.to_lowercase().contains(&q)
            || a.generic_name
                .as_deref()
                .map(|g| g.to_lowercase().contains(&q))
                .unwrap_or(false)
            || a.id.to_lowercase().contains(&q)
    };
    let recommended: Vec<&AppEntry> = apps
        .iter()
        .filter(|a| a.handles(&ow.mime) && matches(a))
        .collect();
    let others: Vec<&AppEntry> = apps
        .iter()
        .filter(|a| !a.handles(&ow.mime) && matches(a))
        .collect();
    let all: Vec<&AppEntry> = recommended.iter().chain(others.iter()).copied().collect();

    // Keyboard navigation of the list while the search box has focus.
    let (up, down, enter) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, Key::Enter),
        )
    });
    if down {
        ow.selected = (ow.selected + 1).min(all.len().saturating_sub(1));
    }
    if up {
        ow.selected = ow.selected.saturating_sub(1);
    }
    let mut launch_app: Option<AppEntry> = None;
    if enter {
        launch_app = all.get(ow.selected).map(|a| (*a).clone());
    }

    ui.add_space(8.0);
    egui::ScrollArea::vertical()
        .max_height(360.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let mut index = 0;
            for (heading, list) in [
                ("Recommended", &recommended),
                ("Other applications", &others),
            ] {
                if list.is_empty() {
                    continue;
                }
                ui.label(
                    RichText::new(heading)
                        .size(12.5)
                        .color(p.on_surface_variant),
                );
                for a in list.iter() {
                    let selected = index == ow.selected;
                    let (rect, resp) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
                    let fill = if selected {
                        p.secondary_container
                    } else if resp.hovered() {
                        p.on_surface.gamma_multiply(0.06)
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter().rect_filled(rect, CornerRadius::same(12), fill);
                    let icon_rect = Rect::from_center_size(
                        pos2(rect.left() + 26.0, rect.center().y),
                        vec2(28.0, 28.0),
                    );
                    let icon_path = ow
                        .icons
                        .entry(a.id.clone())
                        .or_insert_with(|| a.icon_path())
                        .clone();
                    let mut drew = false;
                    if let Some(ip) = icon_path {
                        if let Thumb::Ready(tex) = app.thumbs.get(&ip, 56.0, None) {
                            egui::Image::new(&tex).paint_at(ui, icon_rect);
                            drew = true;
                        }
                    }
                    if !drew {
                        paint_icon(
                            ui,
                            ICON_APPS,
                            icon_rect.center(),
                            24.0,
                            p.on_surface_variant,
                        );
                    }
                    let fg = if selected {
                        p.on_secondary_container
                    } else {
                        p.on_surface
                    };
                    let name = truncated_galley(
                        ui,
                        &a.name,
                        FontId::proportional(14.5),
                        fg,
                        rect.width() - 70.0,
                    );
                    let has_sub = a.generic_name.is_some();
                    let ny = if has_sub {
                        rect.center().y - name.size().y
                    } else {
                        rect.center().y - name.size().y / 2.0
                    };
                    ui.painter().galley(pos2(rect.left() + 52.0, ny), name, fg);
                    if let Some(g) = &a.generic_name {
                        let sub = truncated_galley(
                            ui,
                            g,
                            FontId::proportional(12.0),
                            p.on_surface_variant,
                            rect.width() - 70.0,
                        );
                        ui.painter().galley(
                            pos2(rect.left() + 52.0, rect.center().y + 1.0),
                            sub,
                            p.on_surface_variant,
                        );
                    }
                    if selected && (down || up) {
                        resp.scroll_to_me(None);
                    }
                    if resp.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
                        ow.selected = index;
                    }
                    if resp
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        launch_app = Some((*a).clone());
                    }
                    index += 1;
                }
                ui.add_space(6.0);
            }
            if all.is_empty() {
                ui.label(RichText::new("No matching applications").color(p.on_surface_variant));
            }
        });

    right_buttons(ui, |ui| {
        if widgets::button(ui, ButtonKind::Text, None, "Cancel").clicked() {
            *keep = false;
        }
        if widgets::button(ui, ButtonKind::Tonal, None, "Default app").clicked() {
            app.actions.push(Action::Open(ow.path.clone()));
            *keep = false;
        }
    });

    if let Some(chosen) = launch_app {
        match launch::open_with(
            &chosen,
            std::slice::from_ref(&ow.path),
            app.config.command("terminal"),
        ) {
            Ok(()) => app.toast(format!("Opening with {}", chosen.name), ToastKind::Info),
            Err(e) => app.toast(e.to_string(), ToastKind::Error),
        }
        *keep = false;
    }
}

// --------------------------------------------------------------------- picker

impl Picker {
    fn wants_files(&self) -> bool {
        !matches!(self.spec.kind, PickKind::Folder { .. })
    }

    fn multiple(&self) -> bool {
        matches!(
            self.spec.kind,
            PickKind::Open { multiple: true } | PickKind::Folder { multiple: true }
        )
    }

    /// List `dir` on a background thread; `poll` picks the result up.
    fn enter(&mut self, dir: PathBuf) {
        self.selection.clear();
        self.confirm_overwrite = false;
        self.entries.clear();
        self.error = None;
        self.dir = dir.clone();
        let (tx, rx) = flume::bounded(1);
        let show_hidden = self.show_hidden;
        let wants_files = self.wants_files();
        let filter = self.spec.filters.get(self.filter).cloned();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let result = crate::perf::time("picker list_dir", || scanner::list_dir(&dir)).map(
                |mut entries| {
                    entries.retain(|e| {
                        (show_hidden || !e.hidden)
                            && (e.is_dir
                                || (wants_files
                                    && filter.as_ref().is_none_or(|f| f.matches(&e.name))))
                    });
                    sort_entries(&mut entries, SortKey::Name, true, true);
                    entries
                },
            );
            let _ = tx.send((dir, result));
            ctx.request_repaint();
        });
        self.loading = Some(rx);
    }

    fn poll(&mut self) {
        let Some(rx) = &self.loading else { return };
        if let Ok((dir, result)) = rx.try_recv() {
            self.loading = None;
            if dir == self.dir {
                match result {
                    Ok(entries) => self.entries = entries,
                    Err(e) => self.error = Some(e.to_string()),
                }
            }
        }
    }

    fn refresh(&mut self) {
        let dir = self.dir.clone();
        self.enter(dir);
    }
}

fn picker_ui(ui: &mut Ui, app: &mut App, picker: &mut Picker, keep: &mut bool) {
    picker.poll();
    let p = app.palette.clone();
    ui.label(
        RichText::new(if picker.spec.title.is_empty() {
            "Choose"
        } else {
            &picker.spec.title
        })
        .size(22.0)
        .color(p.on_surface),
    );
    ui.add_space(12.0);

    let mut navigate: Option<PathBuf> = None;
    let mut accept = false;
    let footer_h = if matches!(picker.spec.kind, PickKind::Save { .. }) {
        110.0
    } else {
        64.0
    };
    let body_h = ui.available_height() - footer_h;

    ui.horizontal_top(|ui| {
        // Shortcuts column.
        ui.vertical(|ui| {
            ui.set_width(170.0);
            ui.set_height(body_h);
            egui::ScrollArea::vertical()
                .id_salt("picker-places")
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for place in &app.places {
                        if widgets::nav_item(
                            ui,
                            place.icon,
                            place.name,
                            picker.dir == place.path,
                            None,
                        )
                        .clicked()
                        {
                            navigate = Some(place.path.clone());
                        }
                    }
                    if !app.config.pinned_folders.is_empty() {
                        ui.add_space(6.0);
                        ui.separator();
                        for pin in &app.config.pinned_folders {
                            let label = crate::fs::places::display_name(pin);
                            if widgets::nav_item(ui, ICON_KEEP, &label, picker.dir == *pin, None)
                                .clicked()
                            {
                                navigate = Some(pin.clone());
                            }
                        }
                    }
                    ui.add_space(6.0);
                    ui.separator();
                    for d in &app.drives {
                        if widgets::nav_item(ui, d.icon(), &d.name, picker.dir == d.mount, None)
                            .clicked()
                        {
                            navigate = Some(d.mount.clone());
                        }
                    }
                });
        });

        ui.vertical(|ui| {
            ui.set_height(body_h);
            ui.horizontal(|ui| {
                if widgets::icon_button(
                    ui,
                    ICON_ARROW_UPWARD,
                    "Parent folder",
                    picker.dir.parent().is_some(),
                )
                .clicked()
                {
                    navigate = picker.dir.parent().map(Path::to_path_buf);
                }
                let path_text = picker.dir.to_string_lossy().into_owned();
                let g = truncated_galley(
                    ui,
                    &path_text,
                    FontId::proportional(14.0),
                    p.on_surface,
                    ui.available_width() - 90.0,
                );
                ui.label(egui::WidgetText::Galley(g));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut hidden = picker.show_hidden;
                    if widgets::icon_button_sized(
                        ui,
                        if hidden {
                            ICON_VISIBILITY
                        } else {
                            ICON_VISIBILITY_OFF
                        },
                        "Show hidden files",
                        true,
                        32.0,
                        hidden,
                    )
                    .clicked()
                    {
                        hidden = !hidden;
                    }
                    if hidden != picker.show_hidden {
                        picker.show_hidden = hidden;
                        picker.refresh();
                    }
                });
            });
            ui.add_space(4.0);
            Frame::new()
                .fill(p.surface_container_low)
                .corner_radius(CornerRadius::same(16))
                .inner_margin(Margin::same(6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_height(ui.available_height());
                    if let Some(err) = &picker.error {
                        ui.label(RichText::new(err).color(p.error));
                    }
                    if picker.loading.is_some() && picker.entries.is_empty() {
                        ui.add_space(30.0);
                        ui.vertical_centered(|ui| {
                            ui.add(egui::Spinner::new().size(24.0).color(p.primary));
                        });
                    } else if picker.entries.is_empty() && picker.error.is_none() {
                        ui.add_space(30.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new("Empty folder").color(p.on_surface_variant))
                        });
                    }
                    egui::ScrollArea::vertical()
                        .id_salt(("picker", &picker.dir))
                        .auto_shrink([false, false])
                        .show_rows(ui, 36.0, picker.entries.len(), |ui, range| {
                            for e in picker.entries[range].iter().cloned() {
                                let selected = picker.selection.contains(&e.path);
                                let (rect, resp) = ui.allocate_exact_size(
                                    vec2(ui.available_width(), 36.0),
                                    Sense::click(),
                                );
                                let fill = if selected {
                                    p.secondary_container
                                } else if resp.hovered() {
                                    p.on_surface.gamma_multiply(0.06)
                                } else {
                                    Color32::TRANSPARENT
                                };
                                ui.painter().rect_filled(rect, CornerRadius::same(10), fill);
                                paint_icon(
                                    ui,
                                    entry_icon(&e),
                                    pos2(rect.left() + 20.0, rect.center().y),
                                    20.0,
                                    kind_color(e.kind, &p),
                                );
                                let fg = if selected {
                                    p.on_secondary_container
                                } else {
                                    p.on_surface
                                };
                                let g = truncated_galley(
                                    ui,
                                    &e.name,
                                    FontId::proportional(14.0),
                                    fg,
                                    rect.width() - 140.0,
                                );
                                ui.painter().galley(
                                    pos2(rect.left() + 40.0, rect.center().y - g.size().y / 2.0),
                                    g,
                                    fg,
                                );
                                if !e.is_dir {
                                    ui.painter().text(
                                        pos2(rect.right() - 10.0, rect.center().y),
                                        egui::Align2::RIGHT_CENTER,
                                        format_size(e.size),
                                        FontId::proportional(12.5),
                                        p.on_surface_variant,
                                    );
                                }
                                if resp.clicked() {
                                    let multi =
                                        picker.multiple() && ui.input(|i| i.modifiers.command);
                                    if multi {
                                        if selected {
                                            picker.selection.retain(|s| *s != e.path);
                                        } else {
                                            picker.selection.push(e.path.clone());
                                        }
                                    } else {
                                        picker.selection = vec![e.path.clone()];
                                    }
                                    if let (PickKind::Save { .. }, false) =
                                        (&picker.spec.kind, e.is_dir)
                                    {
                                        picker.name = e.name.clone();
                                    }
                                    picker.confirm_overwrite = false;
                                }
                                if resp.double_clicked() {
                                    if e.is_dir {
                                        navigate = Some(e.path.clone());
                                    } else {
                                        picker.selection = vec![e.path.clone()];
                                        accept = true;
                                    }
                                }
                            }
                        });
                });
        });
    });

    // Footer: file name (save), filter, buttons.
    ui.add_space(10.0);
    let save = matches!(picker.spec.kind, PickKind::Save { .. });
    if save {
        ui.horizontal(|ui| {
            ui.label("Name");
            let resp =
                ui.add(egui::TextEdit::singleline(&mut picker.name).desired_width(f32::INFINITY));
            if resp.changed() {
                picker.confirm_overwrite = false;
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                accept = true;
            }
        });
    }
    ui.horizontal(|ui| {
        if !picker.spec.filters.is_empty() {
            let current = picker
                .spec
                .filters
                .get(picker.filter)
                .map(|f| f.name.clone())
                .unwrap_or_default();
            let mut chosen = picker.filter;
            egui::ComboBox::from_id_salt("picker-filter")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for (i, f) in picker.spec.filters.iter().enumerate() {
                        ui.selectable_value(&mut chosen, i, &f.name);
                    }
                });
            if chosen != picker.filter {
                picker.filter = chosen;
                picker.refresh();
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let default_label = match &picker.spec.kind {
                PickKind::Save { .. } => "Save",
                PickKind::Folder { .. } => "Select folder",
                PickKind::Open { .. } => "Open",
            };
            let label = if picker.confirm_overwrite {
                "Replace".to_string()
            } else {
                picker
                    .spec
                    .accept_label
                    .clone()
                    .unwrap_or_else(|| default_label.into())
            };
            let can_accept = match &picker.spec.kind {
                PickKind::Open { .. } => picker.selection.iter().any(|s| !s.is_dir()),
                PickKind::Save { .. } => ops::validate_name(&picker.name).is_ok(),
                PickKind::Folder { .. } => true,
            };
            let kind = if picker.confirm_overwrite {
                ButtonKind::Danger
            } else {
                ButtonKind::Filled
            };
            if widgets::button_enabled(ui, kind, None, &label, can_accept).clicked() {
                accept = true;
            }
            if widgets::button(ui, ButtonKind::Text, None, "Cancel").clicked() {
                *keep = false;
            }
            if picker.confirm_overwrite {
                ui.label(
                    RichText::new("A file with this name already exists.")
                        .color(p.error)
                        .size(12.5),
                );
            }
        });
    });

    if let Some(dir) = navigate {
        picker.enter(dir);
    }
    if accept {
        let result: Option<Vec<PathBuf>> = match &picker.spec.kind {
            PickKind::Open { .. } => {
                let files: Vec<PathBuf> = picker
                    .selection
                    .iter()
                    .filter(|s| !s.is_dir())
                    .cloned()
                    .collect();
                if files.is_empty() {
                    // Double-click on a folder entry, or Enter on a folder: go in.
                    if let Some(dir) = picker.selection.iter().find(|s| s.is_dir()).cloned() {
                        picker.enter(dir);
                    }
                    None
                } else {
                    Some(files)
                }
            }
            PickKind::Folder { .. } => {
                let dirs: Vec<PathBuf> = picker
                    .selection
                    .iter()
                    .filter(|s| s.is_dir())
                    .cloned()
                    .collect();
                Some(if dirs.is_empty() {
                    vec![picker.dir.clone()]
                } else {
                    dirs
                })
            }
            PickKind::Save { .. } => {
                let target = picker.dir.join(picker.name.trim());
                if target.is_dir() {
                    picker.enter(target);
                    None
                } else if target.exists() && !picker.confirm_overwrite {
                    picker.confirm_overwrite = true;
                    None
                } else {
                    Some(vec![target])
                }
            }
        };
        if let Some(paths) = result {
            match &mut picker.purpose {
                PickerPurpose::MoveTo(sources) => {
                    app.actions.push(Action::Drop {
                        paths: std::mem::take(sources),
                        target: paths[0].clone(),
                        copy: false,
                    });
                }
                PickerPurpose::CopyTo(sources) => {
                    app.actions.push(Action::Drop {
                        paths: std::mem::take(sources),
                        target: paths[0].clone(),
                        copy: true,
                    });
                }
                PickerPurpose::Pin => {
                    for p in paths {
                        app.actions.push(Action::Pin(p));
                    }
                }
                PickerPurpose::Portal(tx) => {
                    if let Some(tx) = tx.take() {
                        let _ = tx.send(PortalResponse::Selected(paths));
                    }
                }
            }
            *keep = false;
        }
    }
}
