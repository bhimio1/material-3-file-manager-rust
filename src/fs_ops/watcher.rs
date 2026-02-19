use flume::Sender;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;

pub struct FsWatcher {
    watcher: RecommendedWatcher,
}

impl FsWatcher {
    pub fn new(tx: Sender<notify::Event>) -> Self {
        let watcher = notify::recommended_watcher(move |res| match res {
            Ok(event) => {
                let _ = tx.send(event);
            }
            Err(e) => eprintln!("watch error: {:?}", e),
        })
        .unwrap();

        Self { watcher }
    }

    pub fn watch(&mut self, path: &Path) {
        let _ = self.watcher.watch(path, RecursiveMode::NonRecursive);
    }
}
