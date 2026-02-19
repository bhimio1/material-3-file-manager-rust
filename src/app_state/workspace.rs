use crate::app_state::config::ConfigContext;
use crate::assets::app_cache::AppCache;
use crate::fs_ops::provider::{FileEntry, FileSystemProvider, LocalFs};
use crate::fs_ops::scanner::SearchOptions;
use crate::ui_components::open_with_dialog::{OpenWithDialog, OpenWithEvent};
use crate::ui_components::toast::{Toast, ToastKind};
use crate::ui_components::universal_picker_modal::{FilePickerEvent, UniversalPickerModal};
use gpui::prelude::*;
use gpui::*;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug)]
pub enum WorkspaceEvent {
    PathChanged(PathBuf),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClipboardOp {
    Copy,
    Cut,
}

impl EventEmitter<WorkspaceEvent> for Workspace {}

#[derive(Clone, Debug, PartialEq)]
pub enum ActiveOverlay {
    ContextMenu,
    DetailsDialog(PathBuf),
    Settings,
    FolderPicker,
    OpenWith(PathBuf),
}

#[derive(Clone, Debug)]
pub struct ExtendedMetadata {
    pub size: u64,
    pub modified: std::time::SystemTime,
    pub mime_type: String,
    pub image_dimensions: Option<(u32, u32)>,
}

impl Default for ExtendedMetadata {
    fn default() -> Self {
        Self {
            size: 0,
            modified: std::time::SystemTime::now(),
            mime_type: "application/octet-stream".to_string(),
            image_dimensions: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PickerAction {
    Navigate,
    MoveSelection,
    CopySelection,
}

pub struct Workspace {
    pub current_path: PathBuf,
    pub items: Vec<FileEntry>,
    pub is_loading: bool,
    pub selection: HashSet<PathBuf>,
    pub last_selected: Option<PathBuf>,
    pub picker_action: Option<PickerAction>,
    pub toasts: Vec<Toast>,
    pub active_overlay: Option<ActiveOverlay>,
    pub context_menu_location: Point<Pixels>,
    pub context_menu_path: Option<PathBuf>,
    pub history: Vec<PathBuf>,
    pub history_index: usize,
    pub filter_query: String,
    pub search_options: SearchOptions,
    pub is_searching: bool,
    pub details_metadata: Option<ExtendedMetadata>,
    pub search_results: Option<Vec<FileEntry>>,
    pub clipboard_state: Option<(ClipboardOp, Vec<PathBuf>)>,
    pub is_dashboard: bool,
    pub group_by_type: bool,
    pub grouped_files: std::collections::HashMap<String, Vec<FileEntry>>,
    pub folder_picker: Option<Entity<UniversalPickerModal>>,
    pub open_with_dialog: Option<Entity<OpenWithDialog>>,
    pub app_cache: Entity<AppCache>,
    pub pending_portal_response:
        Option<tokio::sync::oneshot::Sender<crate::fs_ops::portal::PortalResponse>>,
}

impl Workspace {
    pub fn new<C: 'static>(
        cx: &mut Context<C>,
        initial_path: PathBuf,
        app_cache: Entity<AppCache>,
    ) -> Entity<Self> {
        let ws_entity = cx.new(|_cx| Self {
            app_cache,
            current_path: initial_path.clone(),
            items: Vec::new(),
            is_loading: false,
            selection: HashSet::new(),
            last_selected: None,
            picker_action: None,
            toasts: Vec::new(),
            active_overlay: None,
            context_menu_location: Point::new(px(0.0), px(0.0)),
            context_menu_path: None,
            history: vec![initial_path.clone()],
            history_index: 0,
            filter_query: String::new(),
            search_options: SearchOptions::default(),
            is_searching: false,
            details_metadata: None,
            search_results: None,
            clipboard_state: None,
            is_dashboard: false,
            group_by_type: false,
            grouped_files: std::collections::HashMap::new(),
            folder_picker: None,
            open_with_dialog: None,
            pending_portal_response: None,
        });

        // Start Portal Request Listener
        let rx = crate::fs_ops::portal::get_receiver();
        let weak_ws = ws_entity.downgrade();

        cx.spawn(move |_, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                while let Ok(req) = rx.recv_async().await {
                    let _ = cx.update(|cx| {
                        if let Some(handle) = weak_ws.upgrade() {
                            let _ = handle.update(cx, |this, cx| {
                                this.handle_portal_request(req, cx);
                            });
                        }
                    });
                }
            }
        })
        .detach();

        // Trigger initial load
        ws_entity.update(cx, |ws, cx| {
            ws.reload(cx);
        });

        ws_entity
    }

