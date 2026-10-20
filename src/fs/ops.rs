//! Long-running file operations (copy, move, trash, delete) executed on a
//! background thread with progress reporting and cancellation.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobKind {
    Copy,
    Move,
    Trash,
    Delete,
}

impl JobKind {
    pub fn verb(self) -> &'static str {
        match self {
            JobKind::Copy => "Copying",
            JobKind::Move => "Moving",
            JobKind::Trash => "Moving to trash",
            JobKind::Delete => "Deleting",
        }
    }
    pub fn past(self) -> &'static str {
        match self {
            JobKind::Copy => "Copied",
            JobKind::Move => "Moved",
            JobKind::Trash => "Moved to trash",
            JobKind::Delete => "Deleted",
        }
    }
}

#[derive(Default)]
pub struct Progress {
    pub total_bytes: AtomicU64,
    pub done_bytes: AtomicU64,
    pub total_items: AtomicU64,
    pub done_items: AtomicU64,
    pub current: Mutex<String>,
}

impl Progress {
    pub fn fraction(&self) -> Option<f32> {
        let total = self.total_bytes.load(Ordering::Relaxed);
        if total > 0 {
            return Some((self.done_bytes.load(Ordering::Relaxed) as f64 / total as f64) as f32);
        }
        let items = self.total_items.load(Ordering::Relaxed);
        (items > 0).then(|| self.done_items.load(Ordering::Relaxed) as f32 / items as f32)
    }
}

#[derive(Debug, Default)]
pub struct JobOutcome {
    pub succeeded: usize,
    pub errors: Vec<String>,
    pub cancelled: bool,
    /// Final paths of copied/moved items (used to select them afterwards),
    /// or the original paths of trashed items (used for undo).
    pub created: Vec<PathBuf>,
}

pub struct Job {
    pub id: u64,
    pub kind: JobKind,
    pub label: String,
    pub progress: Arc<Progress>,
    pub cancel: Arc<AtomicBool>,
    pub done: flume::Receiver<JobOutcome>,
    /// Directories whose listing changes when this job finishes.
    pub affected_dirs: Vec<PathBuf>,
}

/// Something that can be poked when progress changes (the egui context).
pub type Notify = Arc<dyn Fn() + Send + Sync>;

static NEXT_JOB_ID: AtomicU64 = AtomicU64::new(1);

fn spawn_job(
    kind: JobKind,
    label: String,
    affected_dirs: Vec<PathBuf>,
    notify: Notify,
    work: impl FnOnce(&Progress, &AtomicBool) -> JobOutcome + Send + 'static,
) -> Job {
    let progress = Arc::new(Progress::default());
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = flume::bounded(1);
    {
        let progress = progress.clone();
        let cancel = cancel.clone();
        std::thread::Builder::new()
            .name(format!("m3fm-{}", kind.verb().to_lowercase()))
            .spawn(move || {
                let outcome = work(&progress, &cancel);
                let _ = tx.send(outcome);
                notify();
            })
            .expect("spawn job thread");
    }
    Job {
        id: NEXT_JOB_ID.fetch_add(1, Ordering::Relaxed),
        kind,
        label,
        progress,
        cancel,
        done: rx,
        affected_dirs,
    }
}

fn describe(paths: &[PathBuf]) -> String {
    match paths {
        [one] => format!(
            "“{}”",
            one.file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default()
        ),
        many => format!("{} items", many.len()),
    }
}

