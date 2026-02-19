use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct NavigationStack {
    pub history: Vec<PathBuf>,
    pub future: Vec<PathBuf>,
    pub current: PathBuf,
}

impl NavigationStack {
    pub fn new(start_path: PathBuf) -> Self {
        Self {
            history: Vec::new(),
            future: Vec::new(),
            current: start_path,
        }
    }

    pub fn push(&mut self, path: PathBuf) {
        if path == self.current {
            return;
        }
        self.history.push(self.current.clone());
        self.current = path;
        self.future.clear();
    }

    pub fn go_back(&mut self) -> Option<&PathBuf> {
        if let Some(prev) = self.history.pop() {
            self.future.push(self.current.clone());
            self.current = prev;
            Some(&self.current)
        } else {
            None
        }
    }

    pub fn go_forward(&mut self) -> Option<&PathBuf> {
        if let Some(next) = self.future.pop() {
            self.history.push(self.current.clone());
            self.current = next;
            Some(&self.current)
        } else {
            None
        }
    }

    pub fn go_up(&mut self) -> Option<&PathBuf> {
        if let Some(parent) = self.current.parent() {
            let parent = parent.to_path_buf();
            self.push(parent);
            Some(&self.current)
        } else {
            None
        }
    }
    
    pub fn current(&self) -> &Path {
        &self.current
    }
}
