//! Directory listing, recursive/content search and folder size calculation.

use super::entry::FileEntry;
use jwalk::WalkDir;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Stop collecting search results after this many matches.
pub const MAX_SEARCH_RESULTS: usize = 5_000;
/// Files larger than this are not content-searched.
const MAX_CONTENT_SEARCH_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchOptions {
    pub recursive: bool,
    pub content_search: bool,
    pub include_hidden: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            recursive: true,
            content_search: false,
            include_hidden: false,
        }
    }
}

/// List a directory (one level). Hidden entries are included; the view
/// filters them so toggling "show hidden" never needs a rescan.
pub fn list_dir(path: &Path) -> std::io::Result<Vec<FileEntry>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let Ok(entry) = entry else { continue };
        if let Some(e) = FileEntry::from_path(entry.path()) {
            entries.push(e);
        }
    }
    Ok(entries)
}

fn content_matches(path: &Path, query_lower: &str) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if meta.len() > MAX_CONTENT_SEARCH_BYTES {
        return false;
    }
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut buf = Vec::with_capacity(meta.len() as usize);
    if file.read_to_end(&mut buf).is_err() {
        return false;
    }
    // A NUL byte in the first KiB means binary: skip it.
    if buf.iter().take(1024).any(|&b| b == 0) {
        return false;
    }
    String::from_utf8_lossy(&buf)
        .to_lowercase()
        .contains(query_lower)
}

/// Search below `root` for names (and optionally contents) containing
/// `query`, case-insensitively. Matches are delivered in batches through
/// `on_batch` so the UI can show results while the walk continues.
/// Returns `true` if the result cap was hit.
pub fn search(
    root: &Path,
    query: &str,
    options: SearchOptions,
    cancel: &AtomicBool,
    mut on_batch: impl FnMut(Vec<FileEntry>),
) -> bool {
    let query_lower = query.to_lowercase();
    let max_depth = if options.recursive { usize::MAX } else { 1 };
    let mut batch = Vec::new();
    let mut total = 0usize;
    let mut last_flush = std::time::Instant::now();

    for entry in WalkDir::new(root)
        .skip_hidden(!options.include_hidden)
        .follow_links(false)
        .sort(true)
        .min_depth(1)
        .max_depth(max_depth)
    {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let path = entry.path();
        let matched = name.contains(&query_lower)
            || (options.content_search
                && entry.file_type().is_file()
                && content_matches(&path, &query_lower));
        if !matched {
            continue;
        }
        if let Some(e) = FileEntry::from_path(path) {
            batch.push(e);
            total += 1;
        }
        if total >= MAX_SEARCH_RESULTS {
            break;
        }
        if batch.len() >= 64 || last_flush.elapsed().as_millis() > 150 {
            on_batch(std::mem::take(&mut batch));
            last_flush = std::time::Instant::now();
        }
    }
    if !batch.is_empty() {
        on_batch(batch);
    }
    total >= MAX_SEARCH_RESULTS
}

