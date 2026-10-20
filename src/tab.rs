//! State of one browser tab: where it is, what it lists, what's selected.

use crate::config::Config;
use crate::fs::entry::sort_entries;
use crate::fs::scanner::{self, SearchOptions};
use crate::fs::watcher::DirWatcher;
use crate::fs::FileEntry;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const SEARCH_DEBOUNCE: Duration = Duration::from_millis(250);
const RELOAD_DEBOUNCE: Duration = Duration::from_millis(250);
const TYPE_AHEAD_TIMEOUT: Duration = Duration::from_millis(900);

#[derive(Clone, Debug, PartialEq)]
pub enum Location {
    Dashboard,
    Folder(PathBuf),
    Settings,
}

enum SearchMsg {
    Batch(Vec<FileEntry>),
    Done { capped: bool },
}

pub struct DeepSearch {
    pub results: Vec<FileEntry>,
    pub running: bool,
    pub capped: bool,
    rx: flume::Receiver<SearchMsg>,
    cancel: Arc<AtomicBool>,
}

impl Drop for DeepSearch {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[derive(Default)]
pub struct SearchState {
    pub query: String,
    pub recursive: bool,
    pub content: bool,
    pub deep: Option<DeepSearch>,
    changed_at: Option<Instant>,
}

impl SearchState {
    pub fn is_active(&self) -> bool {
        !self.query.trim().is_empty()
    }
    pub fn wants_deep(&self) -> bool {
        self.is_active() && (self.recursive || self.content)
    }
    pub fn is_running(&self) -> bool {
        self.changed_at.is_some() || self.deep.as_ref().map(|d| d.running).unwrap_or(false)
    }
}

pub struct RenameState {
    pub path: PathBuf,
    pub text: String,
    /// Select the file stem the first frame the editor is shown.
    pub just_started: bool,
    pub error: Option<String>,
}

/// A run of `view` entries shown under one heading when grouping by type.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub name: String,
    /// Range into `view` (empty when collapsed).
    pub start: usize,
    pub len: usize,
    /// Number of entries in the group, including collapsed ones.
    pub total: usize,
    pub collapsed: bool,
}

pub struct Tab {
    pub location: Location,
    back: Vec<Location>,
    forward: Vec<Location>,

    /// Raw listing of the current folder (all entries, hidden included).
    pub entries: Vec<FileEntry>,
    /// What the view shows: filtered, sorted, or search results.
    pub view: Vec<FileEntry>,
    /// Headings when grouping by type; empty otherwise.
    pub groups: Vec<Group>,
    pub collapsed_groups: HashSet<String>,
    view_dirty: bool,

    pub loading: bool,
    pub error: Option<String>,
    load_rx: Option<flume::Receiver<(PathBuf, std::io::Result<Vec<FileEntry>>)>>,
    watcher: Option<DirWatcher>,
    reload_due: Option<Instant>,

    pub search: SearchState,
    /// Selected paths. Private so every change also refreshes
    /// `selected_idx`; read it through `selection()` / `is_selected()`.
    selection: HashSet<PathBuf>,
    /// Indices of the selection in `view`, sorted. Lets per-frame readers
    /// (status bar, details pane) avoid scanning the whole folder.
    selected_idx: Vec<usize>,
    /// Path → position in `view`.
    view_index: HashMap<PathBuf, usize>,
    pub anchor: Option<PathBuf>,
    pub cursor: Option<PathBuf>,
    /// Scroll so this entry is visible on the next frame.
    pub scroll_to: Option<PathBuf>,
    /// Select these once they show up in the listing (after paste/create).
    pub select_when_listed: Vec<PathBuf>,
    pub rename: Option<RenameState>,
    /// Path bar in text-edit mode.
    pub path_edit: Option<String>,
    pub focus_search: bool,
    type_ahead: (String, Option<Instant>),

    /// Set when a folder was entered; the app records it as recent.
    pub visited: Option<PathBuf>,
    ctx: egui::Context,
}

