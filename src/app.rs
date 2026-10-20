//! Top-level application state, frame layout and action dispatch.

use crate::config::{ColorScheme, Config, ThemeSource, ViewMode};
use crate::fs::apps::{scan_applications, AppEntry};
use crate::fs::ops::{self, Job, JobKind, Notify};
use crate::fs::places::{self, Drive, Place};
use crate::fs::{launch, portal};
use crate::tab::{Location, Tab};
use crate::theme::{matugen, style, M3Palette};
use crate::thumbs::Thumbnails;
use crate::ui::dialogs::{self, Dialog, PickerPurpose};
use crate::ui::toasts::{Toast, Toasts};
use crate::ui::{self, widgets, Action, Actions, DragPaths, ToastKind};
use egui::{CornerRadius, Frame, Key, KeyboardShortcut, Margin, Modifiers};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipOp {
    Copy,
    Cut,
}

pub struct Clip {
    pub op: ClipOp,
    pub paths: Vec<PathBuf>,
    /// Text we placed on the system clipboard, to recognise it on paste.
    text: String,
}

pub struct Startup {
    pub location: Option<PathBuf>,
    pub portal: bool,
}

pub struct App {
    pub ctx: egui::Context,
    pub config: Config,
    last_saved_config: String,
    pub palette: M3Palette,
    system_dark: Option<bool>,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub thumbs: Thumbnails,
    pub apps: Option<Arc<Vec<AppEntry>>>,
    apps_rx: Option<flume::Receiver<Vec<AppEntry>>>,
    pub drives: Vec<Drive>,
    drives_rx: Option<flume::Receiver<Vec<Drive>>>,
    drives_at: Instant,
    pub places: Vec<Place>,
    pub clipboard: Option<Clip>,
    pub jobs: Vec<Job>,
    pub toasts: Toasts,
    pub dialog: Option<Dialog>,
    pub show_details: bool,
    portal_rx: Option<flume::Receiver<portal::PortalRequest>>,
    portal_queue: VecDeque<portal::PortalRequest>,
    _config_watcher: Option<notify::RecommendedWatcher>,
    config_events: flume::Receiver<PathBuf>,
    pub actions: Actions,
    pub settings: ui::settings::SettingsState,
    frame_stats: crate::perf::FrameStats,
}

