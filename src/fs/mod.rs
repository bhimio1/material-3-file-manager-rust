//! Filesystem layer: listing, searching, file operations and integration
//! with the desktop (applications, drives, portal, change watching).

pub mod apps;
pub mod cloud;
pub mod entry;
pub mod launch;
pub mod ops;
pub mod places;
pub mod portal;
pub mod scanner;
pub mod watcher;

pub use entry::{FileEntry, FileKind};
