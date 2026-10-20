//! Watches a directory for changes so open views refresh themselves.

use notify::event::{AccessKind, AccessMode, EventKind, ModifyKind};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};

pub struct DirWatcher {
    watcher: RecommendedWatcher,
    watched: Option<PathBuf>,
    pub events: flume::Receiver<()>,
}

/// Only structural changes matter for a listing. Reads (and the atime /
/// metadata updates they can cause) are ignored, otherwise generating a
/// thumbnail would trigger a reload, which would request thumbnails, …
fn is_relevant(kind: &EventKind) -> bool {
    match kind {
        EventKind::Create(_) | EventKind::Remove(_) => true,
        EventKind::Modify(ModifyKind::Metadata(_)) => false,
        EventKind::Modify(_) => true,
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        _ => false,
    }
}

impl DirWatcher {
    pub fn new(on_change: impl Fn() + Send + 'static) -> notify::Result<Self> {
        let (tx, rx) = flume::unbounded();
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                if is_relevant(&event.kind) {
                    let _ = tx.send(());
                    on_change();
                }
            }
        })?;
        Ok(Self {
            watcher,
            watched: None,
            events: rx,
        })
    }

    pub fn watch(&mut self, path: &Path) {
        if self.watched.as_deref() == Some(path) {
            return;
        }
        if let Some(old) = self.watched.take() {
            let _ = self.watcher.unwatch(&old);
        }
        match self.watcher.watch(path, RecursiveMode::NonRecursive) {
            Ok(()) => self.watched = Some(path.to_path_buf()),
            Err(e) => log::debug!("can't watch {}: {e}", path.display()),
        }
    }

    pub fn unwatch(&mut self) {
        if let Some(old) = self.watched.take() {
            let _ = self.watcher.unwatch(&old);
        }
    }

    /// Drain pending events; true if anything changed.
    pub fn changed(&self) -> bool {
        self.events.try_iter().count() > 0
    }
}
