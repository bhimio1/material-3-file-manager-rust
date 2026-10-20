//! Installed applications from freedesktop `.desktop` files, used by the
//! "Open with" dialog.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct AppEntry {
    /// Desktop file id, e.g. `org.gnome.TextEditor.desktop`.
    pub id: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub exec: String,
    pub icon: Option<String>,
    pub mime_types: Vec<String>,
    pub terminal: bool,
    pub desktop_file: PathBuf,
}

impl AppEntry {
    /// Whether the app declares support for `mime` (exact or `type/*`).
    pub fn handles(&self, mime: &str) -> bool {
        let major = mime.split('/').next().unwrap_or("");
        self.mime_types
            .iter()
            .any(|m| m == mime || (m.ends_with("/*") && m.trim_end_matches("/*") == major))
    }

    /// Resolve the icon name to a file on disk.
    pub fn icon_path(&self) -> Option<PathBuf> {
        let icon = self.icon.as_deref()?;
        let p = Path::new(icon);
        if p.is_absolute() {
            return p.exists().then(|| p.to_path_buf());
        }
        freedesktop_icons::lookup(icon)
            .with_size(48)
            .with_cache()
            .find()
            .or_else(|| freedesktop_icons::lookup(icon).with_cache().find())
    }
}

fn application_dirs() -> Vec<PathBuf> {
    let mut dirs_out = Vec::new();
    // User entries first so they shadow system ones with the same id.
    if let Some(data) = dirs::data_dir() {
        dirs_out.push(data.join("applications"));
        dirs_out.push(data.join("flatpak/exports/share/applications"));
    }
    let system = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    for d in system.split(':') {
        dirs_out.push(Path::new(d).join("applications"));
    }
    dirs_out.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    dirs_out
}

/// Parse the `[Desktop Entry]` group of a desktop file.
pub fn parse_desktop_entry(id: String, path: &Path, contents: &str) -> Option<AppEntry> {
    let mut in_main = false;
    let mut fields: HashMap<&str, &str> = HashMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_main = line == "[Desktop Entry]";
            continue;
        }
        if !in_main || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            fields.entry(k.trim()).or_insert(v.trim());
        }
    }
    let is_true = |k: &str| fields.get(k).map(|v| *v == "true").unwrap_or(false);
    if fields.get("Type").copied().unwrap_or("Application") != "Application"
        || is_true("NoDisplay")
        || is_true("Hidden")
    {
        return None;
    }
    let name = fields.get("Name")?.to_string();
    let exec = fields.get("Exec")?.to_string();
    Some(AppEntry {
        id,
        name,
        generic_name: fields.get("GenericName").map(|s| s.to_string()),
        exec,
        icon: fields
            .get("Icon")
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty()),
        mime_types: fields
            .get("MimeType")
            .map(|m| {
                m.split(';')
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default(),
        terminal: is_true("Terminal"),
        desktop_file: path.to_path_buf(),
    })
}

pub fn scan_applications() -> Vec<AppEntry> {
    let mut by_id: HashMap<String, AppEntry> = HashMap::new();
    let mut seen_ids = std::collections::HashSet::new();
    for dir in application_dirs() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let id = entry.file_name().to_string_lossy().into_owned();
            // First directory wins, even if that entry is hidden.
            if !seen_ids.insert(id.clone()) {
                continue;
            }
            let Ok(contents) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some(app) = parse_desktop_entry(id.clone(), &path, &contents) {
                by_id.insert(id, app);
            }
        }
    }
    let mut apps: Vec<AppEntry> = by_id.into_values().collect();
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

/// Split an `Exec` value into arguments following the desktop entry spec
/// quoting rules.
fn split_exec(exec: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut has_token = false;
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                has_token = true;
            }
            '\\' if in_quotes => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            c if c.is_whitespace() && !in_quotes => {
                if has_token {
                    args.push(std::mem::take(&mut cur));
                    has_token = false;
                }
            }
            c => {
                cur.push(c);
                has_token = true;
            }
        }
    }
    if has_token {
        args.push(cur);
    }
    args
}

fn file_uri(p: &Path) -> String {
    url::Url::from_file_path(p)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| format!("file://{}", p.display()))
}