impl Tab {
    pub fn new(location: Location, ctx: &egui::Context) -> Self {
        let watcher = {
            let ctx = ctx.clone();
            DirWatcher::new(move || ctx.request_repaint_after(RELOAD_DEBOUNCE)).ok()
        };
        let mut tab = Self {
            location: Location::Dashboard,
            back: Vec::new(),
            forward: Vec::new(),
            entries: Vec::new(),
            view: Vec::new(),
            groups: Vec::new(),
            collapsed_groups: HashSet::new(),
            view_dirty: true,
            loading: false,
            error: None,
            load_rx: None,
            watcher,
            reload_due: None,
            search: SearchState {
                recursive: true,
                ..Default::default()
            },
            selection: HashSet::new(),
            selected_idx: Vec::new(),
            view_index: HashMap::new(),
            anchor: None,
            cursor: None,
            scroll_to: None,
            select_when_listed: Vec::new(),
            rename: None,
            path_edit: None,
            focus_search: false,
            type_ahead: (String::new(), None),
            visited: None,
            ctx: ctx.clone(),
        };
        tab.enter(location);
        tab
    }

    pub fn folder(&self) -> Option<&Path> {
        match &self.location {
            Location::Folder(p) => Some(p),
            _ => None,
        }
    }

    pub fn title(&self) -> String {
        match &self.location {
            Location::Dashboard => "Dashboard".into(),
            Location::Settings => "Settings".into(),
            Location::Folder(p) => crate::fs::places::display_name(p),
        }
    }

    // ----------------------------------------------------------------- history