/// `name (copy).ext`, `name (copy 2).ext`, … — the first free name in `dir`.
pub fn unique_destination(dir: &Path, file_name: &str) -> PathBuf {
    let candidate = dir.join(file_name);
    if !candidate.exists() && std::fs::symlink_metadata(&candidate).is_err() {
        return candidate;
    }
    let path = Path::new(file_name);
    // Treat dotfiles like `.bashrc` as having no extension.
    let (stem, ext) = match (path.file_stem(), path.extension()) {
        (Some(stem), Some(ext)) if !file_name.starts_with('.') || file_name[1..].contains('.') => (
            stem.to_string_lossy().into_owned(),
            format!(".{}", ext.to_string_lossy()),
        ),
        _ => (file_name.to_string(), String::new()),
    };
    for n in 1.. {
        let suffix = if n == 1 {
            " (copy)".to_string()
        } else {
            format!(" (copy {n})")
        };
        let candidate = dir.join(format!("{stem}{suffix}{ext}"));
        if std::fs::symlink_metadata(&candidate).is_err() {
            return candidate;
        }
    }
    unreachable!()
}

/// Total bytes of regular files below `paths` (for progress).
fn measure(paths: &[PathBuf], cancel: &AtomicBool) -> u64 {
    let mut total = 0;
    for p in paths {
        match std::fs::symlink_metadata(p) {
            Ok(m) if m.is_dir() => total += super::scanner::dir_stats(p, cancel).bytes,
            Ok(m) if m.is_file() => total += m.len(),
            _ => {}
        }
    }
    total
}

fn copy_file(
    src: &Path,
    dst: &Path,
    progress: &Progress,
    cancel: &AtomicBool,
) -> std::io::Result<()> {
    let mut input = std::fs::File::open(src)?;
    let mut output = std::fs::File::create(dst)?;
    let mut buf = vec![0u8; 1 << 20];
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(output);
            let _ = std::fs::remove_file(dst);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "cancelled",
            ));
        }
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        output.write_all(&buf[..n])?;
        progress.done_bytes.fetch_add(n as u64, Ordering::Relaxed);
    }
    if let Ok(meta) = input.metadata() {
        let _ = std::fs::set_permissions(dst, meta.permissions());
    }
    Ok(())
}

fn copy_recursive(
    src: &Path,
    dst: &Path,
    progress: &Progress,
    cancel: &AtomicBool,
) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(src)?;
    let ft = meta.file_type();
    if ft.is_symlink() {
        let target = std::fs::read_link(src)?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, dst)?;
        #[cfg(not(unix))]
        let _ = target;
    } else if ft.is_dir() {
        std::fs::create_dir(dst)?;
        for entry in std::fs::read_dir(src)? {
            if cancel.load(Ordering::Relaxed) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "cancelled",
                ));
            }
            let entry = entry?;
            copy_recursive(
                &entry.path(),
                &dst.join(entry.file_name()),
                progress,
                cancel,
            )?;
        }
        let _ = std::fs::set_permissions(dst, meta.permissions());
    } else {
        *progress.current.lock().unwrap() = src
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        copy_file(src, dst, progress, cancel)?;
    }
    Ok(())
}

fn remove_any(path: &Path) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Copy or move `sources` into `dest_dir`. Name clashes get a
/// `(copy)` suffix rather than overwriting anything.
pub fn transfer(sources: Vec<PathBuf>, dest_dir: PathBuf, kind: JobKind, notify: Notify) -> Job {
    debug_assert!(matches!(kind, JobKind::Copy | JobKind::Move));
    let label = format!("{} {}", kind.verb(), describe(&sources));
    let mut affected: Vec<PathBuf> = vec![dest_dir.clone()];
    if kind == JobKind::Move {
        affected.extend(
            sources
                .iter()
                .filter_map(|s| s.parent().map(Path::to_path_buf)),
        );
    }
    let tick = notify.clone();
    spawn_job(kind, label, affected, notify, move |progress, cancel| {
        let mut outcome = JobOutcome::default();
        progress
            .total_items
            .store(sources.len() as u64, Ordering::Relaxed);
        if kind == JobKind::Copy {
            progress
                .total_bytes
                .store(measure(&sources, cancel), Ordering::Relaxed);
        }
        for src in &sources {
            if cancel.load(Ordering::Relaxed) {
                outcome.cancelled = true;
                break;
            }
            let name = file_name(src);
            *progress.current.lock().unwrap() = name.clone();
            tick();

            if kind == JobKind::Move && src.parent() == Some(dest_dir.as_path()) {
                // Moving into the folder it's already in: nothing to do.
                outcome.succeeded += 1;
                continue;
            }
            if src.is_dir() && dest_dir.starts_with(src) {
                outcome
                    .errors
                    .push(format!("Can't put “{name}” inside itself"));
                continue;
            }
            let dst = unique_destination(&dest_dir, &name);
            let result = match kind {
                JobKind::Move => std::fs::rename(src, &dst).or_else(|_| {
                    // Cross-device: copy then remove the original.
                    copy_recursive(src, &dst, progress, cancel).and_then(|_| remove_any(src))
                }),
                _ => copy_recursive(src, &dst, progress, cancel),
            };
            match result {
                Ok(()) => {
                    outcome.succeeded += 1;
                    outcome.created.push(dst);
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                    outcome.cancelled = true;
                    let _ = remove_any(&dst);
                    break;
                }
                Err(e) => outcome.errors.push(format!("{name}: {e}")),
            }
            progress.done_items.fetch_add(1, Ordering::Relaxed);
            tick();
        }
        outcome
    })
}

