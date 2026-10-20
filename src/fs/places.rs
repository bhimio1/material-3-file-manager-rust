//! Well-known folders and mounted drives for the sidebar and dashboard.

use egui_material_icons::{icons::*, MaterialIcon};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Place {
    pub name: &'static str,
    pub path: PathBuf,
    pub icon: MaterialIcon,
}

pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

/// XDG user directories that exist on this machine.
pub fn standard_places() -> Vec<Place> {
    let home = home();
    let candidates: [(&'static str, Option<PathBuf>, MaterialIcon); 7] = [
        ("Home", Some(home.clone()), ICON_HOME),
        ("Desktop", dirs::desktop_dir(), ICON_DESKTOP_WINDOWS),
        ("Documents", dirs::document_dir(), ICON_DESCRIPTION),
        ("Downloads", dirs::download_dir(), ICON_DOWNLOAD),
        ("Music", dirs::audio_dir(), ICON_MUSIC_NOTE),
        ("Pictures", dirs::picture_dir(), ICON_IMAGE),
        ("Videos", dirs::video_dir(), ICON_MOVIE),
    ];
    candidates
        .into_iter()
        .filter_map(|(name, path, icon)| {
            let path = path?;
            // XDG dirs that aren't configured resolve to $HOME itself.
            if name != "Home" && path == home {
                return None;
            }
            path.is_dir().then_some(Place { name, path, icon })
        })
        .collect()
}

/// Short user-facing name for a folder ("Home" for $HOME).
pub fn display_name(path: &Path) -> String {
    if path == home() {
        return "Home".into();
    }
    if let Some((_, name)) = cloud_root(path).filter(|(root, _)| root == path) {
        return name;
    }
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Path with the home directory shown as `~`.
pub fn tilde(path: &Path) -> String {
    let home = home();
    match path.strip_prefix(&home) {
        Ok(rel) if rel.as_os_str().is_empty() => "~".into(),
        Ok(rel) => format!("~/{}", rel.display()),
        Err(_) => path.display().to_string(),
    }
}

/// Icon for a folder, using the special-folder icon when it is one.
/// Mount points of connected cloud drives with their display names, kept in
/// sync with the config so naming helpers don't need it passed around.
static CLOUD_MOUNTS: std::sync::RwLock<Vec<(PathBuf, String)>> = std::sync::RwLock::new(Vec::new());

pub fn set_cloud_mounts(mounts: Vec<(PathBuf, String)>) {
    *CLOUD_MOUNTS.write().unwrap() = mounts;
}

/// The cloud drive mount containing `path`: (mount point, name).
pub fn cloud_root(path: &Path) -> Option<(PathBuf, String)> {
    CLOUD_MOUNTS
        .read()
        .unwrap()
        .iter()
        .find(|(root, _)| path.starts_with(root))
        .cloned()
}

pub fn folder_icon(path: &Path) -> MaterialIcon {
    if cloud_root(path).is_some_and(|(root, _)| root == path) {
        return ICON_CLOUD;
    }
    // Called for every folder tile each frame; resolving XDG dirs re-reads
    // user-dirs.dirs, so do it once.
    static PLACES: std::sync::OnceLock<Vec<Place>> = std::sync::OnceLock::new();
    PLACES
        .get_or_init(standard_places)
        .iter()
        .find(|p| p.path == path)
        .map(|p| p.icon)
        .unwrap_or(ICON_FOLDER)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Drive {
    pub name: String,
    pub mount: PathBuf,
    pub total: u64,
    pub available: u64,
    pub removable: bool,
    pub file_system: String,
}

impl Drive {
    pub fn used_fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.total.saturating_sub(self.available)) as f32 / self.total as f32
        }
    }

    pub fn icon(&self) -> MaterialIcon {
        if self.removable {
            ICON_USB
        } else if self.mount == Path::new("/") {
            ICON_HARD_DRIVE
        } else {
            ICON_STORAGE
        }
    }
}

fn is_user_visible_mount(mount: &Path) -> bool {
    let s = mount.to_string_lossy();
    if s == "/" {
        return true;
    }
    const HIDDEN_PREFIXES: [&str; 10] = [
        "/boot",
        "/snap",
        "/var",
        "/proc",
        "/sys",
        "/dev",
        "/tmp",
        "/efi",
        "/run/user",
        "/nix/store",
    ];
    if s.starts_with("/run/media/") {
        return true;
    }
    if s.starts_with("/run/") {
        return false;
    }
    !HIDDEN_PREFIXES
        .iter()
        .any(|p| s == *p || s.starts_with(&format!("{p}/")))
}

pub fn drives() -> Vec<Drive> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut out: Vec<Drive> = Vec::new();
    for disk in disks.list() {
        let mount = disk.mount_point().to_path_buf();
        if !is_user_visible_mount(&mount) || out.iter().any(|d| d.mount == mount) {
            continue;
        }
        let fs = disk.file_system().to_string_lossy().into_owned();
        // rclone mounts are listed under Cloud instead.
        if matches!(fs.as_str(), "squashfs" | "overlay" | "tmpfs" | "devtmpfs")
            || fs.starts_with("fuse.rclone")
        {
            continue;
        }
        let label = mount
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .filter(|_| mount != Path::new("/"));
        out.push(Drive {
            name: label.unwrap_or_else(|| "File System".into()),
            mount,
            total: disk.total_space(),
            available: disk.available_space(),
            removable: disk.is_removable(),
            file_system: fs,
        });
    }
    out.sort_by(|a, b| {
        (a.mount != Path::new("/"))
            .cmp(&(b.mount != Path::new("/")))
            .then(a.name.cmp(&b.name))
    });
    out
}

/// The drive that contains `path` (longest mount-point prefix).
pub fn drive_for<'a>(drives: &'a [Drive], path: &Path) -> Option<&'a Drive> {
    drives
        .iter()
        .filter(|d| path.starts_with(&d.mount))
        .max_by_key(|d| d.mount.as_os_str().len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mount_filtering() {
        assert!(is_user_visible_mount(Path::new("/")));
        assert!(is_user_visible_mount(Path::new("/home")));
        assert!(is_user_visible_mount(Path::new("/run/media/me/USB")));
        assert!(is_user_visible_mount(Path::new("/mnt/data")));
        assert!(!is_user_visible_mount(Path::new("/boot/efi")));
        assert!(!is_user_visible_mount(Path::new("/snap/core/1")));
        assert!(!is_user_visible_mount(Path::new("/run/user/1000/doc")));
        assert!(!is_user_visible_mount(Path::new("/run/credentials")));
        assert!(is_user_visible_mount(Path::new("/variant")));
    }
}