    pub fn load_details_metadata(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let executor = cx.background_executor().clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let meta_result = executor
                    .spawn(async move {
                        let res: Result<ExtendedMetadata, anyhow::Error> = if let Ok(meta) =
                            std::fs::metadata(&path)
                        {
                            let size = if meta.is_dir() {
                                crate::fs_ops::scanner::calculate_recursive_size(&path)
                            } else {
                                meta.len()
                            };

                            let mime_type = if meta.is_dir() {
                                "inode/directory".to_string()
                            } else {
                                crate::assets::mime_resolver::MimeResolver::get_mime(&path)
                                    .to_string()
                            };

                            let mut image_dimensions = None;
                            if !meta.is_dir() && mime_type.starts_with("image/") {
                                if let Ok(reader) = image::ImageReader::open(&path) {
                                    if let Ok(dim) = reader.into_dimensions() {
                                        image_dimensions = Some(dim);
                                    }
                                }
                            }

                            Ok(ExtendedMetadata {
                                size,
                                modified: meta.modified().unwrap_or(std::time::SystemTime::now()),
                                mime_type,
                                image_dimensions,
                            })
                        } else {
                            Err(anyhow::anyhow!("Could not get metadata"))
                        };
                        res
                    })
                    .await;

                let _ = cx.update(|cx| {
                    if let Ok(metadata) = meta_result {
                        let _ = this.update(cx, |ws, cx| {
                            ws.details_metadata = Some(metadata);
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    pub fn show_toast(&mut self, message: String, kind: ToastKind, cx: &mut Context<Self>) {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        let toast = Toast {
            message: message.clone(),
            kind,
            id,
        };
        self.toasts.push(toast);
        cx.notify();

        // Auto-dismiss after 3 seconds
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                cx.background_executor().timer(Duration::from_secs(3)).await;
                let _ = cx.update(|cx| {
                    let _ = this.update(cx, |ws, cx| {
                        ws.toasts.retain(|t| t.id != id);
                        cx.notify();
                    });
                });
            }
        })
        .detach();
    }

    pub fn navigate(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.current_path != path {
            self.history.push(self.current_path.clone());
            self.history_index = self.history.len() - 1;
            self.current_path = path.clone();
        }

        self.is_loading = true;
        self.selection.clear();
        self.last_selected = None;

        cx.emit(WorkspaceEvent::PathChanged(path.clone()));
        cx.notify();

        let fs = LocalFs;
        let path_clone = path.clone();
        let executor = cx.background_executor().clone();
        let show_hidden = cx.config().ui.show_hidden;

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                match fs.list_directory(executor, path_clone, show_hidden).await {
                    Ok(entries) => {
                        let _ = cx.update(|cx| {
                            let _ = this.update(cx, |ws, cx| {
                                ws.items = entries;
                                ws.is_loading = false;
                                ws.compute_grouped_files(cx);
                                cx.notify();
                            });
                        });
                    }
                    Err(e) => {
                        let _ = cx.update(|cx| {
                            let _ = this.update(cx, |ws, cx| {
                                ws.show_toast(
                                    format!("Failed to load directory: {}", e),
                                    ToastKind::Error,
                                    cx,
                                );
                                ws.is_loading = false;
                            });
                        });
                    }
                }
            }
        })
        .detach();
    }