/// Move to the freedesktop trash, or delete permanently.
pub fn remove(paths: Vec<PathBuf>, permanently: bool, notify: Notify) -> Job {
    let kind = if permanently {
        JobKind::Delete
    } else {
        JobKind::Trash
    };
    let label = format!("{} {}", kind.verb(), describe(&paths));
    let affected = paths
        .iter()
        .filter_map(|p| p.parent().map(Path::to_path_buf))
        .collect();
    let tick = notify.clone();
    spawn_job(kind, label, affected, notify, move |progress, cancel| {
        let mut outcome = JobOutcome::default();
        progress
            .total_items
            .store(paths.len() as u64, Ordering::Relaxed);
        for p in &paths {
            if cancel.load(Ordering::Relaxed) {
                outcome.cancelled = true;
                break;
            }
            let name = file_name(p);
            *progress.current.lock().unwrap() = name.clone();
            let result = if permanently {
                remove_any(p).map_err(|e| e.to_string())
            } else {
                trash::delete(p).map_err(|e| e.to_string())
            };
            match result {
                Ok(()) => {
                    outcome.succeeded += 1;
                    outcome.created.push(p.clone());
                }
                Err(e) => outcome.errors.push(format!("{name}: {e}")),
            }
            progress.done_items.fetch_add(1, Ordering::Relaxed);
            tick();
        }
        outcome
    })
}

/// Validate a user-typed file name.
pub fn validate_name(name: &str) -> Result<(), &'static str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        Err("Name can't be empty")
    } else if trimmed == "." || trimmed == ".." {
        Err("“.” and “..” are reserved")
    } else if trimmed.contains('/') || trimmed.contains('\0') {
        Err("Name can't contain “/”")
    } else if trimmed.len() > 255 {
        Err("Name is too long")
    } else {
        Ok(())
    }
}

pub fn rename(path: &Path, new_name: &str) -> anyhow::Result<PathBuf> {
    validate_name(new_name).map_err(anyhow::Error::msg)?;
    let dest = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("can't rename the root"))?
        .join(new_name.trim());
    if dest == path {
        return Ok(dest);
    }
    // Allow case-only renames on case-insensitive filesystems.
    let same_file_case_change =
        dest.to_string_lossy().to_lowercase() == path.to_string_lossy().to_lowercase();
    if std::fs::symlink_metadata(&dest).is_ok() && !same_file_case_change {
        anyhow::bail!("“{}” already exists", new_name.trim());
    }
    std::fs::rename(path, &dest)?;
    Ok(dest)
}

pub fn create_folder(dir: &Path, name: &str) -> anyhow::Result<PathBuf> {
    validate_name(name).map_err(anyhow::Error::msg)?;
    let path = dir.join(name.trim());
    if std::fs::symlink_metadata(&path).is_ok() {
        anyhow::bail!("“{}” already exists", name.trim());
    }
    std::fs::create_dir(&path)?;
    Ok(path)
}

