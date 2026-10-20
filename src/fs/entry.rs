use crate::config::SortKey;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Coarse file type used for icons, colors and the "Type" column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileKind {
    Folder,
    Image,
    Video,
    Audio,
    Pdf,
    Document,
    Spreadsheet,
    Presentation,
    Archive,
    Code,
    Text,
    Executable,
    DiskImage,
    Font,
    Other,
}

impl FileKind {
    pub fn from_path(path: &Path, is_dir: bool) -> Self {
        if is_dir {
            return FileKind::Folder;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tif" | "tiff"
            | "avif" | "heic" | "heif" | "raw" | "cr2" | "nef" | "psd" | "xcf" => FileKind::Image,
            "mp4" | "mkv" | "webm" | "avi" | "mov" | "wmv" | "flv" | "m4v" | "mpg" | "mpeg"
            | "3gp" | "ogv" => FileKind::Video,
            "mp3" | "flac" | "wav" | "ogg" | "oga" | "aac" | "m4a" | "wma" | "opus" | "aiff"
            | "mid" | "midi" => FileKind::Audio,
            "pdf" => FileKind::Pdf,
            "doc" | "docx" | "odt" | "rtf" | "epub" | "pages" => FileKind::Document,
            "xls" | "xlsx" | "ods" | "csv" | "tsv" | "numbers" => FileKind::Spreadsheet,
            "ppt" | "pptx" | "odp" | "key" => FileKind::Presentation,
            "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" | "7z" | "rar" | "lz4" | "lzma"
            | "deb" | "rpm" | "jar" | "apk" => FileKind::Archive,
            "rs" | "py" | "js" | "mjs" | "ts" | "tsx" | "jsx" | "c" | "h" | "cpp" | "hpp"
            | "cc" | "java" | "kt" | "go" | "rb" | "php" | "html" | "htm" | "css" | "scss"
            | "sh" | "bash" | "zsh" | "fish" | "lua" | "swift" | "cs" | "json" | "toml"
            | "yaml" | "yml" | "xml" | "sql" | "nix" | "zig" | "dart" | "vue" | "svelte" => {
                FileKind::Code
            }
            "txt" | "md" | "markdown" | "log" | "ini" | "conf" | "cfg" | "tex" | "rst" | "org"
            | "env" => FileKind::Text,
            "exe" | "msi" | "appimage" | "bin" | "run" => FileKind::Executable,
            "iso" | "img" | "dmg" | "qcow2" | "vdi" | "vmdk" => FileKind::DiskImage,
            "ttf" | "otf" | "woff" | "woff2" => FileKind::Font,
            _ => {
                let mime = mime_guess::from_path(path).first();
                match mime.as_ref().map(|m| m.type_().as_str()) {
                    Some("image") => FileKind::Image,
                    Some("video") => FileKind::Video,
                    Some("audio") => FileKind::Audio,
                    Some("text") => FileKind::Text,
                    Some("font") => FileKind::Font,
                    _ => FileKind::Other,
                }
            }
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FileKind::Folder => "Folder",
            FileKind::Image => "Image",
            FileKind::Video => "Video",
            FileKind::Audio => "Audio",
            FileKind::Pdf => "PDF document",
            FileKind::Document => "Document",
            FileKind::Spreadsheet => "Spreadsheet",
            FileKind::Presentation => "Presentation",
            FileKind::Archive => "Archive",
            FileKind::Code => "Source code",
            FileKind::Text => "Text",
            FileKind::Executable => "Program",
            FileKind::DiskImage => "Disk image",
            FileKind::Font => "Font",
            FileKind::Other => "File",
        }
    }

    /// Whether we can render a thumbnail for this kind.
    pub fn has_thumbnail(self) -> bool {
        matches!(self, FileKind::Image)
    }
}

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    /// Lowercased name, cached for sorting and type-ahead find.
    pub name_lower: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub hidden: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub kind: FileKind,
}

