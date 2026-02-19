use gpui::{App, AsyncApp, Context, Global};
use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Config {
    #[serde(default)]
    pub commands: HashMap<String, String>,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default = "default_pinned_folders")]
    pub pinned_folders: Vec<PathBuf>,
    #[serde(default)]
    pub recent_folders: std::collections::VecDeque<PathBuf>,
    #[serde(default)]
    pub group_files_by_type: bool,
    #[serde(default = "default_file_categories")]
    pub file_categories: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub use_dms: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Default)]
pub struct UiConfig {
    pub theme: Option<String>,
    pub icon_theme: Option<String>,
    #[serde(default)]
    pub show_hidden: bool,
    #[serde(default = "default_view_mode")]
    pub view_mode: String,
    #[serde(default = "default_icon_size")]
    pub icon_size: u32,
    #[serde(default)]
    pub filled_icons: bool,
}

fn default_view_mode() -> String {
    "grid".to_string()
}

fn default_icon_size() -> u32 {
    64
}

pub fn default_pinned_folders() -> Vec<PathBuf> {
    let mut folders = Vec::new();
    if let Some(home) = dirs::home_dir() {
        folders.push(home.clone());
        if let Some(downloads) = dirs::download_dir() {
            folders.push(downloads);
        }
        if let Some(documents) = dirs::document_dir() {
            folders.push(documents);
        }
    }
    folders
}

fn default_file_categories() -> HashMap<String, Vec<String>> {
    let mut categories = HashMap::new();

    categories.insert(
        "Images".to_string(),
        vec![
            ".jpg".to_string(),
            ".jpeg".to_string(),
            ".png".to_string(),
            ".gif".to_string(),
            ".bmp".to_string(),
            ".svg".to_string(),
            ".webp".to_string(),
            ".ico".to_string(),
            ".tiff".to_string(),
        ],
    );

    categories.insert(
        "Videos".to_string(),
        vec![
            ".mp4".to_string(),
            ".avi".to_string(),
            ".mkv".to_string(),
            ".mov".to_string(),
            ".wmv".to_string(),
            ".flv".to_string(),
            ".webm".to_string(),
            ".m4v".to_string(),
            ".mpg".to_string(),
        ],
    );

    categories.insert(
        "Audio".to_string(),
        vec![
            ".mp3".to_string(),
            ".flac".to_string(),
            ".wav".to_string(),
            ".ogg".to_string(),
            ".aac".to_string(),
            ".m4a".to_string(),
            ".wma".to_string(),
            ".opus".to_string(),
        ],
    );

    categories.insert(
        "Documents".to_string(),
        vec![
            ".pdf".to_string(),
            ".doc".to_string(),
            ".docx".to_string(),
            ".txt".to_string(),
            ".odt".to_string(),
            ".rtf".to_string(),
            ".md".to_string(),
            ".tex".to_string(),
        ],
    );

    categories.insert(
        "Archives".to_string(),
        vec![
            ".zip".to_string(),
            ".tar".to_string(),
            ".gz".to_string(),
            ".rar".to_string(),
            ".7z".to_string(),
            ".bz2".to_string(),
            ".xz".to_string(),
            ".tgz".to_string(),
        ],
    );

    categories.insert(
        "Code".to_string(),
        vec![
            ".rs".to_string(),
            ".py".to_string(),
            ".js".to_string(),
            ".ts".to_string(),
            ".c".to_string(),
            ".cpp".to_string(),
            ".java".to_string(),
            ".go".to_string(),
            ".rb".to_string(),
            ".php".to_string(),
            ".html".to_string(),
            ".css".to_string(),
        ],
    );

    categories
}

impl Default for Config {
    fn default() -> Self {
        let mut commands = HashMap::new();
        commands.insert("terminal".to_string(), "kitty".to_string());
        commands.insert("editor".to_string(), "code".to_string());

        Self {
            commands,
            ui: UiConfig::default(),
            pinned_folders: default_pinned_folders(),
            recent_folders: std::collections::VecDeque::new(),
            group_files_by_type: false,
            file_categories: default_file_categories(),
            use_dms: false,
        }
    }
}

impl Config {
    /// Get the category for a file based on its extension (case-insensitive)
    pub fn get_file_category(&self, path: &std::path::Path) -> Option<String> {
        let extension = path.extension()?.to_str()?;
        let ext_lower = format!(".{}", extension.to_lowercase());

        for (category, extensions) in &self.file_categories {
            if extensions.iter().any(|e| e.to_lowercase() == ext_lower) {
                return Some(category.clone());
            }
        }

        None // Returns None for uncategorized files (will go to "Other")
    }

    /// Add a pinned folder (max 10 folders)
#[allow(dead_code)]
    pub fn add_pinned_folder(&mut self, path: PathBuf) -> bool {
        // Don't add duplicates
        if self.pinned_folders.contains(&path) {
            return false;
        }

        if self.pinned_folders.len() >= 10 {
            self.pinned_folders.remove(0); // Remove oldest
        }

        self.pinned_folders.push(path);
        true
    }

    /// Remove a pinned folder
#[allow(dead_code)]
    pub fn remove_pinned_folder(&mut self, path: &std::path::Path) -> bool {
        if let Some(pos) = self.pinned_folders.iter().position(|p| p == path) {
            self.pinned_folders.remove(pos);
            return true;
        }
        false
    }

