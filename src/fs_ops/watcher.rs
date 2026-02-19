use flume::Sender;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};

pub struct FsWatcher {
    watcher: RecommendedWatcher,
    current_path: Option<PathBuf>,
}

impl FsWatcher {
    pub fn new(tx: Sender<notify::Event>) -> notify::Result<Self> {
        let watcher = notify::recommended_watcher(move |res| match res {
            Ok(event) => {
                let _ = tx.send(event);
            }
            Err(e) => eprintln!("watch error: {:?}", e),
        })?;

        Ok(Self {
            watcher,
            current_path: None,
        })
    }

    pub fn watch(&mut self, path: &Path) {
        if let Some(old_path) = &self.current_path {
            let _ = self.watcher.unwatch(old_path);
        }
        if let Err(e) = self.watcher.watch(path, RecursiveMode::NonRecursive) {
            eprintln!("Failed to watch {:?}: {:?}", path, e);
        }
        self.current_path = Some(path.to_path_buf());
    }
}
