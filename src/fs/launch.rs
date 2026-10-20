//! Opening files and folders in other programs.

use super::apps::{build_command, AppEntry};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn spawn(argv: &[String], cwd: Option<&Path>) -> std::io::Result<()> {
    let (program, args) = argv
        .split_first()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "empty command"))?;
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.spawn().map(|_| ())
}

/// Split a user-configured command line ("kitty --single-instance").
fn words(cmd: &str) -> Vec<String> {
    cmd.split_whitespace().map(String::from).collect()
}

/// Open with the default application (or `dms open` when enabled).
pub fn open_default(path: &Path, use_dms: bool) -> anyhow::Result<()> {
    if use_dms
        && spawn(
            &[
                "dms".into(),
                "open".into(),
                path.to_string_lossy().into_owned(),
            ],
            None,
        )
        .is_ok()
    {
        return Ok(());
    }
    open::that_detached(path)
        .map_err(|e| anyhow::anyhow!("No application could open this file ({e})"))
}

pub fn open_with(app: &AppEntry, files: &[PathBuf], terminal: Option<&str>) -> anyhow::Result<()> {
    let mut argv = build_command(app, files);
    if app.terminal {
        let mut wrapped = words(terminal.unwrap_or("xterm"));
        wrapped.push("-e".into());
        wrapped.append(&mut argv);
        argv = wrapped;
    }
    spawn(&argv, files.first().and_then(|f| f.parent()))
        .map_err(|e| anyhow::anyhow!("Couldn't start {}: {e}", app.name))
}

/// Open a terminal in `dir`: the configured command first, then common
/// terminals. Terminals inherit the working directory we spawn them in.
pub fn open_terminal(dir: &Path, configured: Option<&str>) -> anyhow::Result<()> {
    let mut candidates: Vec<Vec<String>> = Vec::new();
    if let Some(cmd) = configured {
        candidates.push(words(cmd));
    }
    for t in [
        "xdg-terminal-exec",
        "kitty",
        "foot",
        "alacritty",
        "wezterm",
        "ghostty",
        "gnome-terminal",
        "konsole",
        "xfce4-terminal",
        "xterm",
    ] {
        candidates.push(vec![t.to_string()]);
    }
    for argv in candidates.iter().filter(|a| !a.is_empty()) {
        if spawn(argv, Some(dir)).is_ok() {
            return Ok(());
        }
    }
    anyhow::bail!("No terminal emulator found. Set one in Settings → Applications.")
}

pub fn open_in_editor(path: &Path, configured: Option<&str>) -> anyhow::Result<()> {
    let mut argv = words(configured.unwrap_or("xdg-open"));
    argv.push(path.to_string_lossy().into_owned());
    let cwd = if path.is_dir() {
        Some(path)
    } else {
        path.parent()
    };
    spawn(&argv, cwd).map_err(|e| anyhow::anyhow!("Couldn't start editor “{}”: {e}", argv[0]))
}
