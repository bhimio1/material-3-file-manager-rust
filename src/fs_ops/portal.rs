#![allow(dead_code)]
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use url::Url;
use zbus::zvariant::{ObjectPath, OwnedValue, Value};
use zbus::{interface, ConnectionBuilder};

// Request sent from Portal Thread to Main Thread
#[derive(Debug)]
pub struct PortalRequest {
    pub mode: crate::ui_components::universal_picker_modal::PickerMode,
    pub response_channel: tokio::sync::oneshot::Sender<PortalResponse>,
}

// Response from Main Thread to Portal Thread
#[derive(Debug)]
pub enum PortalResponse {
    Selected(Vec<PathBuf>),
    Cancelled,
}

// Global channel for easy access across threads
static PORTAL_CHANNEL: OnceLock<(flume::Sender<PortalRequest>, flume::Receiver<PortalRequest>)> =
    OnceLock::new();

pub fn get_receiver() -> flume::Receiver<PortalRequest> {
    let (_, rx) = PORTAL_CHANNEL.get_or_init(flume::unbounded);
    rx.clone()
}

fn get_sender() -> flume::Sender<PortalRequest> {
    let (tx, _) = PORTAL_CHANNEL.get_or_init(flume::unbounded);
    tx.clone()
}

struct FileChooser;

#[interface(name = "org.freedesktop.impl.portal.FileChooser")]
impl FileChooser {
    async fn open_file(
        &self,
        _handle: ObjectPath<'_>,
        _app_id: String,
        _parent_window: String,
        title: String,
        _options: HashMap<String, Value<'_>>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        println!("Portal OpenFile request: {}", title);

        // TODO: parse "multiple" -> OpenFiles
        // TODO: parse "filters"

        // Default to OpenFile mode
        let mode =
            crate::ui_components::universal_picker_modal::PickerMode::OpenFile { filters: vec![] };

        let (tx, rx) = tokio::sync::oneshot::channel();

        if let Err(e) = get_sender().send(PortalRequest {
            mode,
            response_channel: tx,
        }) {
            eprintln!("Failed to send portal request to main thread: {}", e);
            return (2, HashMap::new()); // Error
        }

        match rx.await {
            Ok(PortalResponse::Selected(paths)) => {
                let uris: Vec<String> = paths
                    .iter()
                    .map(|p| {
                        // Convert to file:// URI
                        if let Ok(url) = Url::from_file_path(p) {
                            url.to_string()
                        } else {
                            // Fallback safe encoding
                            format!("file://{}", p.to_string_lossy())
                        }
                    })
                    .collect();

                let mut results = HashMap::new();
                let val = Value::from(uris);
                if let Ok(owned) = OwnedValue::try_from(val) {
                    results.insert("uris".to_string(), owned);
                }
                (0, results) // 0 = Success
            }
            Ok(PortalResponse::Cancelled) => (1, HashMap::new()), // 1 = Cancel
            Err(_) => (2, HashMap::new()),                        // 2 = Error (Channel closed)
        }
    }

    async fn save_file(
        &self,
        _handle: ObjectPath<'_>,
        _app_id: String,
        _parent_window: String,
        title: String,
        options: HashMap<String, Value<'_>>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        println!("Portal SaveFile request: {}", title);

        // Parse 'current_name' from options
        let current_name = if let Some(Value::Str(s)) = options.get("current_name") {
            s.to_string()
        } else {
            String::new()
        };

        let mode = crate::ui_components::universal_picker_modal::PickerMode::SaveFile {
            current_name,
            filters: vec![],
        };

        let (tx, rx) = tokio::sync::oneshot::channel();

        if let Err(_) = get_sender().send(PortalRequest {
            mode,
            response_channel: tx,
        }) {
            return (2, HashMap::new());
        }

        match rx.await {
            Ok(PortalResponse::Selected(paths)) => {
                let uris: Vec<String> = paths
                    .iter()
                    .map(|p| {
                        if let Ok(url) = Url::from_file_path(p) {
                            url.to_string()
                        } else {
                            format!("file://{}", p.to_string_lossy())
                        }
                    })
                    .collect();
                let mut results = HashMap::new();
                let val = Value::from(uris);
                if let Ok(owned) = OwnedValue::try_from(val) {
                    results.insert("uris".to_string(), owned);
                }
                (0, results)
            }
            Ok(PortalResponse::Cancelled) => (1, HashMap::new()),
            Err(_) => (2, HashMap::new()),
        }
    }
}

pub async fn start_portal_service() -> zbus::Result<()> {
    let _conn = ConnectionBuilder::session()?
        .name("org.freedesktop.impl.portal.desktop.material_3_file_manager")?
        .serve_at("/org/freedesktop/portal/desktop", FileChooser)?
        .build()
        .await?;

    // Keep running
    std::future::pending::<()>().await;
    Ok(())
}
