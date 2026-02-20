#![allow(dead_code)]
use gpui::*;
use std::path::PathBuf;

// Material Symbols Rounded icons (downloaded from Google Fonts)
// These are actual SVG files loaded via external_path()

/// Get the path to an icon file - returns absolute path
fn get_icon_path(filename: &str) -> String {
    let mut candidates = Vec::new();

    // 1. Check CARGO_MANIFEST_DIR (Development)
    if let Some(manifest_dir) = option_env!("CARGO_MANIFEST_DIR") {
        candidates.push(PathBuf::from(manifest_dir).join("assets/icons").join(filename));
    }

    // 2. Check current working directory
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("assets/icons").join(filename));
    }

    // 3. Check relative to executable (Distribution/Release)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            candidates.push(parent.join("assets/icons").join(filename));
            // Also check ../share/m3fm/assets/icons if installed in standard linux path
            candidates.push(parent.join("../share/m3fm/assets/icons").join(filename));
        }
    }

    // 4. Fallback relative path
    candidates.push(PathBuf::from("assets/icons").join(filename));

    // 1. Current working directory (good for "cargo run")
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("assets/icons").join(filename));
    }

    // 2. Executable directory and parents (good for "target/debug" or installed)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("assets/icons").join(filename));
            candidates.push(exe_dir.join("../assets/icons").join(filename));
            candidates.push(exe_dir.join("../../assets/icons").join(filename));
            candidates.push(exe_dir.join("../../../assets/icons").join(filename));
        }
    }

    // 3. Compile-time manifest dir (best for dev)
    if let Some(manifest_dir) = option_env!("CARGO_MANIFEST_DIR") {
        candidates.push(PathBuf::from(manifest_dir).join("assets/icons").join(filename));
    }

    // 4. System paths (Linux)
    candidates.push(PathBuf::from("/usr/share/material_3_file_manager/assets/icons").join(filename));
    candidates.push(PathBuf::from("/usr/local/share/material_3_file_manager/assets/icons").join(filename));
    // Hardcoded fallback relative to home (for original author specific setup if needed, but relative is better)
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(PathBuf::from(home).join("Dev/material 3 file manager -rust/assets/icons").join(filename));
    }

    for candidate in candidates {
        if candidate.exists() {
            if let Ok(abs_path) = candidate.canonicalize() {
                return abs_path.to_string_lossy().to_string();
            }
            return candidate.to_string_lossy().to_string();
        }
    }

    // Last resort fallback
    filename.to_string()
}

/// Returns an SVG icon element using Material Symbols
pub fn icon(name: &str) -> Svg {
    let filename = match name {
        "arrow-left" | "arrow_left" | "icons/arrow-left.svg" => "arrow_back.svg",
        "arrow-right" | "arrow_right" | "icons/arrow-right.svg" => "arrow_forward.svg",
        "refresh" => "refresh.svg",
        "close" | "items/x-mark.svg" | "icons/x-mark.svg" => "close.svg",
        "add" => "add.svg",
        "remove" => "remove.svg",
        "grid" => "grid.svg",
        "grid_filled" => "grid_filled.svg",
        "list" => "list.svg",
        "list_filled" => "list_filled.svg",
        "chevron_right" => "chevron_right.svg",
        "magnifying-glass" | "icons/magnifying-glass.svg" | "search" => "search.svg",
        // File Types
        "folder" => "folder.svg",
        "audio" => "audio.svg",
        "video" => "video.svg",
        "image" => "image.svg",
        "archive" => "archive.svg",
        "settings" => "settings.svg",
        "file" => "file.svg",
        "home" => "home.svg",
        "download" => "download.svg",
        "hard_drive" => "hard_drive.svg",
        // Dashboard icons
        "star" => "star.svg",
        "description" => "description.svg",
        "schedule" => "schedule.svg",
        "check" => "check.svg", // Fallback if missing, or use standard icon
        _ => "file.svg", // Fallback
    };

    let path = get_icon_path(filename);
    svg().external_path(path).size_5()
}

/// Returns an Image element for the icon (alternative rendering)
pub fn icon_img(name: &str) -> Img {
    let filename = match name {
        "arrow-left" | "arrow_left" | "icons/arrow-left.svg" => "arrow_back.svg",
        "arrow-right" | "arrow_right" | "icons/arrow-right.svg" => "arrow_forward.svg",
        "refresh" => "refresh.svg",
        "close" | "items/x-mark.svg" | "icons/x-mark.svg" => "close.svg",
        "add" => "add.svg",
        "remove" => "remove.svg",
        "grid" => "grid.svg",
        "grid_filled" => "grid_filled.svg",
        "list" => "list.svg",
        "list_filled" => "list_filled.svg",
        "chevron_right" => "chevron_right.svg",
        "magnifying-glass" | "icons/magnifying-glass.svg" | "search" => "search.svg",
        "folder" => "folder.svg",
        "audio" => "audio.svg",
        "video" => "video.svg",
        "image" => "image.svg",
        "archive" => "archive.svg",
        "settings" => "settings.svg",
        "file" => "file.svg",
        "home" => "home.svg",
        "download" => "download.svg",
        "hard_drive" => "hard_drive.svg",
        "star" => "star.svg",
        "description" => "description.svg",
        "schedule" => "schedule.svg",
        _ => "file.svg",
    };

    let path = get_icon_path(filename);
    img(path)
}