    pub fn navigate(&mut self, location: Location) {
        if location == self.location {
            if self.search.is_active() {
                self.clear_search();
            }
            return;
        }
        let previous = std::mem::replace(&mut self.location, Location::Dashboard);
        self.back.push(previous);
        self.forward.clear();
        self.enter(location);
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }
    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }
    pub fn can_go_up(&self) -> bool {
        self.folder().and_then(Path::parent).is_some()
    }

    pub fn go_back(&mut self) {
        if let Some(loc) = self.back.pop() {
            let current = std::mem::replace(&mut self.location, Location::Dashboard);
            self.forward.push(current.clone());
            self.enter_and_select(loc, Some(current));
        }
    }

    pub fn go_forward(&mut self) {
        if let Some(loc) = self.forward.pop() {
            let current = std::mem::replace(&mut self.location, Location::Dashboard);
            self.back.push(current);
            self.enter(loc);
        }
    }

    pub fn go_up(&mut self) {
        let Some(current) = self.folder().map(Path::to_path_buf) else {
            return;
        };
        if let Some(parent) = current.parent() {
            self.navigate(Location::Folder(parent.to_path_buf()));
            // Keep the folder we came from highlighted.
            self.select_when_listed = vec![current];
        }
    }

    fn enter_and_select(&mut self, loc: Location, came_from: Option<Location>) {
        self.enter(loc);
        if let (Some(Location::Folder(from)), Some(here)) = (came_from, self.folder()) {
            if from.parent() == Some(here) {
                self.select_when_listed = vec![from];
            }
        }
    }

    fn enter(&mut self, location: Location) {
        self.location = location;
        self.search.query.clear();
        self.search.deep = None;
        self.search.changed_at = None;
        self.selection.clear();
        self.selected_idx.clear();
        self.anchor = None;
        self.cursor = None;
        self.rename = None;
        self.path_edit = None;
        self.select_when_listed.clear();
        self.entries.clear();
        self.view.clear();
        self.error = None;
        self.reload_due = None;
        match self.location.clone() {
            Location::Folder(path) => {
                if let Some(w) = &mut self.watcher {
                    w.watch(&path);
                }
                self.visited = Some(path);
                self.reload();
            }
            _ => {
                if let Some(w) = &mut self.watcher {
                    w.unwatch();
                }
                self.loading = false;
                self.load_rx = None;
            }
        }
    }

    // ----------------------------------------------------------------- loading

    /// Re-list the current folder in the background, keeping selection.
    pub fn reload(&mut self) {
        let Some(path) = self.folder().map(Path::to_path_buf) else {
            return;
        };
        let (tx, rx) = flume::bounded(1);
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let result = scanner::list_dir(&path);
            let _ = tx.send((path, result));
            ctx.request_repaint();
        });
        self.load_rx = Some(rx);
        self.loading = true;
    }

    /// Advance background work. Call once per frame before drawing.
    pub fn poll(&mut self, config: &Config) {
        if let Some(rx) = &self.load_rx {
            if let Ok((path, result)) = rx.try_recv() {
                self.load_rx = None;
                if Some(path.as_path()) == self.folder() {
                    self.loading = false;
                    match result {
                        Ok(entries) => {
                            self.entries = entries;
                            self.error = None;
                        }
                        Err(e) => {
                            self.entries.clear();
                            self.error = Some(match e.kind() {
                                std::io::ErrorKind::PermissionDenied => {
                                    "You don't have permission to view this folder.".into()
                                }
                                std::io::ErrorKind::NotFound => {
                                    "This folder no longer exists.".into()
                                }
                                _ => e.to_string(),
                            });
                        }
                    }
                    let present: HashSet<&PathBuf> = self.entries.iter().map(|e| &e.path).collect();
                    self.selection.retain(|p| present.contains(p));
                    self.view_dirty = true;
                }
            }
        }

        if let Some(w) = &self.watcher {
            if w.changed() {
                self.reload_due = Some(Instant::now() + RELOAD_DEBOUNCE);
            }
        }
        if let Some(due) = self.reload_due {
            if Instant::now() >= due {
                self.reload_due = None;
                self.reload();
            } else {
                self.ctx.request_repaint_after(due - Instant::now());
            }
        }

        self.poll_search(config);

        if self.view_dirty {
            self.rebuild_view(config);
        }

        if !self.select_when_listed.is_empty() {
            let wanted: Vec<PathBuf> = self
                .select_when_listed
                .iter()
                .filter(|p| self.view.iter().any(|e| &e.path == *p))
                .cloned()
                .collect();
            if !wanted.is_empty() {
                self.set_selection(wanted.iter().cloned().collect());
                self.cursor = wanted.first().cloned();
                self.anchor = self.cursor.clone();
                self.scroll_to = self.cursor.clone();
                self.select_when_listed.retain(|p| !wanted.contains(p));
            }
            if !self.loading && self.load_rx.is_none() && self.reload_due.is_none() {
                // Give up once a listing completed without them.
                self.select_when_listed.clear();
            }
        }
    }

    pub fn invalidate_view(&mut self) {
        self.view_dirty = true;
    }

    fn rebuild_view(&mut self, config: &Config) {
        self.view_dirty = false;
        let show_hidden = config.ui.show_hidden;
        let query = self.search.query.trim().to_lowercase();
        let mut view: Vec<FileEntry> = if let Some(deep) = &self.search.deep {
            deep.results.clone()
        } else if query.is_empty() {
            self.entries
                .iter()
                .filter(|e| show_hidden || !e.hidden)
                .cloned()
                .collect()
        } else if self.search.wants_deep() {
            // Until deep results arrive, show the direct matches we already
            // have (always a subset of what the deep search will find).
            self.entries
                .iter()
                .filter(|e| (show_hidden || !e.hidden) && e.name_lower.contains(&query))
                .cloned()
                .collect()
        } else {
            let matcher = SkimMatcherV2::default();
            self.entries
                .iter()
                .filter(|e| {
                    (show_hidden || !e.hidden) && matcher.fuzzy_match(&e.name, &query).is_some()
                })
                .cloned()
                .collect()
        };
        sort_entries(
            &mut view,
            config.ui.sort_by,
            config.ui.sort_ascending,
            config.ui.folders_first,
        );

        self.groups.clear();
        if config.group_files_by_type && !self.search.is_active() {
            let group_of = |e: &FileEntry| -> String {
                if e.is_dir {
                    "Folders".into()
                } else {
                    config.file_category(&e.path).unwrap_or("Other").to_string()
                }
            };
            let rank = |name: &str| match name {
                "Folders" => 0,
                "Other" => 2,
                _ => 1,
            };
            // Bucket each entry once (the sort order inside a bucket is kept).
            let mut buckets: Vec<(String, Vec<FileEntry>)> = Vec::new();
            for e in view {
                let g = group_of(&e);
                match buckets.iter_mut().find(|(name, _)| *name == g) {
                    Some((_, members)) => members.push(e),
                    None => buckets.push((g, vec![e])),
                }
            }
            buckets.sort_by(|(a, _), (b, _)| rank(a).cmp(&rank(b)).then_with(|| a.cmp(b)));
            let mut grouped = Vec::new();
            for (name, members) in buckets {
                let collapsed = self.collapsed_groups.contains(&name);
                let start = grouped.len();
                let total = members.len();
                if !collapsed {
                    grouped.extend(members);
                }
                self.groups.push(Group {
                    name,
                    start,
                    len: grouped.len() - start,
                    total,
                    collapsed,
                });
            }
            view = grouped;
        }
        self.view = view;
        self.view_index = self
            .view
            .iter()
            .enumerate()
            .map(|(i, e)| (e.path.clone(), i))
            .collect();
        self.sync_selection();
    }

    pub fn toggle_group(&mut self, name: &str) {
        if !self.collapsed_groups.remove(name) {
            self.collapsed_groups.insert(name.to_string());
        }
        self.view_dirty = true;
    }

    // ------------------------------------------------------------------ search

    pub fn set_query(&mut self, query: String) {
        if query == self.search.query {
            return;
        }
        self.search.query = query;
        self.search.deep = None;
        self.search.changed_at = self.search.wants_deep().then(Instant::now);
        self.view_dirty = true;
    }

    pub fn set_search_options(&mut self, recursive: bool, content: bool) {
        self.search.recursive = recursive;
        self.search.content = content;
        self.search.deep = None;
        self.search.changed_at = self.search.wants_deep().then(Instant::now);
        self.view_dirty = true;
    }

    pub fn clear_search(&mut self) {
        self.search.query.clear();
        self.search.deep = None;
        self.search.changed_at = None;
        self.view_dirty = true;
    }

    fn poll_search(&mut self, config: &Config) {
        if let Some(changed) = self.search.changed_at {
            if changed.elapsed() >= SEARCH_DEBOUNCE {
                self.search.changed_at = None;
                self.start_deep_search(config);
            } else {
                self.ctx
                    .request_repaint_after(SEARCH_DEBOUNCE - changed.elapsed());
            }
        }
        if let Some(deep) = &mut self.search.deep {
            let mut got = false;
            while let Ok(msg) = deep.rx.try_recv() {
                got = true;
                match msg {
                    SearchMsg::Batch(batch) => deep.results.extend(batch),
                    SearchMsg::Done { capped } => {
                        deep.running = false;
                        deep.capped = capped;
                    }
                }
            }
            if got {
                self.view_dirty = true;
            }
        }
    }

    fn start_deep_search(&mut self, config: &Config) {
        let Some(root) = self.folder().map(Path::to_path_buf) else {
            return;
        };
        if !self.search.wants_deep() {
            return;
        }
        let query = self.search.query.trim().to_string();
        let options = SearchOptions {
            recursive: self.search.recursive,
            content_search: self.search.content,
            include_hidden: config.ui.show_hidden,
        };
        let (tx, rx) = flume::unbounded();
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let cancel = cancel.clone();
            let ctx = self.ctx.clone();
            let query = query.clone();
            std::thread::spawn(move || {
                let capped = scanner::search(&root, &query, options, &cancel, |batch| {
                    let _ = tx.send(SearchMsg::Batch(batch));
                    ctx.request_repaint();
                });
                let _ = tx.send(SearchMsg::Done { capped });
                ctx.request_repaint();
            });
        }
        self.search.deep = Some(DeepSearch {
            results: Vec::new(),
            running: true,
            capped: false,
            rx,
            cancel,
        });
        self.view_dirty = true;
    }

    // --------------------------------------------------------------- selection

    pub fn index_of(&self, path: &Path) -> Option<usize> {
        self.view_index.get(path).copied()
    }

    pub fn selection(&self) -> &HashSet<PathBuf> {
        &self.selection
    }

    pub fn is_selected(&self, path: &Path) -> bool {
        self.selection.contains(path)
    }

    pub fn selection_len(&self) -> usize {
        self.selected_idx.len()
    }

    pub fn set_selection(&mut self, selection: HashSet<PathBuf>) {
        self.selection = selection;
        self.sync_selection();
    }

    /// Recompute `selected_idx` after the selection or the view changed.
    fn sync_selection(&mut self) {
        let mut idx: Vec<usize> = self
            .selection
            .iter()
            .filter_map(|p| self.view_index.get(p).copied())
            .collect();
        idx.sort_unstable();
        self.selected_idx = idx;
    }

    pub fn select_only(&mut self, path: &Path) {
        self.selection.clear();
        self.selection.insert(path.to_path_buf());
        self.selected_idx = self.index_of(path).into_iter().collect();
        self.anchor = Some(path.to_path_buf());
        self.cursor = Some(path.to_path_buf());
    }

    pub fn toggle(&mut self, path: &Path) {
        if !self.selection.remove(path) {
            self.selection.insert(path.to_path_buf());
        }
        self.sync_selection();
        self.anchor = Some(path.to_path_buf());
        self.cursor = Some(path.to_path_buf());
    }

    /// Select from the anchor to `path` (shift-click / shift-arrows).
    pub fn select_range(&mut self, path: &Path, additive: bool) {
        let to = self.index_of(path);
        let from = self.anchor.as_deref().and_then(|a| self.index_of(a)).or(to);
        if let (Some(a), Some(b)) = (from, to) {
            if !additive {
                self.selection.clear();
            }
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            for e in &self.view[lo..=hi] {
                self.selection.insert(e.path.clone());
            }
            self.sync_selection();
        }
        self.cursor = Some(path.to_path_buf());
    }

    pub fn select_all(&mut self) {
        self.selection = self.view.iter().map(|e| e.path.clone()).collect();
        self.selected_idx = (0..self.view.len()).collect();
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.selected_idx.clear();
        self.anchor = None;
    }

    /// Selected entries in view order.
    pub fn selected_entries(&self) -> impl Iterator<Item = &FileEntry> + '_ {
        self.selected_idx.iter().map(|&i| &self.view[i])
    }

    pub fn selected_paths(&self) -> Vec<PathBuf> {
        self.selected_entries().map(|e| e.path.clone()).collect()
    }

    /// Keyboard navigation: move the cursor by `delta` entries.
    pub fn move_cursor(&mut self, delta: isize, extend: bool) {
        if self.view.is_empty() {
            return;
        }
        let current = self.cursor.as_deref().and_then(|c| self.index_of(c));
        let next = match current {
            None => {
                if delta >= 0 {
                    0
                } else {
                    self.view.len() - 1
                }
            }
            Some(i) => (i as isize + delta).clamp(0, self.view.len() as isize - 1) as usize,
        };
        let path = self.view[next].path.clone();
        if extend {
            if self.anchor.is_none() {
                self.anchor = current
                    .map(|i| self.view[i].path.clone())
                    .or(Some(path.clone()));
            }
            self.select_range(&path, false);
        } else {
            self.select_only(&path);
        }
        self.scroll_to = Some(path);
    }

    /// Type-to-find: jump to the first entry starting with the typed text.
    pub fn type_ahead(&mut self, text: &str) {
        let now = Instant::now();
        let (buf, last) = &mut self.type_ahead;
        if last.map(|t| now - t > TYPE_AHEAD_TIMEOUT).unwrap_or(true) {
            buf.clear();
        }
        buf.push_str(&text.to_lowercase());
        *last = Some(now);
        let needle = buf.clone();
        if let Some(e) = self.view.iter().find(|e| e.name_lower.starts_with(&needle)) {
            let path = e.path.clone();
            self.select_only(&path);
            self.scroll_to = Some(path);
        }
    }

    // ------------------------------------------------------------------ rename

    pub fn start_rename(&mut self, path: &Path) {
        let Some(name) = self
            .view
            .iter()
            .find(|e| e.path == path)
            .map(|e| e.name.clone())
        else {
            return;
        };
        self.select_only(path);
        self.scroll_to = Some(path.to_path_buf());
        self.rename = Some(RenameState {
            path: path.to_path_buf(),
            text: name,
            just_started: true,
            error: None,
        });
    }
}
