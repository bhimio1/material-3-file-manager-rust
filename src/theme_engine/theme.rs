use super::config::ThemeConfig;
use super::palette::M3Palette;
use gpui::*;
use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;

pub struct Theme {
    pub palette: M3Palette,
}

impl Global for Theme {}

impl Theme {
    pub fn init(cx: &mut App) {
        let palette = Self::load_palette();
        cx.set_global(Theme { palette });
    }

    fn get_theme_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("material_3_file_manager")
            .join("theme.json")
    }

    pub fn load_palette() -> M3Palette {
        let path = Self::get_theme_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<ThemeConfig>(&content) {
                    return M3Palette::from(config);
                } else {
                    eprintln!("Failed to parse theme.json");
                }
            } else {
                eprintln!("Failed to read theme.json");
            }
        }
        // Fallback default
        M3Palette::from_hex(0x4285F4)
    }

    pub fn reload(cx: &mut App) {
        let palette = Self::load_palette();
        cx.update_global::<Theme, _>(|theme, _cx| {
            theme.palette = palette;
        });
    }

    pub fn watch(cx: &mut App) {
        let (tx, rx) = channel();
        let mut watcher = RecommendedWatcher::new(tx, NotifyConfig::default()).unwrap();
        let path = Self::get_theme_path();

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        // Watch the parent directory so we detect creation/deletion
        if let Some(parent) = path.parent() {
            let _ = watcher.watch(parent, RecursiveMode::NonRecursive);
        }

        cx.spawn(|cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let _watcher = watcher; // Keep alive
                loop {
                    // Non-blocking loop to drain events
                    let mut changed = false;
                    while let Ok(Ok(Event { kind: _, paths, .. })) = rx.try_recv() {
                        if paths.iter().any(|p| p.ends_with("theme.json")) {
                            changed = true;
                        }
                    }

                    if changed {
                        // Update on main thread
                        let _ = cx.update(|cx| {
                            Self::reload(cx);
                        });
                    }

                    // Poll every 500ms
                    cx.background_executor()
                        .timer(Duration::from_millis(500))
                        .await;
                }
            }
        })
        .detach();
    }
}

// Helper trait to easily fetch theme from any context
pub trait ThemeContext {
    fn theme(&self) -> &Theme;
}

impl<T> ThemeContext for Context<'_, T> {
    fn theme(&self) -> &Theme {
        self.global::<Theme>()
    }
}
