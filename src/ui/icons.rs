//! Icons and accent colors per file type.

use crate::fs::{FileEntry, FileKind};
use egui::Color32;
use material_3_ui::icons::{icons::*, MaterialIcon};
use material_3_ui::theme::palette::blend;
use material_3_ui::theme::M3Palette;

pub fn kind_icon(kind: FileKind) -> MaterialIcon {
    match kind {
        FileKind::Folder => ICON_FOLDER,
        FileKind::Image => ICON_IMAGE,
        FileKind::Video => ICON_MOVIE,
        FileKind::Audio => ICON_MUSIC_NOTE,
        FileKind::Pdf => ICON_PICTURE_AS_PDF,
        FileKind::Document => ICON_DESCRIPTION,
        FileKind::Spreadsheet => ICON_TABLE_CHART,
        FileKind::Presentation => ICON_SLIDESHOW,
        FileKind::Archive => ICON_FOLDER_ZIP,
        FileKind::Code => ICON_CODE,
        FileKind::Text => ICON_ARTICLE,
        FileKind::Executable => ICON_TERMINAL,
        FileKind::DiskImage => ICON_ALBUM,
        FileKind::Font => ICON_FONT_DOWNLOAD,
        FileKind::Other => ICON_DRAFT,
    }
}

pub fn entry_icon(entry: &FileEntry) -> MaterialIcon {
    if entry.is_dir {
        crate::fs::places::folder_icon(&entry.path)
    } else {
        kind_icon(entry.kind)
    }
}

/// A hue per type, harmonized towards the theme's primary color and
/// lightened in dark mode so it keeps contrast.
pub fn kind_color(kind: FileKind, p: &M3Palette) -> Color32 {
    let base = match kind {
        FileKind::Folder => return p.primary,
        FileKind::Image => Color32::from_rgb(0x1E, 0x8E, 0x3E),
        FileKind::Video => Color32::from_rgb(0xD9, 0x30, 0x25),
        FileKind::Audio => Color32::from_rgb(0x93, 0x34, 0xE6),
        FileKind::Pdf => Color32::from_rgb(0xC5, 0x22, 0x1F),
        FileKind::Document => Color32::from_rgb(0x1A, 0x73, 0xE8),
        FileKind::Spreadsheet => Color32::from_rgb(0x18, 0x80, 0x38),
        FileKind::Presentation => Color32::from_rgb(0xE3, 0x74, 0x00),
        FileKind::Archive => Color32::from_rgb(0xA8, 0x6B, 0x00),
        FileKind::Code => Color32::from_rgb(0x00, 0x89, 0x7B),
        FileKind::Font => Color32::from_rgb(0xC2, 0x18, 0x5B),
        FileKind::Executable | FileKind::DiskImage => Color32::from_rgb(0x5F, 0x63, 0x68),
        FileKind::Text | FileKind::Other => return p.on_surface_variant,
    };
    let harmonized = blend(base, p.primary, 0.15);
    if p.is_dark() {
        blend(harmonized, Color32::WHITE, 0.35)
    } else {
        harmonized
    }
}
