use jwalk::WalkDir;
use std::path::{Path, PathBuf};

use std::io::BufRead;
use std::time::SystemTime;

#[derive(Clone, Debug)]
pub struct ScannedFile {
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub modified: SystemTime,
}

#[derive(Clone, Debug, Copy)] // Added Copy/Clone for easy passing
pub struct SearchOptions {
    pub recursive: bool,
    pub content_search: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            recursive: true,
            content_search: false,
        }
    }
}

// Result struct to return to UI
#[derive(Clone, Debug)]
pub struct ScanResult {
    pub dir: PathBuf,
    pub files: Vec<ScannedFile>,
}

pub fn scan_dir(path: PathBuf, show_hidden: bool) -> ScanResult {
    let mut files = Vec::new();
    eprintln!(
        "[DEBUG] scan_dir() - path={:?}, show_hidden={}",
        path, show_hidden
    );

    // Depth 1 for current view.
    // CRITICAL: jwalk skips hidden files by default on Unix!
    // We must set skip_hidden(false) to see ALL files, then filter ourselves
    for entry in WalkDir::new(&path)
        .skip_hidden(false)
        .min_depth(1)
        .max_depth(1)
        .sort(true)
    {
        if let Ok(entry) = entry {
            let file_name = entry.file_name().to_string_lossy();

            // Skip hidden files if show_hidden is false
            if !show_hidden && file_name.starts_with('.') {
                // eprintln!("[DEBUG] Skipping hidden file: {}", file_name);
                continue;
            }

            // if show_hidden && file_name.starts_with('.') {
            //     eprintln!("[DEBUG] Including hidden file: {}", file_name);
            // }

            let path = entry.path();
            let mut size = 0;
            let mut modified = SystemTime::UNIX_EPOCH;
            let mut is_dir = false;

            if let Ok(metadata) = std::fs::metadata(&path) {
                size = metadata.len();
                if let Ok(m) = metadata.modified() {
                    modified = m;
                }
                is_dir = metadata.is_dir();
            }

            files.push(ScannedFile {
                path,
                is_dir,
                size,
                modified,
            });
        }
    }

    eprintln!("[DEBUG] scan_dir() - found {} files total", files.len());
    ScanResult { dir: path, files }
}

pub fn scan_recursive(path: PathBuf, query: String, options: SearchOptions) -> ScanResult {
    let mut files = Vec::new();
    let query_lower = query.to_lowercase();

    // Configure recursion depth
    let max_depth = if options.recursive { usize::MAX } else { 1 };

    for entry in WalkDir::new(&path)
        .sort(true)
        .max_depth(max_depth)
        .skip_hidden(false)
    // Don't skip hidden
    {
        if let Ok(entry) = entry {
            // Skip the root directory itself
            if entry.depth() == 0 {
                continue;
            }

            let entry_path = entry.path();
            let file_name = entry.file_name().to_string_lossy();
            let mut matched = false;

            // 1. Name Match (Always check first)
            if file_name.to_lowercase().contains(&query_lower) {
                matched = true;
            }

            // 2. Content Search (Only if requested, not already matched, and is a file)
            // Implementation Constraint: Memory Safety (BufReader) & Binary Check (Null Byte)
            if !matched && options.content_search && entry.file_type().is_file() {
                if let Ok(file) = std::fs::File::open(&entry_path) {
                    let reader = std::io::BufReader::new(file);

                    // Helper to check for binary without consuming the reader entirely
                    // Actually, we need to peek or read the first chunk.
                    // Since we are searching, we can just read the first chunk into a buffer.
                    let mut buffer = [0; 1024];
                    // Use std::io::Read trait
                    use std::io::Read;

                    // We need to re-open or seek?
                    // Simpler: Read first chunk. If binary, stop. If text, scan it + continue scan.

                    // Let's implement robust check.
                    // But wait, if we read the first 1024 bytes, we consume them.
                    // We'd have to handle searching IN that buffer, then continuing.

                    // Re-opening is safer/easier logic wise unless performance is critical (open syscall).
                    // For a search tool, open is fine.

                    let mut is_binary = false;
                    // Scope for binary check
                    {
                        if let Ok(mut f_check) = std::fs::File::open(&entry_path) {
                            if let Ok(n) = f_check.read(&mut buffer) {
                                if buffer[..n].iter().any(|&b| b == 0) {
                                    is_binary = true;
                                }
                            }
                        }
                    }

                    if !is_binary {
                        // It's likely text. Search content line by line.
                        // We must re-open or seek to 0. Since we opened a new file for check, `reader` (not used yet) is still at 0?
                        // Ah, I created `reader` before. It's safe if I didn't read from it.
                        // Wait, I didn't verify `reader` *creation* doesn't read. It buffers.
                        // But I didn't call read on `reader` yet.
                        // Actually, I opened `file` and made `reader`.
                        // Then I opened `f_check`.
                        // So `reader` is fresh at pos 0? Yes.

                        // Note: `BufReader` might pre-fill buffer? No, only on read.

                        // Search Loop
                        for line in reader.lines() {
                            if let Ok(l) = line {
                                if l.to_lowercase().contains(&query_lower) {
                                    matched = true;
                                    break; // Early exit
                                }
                            } else {
                                // Encoding error or read error -> treat as binary/skip
                                break;
                            }
                        }
                    }
                }
            }

            if !matched {
                continue;
            }

            let mut size = 0;
            let mut modified = SystemTime::UNIX_EPOCH;
            let mut is_dir = false;

            if let Ok(metadata) = entry.metadata() {
                size = metadata.len();
                if let Ok(m) = metadata.modified() {
                    modified = m;
                }
                is_dir = metadata.is_dir();
            } else {
                if let Ok(metadata) = std::fs::metadata(&entry_path) {
                    size = metadata.len();
                    if let Ok(m) = metadata.modified() {
                        modified = m;
                    }
                    is_dir = metadata.is_dir();
                }
            }

            files.push(ScannedFile {
                path: entry_path,
                is_dir,
                size,
                modified,
            });
        }
    }
    ScanResult { dir: path, files }
}