    /// Save config to file
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(config_path) = ConfigManager::get_config_path() {
            let toml_string = toml::to_string_pretty(self)?;
            fs::write(config_path, toml_string)?;
        }
        Ok(())
    }
}

pub struct ConfigManager {
    pub config: Config, // Public for trait access
    _watcher: Option<RecommendedWatcher>,
}

impl Global for ConfigManager {}

impl ConfigManager {
    pub fn init(cx: &mut App) {
        let config = Self::load_config();

        let mut watcher: Option<RecommendedWatcher> = None;
        let config_path = Self::get_config_path();

        if let Some(path) = config_path.clone() {
            let (tx, rx) = flume::unbounded();
            let tx_clone = tx.clone();

            match RecommendedWatcher::new(
                move |res: notify::Result<Event>| {
                    if let Ok(event) = res {
                        if event.kind.is_modify() || event.kind.is_create() {
                            let _ = tx_clone.send(());
                        }
                    }
                },
                NotifyConfig::default(),
            ) {
                Ok(mut w) => {
                    if let Err(e) = w.watch(&path, RecursiveMode::NonRecursive) {
                        eprintln!("Failed to watch config: {:?}", e);
                    } else {
                        watcher = Some(w);

                        let async_cx = cx.to_async();
                        let async_cx_clone = async_cx.clone();
                        cx.spawn(move |_: &mut AsyncApp| async move {
                            while let Ok(_) = rx.recv_async().await {
                                async_cx_clone.update_global::<ConfigManager, _>(|manager, cx| {
                                    let new_config = Self::load_config();
                                    if manager.config != new_config {
                                        manager.config = new_config;
                                        cx.refresh_windows();
                                    }
                                });
                            }
                        })
                        .detach();
                    }
                }
                Err(e) => eprintln!("Failed to create watcher: {:?}", e),
            }
        }

        cx.set_global(ConfigManager {
            config,
            _watcher: watcher,
        });
    }

    fn get_config_path() -> Option<PathBuf> {
        let path = dirs::config_dir().map(|d| d.join("m3fm").join("config.toml"));
        if let Some(ref p) = path {
            if let Some(parent) = p.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if !p.exists() {
                let default_toml = r#"
[commands]
terminal = "kitty"
editor = "code"

[ui]
theme = "default"
"#;
                let _ = fs::write(p, default_toml);
            }
        }
        path
    }

    pub fn save_config(&self) {
        if let Some(path) = Self::get_config_path() {
            if let Ok(toml_str) = toml::to_string_pretty(&self.config) {
                if let Err(e) = fs::write(path, toml_str) {
                    eprintln!("Failed to save config: {:?}", e);
                }
            }
        }
    }

    fn load_config() -> Config {
        if let Some(path) = Self::get_config_path() {
            if let Ok(contents) = fs::read_to_string(path) {
                if let Ok(config) = toml::from_str(&contents) {
                    return config;
                } else {
                    eprintln!("Failed to parse config.toml");
                }
            }
        }
        Config::default()
    }
}

pub trait ConfigContext {
    fn config(&self) -> &Config;
}

impl<T> ConfigContext for Context<'_, T> {
    fn config(&self) -> &Config {
        &self.global::<ConfigManager>().config
    }
}
// Also implement for App if needed, or WindowContext
impl ConfigContext for App {
    fn config(&self) -> &Config {
        &self.global::<ConfigManager>().config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_categorization() {
        let config = Config::default();

        // Images
        assert_eq!(
            config.get_file_category(std::path::Path::new("test.jpg")),
            Some("Images".to_string())
        );
        assert_eq!(
            config.get_file_category(std::path::Path::new("test.PNG")),
            Some("Images".to_string())
        );

        // Videos
        assert_eq!(
            config.get_file_category(std::path::Path::new("movie.mp4")),
            Some("Videos".to_string())
        );

        // Documents
        assert_eq!(
            config.get_file_category(std::path::Path::new("doc.pdf")),
            Some("Documents".to_string())
        );

        // Code
        assert_eq!(
            config.get_file_category(std::path::Path::new("main.rs")),
            Some("Code".to_string())
        );

        // Unknown/Other
        assert_eq!(
            config.get_file_category(std::path::Path::new("unknown.xyz")),
            None
        );
        assert_eq!(
            config.get_file_category(std::path::Path::new("no_extension")),
            None
        );
    }

    #[test]
    fn test_pinned_folders_limit() {
        let mut config = Config::default();
        config.pinned_folders.clear(); // Start empty

        // Add 10 folders
        for i in 0..10 {
            config.add_pinned_folder(PathBuf::from(format!("/path/{}", i)));
        }

        assert_eq!(config.pinned_folders.len(), 10);
        assert_eq!(config.pinned_folders[0], PathBuf::from("/path/0"));
        assert_eq!(config.pinned_folders[9], PathBuf::from("/path/9"));

        // Add 11th folder - should remove the first one (FIFO)
        config.add_pinned_folder(PathBuf::from("/path/10"));

        assert_eq!(config.pinned_folders.len(), 10);
        assert_eq!(config.pinned_folders[0], PathBuf::from("/path/1")); // 0 is gone, 1 is new head
        assert_eq!(config.pinned_folders[9], PathBuf::from("/path/10")); // 10 is new tail

        // Add duplicate - should not add
        config.add_pinned_folder(PathBuf::from("/path/5"));
        assert_eq!(config.pinned_folders.len(), 10);
        assert_eq!(config.pinned_folders[9], PathBuf::from("/path/10")); // config didn't change order if duplicate
    }
}
