//! App-side state for cloud drives: rclone detection, the Google sign-in
//! flow, mounting, and storage quota. rclone calls run on worker threads
//! and report back as [`CloudEvent`]s.

use crate::app::App;
use crate::fs::cloud::{self, CloudDrive, ConnectSession, Quota};
use crate::tab::Location;
use crate::ui::ToastKind;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

const STATUS_INTERVAL: Duration = Duration::from_secs(2);
const QUOTA_INTERVAL: Duration = Duration::from_secs(300);

enum CloudEvent {
    Version(Option<String>),
    /// An already-authorized Drive remote was found, or `None` to sign in.
    ExistingRemote(Option<String>, Vec<(String, String)>),
    Mounted {
        remote: String,
        result: Result<(), String>,
    },
    Unmounted {
        remote: String,
        result: Result<(), String>,
    },
    Quota {
        remote: String,
        result: Result<Quota, String>,
    },
}

pub struct CloudState {
    /// `None` while checking; `Some(None)` when rclone isn't installed.
    pub rclone: Option<Option<String>>,
    pub mounted: HashSet<String>,
    /// What's in progress per remote ("Mounting…", "Unmounting…").
    pub busy: HashMap<String, &'static str>,
    pub quota: HashMap<String, Quota>,
    pub connect: Option<ConnectSession>,
    /// Looking for an existing rclone remote before starting sign-in.
    pub preparing_connect: bool,
    /// Navigate into this remote's mount once it's mounted.
    open_after_mount: Option<String>,
    tx: flume::Sender<CloudEvent>,
    rx: flume::Receiver<CloudEvent>,
    status_at: Instant,
    quota_at: HashMap<String, Instant>,
    auto_mounted: bool,
}

impl Default for CloudState {
    fn default() -> Self {
        let (tx, rx) = flume::unbounded();
        Self {
            rclone: None,
            mounted: HashSet::new(),
            busy: HashMap::new(),
            quota: HashMap::new(),
            connect: None,
            preparing_connect: false,
            open_after_mount: None,
            tx,
            rx,
            status_at: Instant::now() - STATUS_INTERVAL,
            quota_at: HashMap::new(),
            auto_mounted: false,
        }
    }
}

impl CloudState {
    pub fn rclone_missing(&self) -> bool {
        matches!(self.rclone, Some(None))
    }
}