pub fn create_file(dir: &Path, name: &str) -> anyhow::Result<PathBuf> {
    validate_name(name).map_err(anyhow::Error::msg)?;
    let path = dir.join(name.trim());
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => {
                anyhow::anyhow!("“{}” already exists", name.trim())
            }
            _ => e.into(),
        })?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn noop() -> Notify {
        Arc::new(|| {})
    }

    #[test]
    fn unique_names() {
        let tmp = TempDir::new().unwrap();
        let d = tmp.path();
        assert_eq!(unique_destination(d, "a.txt"), d.join("a.txt"));
        std::fs::write(d.join("a.txt"), "").unwrap();
        assert_eq!(unique_destination(d, "a.txt"), d.join("a (copy).txt"));
        std::fs::write(d.join("a (copy).txt"), "").unwrap();
        assert_eq!(unique_destination(d, "a.txt"), d.join("a (copy 2).txt"));
        std::fs::write(d.join(".bashrc"), "").unwrap();
        assert_eq!(unique_destination(d, ".bashrc"), d.join(".bashrc (copy)"));
        std::fs::create_dir(d.join("dir")).unwrap();
        assert_eq!(unique_destination(d, "dir"), d.join("dir (copy)"));
    }

    #[test]
    fn copy_and_move_trees() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(src.join("nested")).unwrap();
        std::fs::write(src.join("nested/file.txt"), "hello").unwrap();
        let dest = tmp.path().join("dest");
        std::fs::create_dir(&dest).unwrap();

        let job = transfer(vec![src.clone()], dest.clone(), JobKind::Copy, noop());
        let out = job.done.recv().unwrap();
        assert_eq!(out.succeeded, 1, "{:?}", out.errors);
        assert_eq!(
            std::fs::read_to_string(dest.join("src/nested/file.txt")).unwrap(),
            "hello"
        );
        assert_eq!(job.progress.fraction(), Some(1.0));

        // Copy again into the same place: gets a unique name.
        let out = transfer(vec![src.clone()], dest.clone(), JobKind::Copy, noop())
            .done
            .recv()
            .unwrap();
        assert_eq!(out.created, vec![dest.join("src (copy)")]);

        let out = transfer(vec![src.clone()], dest.join("src"), JobKind::Move, noop())
            .done
            .recv()
            .unwrap();
        assert_eq!(out.created, vec![dest.join("src/src")]);
        assert!(!src.exists());
    }

    #[test]
    fn refuses_copy_into_itself() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("a");
        std::fs::create_dir_all(src.join("b")).unwrap();
        let out = transfer(vec![src.clone()], src.join("b"), JobKind::Copy, noop())
            .done
            .recv()
            .unwrap();
        assert_eq!(out.succeeded, 0);
        assert_eq!(out.errors.len(), 1);
    }

    #[test]
    fn rename_and_create() {
        let tmp = TempDir::new().unwrap();
        let d = tmp.path();
        let f = create_file(d, "x.txt").unwrap();
        assert!(create_file(d, "x.txt").is_err());
        let folder = create_folder(d, "New").unwrap();
        assert!(create_folder(d, "New").is_err());
        assert!(rename(&f, "New").is_err(), "must not clobber");
        assert!(rename(&f, "a/b").is_err());
        let renamed = rename(&f, "y.txt").unwrap();
        assert!(renamed.exists() && folder.exists());
    }

    #[test]
    fn permanent_delete() {
        let tmp = TempDir::new().unwrap();
        let d = tmp.path().join("gone");
        std::fs::create_dir_all(d.join("x")).unwrap();
        let out = remove(vec![d.clone()], true, noop()).done.recv().unwrap();
        assert_eq!(out.succeeded, 1);
        assert!(!d.exists());
    }
}
