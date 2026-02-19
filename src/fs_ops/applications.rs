use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct AppEntry {
    pub name: String,
    pub icon_path: Option<String>,
    pub exec: String,
}

pub fn scan_applications() -> Vec<AppEntry> {
    let mut apps: Vec<AppEntry> = Vec::new();
    let paths = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        dirs::home_dir()
            .map(|h| h.join(".local/share/applications"))
            .unwrap_or_default(),
    ];

    for path in paths {
        if !path.exists() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                if let Some(ext) = path.extension() {
                    if ext != "desktop" {
                        continue;
                    }
                } else {
                    continue;
                }

                if let Ok(content) = std::fs::read_to_string(&path) {
                    let mut name = String::new();
                    let mut icon = None;
                    let mut exec = String::new();
                    let mut no_display = false;

                    for line in content.lines() {
                        if line.starts_with("Name=") && name.is_empty() {
                            name = line.trim_start_matches("Name=").to_string();
                        } else if line.starts_with("Icon=") {
                            icon = Some(line.trim_start_matches("Icon=").to_string());
                        } else if line.starts_with("Exec=") && exec.is_empty() {
                            exec = line.trim_start_matches("Exec=").to_string();
                        } else if line == "NoDisplay=true" {
                            no_display = true;
                        }
                    }

                    if !name.is_empty() && !exec.is_empty() && !no_display {
                        apps.push(AppEntry {
                            name,
                            icon_path: icon,
                            exec,
                        });
                    }
                }
            }
        }
    }
    apps.sort_by(|a, b| a.name.cmp(&b.name));
    apps.dedup_by(|a, b| a.name == b.name);
    apps
}