pub const SHORTCUTS: &[(&str, &str)] = &[
    ("Ctrl+T / Ctrl+W", "New tab / close tab"),
    ("Ctrl+Tab", "Next tab"),
    ("Alt+← / Alt+→", "Back / forward"),
    ("Alt+↑ or Backspace", "Parent folder"),
    ("Alt+Home", "Home folder"),
    ("Ctrl+L", "Type a location"),
    ("Ctrl+F", "Search"),
    ("F5 / Ctrl+R", "Reload"),
    ("Enter / Ctrl+Enter", "Open / open folder in new tab"),
    ("Arrows, Home, End", "Move selection (Shift extends)"),
    ("Type letters", "Jump to a file by name"),
    ("Ctrl+A", "Select all"),
    ("Ctrl+C / X / V", "Copy / cut / paste"),
    ("F2", "Rename"),
    ("Delete", "Move to trash"),
    ("Shift+Delete", "Delete permanently"),
    ("Ctrl+Shift+N", "New folder"),
    ("Alt+Enter", "Properties"),
    ("Ctrl+D", "Pin current folder"),
    ("Ctrl+H", "Show hidden files"),
    ("Ctrl+G", "Group by type"),
    ("Ctrl+1 / Ctrl+2", "Grid / list view"),
    (
        "Ctrl+= / Ctrl+-",
        "Bigger / smaller icons (also Ctrl+scroll)",
    ),
    ("F9", "Details pane"),
    ("Ctrl+,", "Settings"),
    ("F1", "This list"),
];

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, startup: Startup) -> Self {
        let ctx = cc.egui_ctx.clone();
        style::install_fonts(&ctx);
        egui_extras::install_image_loaders(&ctx);
        ctx.options_mut(|o| o.zoom_with_keyboard = false);

        let config = Config::load();
        let last_saved_config = config.to_toml();

        // Watch our config dir and matugen's output for live reloads.
        let (cfg_tx, cfg_rx) = flume::unbounded();
        let config_watcher = {
            use notify::Watcher;
            let ctx = ctx.clone();
            let tx = cfg_tx.clone();
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if let Ok(ev) = res {
                    if ev.kind.is_create() || ev.kind.is_modify() {
                        for p in ev.paths {
                            let _ = tx.send(p);
                        }
                        ctx.request_repaint_after(Duration::from_millis(100));
                    }
                }
            })
            .ok()
            .map(|mut w| {
                for dir in [
                    Config::path().and_then(|p| p.parent().map(Path::to_path_buf)),
                    matugen::config_dir(),
                ]
                .into_iter()
                .flatten()
                {
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = w.watch(&dir, notify::RecursiveMode::NonRecursive);
                }
                w
            })
        };

        let start = startup
            .location
            .filter(|p| p.is_dir())
            .map(Location::Folder)
            .unwrap_or(Location::Dashboard);

        let (apps_tx, apps_rx) = flume::bounded(1);
        std::thread::spawn(move || {
            let _ = apps_tx.send(scan_applications());
        });

        let portal_rx = startup.portal.then(|| {
            let ctx = ctx.clone();
            portal::start(move || {
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                ctx.request_repaint();
            });
            portal::requests()
        });

        let show_details = config.ui.show_preview_pane;
        let mut app = Self {
            thumbs: Thumbnails::new(ctx.clone()),
            tabs: vec![Tab::new(start, &ctx)],
            active: 0,
            palette: M3Palette::from_seed(crate::theme::palette::DEFAULT_SEED, true),
            system_dark: ctx.system_theme().map(|t| t == egui::Theme::Dark),
            config,
            last_saved_config,
            apps: None,
            apps_rx: Some(apps_rx),
            drives: places::drives(),
            drives_rx: None,
            drives_at: Instant::now(),
            places: places::standard_places(),
            clipboard: None,
            jobs: Vec::new(),
            toasts: Toasts::default(),
            dialog: None,
            show_details,
            portal_rx,
            portal_queue: VecDeque::new(),
            _config_watcher: config_watcher,
            config_events: cfg_rx,
            actions: Vec::new(),
            settings: Default::default(),
            frame_stats: Default::default(),
            ctx,
        };
        app.apply_theme();
        app
    }

    pub fn tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    pub fn tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    pub fn notify(&self) -> Notify {
        let ctx = self.ctx.clone();
        Arc::new(move || ctx.request_repaint())
    }

    pub fn toast(&mut self, text: impl Into<String>, kind: ToastKind) {
        self.toasts.push(Toast::new(text, kind));
    }

    // ------------------------------------------------------------------ theme

    fn wants_dark(&self) -> bool {
        match self.config.ui.color_scheme {
            ColorScheme::Light => false,
            ColorScheme::Dark => true,
            ColorScheme::System => self.system_dark.unwrap_or(true),
        }
    }

    /// Which palette source is in effect right now.
    pub fn effective_theme_source(&self) -> ThemeSource {
        let has_json = matugen::theme_json_path()
            .map(|p| p.exists())
            .unwrap_or(false);
        match self.config.ui.theme_source {
            ThemeSource::Auto if has_json => ThemeSource::Matugen,
            ThemeSource::Auto => ThemeSource::Seed,
            ThemeSource::Matugen if !has_json => ThemeSource::Seed,
            other => other,
        }
    }

    pub fn apply_theme(&mut self) {
        let dark = self.wants_dark();
        let palette = match self.effective_theme_source() {
            ThemeSource::Matugen => {
                let path = matugen::theme_json_path().unwrap_or_default();
                match M3Palette::from_theme_json(&path) {
                    Ok(p) => p,
                    Err(e) => {
                        self.toast(format!("Couldn't read theme.json: {e}"), ToastKind::Error);
                        M3Palette::from_seed(&self.config.ui.seed_color, dark)
                    }
                }
            }
            _ => M3Palette::from_seed(&self.config.ui.seed_color, dark),
        };
        style::apply(&self.ctx, &palette);
        widgets::set_palette(&self.ctx, palette.clone());
        widgets::set_filled_icons(self.config.ui.filled_icons);
        let scale = self.config.ui.ui_scale.clamp(0.6, 2.5);
        if (self.ctx.zoom_factor() - scale).abs() > f32::EPSILON {
            self.ctx.set_zoom_factor(scale);
        }
        self.palette = palette;
    }

    // ----------------------------------------------------------------- config

    pub fn save_config(&mut self) {
        match self.config.save() {
            Ok(contents) => self.last_saved_config = contents,
            Err(e) => self.toast(format!("Couldn't save settings: {e}"), ToastKind::Error),
        }
    }

    fn on_config_changed(&mut self) {
        self.save_config();
        self.apply_theme();
        for tab in &mut self.tabs {
            tab.invalidate_view();
        }
    }

    fn poll_config_files(&mut self) {
        let changed: Vec<PathBuf> = self.config_events.try_iter().collect();
        if changed.is_empty() {
            return;
        }
        let config_path = Config::path();
        if changed.iter().any(|p| Some(p) == config_path.as_ref()) {
            if let Some(contents) = config_path
                .as_ref()
                .and_then(|p| std::fs::read_to_string(p).ok())
            {
                if contents != self.last_saved_config {
                    match toml::from_str::<Config>(&contents) {
                        Ok(cfg) => {
                            self.config = cfg;
                            self.last_saved_config = contents;
                            self.apply_theme();
                            for tab in &mut self.tabs {
                                tab.invalidate_view();
                            }
                            self.toast("Settings reloaded from config.toml", ToastKind::Info);
                        }
                        Err(e) => {
                            self.toast(format!("config.toml has an error: {e}"), ToastKind::Error)
                        }
                    }
                }
            }
        }
        if changed
            .iter()
            .any(|p| p.file_name().map(|n| n == "theme.json").unwrap_or(false))
        {
            self.apply_theme();
        }
    }

    // ---------------------------------------------------------- background work

    fn poll_background(&mut self) {
        if let Some(rx) = &self.apps_rx {
            if let Ok(apps) = rx.try_recv() {
                self.apps = Some(Arc::new(apps));
                self.apps_rx = None;
            }
        }

        // Refresh drives every few seconds off the UI thread.
        if let Some(rx) = &self.drives_rx {
            if let Ok(drives) = rx.try_recv() {
                self.drives = drives;
                self.drives_rx = None;
            }
        } else if self.drives_at.elapsed() > Duration::from_secs(5) {
            self.drives_at = Instant::now();
            let (tx, rx) = flume::bounded(1);
            let ctx = self.ctx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(places::drives());
                ctx.request_repaint();
            });
            self.drives_rx = Some(rx);
        }
        self.ctx.request_repaint_after(Duration::from_secs(5));

        let system_dark = self.ctx.system_theme().map(|t| t == egui::Theme::Dark);
        if system_dark != self.system_dark {
            self.system_dark = system_dark;
            if self.config.ui.color_scheme == ColorScheme::System {
                self.apply_theme();
            }
        }

        for tab in &mut self.tabs {
            tab.poll(&self.config);
        }
        let visited: Vec<PathBuf> = self
            .tabs
            .iter_mut()
            .filter_map(|t| t.visited.take())
            .collect();
        if !visited.is_empty() {
            for path in &visited {
                self.config.push_recent(path);
            }
            self.save_config();
        }

        self.poll_jobs();
        self.poll_config_files();
        self.thumbs.poll();

        if let Some(rx) = &self.portal_rx {
            self.portal_queue.extend(rx.try_iter());
        }
        if self.dialog.is_none() {
            if let Some(req) = self.portal_queue.pop_front() {
                let start = req
                    .spec
                    .start_dir
                    .clone()
                    .filter(|d| d.is_dir())
                    .or_else(|| self.tab().folder().map(Path::to_path_buf))
                    .unwrap_or_else(places::home);
                self.dialog = Some(Dialog::picker(
                    req.spec,
                    PickerPurpose::Portal(Some(req.respond)),
                    start,
                    &self.ctx,
                ));
            }
        }
    }

    fn poll_jobs(&mut self) {
        let mut finished = Vec::new();
        self.jobs.retain(|job| match job.done.try_recv() {
            Ok(outcome) => {
                finished.push((
                    job.kind,
                    job.affected_dirs.clone(),
                    outcome,
                    job.label.clone(),
                ));
                false
            }
            Err(flume::TryRecvError::Empty) => true,
            Err(flume::TryRecvError::Disconnected) => false,
        });
        for (kind, dirs, outcome, _label) in finished {
            for tab in &mut self.tabs {
                if tab
                    .folder()
                    .map(|f| dirs.iter().any(|d| d == f))
                    .unwrap_or(false)
                {
                    tab.reload();
                }
            }
            if let (true, Some(folder)) = (
                matches!(kind, JobKind::Copy | JobKind::Move),
                self.current_folder(),
            ) {
                let created: Vec<PathBuf> = outcome
                    .created
                    .iter()
                    .filter(|p| p.parent() == Some(folder.as_path()))
                    .cloned()
                    .collect();
                if !created.is_empty() {
                    self.tab_mut().select_when_listed = created;
                }
            }
            let n = outcome.succeeded;
            let noun = if n == 1 { "item" } else { "items" };
            if outcome.cancelled {
                self.toast(
                    format!("{} cancelled after {n} {noun}", kind.verb()),
                    ToastKind::Info,
                );
            } else if !outcome.errors.is_empty() {
                let first = outcome.errors.first().cloned().unwrap_or_default();
                let more = if outcome.errors.len() > 1 {
                    format!(" (+{} more)", outcome.errors.len() - 1)
                } else {
                    String::new()
                };
                self.toast(
                    format!("{} {n} {noun}. Failed: {first}{more}", kind.past()),
                    ToastKind::Error,
                );
            } else if kind == JobKind::Trash {
                let mut t = Toast::new(format!("Moved {n} {noun} to trash"), ToastKind::Success);
                t.action = Some(("Undo".into(), Action::UndoTrash(outcome.created.clone())));
                self.toasts.push(t);
            } else {
                self.toast(format!("{} {n} {noun}", kind.past()), ToastKind::Success);
            }
        }
        if !self.jobs.is_empty() {
            self.ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    // ----------------------------------------------------------------- actions

    fn current_folder(&self) -> Option<PathBuf> {
        self.tab().folder().map(Path::to_path_buf)
    }

    fn open_tab(&mut self, location: Location) {
        let tab = Tab::new(location, &self.ctx);
        self.tabs.insert(self.active + 1, tab);
        self.active += 1;
    }

    fn set_clipboard(&mut self, op: ClipOp, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let text = paths
            .iter()
            .map(|p| p.to_string_lossy())
            .collect::<Vec<_>>()
            .join("\n");
        self.ctx.copy_text(text.clone());
        let n = paths.len();
        let verb = if op == ClipOp::Copy { "Copied" } else { "Cut" };
        self.toast(
            format!(
                "{verb} {n} {} — paste with Ctrl+V",
                if n == 1 { "item" } else { "items" }
            ),
            ToastKind::Info,
        );
        self.clipboard = Some(Clip { op, paths, text });
    }

    fn paste_into(&mut self, dir: PathBuf, system_text: Option<String>) {
        // Text copied elsewhere (paths or file:// URIs) takes precedence
        // over a stale internal clipboard.
        if let Some(text) = system_text {
            let ours = self
                .clipboard
                .as_ref()
                .map(|c| c.text == text)
                .unwrap_or(false);
            if !ours {
                let paths = parse_pasted_paths(&text);
                if paths.is_empty() {
                    return;
                }
                let job = ops::transfer(paths, dir, JobKind::Copy, self.notify());
                self.jobs.push(job);
                return;
            }
        }
        let Some(clip) = &self.clipboard else {
            self.toast("Nothing to paste", ToastKind::Info);
            return;
        };
        let kind = if clip.op == ClipOp::Copy {
            JobKind::Copy
        } else {
            JobKind::Move
        };
        let job = ops::transfer(clip.paths.clone(), dir, kind, self.notify());
        self.jobs.push(job);
        if kind == JobKind::Move {
            self.clipboard = None;
        }
    }

    fn process_actions(&mut self) {
        let actions = std::mem::take(&mut self.actions);
        for action in actions {
            self.process(action);
        }
    }

    fn process(&mut self, action: Action) {
        match action {
            Action::Navigate(loc) => {
                // Settings is a single tab of its own, so opening it never
                // replaces the folder you were looking at.
                if loc == Location::Settings {
                    match self
                        .tabs
                        .iter()
                        .position(|t| t.location == Location::Settings)
                    {
                        Some(i) => self.active = i,
                        None => self.open_tab(Location::Settings),
                    }
                    return;
                }
                self.tab_mut().navigate(loc);
            }
            Action::OpenInNewTab(loc) => self.open_tab(loc),
            Action::Open(path) => {
                if path.is_dir() {
                    self.tab_mut().navigate(Location::Folder(path));
                } else if let Err(e) = launch::open_default(&path, self.config.use_dms) {
                    self.toast(e.to_string(), ToastKind::Error);
                }
            }
            Action::OpenWith(path) => {
                if self.config.use_dms {
                    if let Err(e) = launch::open_default(&path, true) {
                        self.toast(e.to_string(), ToastKind::Error);
                    }
                } else {
                    self.dialog = Some(Dialog::open_with(path));
                }
            }
            Action::OpenInTerminal(path) => {
                let dir = if path.is_dir() {
                    path
                } else {
                    path.parent().map(Path::to_path_buf).unwrap_or(path)
                };
                if let Err(e) = launch::open_terminal(&dir, self.config.command("terminal")) {
                    self.toast(e.to_string(), ToastKind::Error);
                }
            }
            Action::OpenInEditor(path) => {
                if let Err(e) = launch::open_in_editor(&path, self.config.command("editor")) {
                    self.toast(e.to_string(), ToastKind::Error);
                }
            }
            Action::Back => self.tab_mut().go_back(),
            Action::Forward => self.tab_mut().go_forward(),
            Action::Up => self.tab_mut().go_up(),
            Action::Reload => {
                self.tab_mut().reload();
                self.drives = places::drives();
            }
            Action::NewTab => {
                let loc = self
                    .current_folder()
                    .map(Location::Folder)
                    .unwrap_or(Location::Folder(places::home()));
                self.open_tab(loc);
            }
            Action::CloseTab(i) => {
                if self.tabs.len() > 1 && i < self.tabs.len() {
                    self.tabs.remove(i);
                    if self.active > i || self.active >= self.tabs.len() {
                        self.active = self.active.saturating_sub(1);
                    }
                } else {
                    self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Action::ActivateTab(i) => {
                if i < self.tabs.len() {
                    self.active = i;
                }
            }
            Action::Copy(paths) => self.set_clipboard(ClipOp::Copy, paths),
            Action::Cut(paths) => self.set_clipboard(ClipOp::Cut, paths),
            Action::Paste(dir) => self.paste_into(dir, None),
            Action::Trash(paths) => {
                if !paths.is_empty() {
                    let job = ops::remove(paths, false, self.notify());
                    self.jobs.push(job);
                }
            }
            Action::UndoTrash(paths) => {
                let notify = self.notify();
                let (tx, rx) = flume::bounded(1);
                std::thread::spawn(move || {
                    let _ = tx.send(restore_from_trash(&paths));
                    notify();
                });
                self.toasts.pending_restore = Some(rx);
            }
            Action::DeleteForever(paths) => {
                if !paths.is_empty() {
                    self.dialog = Some(Dialog::ConfirmDelete { paths });
                }
            }
            Action::StartRename(path) => self.tab_mut().start_rename(&path),
            Action::CommitRename { path, name } => match ops::rename(&path, &name) {
                Ok(new_path) => {
                    let tab = self.tab_mut();
                    tab.rename = None;
                    tab.select_when_listed = vec![new_path];
                    tab.reload();
                }
                Err(e) => {
                    if let Some(r) = &mut self.tab_mut().rename {
                        r.error = Some(e.to_string());
                    }
                }
            },
            Action::NewFolder(dir) => self.dialog = Some(Dialog::new_name(dir, true)),
            Action::NewFile(dir) => self.dialog = Some(Dialog::new_name(dir, false)),
            Action::MoveTo(paths) | Action::CopyTo(paths) if paths.is_empty() => {}
            Action::MoveTo(paths) => {
                let start = self.current_folder().unwrap_or_else(places::home);
                self.dialog = Some(Dialog::folder_picker(
                    "Move to…",
                    "Move here",
                    PickerPurpose::MoveTo(paths),
                    start,
                    &self.ctx,
                ));
            }
            Action::CopyTo(paths) => {
                let start = self.current_folder().unwrap_or_else(places::home);
                self.dialog = Some(Dialog::folder_picker(
                    "Copy to…",
                    "Copy here",
                    PickerPurpose::CopyTo(paths),
                    start,
                    &self.ctx,
                ));
            }
            Action::Drop {
                paths,
                target,
                copy,
            } => {
                let paths: Vec<PathBuf> = paths
                    .into_iter()
                    .filter(|p| *p != target && (copy || p.parent() != Some(target.as_path())))
                    .collect();
                if !paths.is_empty() && target.is_dir() {
                    let kind = if copy { JobKind::Copy } else { JobKind::Move };
                    let job = ops::transfer(paths, target, kind, self.notify());
                    self.jobs.push(job);
                }
            }
            Action::CancelJob(id) => {
                if let Some(job) = self.jobs.iter().find(|j| j.id == id) {
                    job.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
            Action::Pin(path) => {
                if self.config.add_pinned_folder(path.clone()) {
                    self.save_config();
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    self.toast(format!("Pinned “{name}”"), ToastKind::Success);
                }
            }
            Action::Unpin(path) => {
                if self.config.remove_pinned_folder(&path) {
                    self.save_config();
                }
            }
            Action::PickFolderToPin => {
                self.dialog = Some(Dialog::folder_picker(
                    "Pin a folder",
                    "Pin",
                    PickerPurpose::Pin,
                    places::home(),
                    &self.ctx,
                ));
            }
            Action::ClearRecent => {
                self.config.recent_folders.clear();
                self.save_config();
            }
            Action::Properties(path) => self.dialog = Some(Dialog::properties(path, &self.ctx)),
            Action::CopyPathText(paths) => {
                let text = paths
                    .iter()
                    .map(|p| p.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("\n");
                self.ctx.copy_text(text);
                self.toast("Path copied to clipboard", ToastKind::Info);
            }
            Action::ConfigChanged => self.on_config_changed(),
            Action::ReloadTheme => {
                self.apply_theme();
                self.toast("Theme reloaded", ToastKind::Info);
            }
            Action::ShowShortcuts => self.dialog = Some(Dialog::Shortcuts),
            Action::SelectAll => self.tab_mut().select_all(),
            Action::Toast(text, kind) => self.toast(text, kind),
            Action::ToggleDetails => {
                self.show_details = !self.show_details;
                self.config.ui.show_preview_pane = self.show_details;
                self.save_config();
            }
            Action::FocusSearch => self.tab_mut().focus_search = true,
            Action::EditPath => {
                if let Some(folder) = self.current_folder() {
                    let mut text = folder.to_string_lossy().into_owned();
                    if !text.ends_with('/') {
                        text.push('/');
                    }
                    self.tab_mut().path_edit = Some(text);
                }
            }
        }
    }

    // --------------------------------------------------------------- keyboard

    fn handle_shortcuts(&mut self) {
        let typing = self.ctx.egui_wants_keyboard_input();
        let modal = self.dialog.is_some();
        let renaming = self.tab().rename.is_some();
        let mut actions: Actions = Vec::new();
        let folder = self.current_folder();
        // Most frames have no key input; don't copy the selection for those.
        let has_keys = self.ctx.input(|i| {
            i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Key { pressed: true, .. } | egui::Event::Copy | egui::Event::Cut
                )
            })
        });
        let selected = if has_keys {
            self.tab().selected_paths()
        } else {
            Vec::new()
        };
        let ctrl = Modifiers::COMMAND;
        let ctrl_shift = Modifiers::COMMAND | Modifiers::SHIFT;

        let mut text_input = String::new();
        let mut system_paste: Option<String> = None;
        let mut copy_event = false;
        let mut cut_event = false;

        self.ctx.input_mut(|i| {
            let sc = |i: &mut egui::InputState, m: Modifiers, k: Key| {
                i.consume_shortcut(&KeyboardShortcut::new(m, k))
            };

            // Shortcuts that work everywhere (even while typing).
            if sc(i, ctrl, Key::T) {
                actions.push(Action::NewTab);
            }
            if sc(i, ctrl, Key::W) {
                actions.push(Action::CloseTab(self.active));
            }
            if sc(i, ctrl_shift, Key::Tab) {
                let n = self.tabs.len();
                actions.push(Action::ActivateTab((self.active + n - 1) % n));
            } else if sc(i, ctrl, Key::Tab) {
                actions.push(Action::ActivateTab((self.active + 1) % self.tabs.len()));
            }
            if sc(i, ctrl, Key::Comma) {
                actions.push(Action::Navigate(Location::Settings));
            }
            if modal {
                return;
            }
            if sc(i, ctrl, Key::F) {
                actions.push(Action::FocusSearch);
            }
            if sc(i, ctrl, Key::L) {
                actions.push(Action::EditPath);
            }
            if sc(i, Modifiers::ALT, Key::ArrowLeft) {
                actions.push(Action::Back);
            }
            if sc(i, Modifiers::ALT, Key::ArrowRight) {
                actions.push(Action::Forward);
            }
            if sc(i, Modifiers::ALT, Key::ArrowUp) {
                actions.push(Action::Up);
            }
            if sc(i, Modifiers::ALT, Key::Home) {
                actions.push(Action::Navigate(Location::Folder(places::home())));
            }
            if sc(i, Modifiers::NONE, Key::F5) || sc(i, ctrl, Key::R) {
                actions.push(Action::Reload);
            }
            if sc(i, Modifiers::NONE, Key::F9) {
                actions.push(Action::ToggleDetails);
            }
            if sc(i, Modifiers::NONE, Key::F1) {
                actions.push(Action::ShowShortcuts);
            }
            if sc(i, ctrl, Key::H) {
                self.config.ui.show_hidden = !self.config.ui.show_hidden;
                actions.push(Action::ConfigChanged);
            }
            if sc(i, ctrl, Key::G) {
                self.config.group_files_by_type = !self.config.group_files_by_type;
                actions.push(Action::ConfigChanged);
            }
            if sc(i, ctrl, Key::Num1) {
                self.config.ui.view_mode = ViewMode::Grid;
                actions.push(Action::ConfigChanged);
            }
            if sc(i, ctrl, Key::Num2) {
                self.config.ui.view_mode = ViewMode::List;
                actions.push(Action::ConfigChanged);
            }
            if sc(i, ctrl, Key::Equals) || sc(i, ctrl, Key::Plus) {
                self.config.ui.icon_size = (self.config.ui.icon_size + 16).min(160);
                actions.push(Action::ConfigChanged);
            }
            if sc(i, ctrl, Key::Minus) {
                self.config.ui.icon_size = self.config.ui.icon_size.saturating_sub(16).max(32);
                actions.push(Action::ConfigChanged);
            }
            if let Some(dir) = &folder {
                if sc(i, ctrl_shift, Key::N) {
                    actions.push(Action::NewFolder(dir.clone()));
                }
                if sc(i, ctrl, Key::D) {
                    actions.push(Action::Pin(dir.clone()));
                }
            }

            if typing || renaming || folder.is_none() {
                return;
            }
            // File-view keys (only when no text field has focus).
            for ev in &i.events {
                match ev {
                    egui::Event::Copy => copy_event = true,
                    egui::Event::Cut => cut_event = true,
                    egui::Event::Paste(text) => system_paste = Some(text.clone()),
                    egui::Event::Text(t) if !i.modifiers.command && !i.modifiers.alt => {
                        text_input.push_str(t)
                    }
                    _ => {}
                }
            }
            if sc(i, ctrl, Key::A) {
                actions.push(Action::SelectAll);
            }
            if sc(i, Modifiers::SHIFT, Key::Delete) {
                actions.push(Action::DeleteForever(selected.clone()));
            } else if sc(i, Modifiers::NONE, Key::Delete) {
                actions.push(Action::Trash(selected.clone()));
            }
            if sc(i, Modifiers::NONE, Key::F2) {
                if let Some(p) = selected.first() {
                    actions.push(Action::StartRename(p.clone()));
                }
            }
            if sc(i, Modifiers::ALT, Key::Enter) {
                if let Some(p) = selected.first().cloned().or(folder.clone()) {
                    actions.push(Action::Properties(p));
                }
            }
            if sc(i, ctrl, Key::Enter) {
                for p in selected.iter().filter(|p| p.is_dir()) {
                    actions.push(Action::OpenInNewTab(Location::Folder(p.clone())));
                }
            } else if sc(i, Modifiers::NONE, Key::Enter) {
                if let [one] = selected.as_slice() {
                    actions.push(Action::Open(one.clone()));
                } else {
                    for p in selected.iter().filter(|p| !p.is_dir()) {
                        actions.push(Action::Open(p.clone()));
                    }
                }
            }
            if sc(i, Modifiers::NONE, Key::Backspace) {
                actions.push(Action::Up);
            }
        });

        let mut final_actions = actions;
        if copy_event {
            final_actions.push(Action::Copy(selected.clone()));
        }
        if cut_event {
            final_actions.push(Action::Cut(selected.clone()));
        }
        if let (Some(text), Some(dir)) = (system_paste, folder.clone()) {
            self.paste_into(dir, Some(text));
        }
        let text_input: String = text_input
            .chars()
            .filter(|c| !c.is_control() && *c != ' ')
            .collect();
        if !text_input.is_empty() {
            self.tab_mut().type_ahead(&text_input);
        }
        self.actions.extend(final_actions);
    }

    // ------------------------------------------------------------------ layout

    fn layout(&mut self, ui: &mut egui::Ui) {
        let p = self.palette.clone();
        let surface = Frame::new().fill(p.surface);

        egui::Panel::left("sidebar")
            .resizable(true)
            .default_size(252.0)
            .size_range(200.0..=360.0)
            .show_separator_line(false)
            .frame(surface.inner_margin(Margin {
                left: 12,
                right: 4,
                top: 12,
                bottom: 12,
            }))
            .show(ui, |ui| ui::sidebar::show(ui, self));

        egui::Panel::top("tabs")
            .show_separator_line(false)
            .frame(surface.inner_margin(Margin {
                left: 8,
                right: 12,
                top: 10,
                bottom: 0,
            }))
            .show(ui, |ui| ui::tab_bar::show(ui, self));

        let is_folder = self.tab().folder().is_some();
        if is_folder {
            egui::Panel::top("toolbar")
                .show_separator_line(false)
                .frame(surface.inner_margin(Margin {
                    left: 8,
                    right: 12,
                    top: 8,
                    bottom: 6,
                }))
                .show(ui, |ui| ui::toolbar::show(ui, self));

            egui::Panel::bottom("status")
                .show_separator_line(false)
                .frame(surface.inner_margin(Margin {
                    left: 16,
                    right: 16,
                    top: 2,
                    bottom: 6,
                }))
                .show(ui, |ui| ui::status_bar::show(ui, self));

            if self.show_details {
                egui::Panel::right("details")
                    .resizable(true)
                    .default_size(300.0)
                    .size_range(240.0..=480.0)
                    .show_separator_line(false)
                    .frame(surface.inner_margin(Margin {
                        left: 0,
                        right: 12,
                        top: 0,
                        bottom: 0,
                    }))
                    .show(ui, |ui| ui::details::show(ui, self));
            }
        }

        egui::CentralPanel::default()
            .frame(surface.inner_margin(Margin {
                left: 8,
                right: 8,
                top: if is_folder { 0 } else { 8 },
                bottom: if is_folder { 0 } else { 12 },
            }))
            .show(ui, |ui| {
                Frame::new()
                    .fill(p.surface_container_low)
                    .corner_radius(CornerRadius::same(24))
                    .show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        match self.tab().location.clone() {
                            Location::Dashboard => ui::dashboard::show(ui, self),
                            Location::Settings => ui::settings::show(ui, self),
                            Location::Folder(_) => ui::file_view::show(ui, self),
                        }
                    });
            });
    }
}

/// Paths from pasted text: one per line, absolute paths or file:// URIs.
fn parse_pasted_paths(text: &str) -> Vec<PathBuf> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            if let Some(rest) = l.strip_prefix("file://") {
                url::Url::parse(&format!("file://{rest}"))
                    .ok()?
                    .to_file_path()
                    .ok()
            } else {
                Some(PathBuf::from(l))
            }
        })
        .filter(|p| p.is_absolute() && std::fs::symlink_metadata(p).is_ok())
        .collect()
}

