//! Persistent user configuration (`~/.config/m3fm/config.toml`).
//!
//! The on-disk format is backwards compatible with the original gpui build:
//! every field that existed there keeps its name and meaning, new fields all
//! have serde defaults so old files keep loading.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};

pub const MAX_PINNED: usize = 24;
pub const MAX_RECENT: usize = 12;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    #[default]
    Grid,
    List,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ColorScheme {
    #[default]
    System,
    Light,
    Dark,
}

/// Where the Material palette comes from.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThemeSource {
    /// Use matugen's `theme.json` when it exists, otherwise the seed color.
    #[default]
    Auto,
    /// Always use `theme.json` (written by matugen).
    Matugen,
    /// Generate a tonal palette from `seed_color`.
    Seed,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SortKey {
    #[default]
    Name,
    Size,
    Modified,
    Type,
}

impl SortKey {
    pub const ALL: [SortKey; 4] = [
        SortKey::Name,
        SortKey::Size,
        SortKey::Modified,
        SortKey::Type,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SortKey::Name => "Name",
            SortKey::Size => "Size",
            SortKey::Modified => "Modified",
            SortKey::Type => "Type",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct UiConfig {
    /// Legacy field from the gpui build; kept so it round-trips.
    pub theme: Option<String>,
    pub icon_theme: Option<String>,
    pub show_hidden: bool,
    pub view_mode: ViewMode,
    /// Grid tile icon size in points.
    pub icon_size: u32,
    pub filled_icons: bool,
    pub color_scheme: ColorScheme,
    pub theme_source: ThemeSource,
    pub seed_color: String,
    pub show_thumbnails: bool,
    pub show_preview_pane: bool,
    pub sort_by: SortKey,
    pub sort_ascending: bool,
    pub folders_first: bool,
    pub ui_scale: f32,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: None,
            icon_theme: None,
            show_hidden: false,
            view_mode: ViewMode::Grid,
            icon_size: 64,
            filled_icons: true,
            color_scheme: ColorScheme::System,
            theme_source: ThemeSource::Auto,
            seed_color: "#4285F4".to_string(),
            show_thumbnails: true,
            show_preview_pane: true,
            sort_by: SortKey::Name,
            sort_ascending: true,
            folders_first: true,
            ui_scale: 1.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Config {
    #[serde(default = "default_commands")]
    pub commands: HashMap<String, String>,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default = "default_pinned_folders")]
    pub pinned_folders: Vec<PathBuf>,
    #[serde(default)]
    pub recent_folders: VecDeque<PathBuf>,
    #[serde(default)]
    pub group_files_by_type: bool,
    #[serde(default = "default_file_categories")]
    pub file_categories: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub use_dms: bool,
}

fn default_commands() -> HashMap<String, String> {
    let mut commands = HashMap::new();
    commands.insert("terminal".to_string(), "kitty".to_string());
    commands.insert("editor".to_string(), "code".to_string());
    commands
}

pub fn default_pinned_folders() -> Vec<PathBuf> {
    let mut folders = Vec::new();
    if let Some(home) = dirs::home_dir() {
        folders.push(home);
    }
    folders.extend(dirs::download_dir());
    folders.extend(dirs::document_dir());
    folders
}

pub fn default_file_categories() -> BTreeMap<String, Vec<String>> {
    let table: [(&str, &[&str]); 6] = [
        (
            "Images",
            &[
                "jpg", "jpeg", "png", "gif", "bmp", "svg", "webp", "ico", "tiff", "avif", "heic",
            ],
        ),
        (
            "Videos",
            &[
                "mp4", "avi", "mkv", "mov", "wmv", "flv", "webm", "m4v", "mpg",
            ],
        ),
        (
            "Audio",
            &["mp3", "flac", "wav", "ogg", "aac", "m4a", "wma", "opus"],
        ),
        (
            "Documents",
            &[
                "pdf", "doc", "docx", "txt", "odt", "rtf", "md", "tex", "xls", "xlsx", "ods",
                "ppt", "pptx", "odp", "epub",
            ],
        ),
        (
            "Archives",
            &["zip", "tar", "gz", "rar", "7z", "bz2", "xz", "tgz", "zst"],
        ),
        (
            "Code",
            &[
                "rs", "py", "js", "ts", "c", "cpp", "h", "hpp", "java", "go", "rb", "php", "html",
                "css", "sh", "toml", "json", "yaml", "yml",
            ],
        ),
    ];
    table
        .iter()
        .map(|(name, exts)| {
            (
                name.to_string(),
                exts.iter().map(|e| format!(".{e}")).collect(),
            )
        })
        .collect()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            commands: default_commands(),
            ui: UiConfig::default(),
            pinned_folders: default_pinned_folders(),
            recent_folders: VecDeque::new(),
            group_files_by_type: false,
            file_categories: default_file_categories(),
            use_dms: false,
        }
    }
}

