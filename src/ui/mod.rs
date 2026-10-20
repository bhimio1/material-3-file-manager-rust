//! User interface. Panels draw from app state and push [`Action`]s, which
//! the app applies after the frame is laid out.

pub mod dashboard;
pub mod details;
pub mod dialogs;
pub mod file_view;
pub mod icons;
pub mod settings;
pub mod sidebar;
pub mod status_bar;
pub mod tab_bar;
pub mod toasts;
pub mod toolbar;
pub mod widgets;

use crate::tab::Location;
use std::path::PathBuf;

/// Payload carried while dragging files inside the app.
#[derive(Clone, Debug)]
pub struct DragPaths(pub Vec<PathBuf>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

#[derive(Clone, Debug)]
pub enum Action {
    Navigate(Location),
    OpenInNewTab(Location),
    /// Folder → navigate, file → default application.
    Open(PathBuf),
    OpenWith(PathBuf),
    OpenInTerminal(PathBuf),
    OpenInEditor(PathBuf),
    Back,
    Forward,
    Up,
    Reload,

    NewTab,
    CloseTab(usize),
    ActivateTab(usize),

    Copy(Vec<PathBuf>),
    Cut(Vec<PathBuf>),
    Paste(PathBuf),
    Trash(Vec<PathBuf>),
    /// Restore items this app just moved to the trash.
    UndoTrash(Vec<PathBuf>),
    /// Asks for confirmation first.
    DeleteForever(Vec<PathBuf>),
    StartRename(PathBuf),
    CommitRename {
        path: PathBuf,
        name: String,
    },
    NewFolder(PathBuf),
    NewFile(PathBuf),
    MoveTo(Vec<PathBuf>),
    CopyTo(Vec<PathBuf>),
    Drop {
        paths: Vec<PathBuf>,
        target: PathBuf,
        copy: bool,
    },
    CancelJob(u64),

    Pin(PathBuf),
    Unpin(PathBuf),
    PickFolderToPin,
    ClearRecent,
    Properties(PathBuf),
    CopyPathText(Vec<PathBuf>),

    /// The config was edited in place; persist it and refresh views.
    ConfigChanged,
    ReloadTheme,
    ShowShortcuts,
    SelectAll,
    Toast(String, ToastKind),
    ToggleDetails,
    FocusSearch,
    EditPath,
}

pub type Actions = Vec<Action>;
