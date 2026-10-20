//! Cloud storage through rclone: Google Drive is connected with rclone's
//! browser sign-in and mounted with `rclone mount`, so every other part of
//! the app just sees a normal folder.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const DRIVE_WEB_URL: &str = "https://drive.google.com";
pub const INSTALL_URL: &str = "https://rclone.org/install/";

/// A connected cloud drive (persisted in config.toml).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CloudDrive {
    /// Display name, e.g. "Google Drive".
    pub name: String,
    /// rclone remote name, without the trailing colon.
    pub remote: String,
    pub mount_point: PathBuf,
    #[serde(default = "default_true")]
    pub auto_mount: bool,
}

fn default_true() -> bool {
    true
}

impl CloudDrive {
    pub fn google(remote: String) -> Self {
        Self {
            name: "Google Drive".into(),
            remote,
            mount_point: dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/"))
                .join("GoogleDrive"),
            auto_mount: true,
        }
    }

    pub fn contains(&self, path: &Path) -> bool {
        path.starts_with(&self.mount_point)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Quota {
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub used: u64,
    #[serde(default)]
    pub free: u64,
    #[serde(default)]
    pub trashed: u64,
}

impl Quota {
    pub fn used_fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.used as f32 / self.total as f32
        }
    }
}

fn rclone() -> Command {
    let mut cmd = Command::new("rclone");
    cmd.stdin(Stdio::null());
    cmd
}

fn run(cmd: &mut Command) -> anyhow::Result<String> {
    let out = cmd.output().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => anyhow::anyhow!("rclone isn't installed"),
        _ => e.into(),
    })?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        // rclone prefixes messages with a timestamp and level; keep the text.
        let msg = err
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .map(|l| {
                l.split_once(": ")
                    .map(|(_, m)| m)
                    .unwrap_or(l)
                    .trim()
                    .to_string()
            })
            .unwrap_or_else(|| format!("rclone exited with {}", out.status));
        anyhow::bail!(msg)
    }
}

/// `Some("v1.66.0")` when rclone is installed.
pub fn rclone_version() -> Option<String> {
    let out = run(rclone().arg("version")).ok()?;
    out.lines()
        .next()?
        .split_whitespace()
        .nth(1)
        .map(String::from)
}

/// Parse `rclone listremotes --long` into (name, type) pairs.
pub fn parse_listremotes(out: &str) -> Vec<(String, String)> {
    out.lines()
        .filter_map(|l| {
            let (name, kind) = l.split_once(':')?;
            Some((name.trim().to_string(), kind.trim().to_string()))
        })
        .filter(|(n, _)| !n.is_empty())
        .collect()
}

pub fn list_remotes() -> anyhow::Result<Vec<(String, String)>> {
    Ok(parse_listremotes(&run(
        rclone().args(["listremotes", "--long"])
    )?))
}

/// Whether a remote finished sign-in (has an OAuth token).
pub fn remote_is_authorized(remote: &str) -> bool {
    run(rclone().args(["config", "show", remote]))
        .map(|s| s.lines().any(|l| l.trim_start().starts_with("token")))
        .unwrap_or(false)
}

/// An unused remote name: `gdrive`, `gdrive2`, …
pub fn free_remote_name(existing: &[(String, String)]) -> String {
    (1..)
        .map(|n| {
            if n == 1 {
                "gdrive".to_string()
            } else {
                format!("gdrive{n}")
            }
        })
        .find(|name| !existing.iter().any(|(e, _)| e == name))
        .unwrap()
}

pub fn delete_remote(remote: &str) -> anyhow::Result<()> {
    run(rclone().args(["config", "delete", remote])).map(|_| ())
}