impl Config {
    /// Category for a file based on its extension (case-insensitive).
    /// Extensions are stored with a leading dot (`.png`) for compatibility,
    /// but entries without the dot are accepted too.
    pub fn file_category(&self, path: &Path) -> Option<&str> {
        let ext = path.extension()?.to_str()?.to_lowercase();
        self.file_categories
            .iter()
            .find(|(_, exts)| {
                exts.iter()
                    .any(|e| e.trim_start_matches('.').eq_ignore_ascii_case(&ext))
            })
            .map(|(name, _)| name.as_str())
    }

    pub fn command(&self, key: &str) -> Option<&str> {
        self.commands
            .get(key)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }

    pub fn is_pinned(&self, path: &Path) -> bool {
        self.pinned_folders.iter().any(|p| p == path)
    }

    /// Pin a folder. Returns false when it was already pinned. When the list
    /// is full the oldest pin is dropped.
    pub fn add_pinned_folder(&mut self, path: PathBuf) -> bool {
        if self.is_pinned(&path) {
            return false;
        }
        if self.pinned_folders.len() >= MAX_PINNED {
            self.pinned_folders.remove(0);
        }
        self.pinned_folders.push(path);
        true
    }

    pub fn remove_pinned_folder(&mut self, path: &Path) -> bool {
        let before = self.pinned_folders.len();
        self.pinned_folders.retain(|p| p != path);
        before != self.pinned_folders.len()
    }

    pub fn push_recent(&mut self, path: &Path) {
        if self
            .recent_folders
            .front()
            .map(|p| p == path)
            .unwrap_or(false)
        {
            return;
        }
        self.recent_folders.retain(|p| p != path);
        self.recent_folders.push_front(path.to_path_buf());
        self.recent_folders.truncate(MAX_RECENT);
    }

    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("m3fm").join("config.toml"))
    }

    /// Load the config, creating a default file on first run.
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(config) => config,
                Err(e) => {
                    log::warn!("Failed to parse {}: {e}", path.display());
                    Self::default()
                }
            },
            Err(_) => {
                let config = Self::default();
                let _ = config.save();
                config
            }
        }
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_default()
    }

    /// Write the config atomically. Returns the serialized contents so the
    /// file watcher can recognise (and ignore) our own write.
    pub fn save(&self) -> anyhow::Result<String> {
        let path = Self::path().ok_or_else(|| anyhow::anyhow!("no config directory"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = self.to_toml();
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, &contents)?;
        std::fs::rename(&tmp, &path)?;
        Ok(contents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_categorization() {
        let config = Config::default();
        assert_eq!(config.file_category(Path::new("test.jpg")), Some("Images"));
        assert_eq!(config.file_category(Path::new("test.PNG")), Some("Images"));
        assert_eq!(config.file_category(Path::new("movie.mp4")), Some("Videos"));
        assert_eq!(
            config.file_category(Path::new("doc.pdf")),
            Some("Documents")
        );
        assert_eq!(config.file_category(Path::new("main.rs")), Some("Code"));
        assert_eq!(config.file_category(Path::new("unknown.xyz")), None);
        assert_eq!(config.file_category(Path::new("no_extension")), None);
    }

    #[test]
    fn pinned_folders_limit_and_dedup() {
        let mut config = Config::default();
        config.pinned_folders.clear();
        for i in 0..MAX_PINNED {
            assert!(config.add_pinned_folder(PathBuf::from(format!("/path/{i}"))));
        }
        assert!(!config.add_pinned_folder(PathBuf::from("/path/3")));
        config.add_pinned_folder(PathBuf::from("/path/new"));
        assert_eq!(config.pinned_folders.len(), MAX_PINNED);
        assert_eq!(config.pinned_folders[0], PathBuf::from("/path/1"));
        assert!(config.remove_pinned_folder(Path::new("/path/new")));
        assert!(!config.is_pinned(Path::new("/path/new")));
    }

    #[test]
    fn recent_folders_are_mru_and_bounded() {
        let mut config = Config::default();
        for i in 0..20 {
            config.push_recent(Path::new(&format!("/r/{i}")));
        }
        config.push_recent(Path::new("/r/15"));
        assert_eq!(config.recent_folders.len(), MAX_RECENT);
        assert_eq!(config.recent_folders[0], PathBuf::from("/r/15"));
        assert_eq!(
            config
                .recent_folders
                .iter()
                .filter(|p| **p == Path::new("/r/15"))
                .count(),
            1
        );
    }

    #[test]
    fn legacy_config_still_parses() {
        let legacy = r#"
use_dms = true
group_files_by_type = true

[commands]
terminal = "foot"

[ui]
theme = "default"
show_hidden = true
view_mode = "list"
"#;
        let config: Config = toml::from_str(legacy).unwrap();
        assert!(config.use_dms);
        assert!(config.ui.show_hidden);
        assert_eq!(config.ui.view_mode, ViewMode::List);
        assert_eq!(config.ui.icon_size, 64);
        assert_eq!(config.command("terminal"), Some("foot"));
        assert!(!config.file_categories.is_empty());
    }
}