/// Convenience wrapper collecting every match.
#[cfg(test)]
pub fn search_collect(root: &Path, query: &str, options: SearchOptions) -> Vec<FileEntry> {
    let mut out = Vec::new();
    search(root, query, options, &AtomicBool::new(false), |b| {
        out.extend(b)
    });
    out
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DirStats {
    pub bytes: u64,
    pub files: u64,
    pub dirs: u64,
}

/// Recursive size of a folder (symlinks are not followed).
pub fn dir_stats(path: &Path, cancel: &AtomicBool) -> DirStats {
    let mut stats = DirStats::default();
    for entry in WalkDir::new(path)
        .skip_hidden(false)
        .follow_links(false)
        .min_depth(1)
    {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let Ok(entry) = entry else { continue };
        let ft = entry.file_type();
        if ft.is_dir() {
            stats.dirs += 1;
        } else {
            stats.files += 1;
            if ft.is_file() {
                if let Ok(meta) = entry.metadata() {
                    stats.bytes += meta.len();
                }
            }
        }
    }
    stats
}

/// Read the beginning of a text file for the preview pane. Returns `None`
/// for binary files.
pub fn read_text_preview(path: &Path, max_bytes: usize) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut buf = vec![0u8; max_bytes];
    let n = file.read(&mut buf).ok()?;
    buf.truncate(n);
    if buf.contains(&0) {
        return None;
    }
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// Sub-directory names of `path` that start with `prefix` (for path-bar
/// completion).
pub fn complete_dir(partial: &str) -> Vec<PathBuf> {
    let p = Path::new(partial);
    let (dir, prefix) = if partial.ends_with('/') {
        (p.to_path_buf(), String::new())
    } else {
        (
            p.parent().map(Path::to_path_buf).unwrap_or_default(),
            p.file_name()
                .map(|f| f.to_string_lossy().to_lowercase())
                .unwrap_or_default(),
        )
    };
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = rd
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.starts_with(&prefix) && (!name.starts_with('.') || prefix.starts_with('.'))
        })
        .map(|e| e.path())
        .collect();
    out.sort();
    out.truncate(8);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn recursive_search() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir(root.join("subdir")).unwrap();
        File::create(root.join("subdir/target.txt")).unwrap();
        File::create(root.join("root_target.txt")).unwrap();

        let rec = search_collect(root, "TARGET", SearchOptions::default());
        assert_eq!(rec.len(), 2, "should find both files recursively");

        let flat = search_collect(
            root,
            "target",
            SearchOptions {
                recursive: false,
                ..Default::default()
            },
        );
        assert_eq!(flat.len(), 1);
        assert!(flat[0].path.ends_with("root_target.txt"));
    }

    #[test]
    fn hidden_entries_respect_option() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir(root.join(".hidden")).unwrap();
        File::create(root.join(".hidden/match.txt")).unwrap();
        assert!(search_collect(root, "match", SearchOptions::default()).is_empty());
        let opts = SearchOptions {
            include_hidden: true,
            ..Default::default()
        };
        assert_eq!(search_collect(root, "match", opts).len(), 1);
    }

    #[test]
    fn content_search_skips_binary() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        writeln!(
            File::create(root.join("hello.txt")).unwrap(),
            "the Secret keyword"
        )
        .unwrap();
        writeln!(File::create(root.join("other.txt")).unwrap(), "random").unwrap();
        let mut bin = b"text".to_vec();
        bin.push(0);
        bin.extend_from_slice(b"secret in binary");
        File::create(root.join("blob.bin"))
            .unwrap()
            .write_all(&bin)
            .unwrap();

        let opts = SearchOptions {
            content_search: true,
            ..Default::default()
        };
        let res = search_collect(root, "secret", opts);
        assert_eq!(res.len(), 1);
        assert!(res[0].path.ends_with("hello.txt"));

        assert!(search_collect(root, "secret", SearchOptions::default()).is_empty());
    }

    #[test]
    fn listing_and_stats() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir(root.join("d")).unwrap();
        std::fs::write(root.join("d/a"), b"12345").unwrap();
        std::fs::write(root.join(".h"), b"1").unwrap();
        let list = list_dir(root).unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|e| e.hidden && e.name == ".h"));
        let stats = dir_stats(root, &AtomicBool::new(false));
        assert_eq!(
            stats,
            DirStats {
                bytes: 6,
                files: 2,
                dirs: 1
            }
        );
    }

    #[test]
    fn cancelled_search_stops() {
        let tmp = TempDir::new().unwrap();
        for i in 0..50 {
            File::create(tmp.path().join(format!("f{i}"))).unwrap();
        }
        let cancel = AtomicBool::new(true);
        let mut n = 0;
        search(tmp.path(), "f", SearchOptions::default(), &cancel, |b| {
            n += b.len()
        });
        assert_eq!(n, 0);
    }
}