/// Undo the octal escapes used in /proc/mounts (`\040` for space, …).
fn unescape_mount_field(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 3 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 4], 8) {
                out.push(v as char);
                i += 4;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Whether `mount_point` is an active rclone mount according to `mounts`
/// (the contents of /proc/self/mounts).
pub fn is_mounted_in(mounts: &str, mount_point: &Path) -> bool {
    mounts.lines().any(|line| {
        let mut f = line.split_whitespace();
        let (_dev, mp, fstype) = (f.next(), f.next(), f.next());
        matches!((mp, fstype), (Some(mp), Some(t)) if t.starts_with("fuse.rclone") && Path::new(&unescape_mount_field(mp)) == mount_point)
    })
}

pub fn is_mounted(mount_point: &Path) -> bool {
    std::fs::read_to_string("/proc/self/mounts")
        .map(|m| is_mounted_in(&m, mount_point))
        .unwrap_or(false)
}

/// Mount the drive in the background (`rclone mount --daemon`), returning
/// once the mount is ready or failed.
pub fn mount(drive: &CloudDrive) -> anyhow::Result<()> {
    let mp = &drive.mount_point;
    if is_mounted(mp) {
        return Ok(());
    }
    std::fs::create_dir_all(mp)?;
    if std::fs::read_dir(mp)?.next().is_some() {
        anyhow::bail!(
            "{} isn't empty; choose an empty folder to mount into",
            mp.display()
        );
    }
    run(rclone()
        .arg("mount")
        .arg(format!("{}:", drive.remote))
        .arg(mp)
        .args([
            "--daemon",
            "--daemon-wait",
            "30s",
            // Full caching lets apps seek and write normally and keeps
            // re-opened files local.
            "--vfs-cache-mode",
            "full",
            "--dir-cache-time",
            "5m",
            "--poll-interval",
            "1m",
        ]))
    .map(|_| ())
}

pub fn unmount(mount_point: &Path) -> anyhow::Result<()> {
    let mut last_err = None;
    for (prog, args) in [
        ("fusermount3", vec!["-u"]),
        ("fusermount", vec!["-u"]),
        ("umount", vec![]),
    ] {
        match Command::new(prog)
            .args(&args)
            .arg(mount_point)
            .stdin(Stdio::null())
            .output()
        {
            Ok(out) if out.status.success() => return Ok(()),
            Ok(out) => last_err = Some(String::from_utf8_lossy(&out.stderr).trim().to_string()),
            Err(_) => continue,
        }
    }
    anyhow::bail!(last_err.unwrap_or_else(|| "no unmount tool (fusermount3) found".into()))
}

pub fn about(remote: &str) -> anyhow::Result<Quota> {
    let json = run(rclone().args(["about", &format!("{remote}:"), "--json"]))?;
    Ok(serde_json::from_str(&json)?)
}

/// A running `rclone config create` waiting for the browser sign-in.
pub struct ConnectSession {
    pub remote: String,
    child: Arc<Mutex<Child>>,
    /// The local auth URL rclone printed, for when no browser opened.
    pub auth_url: Arc<Mutex<Option<String>>>,
    pub done: flume::Receiver<Result<(), String>>,
}

impl ConnectSession {
    /// Start sign-in for a new remote. rclone opens the browser itself and
    /// listens on localhost for Google's redirect.
    pub fn start(
        remote: String,
        notify: impl Fn() + Send + Sync + 'static,
    ) -> anyhow::Result<Self> {
        let mut child = rclone()
            .args(["config", "create", &remote, "drive", "scope=drive"])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => anyhow::anyhow!("rclone isn't installed"),
                _ => e.into(),
            })?;
        let stderr = child.stderr.take().expect("piped stderr");
        let child = Arc::new(Mutex::new(child));
        let auth_url = Arc::new(Mutex::new(None));
        let notify = Arc::new(notify);

        {
            let auth_url = auth_url.clone();
            let notify = notify.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    if let Some(url) = parse_auth_url(&line) {
                        *auth_url.lock().unwrap() = Some(url);
                        notify();
                    }
                }
            });
        }

        let (tx, rx) = flume::bounded(1);
        {
            let child = child.clone();
            let remote = remote.clone();
            std::thread::spawn(move || {
                let status = loop {
                    if let Ok(Some(status)) = child.lock().unwrap().try_wait() {
                        break status;
                    }
                    std::thread::sleep(Duration::from_millis(200));
                };
                let result = if status.success() && remote_is_authorized(&remote) {
                    Ok(())
                } else {
                    // rclone saves the remote before auth; don't leave a
                    // half-configured one behind.
                    let _ = delete_remote(&remote);
                    Err("Sign-in didn't complete".to_string())
                };
                let _ = tx.send(result);
                notify();
            });
        }
        Ok(Self {
            remote,
            child,
            auth_url,
            done: rx,
        })
    }

    pub fn cancel(&self) {
        let _ = self.child.lock().unwrap().kill();
    }
}

impl Drop for ConnectSession {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Extract the auth link from rclone's
/// "If your browser doesn't open automatically go to the following link: …".
pub fn parse_auth_url(line: &str) -> Option<String> {
    let idx = line.find("http://127.0.0.1")?;
    Some(line[idx..].split_whitespace().next()?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listremotes() {
        let r = parse_listremotes("gdrive:     drive\nphotos: s3\n\n");
        assert_eq!(
            r,
            vec![
                ("gdrive".into(), "drive".into()),
                ("photos".into(), "s3".into())
            ]
        );
        assert_eq!(free_remote_name(&r), "gdrive2");
        assert_eq!(free_remote_name(&[]), "gdrive");
    }

    #[test]
    fn mount_detection() {
        let mounts = "/dev/sda1 / ext4 rw 0 0\ngdrive: /home/me/Google\\040Drive fuse.rclone rw,nosuid 0 0\n";
        assert!(is_mounted_in(mounts, Path::new("/home/me/Google Drive")));
        assert!(!is_mounted_in(mounts, Path::new("/home/me/Google")));
        assert!(!is_mounted_in(mounts, Path::new("/")));
    }

    #[test]
    fn auth_url() {
        let line = "2026/10/20 05:31:44 NOTICE: If your browser doesn't open automatically go to the following link: http://127.0.0.1:53682/auth?state=abc";
        assert_eq!(
            parse_auth_url(line).as_deref(),
            Some("http://127.0.0.1:53682/auth?state=abc")
        );
        assert_eq!(parse_auth_url("Waiting for code..."), None);
    }

    #[test]
    fn quota_json() {
        let q: Quota = serde_json::from_str(
            r#"{"total":16106127360,"used":5368709120,"trashed":1024,"free":10737418240}"#,
        )
        .unwrap();
        assert_eq!(q.free, 10737418240);
        assert!((q.used_fraction() - 1.0 / 3.0).abs() < 0.001);
    }

    #[test]
    fn mount_refuses_non_empty_folder() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("x"), "").unwrap();
        let drive = CloudDrive {
            mount_point: tmp.path().to_path_buf(),
            ..CloudDrive::google("nope".into())
        };
        let err = mount(&drive).unwrap_err().to_string();
        assert!(err.contains("isn't empty"), "{err}");
    }
}