/// Expand field codes into a ready-to-spawn argv. Files are passed as
/// separate arguments; nothing goes through a shell.
pub fn build_command(app: &AppEntry, files: &[PathBuf]) -> Vec<String> {
    let mut argv = Vec::new();
    let mut used_files = false;
    for arg in split_exec(&app.exec) {
        match arg.as_str() {
            "%f" => {
                used_files = true;
                argv.extend(files.first().map(|f| f.to_string_lossy().into_owned()));
            }
            "%F" => {
                used_files = true;
                argv.extend(files.iter().map(|f| f.to_string_lossy().into_owned()));
            }
            "%u" => {
                used_files = true;
                argv.extend(files.first().map(|f| file_uri(f)));
            }
            "%U" => {
                used_files = true;
                argv.extend(files.iter().map(|f| file_uri(f)));
            }
            "%i" => {
                if let Some(icon) = &app.icon {
                    argv.push("--icon".into());
                    argv.push(icon.clone());
                }
            }
            "%c" => argv.push(app.name.clone()),
            "%k" => argv.push(app.desktop_file.to_string_lossy().into_owned()),
            "%d" | "%D" | "%n" | "%N" | "%v" | "%m" => {}
            _ => {
                // Field codes embedded in a larger argument.
                let mut out = String::new();
                let mut chars = arg.chars().peekable();
                while let Some(c) = chars.next() {
                    if c == '%' {
                        match chars.next() {
                            Some('%') => out.push('%'),
                            Some('f') | Some('F') => {
                                used_files = true;
                                if let Some(f) = files.first() {
                                    out.push_str(&f.to_string_lossy());
                                }
                            }
                            Some('u') | Some('U') => {
                                used_files = true;
                                if let Some(f) = files.first() {
                                    out.push_str(&file_uri(f));
                                }
                            }
                            Some('c') => out.push_str(&app.name),
                            _ => {}
                        }
                    } else {
                        out.push(c);
                    }
                }
                if !out.is_empty() {
                    argv.push(out);
                }
            }
        }
    }
    if !used_files {
        argv.extend(files.iter().map(|f| f.to_string_lossy().into_owned()));
    }
    argv
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESKTOP: &str = "[Desktop Entry]\nType=Application\nName=Editor\nName[de]=Bearbeiter\nExec=editor --new-window %U\nIcon=accessories-text-editor\nMimeType=text/plain;text/*;\n\n[Desktop Action new]\nName=Other\nExec=other\n";

    fn app(exec: &str) -> AppEntry {
        let mut a =
            parse_desktop_entry("e.desktop".into(), Path::new("/x/e.desktop"), DESKTOP).unwrap();
        a.exec = exec.into();
        a
    }

    #[test]
    fn parses_only_main_group() {
        let a =
            parse_desktop_entry("e.desktop".into(), Path::new("/x/e.desktop"), DESKTOP).unwrap();
        assert_eq!(a.name, "Editor");
        assert_eq!(a.exec, "editor --new-window %U");
        assert!(a.handles("text/plain"));
        assert!(a.handles("text/x-rust"));
        assert!(!a.handles("image/png"));
        assert!(parse_desktop_entry(
            "h".into(),
            Path::new("/h"),
            "[Desktop Entry]\nName=H\nExec=h\nNoDisplay=true\n"
        )
        .is_none());
    }

    #[test]
    fn expands_field_codes_without_a_shell() {
        let files = vec![PathBuf::from("/tmp/a b\"c.txt")];
        assert_eq!(
            build_command(&app("editor %f"), &files),
            vec!["editor".to_string(), "/tmp/a b\"c.txt".to_string()]
        );
        assert_eq!(build_command(&app("viewer"), &files)[1], "/tmp/a b\"c.txt");
        let uri = build_command(&app("x %U"), &files);
        assert!(uri[1].starts_with("file:///tmp/a%20b"));
        assert_eq!(
            build_command(&app(r#"sh -c "echo \"hi\" 100%%" %F"#), &files),
            vec!["sh", "-c", "echo \"hi\" 100%", "/tmp/a b\"c.txt"]
        );
    }
}