    fn compute_grouped_files(&mut self, cx: &mut Context<Self>) {
        if !self.group_by_type {
            self.grouped_files.clear();
            return;
        }

        let config = cx.config();
        let mut groups: std::collections::HashMap<String, Vec<FileEntry>> =
            std::collections::HashMap::new();
        let mut folders: Vec<FileEntry> = Vec::new();
        let mut other_files: Vec<FileEntry> = Vec::new();

        for entry in &self.items {
            if entry.is_dir {
                folders.push(entry.clone());
            } else {
                if let Some(category) = config.get_file_category(&entry.path) {
                    groups
                        .entry(category)
                        .or_insert_with(Vec::new)
                        .push(entry.clone());
                } else {
                    other_files.push(entry.clone());
                }
            }
        }

        if !folders.is_empty() {
            groups.insert("Folders".to_string(), folders);
        }

        if !other_files.is_empty() {
            groups.insert("Other".to_string(), other_files);
        }

        self.grouped_files = groups;
    }

    pub fn toggle_grouping(&mut self, cx: &mut Context<Self>) {
        self.group_by_type = !self.group_by_type;
        cx.update_global::<crate::app_state::config::ConfigManager, _>(|mgr, _cx| {
            mgr.config.group_files_by_type = self.group_by_type;
            let _ = mgr.config.save();
        });
        self.compute_grouped_files(cx);
        cx.notify();
    }

    pub fn toggle_search_recursive(&mut self, cx: &mut Context<Self>) {
        self.search_options.recursive = !self.search_options.recursive;
        if !self.filter_query.is_empty() {
            let query = self.filter_query.clone();
            self.perform_search(query, cx);
        }
        cx.notify();
    }

    pub fn toggle_search_content(&mut self, cx: &mut Context<Self>) {
        self.search_options.content_search = !self.search_options.content_search;
        if !self.filter_query.is_empty() {
            let query = self.filter_query.clone();
            self.perform_search(query, cx);
        }
        cx.notify();
    }

    pub fn open(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if path.is_dir() {
            if self.history_index < self.history.len() - 1 {
                self.history.truncate(self.history_index + 1);
            }
            if self.history.last() != Some(&path) {
                self.history.push(path.clone());
                self.history_index = self.history.len() - 1;
            }
            self.navigate(path, cx);
            return;
        }

        let config = cx.config();

        if config.use_dms {
            match std::process::Command::new("dms")
                .arg("open")
                .arg(&path)
                .spawn()
            {
                Ok(_) => {}
                Err(_e) => {
                    if let Err(fallback_err) = open::that(&path) {
                        self.show_toast(
                            format!("Failed to open file: DMS not available and system open failed ({})", fallback_err),
                            ToastKind::Error,
                            cx,
                        );
                    } else {
                        eprintln!("[DEBUG] Fallback to system open succeeded");
                    }
                }
            }
        } else {
            if let Err(e) = open::that(&path) {
                self.show_toast(format!("Failed to open file: {}", e), ToastKind::Error, cx);
            }
        }
    }

    pub fn go_back(&mut self, cx: &mut Context<Self>) {
        if self.history_index > 0 {
            self.history_index -= 1;
            let path = self.history[self.history_index].clone();
            self.navigate(path, cx);
        }
    }

    pub fn go_forward(&mut self, cx: &mut Context<Self>) {
        if self.history_index < self.history.len() - 1 {
            self.history_index += 1;
            let path = self.history[self.history_index].clone();
            self.navigate(path, cx);
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.history_index > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.history_index < self.history.len() - 1
    }

    pub fn set_filter_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.filter_query = query;
        cx.notify();
    }

    pub fn toggle_selection(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.selection.contains(&path) {
            self.selection.remove(&path);
            if self.last_selected == Some(path.clone()) {
                self.last_selected = None;
            }
        } else {
            self.selection.insert(path.clone());
            self.last_selected = Some(path);
        }
        cx.notify();
    }

    pub fn set_selection(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.selection.clear();
        self.selection.insert(path.clone());
        self.last_selected = Some(path);
        cx.notify();
    }

    pub fn select_range(&mut self, target: PathBuf, cx: &mut Context<Self>) {
        if let Some(start) = &self.last_selected {
            let start_idx = self.items.iter().position(|i| i.path == *start);
            let end_idx = self.items.iter().position(|i| i.path == target);

            if let (Some(s), Some(e)) = (start_idx, end_idx) {
                let (min, max) = if s < e { (s, e) } else { (e, s) };

                self.selection.clear();
                for i in min..=max {
                    if let Some(item) = self.items.get(i) {
                        self.selection.insert(item.path.clone());
                    }
                }
            }
        } else {
            self.set_selection(target, cx);
        }
        cx.notify();
    }