// Keep IconPaths for compatibility with icon_cache.rs
pub struct IconPaths;

impl IconPaths {
    // Navigation icons
    pub const BACK: &'static str = "M19 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H19v-2z";
    pub const FORWARD: &'static str = "M12 4l-1.41 1.41L16.17 11H4v2h12.17l-5.58 5.59L12 20l8-8z";
    pub const REFRESH: &'static str = "M17.65 6.35C16.2 4.9 14.21 4 12 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08c-.82 2.33-3.04 4-5.65 4-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z";
    pub const CLOSE: &'static str = "M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 17.59 13.41 12z";
    pub const ADD: &'static str = "M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z";
    pub const GRID: &'static str =
        "M4 11h5V5H4v6zm0 7h5v-6H4v6zm6 0h5v-6h-5v6zm6 0h5v-6h-5v6zm-6-7h5V5h-5v6zm6-6v6h5V5h-5z";
    pub const LIST: &'static str =
        "M3 13h2v-2H3v2zm0 4h2v-2H3v2zm0-8h2V7H3v2zm4 4h14v-2H7v2zm0 4h14v-2H7v2zM7 7v2h14V7H7z";

    // File type icons
    pub const FOLDER: &'static str = "M10 4H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z";
    pub const FILE_GENERIC: &'static str =
        "M13 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V9l-7-7z";
    pub const AUDIO: &'static str = "M12 3v9.28c-.47-.17-.97-.28-1.5-.28C8.01 12 6 14.01 6 16.5S8.01 21 10.5 21c2.31 0 4.2-1.75 4.45-4H15V6h4V3h-7z";
    pub const IMAGE: &'static str = "M21 19V5c0-1.1-.9-2-2-2H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2zM8.5 13.5l2.5 3.01L14.5 12l4.5 6H5l3.5-4.5z";
    pub const VIDEO: &'static str = "M17 10.5V7c0-.55-.45-1-1-1H4c-.55 0-1 .45-1 1v10c0 .55.45 1 1 1h12c.55 0 1-.45 1-1v-3.5l4 4v-11l-4 4z";
    pub const ARCHIVE: &'static str = "M20 6h-4V4c0-1.11-.89-2-2-2h-4c-1.11 0-2 .89-2 2v2H4c-1.11 0-1.99.89-1.99 2L2 19c0 1.11.89 2 2 2h16c1.11 0 2-.89 2-2V8c0-1.11-.89-2-2-2zm-6 0h-4V4h4v2z";
    pub const SETTINGS: &'static str = "M19.14 12.94c.04-.3.06-.61.06-.94 0-.32-.02-.64-.07-.94l2.03-1.58a.49.49 0 00.12-.61l-1.92-3.32a.488.488 0 00-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54a.484.484 0 00-.48-.41h-3.84c-.24 0-.43.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96c-.18-.08-.39 0-.49.22l-1.92 3.32c-.12.21-.07.47.12.61l2.03 1.58c-.05.3-.09.63-.09.94s.04.64.09.94l-2.03 1.58a.49.49 0 00-.12.61l1.92 3.32c.1.22.31.29.5.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.58 1.62-.94l2.39.96c.19.08.4.01.5-.22l1.92-3.32c.12-.22.07-.47-.12-.61l-2.01-1.58zM12 15.6c-1.98 0-3.6-1.62-3.6-3.6s1.62-3.6 3.6-3.6 3.6 1.62 3.6 3.6-1.62 3.6-3.6 3.6z";
    pub const HARD_DRIVE: &'static str = "M6 15h12v2H6v-2zm0-8h12v2H6V7zm0 4h12v2H6v-2zm13-8H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2z";
    pub const CATEGORY: &'static str = "M12 2l-5.5 9h11L12 2zm0 3.84L13.93 9h-3.87L12 5.84zM17.5 13c-2.49 0-4.5 2.01-4.5 4.5s2.01 4.5 4.5 4.5 4.5-2.01 4.5-4.5-2.01-4.5-4.5-4.5zm0 7c-1.38 0-2.5-1.12-2.5-2.5s1.12-2.5 2.5-2.5 2.5 1.12 2.5 2.5-1.12 2.5-2.5 2.5zM3 13c-1.1 0-2 .9-2 2v5c0 1.1.9 2 2 2h5c1.1 0 2-.9 2-2v-5c0-1.1-.9-2-2-2H3zm5 7H3v-5h5v5z";
}