impl FileEntry {
    /// Build an entry from a path, following symlinks for type and size.
    pub fn from_path(path: PathBuf) -> Option<Self> {
        let link_meta = std::fs::symlink_metadata(&path).ok()?;
        let is_symlink = link_meta.file_type().is_symlink();
        let meta = if is_symlink {
            std::fs::metadata(&path).unwrap_or(link_meta)
        } else {
            link_meta
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        let is_dir = meta.is_dir();
        Some(Self {
            kind: FileKind::from_path(&path, is_dir),
            name_lower: name.to_lowercase(),
            hidden: name.starts_with('.'),
            is_dir,
            is_symlink,
            size: if is_dir { 0 } else { meta.len() },
            modified: meta.modified().ok(),
            name,
            path,
        })
    }

    pub fn extension(&self) -> Option<String> {
        if self.is_dir {
            return None;
        }
        self.path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
    }

    pub fn type_label(&self) -> String {
        match (self.kind, self.extension()) {
            (FileKind::Folder, _) => "Folder".into(),
            (FileKind::Other, Some(ext)) => format!("{} file", ext.to_uppercase()),
            (kind, _) => kind.label().into(),
        }
    }
}

/// "1 item", "3 items".
pub fn plural(n: impl Into<u64>, noun: &str) -> String {
    let n = n.into();
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

pub fn format_size(bytes: u64) -> String {
    humansize::format_size(bytes, humansize::DECIMAL)
}

pub fn format_time(t: Option<SystemTime>) -> String {
    let Some(t) = t else { return "—".into() };
    let dt: chrono::DateTime<chrono::Local> = t.into();
    let now = chrono::Local::now();
    let date = dt.date_naive();
    if date == now.date_naive() {
        format!("Today, {}", dt.format("%H:%M"))
    } else if Some(date) == now.date_naive().pred_opt() {
        format!("Yesterday, {}", dt.format("%H:%M"))
    } else if dt.format("%Y").to_string() == now.format("%Y").to_string() {
        dt.format("%b %-d, %H:%M").to_string()
    } else {
        dt.format("%b %-d, %Y").to_string()
    }
}

pub fn format_time_full(t: Option<SystemTime>) -> String {
    let Some(t) = t else { return "—".into() };
    let dt: chrono::DateTime<chrono::Local> = t.into();
    dt.format("%A, %B %-d, %Y at %H:%M:%S").to_string()
}

/// "Natural" comparison so `file2` sorts before `file10`.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut ai, mut bi) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ca), Some(cb)) if ca.is_ascii_digit() && cb.is_ascii_digit() => {
                let mut na = String::new();
                while let Some(c) = ai.peek().copied().filter(char::is_ascii_digit) {
                    na.push(c);
                    ai.next();
                }
                let mut nb = String::new();
                while let Some(c) = bi.peek().copied().filter(char::is_ascii_digit) {
                    nb.push(c);
                    bi.next();
                }
                let ta = na.trim_start_matches('0');
                let tb = nb.trim_start_matches('0');
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(ca), Some(cb)) => {
                if ca != cb {
                    return ca.cmp(&cb);
                }
                ai.next();
                bi.next();
            }
        }
    }
}

pub fn sort_entries(entries: &mut [FileEntry], key: SortKey, ascending: bool, folders_first: bool) {
    entries.sort_by(|a, b| {
        if folders_first && a.is_dir != b.is_dir {
            return b.is_dir.cmp(&a.is_dir);
        }
        let by_name = || natural_cmp(&a.name_lower, &b.name_lower);
        let ord = match key {
            SortKey::Name => by_name(),
            SortKey::Size => a.size.cmp(&b.size).then_with(by_name),
            SortKey::Modified => a.modified.cmp(&b.modified).then_with(by_name),
            SortKey::Type => a.extension().cmp(&b.extension()).then_with(by_name),
        };
        if ascending {
            ord
        } else {
            ord.reverse()
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, is_dir: bool, size: u64) -> FileEntry {
        FileEntry {
            path: PathBuf::from(name),
            name: name.into(),
            name_lower: name.to_lowercase(),
            is_dir,
            is_symlink: false,
            hidden: false,
            size,
            modified: None,
            kind: FileKind::from_path(Path::new(name), is_dir),
        }
    }

    #[test]
    fn natural_order() {
        let mut v = vec!["file10", "file2", "File1", "file02b", "a"];
        v.sort_by(|a, b| natural_cmp(&a.to_lowercase(), &b.to_lowercase()));
        assert_eq!(v, vec!["a", "File1", "file2", "file02b", "file10"]);
    }

    #[test]
    fn folders_first_and_direction() {
        let mut v = vec![
            entry("b.txt", false, 5),
            entry("zdir", true, 0),
            entry("a.txt", false, 10),
        ];
        sort_entries(&mut v, SortKey::Name, true, true);
        let names: Vec<_> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["zdir", "a.txt", "b.txt"]);

        sort_entries(&mut v, SortKey::Size, false, false);
        let names: Vec<_> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["a.txt", "b.txt", "zdir"]);
    }

    #[test]
    fn kinds() {
        assert_eq!(
            FileKind::from_path(Path::new("x.PNG"), false),
            FileKind::Image
        );
        assert_eq!(
            FileKind::from_path(Path::new("x.rs"), false),
            FileKind::Code
        );
        assert_eq!(FileKind::from_path(Path::new("x"), true), FileKind::Folder);
        assert_eq!(
            FileKind::from_path(Path::new("x.unknownext"), false),
            FileKind::Other
        );
    }
}
