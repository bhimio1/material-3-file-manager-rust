//! Persistent user configuration (`~/.config/m3fm/config.toml`).
//!
//! The on-disk format is backwards compatible with the original gpui build:
//! every field that existed there keeps its name and meaning, new fields all
//! have serde defaults so old files keep loading.

use material_3_ui::theme::{ColorScheme, ThemeSettings, ThemeSource};
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

/// The `[ui]` table. Appearance lives in a shared [`ThemeSettings`] in
/// memory but is stored under this file's own names (`theme_source`,
/// `color_scheme`, …), so existing config files keep their theme and older
/// versions of the app can still read new ones.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(from = "UiConfigFile", into = "UiConfigFile")]
pub struct UiConfig {
    /// Legacy field from the gpui build; kept so it round-trips.
    pub theme: Option<String>,
    pub icon_theme: Option<String>,
    pub show_hidden: bool,
    pub view_mode: ViewMode,
    /// Grid tile icon size in points.
    pub icon_size: u32,
    pub appearance: ThemeSettings,
    pub show_thumbnails: bool,
    pub show_preview_pane: bool,
    pub sort_by: SortKey,
    pub sort_ascending: bool,
    pub folders_first: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: None,
            icon_theme: None,
            show_hidden: false,
            view_mode: ViewMode::Grid,
            icon_size: 64,
            appearance: ThemeSettings::default(),
            show_thumbnails: true,
            show_preview_pane: true,
            sort_by: SortKey::Name,
            sort_ascending: true,
            folders_first: true,
        }
    }
}

/// Where the palette comes from, as stored in `theme_source`. `matugen`
/// is from older versions, where it meant the same as `auto` (the desktop's
/// palette file, else the seed color).
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum StoredThemeSource {
    Auto,
    Matugen,
    Seed,
}

/// [`UiConfig`] as written to config.toml.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
struct UiConfigFile {
    theme: Option<String>,
    icon_theme: Option<String>,
    show_hidden: bool,
    view_mode: ViewMode,
    icon_size: u32,
    filled_icons: bool,
    color_scheme: ColorScheme,
    theme_source: StoredThemeSource,
    seed_color: String,
    show_thumbnails: bool,
    show_preview_pane: bool,
    sort_by: SortKey,
    sort_ascending: bool,
    folders_first: bool,
    ui_scale: f32,
}

impl Default for UiConfigFile {
    fn default() -> Self {
        UiConfig::default().into()
    }
}

impl From<UiConfigFile> for UiConfig {
    fn from(f: UiConfigFile) -> Self {
        Self {
            theme: f.theme,
            icon_theme: f.icon_theme,
            show_hidden: f.show_hidden,
            view_mode: f.view_mode,
            icon_size: f.icon_size,
            appearance: ThemeSettings {
                source: match f.theme_source {
                    StoredThemeSource::Auto | StoredThemeSource::Matugen => ThemeSource::Auto,
                    StoredThemeSource::Seed => ThemeSource::Seed,
                },
                scheme: f.color_scheme,
                seed_color: f.seed_color,
                filled_icons: f.filled_icons,
                ui_scale: f.ui_scale,
            },
            show_thumbnails: f.show_thumbnails,
            show_preview_pane: f.show_preview_pane,
            sort_by: f.sort_by,
            sort_ascending: f.sort_ascending,
            folders_first: f.folders_first,
        }
    }
}

impl From<UiConfig> for UiConfigFile {
    fn from(c: UiConfig) -> Self {
        let a = c.appearance;
        Self {
            theme: c.theme,
            icon_theme: c.icon_theme,
            show_hidden: c.show_hidden,
            view_mode: c.view_mode,
            icon_size: c.icon_size,
            filled_icons: a.filled_icons,
            color_scheme: a.scheme,
            theme_source: match a.source {
                ThemeSource::Auto => StoredThemeSource::Auto,
                ThemeSource::Seed => StoredThemeSource::Seed,
            },
            seed_color: a.seed_color,
            show_thumbnails: c.show_thumbnails,
            show_preview_pane: c.show_preview_pane,
            sort_by: c.sort_by,
            sort_ascending: c.sort_ascending,
            folders_first: c.folders_first,
            ui_scale: a.ui_scale,
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
    /// Cloud drives mounted through rclone.
    #[serde(default)]
    pub cloud_drives: Vec<crate::fs::cloud::CloudDrive>,
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
            cloud_drives: Vec::new(),
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

    /// The cloud drive whose mount contains `path`, if any.
    pub fn cloud_drive_for(&self, path: &Path) -> Option<&crate::fs::cloud::CloudDrive> {
        self.cloud_drives.iter().find(|d| d.contains(path))
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
        assert_eq!(config.ui.appearance, ThemeSettings::default());
    }

    #[test]
    fn appearance_keeps_its_stored_names() {
        let old = r##"
[ui]
filled_icons = false
color_scheme = "light"
theme_source = "seed"
seed_color = "#B3261E"
ui_scale = 1.25
"##;
        let config: Config = toml::from_str(old).unwrap();
        let a = &config.ui.appearance;
        assert_eq!(a.source, ThemeSource::Seed);
        assert_eq!(a.scheme, ColorScheme::Light);
        assert_eq!(a.seed_color, "#B3261E");
        assert!(!a.filled_icons);
        assert_eq!(a.ui_scale, 1.25);

        // Saved under the same names, so the file round-trips.
        let saved = config.to_toml();
        assert!(saved.contains("theme_source = \"seed\""));
        assert!(saved.contains("color_scheme = \"light\""));
        assert!(!saved.contains("[ui.appearance]"));
        let back: Config = toml::from_str(&saved).unwrap();
        assert_eq!(back, config);
    }

    #[test]
    fn matugen_source_means_auto() {
        let config: Config = toml::from_str("[ui]\ntheme_source = \"matugen\"\n").unwrap();
        assert_eq!(config.ui.appearance.source, ThemeSource::Auto);
        assert!(config.to_toml().contains("theme_source = \"auto\""));
    }
}