pub fn calculate_recursive_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.metadata().ok())
        .filter(|meta| meta.is_file())
        .map(|meta| meta.len())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_recursive_search() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let sub = root.join("subdir");
        std::fs::create_dir(&sub).unwrap();

        File::create(sub.join("target.txt")).unwrap();
        File::create(root.join("root_target.txt")).unwrap();

        // Test Recursive ON
        let opts_rec = SearchOptions {
            recursive: true,
            content_search: false,
        };
        let res_rec = scan_recursive(root.to_path_buf(), "target".to_string(), opts_rec);
        assert_eq!(res_rec.files.len(), 2, "Should find both files recursively");

        // Test Recursive OFF
        let opts_flat = SearchOptions {
            recursive: false,
            content_search: false,
        };
        let res_flat = scan_recursive(root.to_path_buf(), "target".to_string(), opts_flat);
        assert_eq!(
            res_flat.files.len(),
            1,
            "Should only find root file when not recursive"
        );
        assert!(res_flat.files[0].path.ends_with("root_target.txt"));
    }

    #[test]
    fn test_content_search_text() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let mut f = File::create(root.join("hello.txt")).unwrap();
        writeln!(f, "This file contains the secret keyword.").unwrap();

        let mut f2 = File::create(root.join("other.txt")).unwrap();
        writeln!(f2, "Just random text.").unwrap();

        // Search for "secret"
        let opts = SearchOptions {
            recursive: true,
            content_search: true,
        };
        let res = scan_recursive(root.to_path_buf(), "secret".to_string(), opts);

        assert_eq!(res.files.len(), 1);
        assert!(res.files[0].path.ends_with("hello.txt"));
    }

    #[test]
    fn test_content_search_binary_skip() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create a "fake" binary file with a null byte
        let mut f = File::create(root.join("binary.bin")).unwrap();
        let mut data = b"some text before".to_vec();
        data.push(0); // Null byte
        data.extend_from_slice(b"secret keyword inside binary");
        f.write_all(&data).unwrap();

        // Create a text file with same keyword
        let mut f2 = File::create(root.join("text.txt")).unwrap();
        writeln!(f2, "secret keyword is here").unwrap();

        // Search for "secret"
        let opts = SearchOptions {
            recursive: true,
            content_search: true,
        };
        let res = scan_recursive(root.to_path_buf(), "secret".to_string(), opts);

        assert_eq!(res.files.len(), 1, "Should skip binary file");
        assert!(res.files[0].path.ends_with("text.txt"));
    }

    #[test]
    fn test_content_search_off() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let mut f = File::create(root.join("hidden.txt")).unwrap();
        writeln!(f, "findme").unwrap();

        // Search for "findme" with content search OFF
        let opts = SearchOptions {
            recursive: true,
            content_search: false,
        };
        let res = scan_recursive(root.to_path_buf(), "findme".to_string(), opts);

        assert_eq!(
            res.files.len(),
            0,
            "Should not find file by content calls if disabled"
        );
    }
}