    pub fn delete_selection(&mut self, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            return;
        }

        let paths: Vec<PathBuf> = self.selection.drain().collect();
        let fs = LocalFs;
        let executor = cx.background_executor().clone();

        self.is_loading = true;
        cx.notify();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                for path in paths {
                    if let Err(e) = fs.delete(executor.clone(), path).await {
                        let _ = cx.update(|cx| {
                            let _ = this.update(cx, |ws, cx| {
                                ws.show_toast(
                                    format!("Failed to delete: {}", e),
                                    ToastKind::Error,
                                    cx,
                                );
                            });
                        });
                    }
                }

                let _ = cx.update(|cx| {
                    let _ = this.update(cx, |ws, cx| {
                        ws.show_toast("Deleted items".to_string(), ToastKind::Info, cx);
                        ws.is_loading = false;
                        ws.reload(cx);
                    });
                });
            }
        })
        .detach();
    }

    pub fn create_folder(&mut self, name: String, cx: &mut Context<Self>) {
        let mut path = self.current_path.clone();
        path.push(name);

        let fs = LocalFs;
        let executor = cx.background_executor().clone();

        self.is_loading = true;
        cx.notify();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                if let Err(e) = fs.create_dir(executor.clone(), path).await {
                    let _ = cx.update(|cx| {
                        let _ = this.update(cx, |ws, cx| {
                            ws.show_toast(
                                format!("Failed to create folder: {}", e),
                                ToastKind::Error,
                                cx,
                            );
                            ws.is_loading = false;
                        });
                    });
                    return;
                }

                let _ = cx.update(|cx| {
                    let _ = this.update(cx, |ws, cx| {
                        ws.show_toast("Folder created".to_string(), ToastKind::Info, cx);
                        ws.is_loading = false;
                        ws.reload(cx);
                    });
                });
            }
        })
        .detach();
    }

    pub fn open_context_menu(
        &mut self,
        position: Point<Pixels>,
        path: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        self.active_overlay = Some(ActiveOverlay::ContextMenu);
        self.context_menu_location = position;
        self.context_menu_path = path;
        cx.notify();
    }

    pub fn dismiss_overlay(&mut self, cx: &mut Context<Self>) {
        self.active_overlay = None;
        self.context_menu_path = None;
        cx.notify();
    }

    pub fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.active_overlay = Some(ActiveOverlay::Settings);
        cx.notify();
    }

    pub fn open_with(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let config = cx.config();

        if config.use_dms {
            match std::process::Command::new("dms")
                .arg("open")
                .arg(&path)
                .spawn()
            {
                Ok(_) => {}
                Err(_e) => {
                    if let Err(fallback_err) = open::that(&path) {
                        self.show_toast(
                            format!("Failed to open file: DMS not available and system open failed ({})", fallback_err),
                            ToastKind::Error,
                            cx,
                        );
                    }
                }
            }
            return;
        }

        let apps = self.app_cache.read(cx).apps.clone();

        let workspace_entity = cx.entity().clone();
        let dialog = cx.new(|cx| OpenWithDialog::new(workspace_entity, path.clone(), apps, cx));
        cx.subscribe(&dialog, Self::handle_open_with_event).detach();

        self.open_with_dialog = Some(dialog);
        self.active_overlay = Some(ActiveOverlay::OpenWith(path));
        cx.notify();
    }

    pub fn handle_portal_request(
        &mut self,
        req: crate::fs_ops::portal::PortalRequest,
        cx: &mut Context<Self>,
    ) {
        println!("Workspace received portal request: {:?}", req.mode);
        self.pending_portal_response = Some(req.response_channel);

        let workspace_entity = cx.entity().clone();
        let new_picker = cx.new(|cx| UniversalPickerModal::new(workspace_entity, req.mode, cx));

        self.folder_picker = Some(new_picker.clone());

        cx.subscribe(
            &new_picker,
            |ws: &mut Workspace,
             _picker: Entity<UniversalPickerModal>,
             event: &FilePickerEvent,
             cx: &mut Context<Workspace>| {
                ws.handle_folder_picker_event(_picker, event, cx);
            },
        )
        .detach();

        self.active_overlay = Some(ActiveOverlay::FolderPicker);
        cx.notify();
    }

    pub fn open_folder_picker(&mut self, action: PickerAction, cx: &mut Context<Self>) {
        let workspace_entity = cx.entity().clone();
        let mode = crate::ui_components::universal_picker_modal::PickerMode::OpenFolder;

        // Dispose existing if any?
        if let Some(picker) = self.folder_picker.take() {
            // picker.update(cx, |p, cx| p.cancel(cx)); // Optional cleanup
        }

        let new_picker = cx.new(|cx| UniversalPickerModal::new(workspace_entity, mode, cx));
        self.folder_picker = Some(new_picker.clone());
        self.picker_action = Some(action);

        cx.subscribe(
            &new_picker,
            |ws: &mut Workspace,
             _picker: Entity<UniversalPickerModal>,
             event: &FilePickerEvent,
             cx: &mut Context<Workspace>| {
                ws.handle_folder_picker_event(_picker, event, cx);
            },
        )
        .detach();

        self.active_overlay = Some(ActiveOverlay::FolderPicker);
        cx.notify();
    }

    fn handle_folder_picker_event(
        &mut self,
        _picker: Entity<UniversalPickerModal>,
        event: &FilePickerEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            FilePickerEvent::PathsSelected(paths) => {
                // Remove picker from state
                self.folder_picker = None;

                if let Some(tx) = self.pending_portal_response.take() {
                    let _ = tx.send(crate::fs_ops::portal::PortalResponse::Selected(
                        paths.clone(),
                    ));
                } else {
                    if let Some(path) = paths.first() {
                        match self.picker_action.take().unwrap_or(PickerAction::Navigate) {
                            PickerAction::Navigate => self.navigate(path.clone(), cx),
                            PickerAction::MoveSelection => self.move_selection_to(path.clone(), cx),
                            PickerAction::CopySelection => {
                                /* TODO */
                                self.navigate(path.clone(), cx)
                            }
                        }
                    }
                }
                self.dismiss_overlay(cx);
            }
            FilePickerEvent::Cancelled => {
                if let Some(tx) = self.pending_portal_response.take() {
                    let _ = tx.send(crate::fs_ops::portal::PortalResponse::Cancelled);
                }
                self.dismiss_overlay(cx);
            }
        }
    }

    fn handle_open_with_event(
        &mut self,
        _dialog: Entity<OpenWithDialog>,
        event: &OpenWithEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            OpenWithEvent::Open(exec) => {
                if let Some(ActiveOverlay::OpenWith(path)) = &self.active_overlay {
                    let path_str = path.to_string_lossy().to_string();
                    let cmd_str = if exec.contains("%f")
                        | exec.contains("%F")
                        | exec.contains("%u")
                        | exec.contains("%U")
                    {
                        exec.replace("%f", &format!("\"{}\"", path_str))
                            .replace("%F", &format!("\"{}\"", path_str))
                            .replace("%u", &format!("\"{}\"", path_str))
                            .replace("%U", &format!("\"{}\"", path_str))
                    } else {
                        format!("{} \"{}\"", exec, path_str)
                    };

                    std::process::Command::new("sh")
                        .arg("-c")
                        .arg(cmd_str)
                        .spawn()
                        .ok();

                    self.show_toast("Launching application...".to_string(), ToastKind::Info, cx);
                }
                self.dismiss_overlay(cx);
            }
            OpenWithEvent::Close => {
                self.dismiss_overlay(cx);
            }
        }
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let path = self.current_path.clone();
        self.navigate(path, cx);
        self.details_metadata = None;
    }

    pub fn copy_text_to_clipboard(&mut self, text: String, _cx: &mut Context<Self>) {
        std::thread::spawn(move || {
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let _ = cb.set_text(text);
            }
        });
    }

    pub fn delete_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let fs = LocalFs;
        let executor = cx.background_executor().clone();

        self.is_loading = true;
        cx.notify();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                if let Err(e) = fs.delete(executor.clone(), path).await {
                    let _ = cx.update(|cx| {
                        let _ = this.update(cx, |ws, cx| {
                            ws.show_toast(format!("Failed to delete: {}", e), ToastKind::Error, cx);
                            ws.is_loading = false;
                        });
                    });
                    return;
                }

                let _ = cx.update(|cx| {
                    let _ = this.update(cx, |ws, cx| {
                        ws.show_toast("Item deleted".to_string(), ToastKind::Info, cx);
                        ws.is_loading = false;
                        ws.reload(cx);
                    });
                });
            }
        })
        .detach();
    }

    pub fn delete_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let fs = LocalFs;
        let executor = cx.background_executor().clone();

        self.is_loading = true;
        cx.notify();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                for path in paths {
                    if let Err(e) = fs.delete(executor.clone(), path).await {
                        let _ = cx.update(|cx| {
                            let _ = this.update(cx, |ws, cx| {
                                ws.show_toast(
                                    format!("Failed to delete: {}", e),
                                    ToastKind::Error,
                                    cx,
                                );
                            });
                        });
                    }
                }

                let _ = cx.update(|cx| {
                    let _ = this.update(cx, |ws, cx| {
                        ws.show_toast("Deleted items".to_string(), ToastKind::Info, cx);
                        ws.is_loading = false;
                        ws.reload(cx);
                    });
                });
            }
        })
        .detach();
    }

    pub fn perform_search(&mut self, query: String, cx: &mut Context<Self>) {
        if query.is_empty() {
            return;
        }

        self.is_loading = true;
        self.search_results = None;
        cx.notify();

        let fs = LocalFs;
        let path = self.current_path.clone();
        let executor = cx.background_executor().clone();
        let options = self.search_options.clone();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let result = fs.search(executor, path, query, options).await;

                let _ = cx.update(|cx| match result {
                    Ok(entries) => {
                        let _ = this.update(cx, |ws, cx| {
                            ws.is_loading = false;
                            ws.search_results = Some(entries);
                            cx.notify();
                        });
                    }
                    Err(e) => {
                        let _ = this.update(cx, |ws, cx| {
                            ws.is_loading = false;
                            ws.show_toast(format!("Search failed: {}", e), ToastKind::Error, cx);
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    pub fn clear_search(&mut self, cx: &mut Context<Self>) {
        self.filter_query.clear();
        self.search_results = None;
        self.is_searching = false;
        cx.notify();
    }

    pub fn copy_selection(&mut self, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            return;
        }
        self.clipboard_state = Some((ClipboardOp::Copy, self.selection.iter().cloned().collect()));
        self.show_toast(
            format!("Copied {} items", self.selection.len()),
            ToastKind::Info,
            cx,
        );
    }

    pub fn cut_selection(&mut self, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            return;
        }
        self.clipboard_state = Some((ClipboardOp::Cut, self.selection.iter().cloned().collect()));
        self.show_toast(
            format!("Cut {} items", self.selection.len()),
            ToastKind::Info,
            cx,
        );
    }

    pub fn paste_clipboard(&mut self, cx: &mut Context<Self>) {
        let Some((op, sources)) = self.clipboard_state.clone() else {
            return;
        };

        if sources.is_empty() {
            return;
        }

        let target_dir = self.current_path.clone();
        let fs = LocalFs;
        let executor = cx.background_executor().clone();

        self.is_loading = true;
        cx.notify();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let mut success_count = 0;
                let mut error_count = 0;

                for source in sources {
                    if let Some(file_name) = source.file_name() {
                        let dest = target_dir.join(file_name);

                        // Simple conflict resolution: append " copy" if exists
                        // Note: This is a basic implementation. Ideally, we\"d check existence and generate a unique name.
                        let dest = if dest.exists() {
                            let mut new_name = file_name.to_string_lossy().to_string();
                            new_name.push_str(" copy");
                            target_dir.join(new_name)
                        } else {
                            dest
                        };

                        let result = match op {
                            ClipboardOp::Copy => fs.copy(executor.clone(), source.clone(), dest).await,
                            ClipboardOp::Cut => {
                                // Try rename first
                                if let Ok(_) = std::fs::rename(&source, &dest) {
                                    Ok(())
                                } else {
                                    // Fallback to copy then delete
                                    match fs.copy(executor.clone(), source.clone(), dest.clone()).await {
                                        Ok(_) => fs.delete(executor.clone(), source.clone()).await,
                                        Err(e) => Err(e),
                                    }
                                }
                            }
                        };

                        match result {
                            Ok(_) => success_count += 1,
                            Err(_) => error_count += 1,
                        }
                    }
                }

                let _ = cx.update(|cx| {
                    let _ = this.update(cx, |ws, cx| {
                        ws.is_loading = false;
                        let action = match op {
                            ClipboardOp::Copy => "Copied",
                            ClipboardOp::Cut => "Moved",
                        };
                        if error_count > 0 {
                            ws.show_toast(
                                format!("{} {} items. {} failed.", action, success_count, error_count),
                                ToastKind::Error,
                                cx,
                            );
                        } else {
                            ws.show_toast(
                                format!("{} {} items.", action, success_count),
                                ToastKind::Success,
                                cx,
                            );
                        }

                        if op == ClipboardOp::Cut && error_count == 0 {
                             ws.clipboard_state = None;
                        }

                        ws.reload(cx);
                    });
                });
            }
        })
        .detach();
    }

    pub fn open_in_terminal(&mut self, path: PathBuf, _cx: &mut Context<Self>) {
        let path_str = path.to_string_lossy();
        // Spawn terminal at path
        // Using standard process spawn for now, could be enhanced
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-terminal-exec")
                .arg(&*path_str)
                .spawn()
                .or_else(|_| {
                    std::process::Command::new("kitty")
                        .arg("--directory")
                        .arg(&*path_str)
                        .spawn()
                })
                .or_else(|_| {
                    std::process::Command::new("gnome-terminal")
                        .arg("--working-directory")
                        .arg(&*path_str)
                        .spawn()
                });
        }
    }

    pub fn move_selection_to(&mut self, target_dir: PathBuf, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            return;
        }

        let selection = self.selection.clone();

        // Clear selection to avoid confusion
        self.selection.clear();
        self.last_selected = None;
        cx.notify();

        cx.spawn(
            move |this: WeakEntity<Workspace>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    let mut success_count = 0;
                    let mut error_count = 0;

                    for source in selection {
                        if let Some(file_name) = source.file_name() {
                            let dest = target_dir.join(file_name);

                            if dest.exists() {
                                // For now skip if exists
                                // could implement "rename if exists" later
                                error_count += 1;
                                continue;
                            }

                            // Try rename first (atomic move)
                            let result = std::fs::rename(&source, &dest);

                            match result {
                                Ok(_) => success_count += 1,
                                Err(_e) => {
                                    // Check for cross-device link error (EXDEV / OS error 18)
                                    // or any error
                                    // Fallback using fs_extra for directories or copy+delete
                                    let move_result = if source.is_dir() {
                                        let mut options = fs_extra::dir::CopyOptions::new();
                                        options.copy_inside = true;
                                        match fs_extra::dir::move_dir(&source, &dest, &options) {
                                            Ok(_) => Ok(()),
                                            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())),
                                        }
                                    } else {
                                        // File fallback
                                        match std::fs::copy(&source, &dest) {
                                            Ok(_) => {
                                                let _ = std::fs::remove_file(&source);
                                                Ok(())
                                            }
                                            Err(e) => Err(e),
                                        }
                                    };

                                    match move_result {
                                        Ok(_) => success_count += 1,
                                        Err(_) => error_count += 1,
                                    }
                                }
                            }
                        }
                    }

                    // Notify user of results via toast
                    let _ = this.update(&mut cx, |ws, cx| {
                        if error_count > 0 {
                            ws.show_toast(
                                format!("Moved {} files. {} failed.", success_count, error_count),
                                ToastKind::Error,
                                cx,
                            );
                        } else {
                            ws.show_toast(
                                format!("Moved {} files.", success_count),
                                ToastKind::Success,
                                cx,
                            );
                        }
                        ws.reload(cx);
                    });
                }
            },
        )
        .detach();
    }
}
