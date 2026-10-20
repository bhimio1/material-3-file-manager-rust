//! An `org.freedesktop.impl.portal.FileChooser` backend, so other apps'
//! open/save dialogs can be served by this file manager.
//!
//! Started with `m3fm --portal`. Requests are forwarded to the UI thread,
//! which shows its picker dialog and answers through a one-shot channel.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use zbus::interface;
use zbus::zvariant::{ObjectPath, OwnedValue, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct Filter {
    pub name: String,
    /// Lowercase glob suffixes such as `*.png`, or `*` for everything.
    pub patterns: Vec<String>,
}

impl Filter {
    pub fn matches(&self, file_name: &str) -> bool {
        let lower = file_name.to_lowercase();
        self.patterns
            .iter()
            .any(|p| p == "*" || lower.ends_with(p.trim_start_matches('*')))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PickKind {
    Open { multiple: bool },
    Folder { multiple: bool },
    Save { current_name: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PickerSpec {
    pub title: String,
    pub kind: PickKind,
    pub filters: Vec<Filter>,
    pub start_dir: Option<PathBuf>,
    pub accept_label: Option<String>,
}

#[derive(Debug)]
pub enum PortalResponse {
    Selected(Vec<PathBuf>),
    Cancelled,
}

#[derive(Debug)]
pub struct PortalRequest {
    pub spec: PickerSpec,
    pub respond: flume::Sender<PortalResponse>,
}

static CHANNEL: OnceLock<(flume::Sender<PortalRequest>, flume::Receiver<PortalRequest>)> =
    OnceLock::new();
static WAKE: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

fn channel() -> &'static (flume::Sender<PortalRequest>, flume::Receiver<PortalRequest>) {
    CHANNEL.get_or_init(flume::unbounded)
}

pub fn requests() -> flume::Receiver<PortalRequest> {
    channel().1.clone()
}

fn bool_opt(options: &HashMap<String, Value<'_>>, key: &str) -> bool {
    matches!(options.get(key), Some(Value::Bool(true)))
}

fn str_opt(options: &HashMap<String, Value<'_>>, key: &str) -> Option<String> {
    match options.get(key) {
        Some(Value::Str(s)) => Some(s.to_string()),
        _ => None,
    }
}

/// `ay` byte strings (nul-terminated paths).
fn bytes_opt(options: &HashMap<String, Value<'_>>, key: &str) -> Option<PathBuf> {
    let Some(Value::Array(arr)) = options.get(key) else {
        return None;
    };
    let bytes: Vec<u8> = arr
        .iter()
        .filter_map(|v| match v {
            Value::U8(b) => Some(*b),
            _ => None,
        })
        .take_while(|b| *b != 0)
        .collect();
    let s = String::from_utf8(bytes).ok()?;
    (!s.is_empty()).then(|| PathBuf::from(s))
}

/// `a(sa(us))`: name plus (kind, pattern) pairs; kind 0 = glob, 1 = mime.
fn parse_filters(options: &HashMap<String, Value<'_>>) -> Vec<Filter> {
    let Some(Value::Array(list)) = options.get("filters") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in list.iter() {
        let Value::Structure(s) = item else { continue };
        let fields = s.fields();
        let (Some(Value::Str(name)), Some(Value::Array(pats))) = (fields.first(), fields.get(1))
        else {
            continue;
        };
        let mut patterns = Vec::new();
        for p in pats.iter() {
            let Value::Structure(ps) = p else { continue };
            match ps.fields() {
                [Value::U32(0), Value::Str(glob)] => patterns.push(glob.to_lowercase()),
                [Value::U32(1), Value::Str(mime)] => {
                    if let Some(exts) = mime_guess::get_mime_extensions_str(mime) {
                        patterns.extend(exts.iter().map(|e| format!("*.{e}")));
                    } else if mime.ends_with("/*") {
                        patterns.push("*".into());
                    }
                }
                _ => {}
            }
        }
        if !patterns.is_empty() {
            out.push(Filter {
                name: name.to_string(),
                patterns,
            });
        }
    }
    out
}

async fn ask(spec: PickerSpec) -> (u32, HashMap<String, OwnedValue>) {
    let (tx, rx) = flume::bounded(1);
    if channel()
        .0
        .send(PortalRequest { spec, respond: tx })
        .is_err()
    {
        return (2, HashMap::new());
    }
    if let Some(wake) = WAKE.get() {
        wake();
    }
    match rx.recv_async().await {
        Ok(PortalResponse::Selected(paths)) => {
            let uris: Vec<String> = paths
                .iter()
                .map(|p| {
                    url::Url::from_file_path(p)
                        .map(|u| u.to_string())
                        .unwrap_or_else(|_| format!("file://{}", p.display()))
                })
                .collect();
            let mut results = HashMap::new();
            if let Ok(v) = OwnedValue::try_from(Value::from(uris)) {
                results.insert("uris".to_string(), v);
            }
            (0, results)
        }
        Ok(PortalResponse::Cancelled) => (1, HashMap::new()),
        Err(_) => (2, HashMap::new()),
    }
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
        options: HashMap<String, Value<'_>>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        let multiple = bool_opt(&options, "multiple");
        let kind = if bool_opt(&options, "directory") {
            PickKind::Folder { multiple }
        } else {
            PickKind::Open { multiple }
        };
        ask(PickerSpec {
            title,
            kind,
            filters: parse_filters(&options),
            start_dir: bytes_opt(&options, "current_folder"),
            accept_label: str_opt(&options, "accept_label"),
        })
        .await
    }

    async fn save_file(
        &self,
        _handle: ObjectPath<'_>,
        _app_id: String,
        _parent_window: String,
        title: String,
        options: HashMap<String, Value<'_>>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        ask(PickerSpec {
            title,
            kind: PickKind::Save {
                current_name: str_opt(&options, "current_name").unwrap_or_default(),
            },
            filters: parse_filters(&options),
            start_dir: bytes_opt(&options, "current_folder"),
            accept_label: str_opt(&options, "accept_label"),
        })
        .await
    }
}

/// Register on the session bus from a background thread.
pub fn start(wake: impl Fn() + Send + Sync + 'static) {
    let _ = WAKE.set(Box::new(wake));
    std::thread::Builder::new()
        .name("m3fm-portal".into())
        .spawn(|| {
            let result: zbus::Result<()> = zbus::block_on(async {
                let _conn = zbus::connection::Builder::session()?
                    .name("org.freedesktop.impl.portal.desktop.material_3_file_manager")?
                    .serve_at("/org/freedesktop/portal/desktop", FileChooser)?
                    .build()
                    .await?;
                std::future::pending::<()>().await;
                Ok(())
            });
            if let Err(e) = result {
                log::error!("File chooser portal failed to start: {e}");
            }
        })
        .expect("spawn portal thread");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_matching() {
        let f = Filter {
            name: "Images".into(),
            patterns: vec!["*.png".into(), "*.jpg".into()],
        };
        assert!(f.matches("A.PNG"));
        assert!(!f.matches("a.gif"));
        assert!(Filter {
            name: "All".into(),
            patterns: vec!["*".into()]
        }
        .matches("x"));
    }
}