impl App {
    fn cloud_spawn(&self, work: impl FnOnce() -> CloudEvent + Send + 'static) {
        let tx = self.cloud.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(work());
            ctx.request_repaint();
        });
    }

    pub fn cloud_init(&mut self) {
        self.cloud_spawn(|| CloudEvent::Version(cloud::rclone_version()));
    }

    fn drive(&self, remote: &str) -> Option<CloudDrive> {
        self.config
            .cloud_drives
            .iter()
            .find(|d| d.remote == remote)
            .cloned()
    }

    /// Called every frame from `logic`.
    pub fn poll_cloud(&mut self) {
        let events: Vec<CloudEvent> = self.cloud.rx.try_iter().collect();
        for event in events {
            self.on_cloud_event(event);
        }

        if let Some(session) = &self.cloud.connect {
            if let Ok(result) = session.done.try_recv() {
                let remote = session.remote.clone();
                self.cloud.connect = None;
                match result {
                    Ok(()) => self.cloud_add_drive(remote),
                    Err(e) => self.toast(format!("Google Drive: {e}"), ToastKind::Error),
                }
            }
        }

        // Mount state can change outside the app (fusermount, logout…).
        if self.cloud.status_at.elapsed() >= STATUS_INTERVAL && !self.config.cloud_drives.is_empty()
        {
            self.cloud.status_at = Instant::now();
            let mounts = std::fs::read_to_string("/proc/self/mounts").unwrap_or_default();
            self.cloud.mounted = self
                .config
                .cloud_drives
                .iter()
                .filter(|d| cloud::is_mounted_in(&mounts, &d.mount_point))
                .map(|d| d.remote.clone())
                .collect();
            let due: Vec<String> = self
                .cloud
                .mounted
                .iter()
                .filter(|r| {
                    self.cloud
                        .quota_at
                        .get(*r)
                        .map(|t| t.elapsed() >= QUOTA_INTERVAL)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();
            for remote in due {
                self.cloud.quota_at.insert(remote.clone(), Instant::now());
                self.cloud_spawn(move || CloudEvent::Quota {
                    result: cloud::about(&remote).map_err(|e| e.to_string()),
                    remote,
                });
            }
            self.ctx.request_repaint_after(STATUS_INTERVAL);
        }
    }

    fn on_cloud_event(&mut self, event: CloudEvent) {
        match event {
            CloudEvent::Version(version) => {
                let installed = version.is_some();
                self.cloud.rclone = Some(version);
                if installed && !self.cloud.auto_mounted {
                    self.cloud.auto_mounted = true;
                    let mounts = std::fs::read_to_string("/proc/self/mounts").unwrap_or_default();
                    for d in self.config.cloud_drives.clone() {
                        if d.auto_mount && !cloud::is_mounted_in(&mounts, &d.mount_point) {
                            self.cloud_mount(&d.remote, false);
                        }
                    }
                }
            }
            CloudEvent::ExistingRemote(found, remotes) => {
                self.cloud.preparing_connect = false;
                match found {
                    Some(remote) => self.cloud_add_drive(remote),
                    None => {
                        let remote = cloud::free_remote_name(&remotes);
                        let ctx = self.ctx.clone();
                        match ConnectSession::start(remote, move || ctx.request_repaint()) {
                            Ok(session) => self.cloud.connect = Some(session),
                            Err(e) => {
                                self.toast(format!("Couldn't start sign-in: {e}"), ToastKind::Error)
                            }
                        }
                    }
                }
            }
            CloudEvent::Mounted { remote, result } => {
                self.cloud.busy.remove(&remote);
                self.cloud.status_at = Instant::now() - STATUS_INTERVAL;
                match result {
                    Ok(()) => {
                        self.cloud.mounted.insert(remote.clone());
                        if self.cloud.open_after_mount.as_deref() == Some(remote.as_str()) {
                            self.cloud.open_after_mount = None;
                            if let Some(d) = self.drive(&remote) {
                                self.tab_mut().navigate(Location::Folder(d.mount_point));
                            }
                        }
                    }
                    Err(e) => {
                        self.cloud.open_after_mount = None;
                        self.toast(
                            format!("Couldn't mount Google Drive: {e}"),
                            ToastKind::Error,
                        );
                    }
                }
            }
            CloudEvent::Unmounted { remote, result } => {
                self.cloud.busy.remove(&remote);
                self.cloud.status_at = Instant::now() - STATUS_INTERVAL;
                match result {
                    Ok(()) => {
                        self.cloud.mounted.remove(&remote);
                        // Don't leave tabs pointing into a folder that just emptied.
                        if let Some(d) = self.drive(&remote) {
                            for tab in &mut self.tabs {
                                if tab.folder().map(|f| d.contains(f)).unwrap_or(false) {
                                    tab.navigate(Location::Dashboard);
                                }
                            }
                        }
                    }
                    Err(e) => self.toast(
                        format!(
                            "Couldn't unmount: {e}. Close files open from the drive and try again."
                        ),
                        ToastKind::Error,
                    ),
                }
            }
            CloudEvent::Quota { remote, result } => {
                if let Ok(q) = result {
                    self.cloud.quota.insert(remote, q);
                }
            }
        }
    }

    /// Start connecting Google Drive: reuse an authorized Drive remote if
    /// rclone already has one, otherwise run the browser sign-in.
    pub fn cloud_connect(&mut self) {
        if self.cloud.rclone_missing() {
            self.toast(
                "Google Drive needs rclone. Install it, then try again.",
                ToastKind::Error,
            );
            return;
        }
        if self.cloud.connect.is_some() || self.cloud.preparing_connect {
            return;
        }
        self.cloud.preparing_connect = true;
        let configured: Vec<String> = self
            .config
            .cloud_drives
            .iter()
            .map(|d| d.remote.clone())
            .collect();
        self.cloud_spawn(move || {
            let remotes = cloud::list_remotes().unwrap_or_default();
            let existing = remotes
                .iter()
                .filter(|(name, kind)| kind == "drive" && !configured.contains(name))
                .map(|(name, _)| name.clone())
                .find(|name| cloud::remote_is_authorized(name));
            CloudEvent::ExistingRemote(existing, remotes)
        });
    }

    pub fn cloud_cancel_connect(&mut self) {
        if let Some(session) = self.cloud.connect.take() {
            session.cancel();
        }
        self.cloud.preparing_connect = false;
    }

    fn cloud_add_drive(&mut self, remote: String) {
        let mut drive = CloudDrive::google(remote.clone());
        // A second Google account gets its own folder.
        let base = drive.mount_point.clone();
        let mut n = 2;
        while self
            .config
            .cloud_drives
            .iter()
            .any(|d| d.mount_point == drive.mount_point)
        {
            drive.mount_point = base.with_file_name(format!("GoogleDrive{n}"));
            n += 1;
        }
        self.config.cloud_drives.push(drive);
        self.save_config();
        self.toast("Google Drive connected", ToastKind::Success);
        self.cloud_mount(&remote, true);
    }

    pub fn cloud_mount(&mut self, remote: &str, open_after: bool) {
        let Some(drive) = self.drive(remote) else {
            return;
        };
        if open_after {
            self.cloud.open_after_mount = Some(remote.to_string());
        }
        if self.cloud.mounted.contains(remote) {
            self.on_cloud_event(CloudEvent::Mounted {
                remote: remote.to_string(),
                result: Ok(()),
            });
            return;
        }
        if self.cloud.busy.contains_key(remote) {
            return;
        }
        self.cloud.busy.insert(remote.to_string(), "Mounting…");
        let remote = remote.to_string();
        self.cloud_spawn(move || CloudEvent::Mounted {
            result: cloud::mount(&drive).map_err(|e| e.to_string()),
            remote,
        });
    }

    pub fn cloud_unmount(&mut self, remote: &str) {
        let Some(drive) = self.drive(remote) else {
            return;
        };
        self.cloud.busy.insert(remote.to_string(), "Unmounting…");
        let remote = remote.to_string();
        self.cloud_spawn(move || CloudEvent::Unmounted {
            result: cloud::unmount(&drive.mount_point).map_err(|e| e.to_string()),
            remote,
        });
    }

    /// Forget the drive and sign out (deletes the rclone remote).
    pub fn cloud_disconnect(&mut self, remote: &str) {
        let Some(drive) = self.drive(remote) else {
            return;
        };
        self.config.cloud_drives.retain(|d| d.remote != remote);
        self.config.remove_pinned_folder(&drive.mount_point);
        self.save_config();
        self.cloud.mounted.remove(remote);
        self.cloud.quota.remove(remote);
        for tab in &mut self.tabs {
            if tab.folder().map(|f| drive.contains(f)).unwrap_or(false) {
                tab.navigate(Location::Dashboard);
            }
        }
        let ctx = self.ctx.clone();
        let remote = remote.to_string();
        std::thread::spawn(move || {
            if cloud::is_mounted(&drive.mount_point) {
                let _ = cloud::unmount(&drive.mount_point);
            }
            let _ = cloud::delete_remote(&remote);
            // Remove the (now empty) mount folder we created.
            let _ = std::fs::remove_dir(&drive.mount_point);
            ctx.request_repaint();
        });
        self.toast("Google Drive disconnected", ToastKind::Info);
    }

    /// Open the drive in the current tab, mounting it first if needed.
    pub fn cloud_open(&mut self, remote: &str) {
        if self.cloud.mounted.contains(remote) {
            if let Some(d) = self.drive(remote) {
                self.tab_mut().navigate(Location::Folder(d.mount_point));
            }
        } else {
            self.cloud_mount(remote, true);
        }
    }
}