/// Restore the most recently trashed item for each original path.
fn restore_from_trash(paths: &[PathBuf]) -> Result<usize, String> {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let items = trash::os_limited::list().map_err(|e| e.to_string())?;
        let mut chosen = Vec::new();
        for p in paths {
            if let Some(item) = items
                .iter()
                .filter(|i| i.original_path() == *p)
                .max_by_key(|i| i.time_deleted)
            {
                chosen.push(item.clone());
            }
        }
        let n = chosen.len();
        trash::os_limited::restore_all(chosen).map_err(|e| e.to_string())?;
        Ok(n)
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        let _ = paths;
        Err("Restoring from trash isn't supported on this platform".into())
    }
}

impl eframe::App for App {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_background();
        if let Some(rx) = &self.toasts.pending_restore {
            if let Ok(result) = rx.try_recv() {
                self.toasts.pending_restore = None;
                match result {
                    Ok(n) => self.toast(
                        format!("Restored {n} item{}", if n == 1 { "" } else { "s" }),
                        ToastKind::Success,
                    ),
                    Err(e) => self.toast(format!("Couldn't restore: {e}"), ToastKind::Error),
                }
                for tab in &mut self.tabs {
                    tab.reload();
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let started = Instant::now();
        self.handle_shortcuts();
        self.layout(ui);
        dialogs::show(&self.ctx.clone(), self);
        ui::toasts::show(&self.ctx.clone(), self);
        paint_drag_ghost(&self.ctx, &self.palette);
        if !ui.ctx().input(|i| i.raw.dropped_files.is_empty()) {
            self.handle_os_drop();
        }
        self.process_actions();
        self.frame_stats.record(
            &self.ctx,
            started.elapsed().as_secs_f32() * 1e3,
            frame.info().cpu_usage.map(|s| s * 1e3),
        );
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.palette.surface.to_normalized_gamma_f32()
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_config();
    }
}

impl App {
    /// Files dropped from another application: copy into the current folder.
    fn handle_os_drop(&mut self) {
        let paths: Vec<PathBuf> = self.ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        });
        if let (false, Some(dir)) = (paths.is_empty(), self.current_folder()) {
            self.actions.push(Action::Drop {
                paths,
                target: dir,
                copy: true,
            });
        }
    }
}

fn paint_drag_ghost(ctx: &egui::Context, p: &M3Palette) {
    let Some(payload) = egui::DragAndDrop::payload::<DragPaths>(ctx) else {
        return;
    };
    let Some(pos) = ctx.pointer_interact_pos() else {
        return;
    };
    let copy = ctx.input(|i| i.modifiers.command);
    let n = payload.0.len();
    let label = format!(
        "{} {} {}",
        if copy { "Copy" } else { "Move" },
        n,
        if n == 1 { "item" } else { "items" }
    );
    egui::Area::new(egui::Id::unique("drag-ghost"))
        .order(egui::Order::Tooltip)
        .fixed_pos(pos + egui::vec2(14.0, 10.0))
        .interactable(false)
        .show(ctx, |ui| {
            Frame::new()
                .fill(p.inverse_surface)
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::symmetric(12, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        widgets::icon(
                            ui,
                            if copy {
                                egui_material_icons::icons::ICON_FILE_COPY
                            } else {
                                egui_material_icons::icons::ICON_DRIVE_FILE_MOVE
                            },
                            18.0,
                            p.inverse_on_surface,
                        );
                        ui.label(egui::RichText::new(label).color(p.inverse_on_surface));
                    });
                });
        });
}
