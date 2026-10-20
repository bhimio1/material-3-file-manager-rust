//! The folder contents: virtualized grid or list, optionally grouped by
//! type, with selection, rubber-band, drag-and-drop and inline rename.

use super::icons::{entry_icon, kind_color};
use super::widgets::{self, menu_item, paint_icon, ButtonKind};
use super::{Action, DragPaths};
use crate::app::App;
use crate::config::{Config, SortKey, ViewMode};
use crate::fs::entry::{format_size, format_time};
use crate::fs::FileEntry;
use crate::tab::{Location, Tab};
use crate::theme::M3Palette;
use crate::thumbs::{Thumb, Thumbnails};
use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontId, Id, Key, Modifiers, Pos2, Rect, RichText,
    Sense, Stroke, StrokeKind, Ui, Vec2,
};
use egui_material_icons::icons::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const PAD: f32 = 12.0;
const GAP: f32 = 6.0;
const GROUP_HEADER_H: f32 = 44.0;
const LIST_ROW_H: f32 = 40.0;
const LIST_ROW_H_TALL: f32 = 52.0;
const LIST_HEADER_H: f32 = 36.0;

#[derive(Clone, Copy, Debug)]
enum Row {
    Header(usize),
    Items { start: usize, end: usize },
}

struct Layout {
    rows: Vec<Row>,
    offsets: Vec<f32>,
    heights: Vec<f32>,
    total: f32,
    grid: bool,
    cols: usize,
    tile: Vec2,
}

impl Layout {
    fn build(tab: &Tab, grid: bool, width: f32, icon: f32, tall_rows: bool) -> Self {
        let inner_w = (width - PAD * 2.0).max(60.0);
        let (cols, tile) = if grid {
            let min_w = (icon + 44.0).max(92.0);
            let cols = (((inner_w + GAP) / (min_w + GAP)).floor() as usize).max(1);
            let w = (inner_w - GAP * (cols as f32 - 1.0)) / cols as f32;
            (cols, vec2(w, icon + 62.0))
        } else {
            (
                1,
                vec2(
                    inner_w,
                    if tall_rows {
                        LIST_ROW_H_TALL
                    } else {
                        LIST_ROW_H
                    },
                ),
            )
        };
        let row_h = if grid { tile.y + GAP } else { tile.y };

        let mut layout = Layout {
            rows: Vec::new(),
            offsets: Vec::new(),
            heights: Vec::new(),
            total: PAD / 2.0,
            grid,
            cols,
            tile,
        };
        let push = |l: &mut Layout, row: Row, h: f32| {
            l.offsets.push(l.total);
            l.heights.push(h);
            l.rows.push(row);
            l.total += h;
        };
        let add_items = |l: &mut Layout, start: usize, end: usize| {
            let mut i = start;
            while i < end {
                let e = (i + cols).min(end);
                push(l, Row::Items { start: i, end: e }, row_h);
                i = e;
            }
        };
        if tab.groups.is_empty() {
            add_items(&mut layout, 0, tab.view.len());
        } else {
            for (gi, g) in tab.groups.iter().enumerate() {
                push(&mut layout, Row::Header(gi), GROUP_HEADER_H);
                add_items(&mut layout, g.start, g.start + g.len);
            }
        }
        layout.total += PAD;
        layout
    }

    /// Index of the first row intersecting `y`.
    fn row_at(&self, y: f32) -> usize {
        match self
            .offsets
            .binary_search_by(|o| o.partial_cmp(&y).unwrap())
        {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        }
    }

    /// Rect of item `idx` relative to the content origin.
    fn item_rect(&self, idx: usize) -> Option<Rect> {
        let row = self
            .rows
            .iter()
            .position(|r| matches!(r, Row::Items { start, end } if idx >= *start && idx < *end))?;
        let Row::Items { start, .. } = self.rows[row] else {
            return None;
        };
        Some(self.slot_rect(row, idx - start))
    }

    fn slot_rect(&self, row: usize, col: usize) -> Rect {
        let y = self.offsets[row];
        if self.grid {
            Rect::from_min_size(pos2(PAD + col as f32 * (self.tile.x + GAP), y), self.tile)
        } else {
            Rect::from_min_size(pos2(PAD, y), vec2(self.tile.x, self.heights[row] - 2.0))
        }
    }
}

/// x positions of list columns inside a row rect.
struct Columns {
    name: (f32, f32),
    size: Option<f32>,
    kind: Option<f32>,
    modified: Option<f32>,
}

impl Columns {
    fn new(row: Rect) -> Self {
        // Columns are carved off the right edge; narrow windows drop the
        // less important ones first.
        let w = row.width();
        let mut right = row.right() - 4.0;
        let mut take = |show: bool, width: f32| {
            show.then(|| {
                right -= width;
                right
            })
        };
        let modified = take(w > 520.0, 170.0);
        let kind = take(w > 680.0, 140.0);
        let size = take(w > 380.0, 96.0);
        Columns {
            name: (row.left() + 48.0, right - 12.0),
            size,
            kind,
            modified,
        }
    }
}

struct RubberBand {
    start: Pos2,
    base: HashSet<PathBuf>,
}

pub fn show(ui: &mut Ui, app: &mut App) {
    let p = app.palette.clone();
    let cut: HashSet<PathBuf> = app
        .clipboard
        .as_ref()
        .filter(|c| c.op == crate::app::ClipOp::Cut)
        .map(|c| c.paths.iter().cloned().collect())
        .unwrap_or_default();
    let has_clipboard = app.clipboard.is_some();
    let dialog_open = app.dialog.is_some();
    let App {
        tabs,
        active,
        thumbs,
        config,
        actions,
        ..
    } = app;
    let tab = &mut tabs[*active];
    let Some(folder) = tab.folder().map(Path::to_path_buf) else {
        return;
    };

    let grid = config.ui.view_mode == ViewMode::Grid;
    let tall_rows = tab.search.deep.is_some();
    let full = ui.available_rect_before_wrap();

    // Ctrl+scroll resizes grid icons.
    if ui.rect_contains_pointer(full) {
        let zoom = ui.input(|i| i.zoom_delta());
        if zoom != 1.0 {
            let size = (config.ui.icon_size as f32 * zoom)
                .clamp(32.0, 160.0)
                .round() as u32;
            if size != config.ui.icon_size {
                config.ui.icon_size = size;
                actions.push(Action::ConfigChanged);
            }
        }
    }

    if !grid {
        list_header(ui, config, actions, &p, full.width());
    }

    if tab.view.is_empty() && tab.groups.is_empty() {
        let bg = ui.interact(
            ui.available_rect_before_wrap(),
            Id::unique("empty-bg"),
            Sense::click(),
        );
        bg.context_menu(|ui| background_menu(ui, &folder, has_clipboard, config, actions));
        empty_state(ui, tab, &folder, actions, &p);
        return;
    }

    let icon_size = config.ui.icon_size as f32;
    let layout = Layout::build(tab, grid, ui.available_width(), icon_size, tall_rows);
    let keyboard_ok = !dialog_open && tab.rename.is_none() && !ui.ctx().egui_wants_keyboard_input();
    let mut visible_rows = 1usize;

    let scroll = egui::ScrollArea::vertical()
        .id_salt(("files", &folder))
        .auto_shrink([false, false])
        .show_viewport(ui, |ui, viewport| {
            let origin = ui.max_rect().min;
            let height = layout.total.max(viewport.height());
            ui.set_height(height);
            let content = Rect::from_min_size(origin, vec2(ui.available_width(), height));
            visible_rows = ((viewport.height() / layout.tile.y.max(1.0)) as usize).max(1);

            // Background: click to clear, drag for rubber-band, menu.
            let bg = ui.interact(
                content,
                Id::unique("files-bg").with(&folder),
                Sense::click_and_drag(),
            );
            if bg.clicked() && !ui.input(|i| i.modifiers.command || i.modifiers.shift) {
                tab.clear_selection();
                if tab.rename.is_some() {
                    commit_rename(tab, actions);
                }
            }
            bg.context_menu(|ui| background_menu(ui, &folder, has_clipboard, config, actions));
            rubber_band(ui, &bg, tab, &layout, origin, &p);

            // Visible rows only.
            let first = layout.row_at(viewport.min.y);
            for row in first..layout.rows.len() {
                if layout.offsets[row] > viewport.max.y {
                    break;
                }
                match layout.rows[row] {
                    Row::Header(gi) => {
                        let r = Rect::from_min_size(
                            origin + vec2(PAD, layout.offsets[row]),
                            vec2(content.width() - PAD * 2.0, GROUP_HEADER_H),
                        );
                        group_header(ui, tab, gi, r, &p);
                    }
                    Row::Items { start, end } => {
                        for idx in start..end {
                            let r = layout
                                .slot_rect(row, idx - start)
                                .translate(origin.to_vec2());
                            item(
                                ui,
                                tab,
                                thumbs,
                                config,
                                idx,
                                r,
                                grid,
                                &cut,
                                has_clipboard,
                                actions,
                                &p,
                            );
                        }
                    }
                }
            }

            // Drops on empty space go into this folder. Checked after the
            // items so a drop on a folder row lands in that folder instead.
            if let Some(payload) = bg.dnd_release_payload::<DragPaths>() {
                actions.push(Action::Drop {
                    paths: payload.0.clone(),
                    target: folder.clone(),
                    copy: ui.input(|i| i.modifiers.command),
                });
            }

            if let Some(path) = tab.scroll_to.take() {
                if let Some(r) = tab.index_of(&path).and_then(|i| layout.item_rect(i)) {
                    let r = r.translate(origin.to_vec2()).expand(6.0);
                    ui.scroll_to_rect(r, None);
                }
            }

            // Auto-scroll while dragging near the edges.
            if ui.ctx().dragged_id().is_some() {
                if let Some(pos) = ui.ctx().pointer_interact_pos() {
                    let screen = ui.clip_rect();
                    if pos.y < screen.top() + 32.0
                        && pos.x > screen.left()
                        && pos.x < screen.right()
                    {
                        ui.scroll_with_delta(vec2(0.0, 12.0));
                    } else if pos.y > screen.bottom() - 32.0
                        && pos.x > screen.left()
                        && pos.x < screen.right()
                    {
                        ui.scroll_with_delta(vec2(0.0, -12.0));
                    }
                }
            }
        });
    let _ = scroll;

    if keyboard_ok {
        keyboard(ui, tab, &layout, visible_rows);
    }
}

fn keyboard(ui: &Ui, tab: &mut Tab, layout: &Layout, visible_rows: usize) {
    let cols = layout.cols as isize;
    let page = cols * visible_rows as isize;
    let mut moves: Vec<(isize, bool)> = Vec::new();
    ui.input_mut(|i| {
        // Shift variants first: a no-modifier pattern also matches Shift+key,
        // and the event's own modifiers are what count (not the state at the
        // end of the frame).
        for (m, extend) in [(Modifiers::SHIFT, true), (Modifiers::NONE, false)] {
            let mut key = |k: Key, delta: isize| {
                while i.consume_key(m, k) {
                    moves.push((delta, extend));
                }
            };
            key(Key::ArrowDown, cols);
            key(Key::ArrowUp, -cols);
            if layout.grid {
                key(Key::ArrowRight, 1);
                key(Key::ArrowLeft, -1);
            }
            key(Key::PageDown, page);
            key(Key::PageUp, -page);
            key(Key::Home, isize::MIN / 2);
            key(Key::End, isize::MAX / 2);
        }
        if i.consume_key(Modifiers::NONE, Key::Escape) {
            if tab.search.is_active() {
                tab.clear_search();
            } else {
                tab.clear_selection();
            }
        }
    });
    for (delta, extend) in moves {
        tab.move_cursor(delta, extend);
    }
}

fn list_header(
    ui: &mut Ui,
    config: &mut Config,
    actions: &mut Vec<Action>,
    p: &M3Palette,
    width: f32,
) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, LIST_HEADER_H), Sense::hover());
    let row = Rect::from_min_max(
        pos2(rect.left() + PAD, rect.top()),
        pos2(rect.right() - PAD, rect.bottom()),
    );
    let cols = Columns::new(row);
    let mut header = |x: f32, w: f32, label: &str, key: SortKey, right: bool| {
        let r = Rect::from_min_size(
            pos2(x - 8.0, row.top() + 4.0),
            vec2(w + 16.0, row.height() - 8.0),
        );
        let resp = ui.interact(r, Id::unique(("col", label)), Sense::click());
        let active = config.ui.sort_by == key;
        if resp.hovered() {
            ui.painter()
                .rect_filled(r, CornerRadius::same(8), p.on_surface.gamma_multiply(0.06));
        }
        let color = if active {
            p.primary
        } else {
            p.on_surface_variant
        };
        let anchor = if right {
            Align2::RIGHT_CENTER
        } else {
            Align2::LEFT_CENTER
        };
        let tx = if right { x + w } else { x };
        let text_rect = ui.painter().text(
            pos2(tx, r.center().y),
            anchor,
            label,
            widgets::title_font(12.5),
            color,
        );
        if active {
            let icon = if config.ui.sort_ascending {
                ICON_ARROW_UPWARD
            } else {
                ICON_ARROW_DOWNWARD
            };
            let ix = if right {
                text_rect.left() - 10.0
            } else {
                text_rect.right() + 10.0
            };
            paint_icon(ui, icon, pos2(ix, r.center().y), 14.0, color);
        }
        if resp
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            if active {
                config.ui.sort_ascending = !config.ui.sort_ascending;
            } else {
                config.ui.sort_by = key;
                config.ui.sort_ascending = matches!(key, SortKey::Name | SortKey::Type);
            }
            actions.push(Action::ConfigChanged);
        }
    };
    header(cols.name.0, 120.0, "Name", SortKey::Name, false);
    if let Some(x) = cols.size {
        header(x, 80.0, "Size", SortKey::Size, true);
    }
    if let Some(x) = cols.kind {
        header(x + 12.0, 110.0, "Type", SortKey::Type, false);
    }
    if let Some(x) = cols.modified {
        header(x + 12.0, 140.0, "Modified", SortKey::Modified, false);
    }
    ui.painter().hline(
        row.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, p.outline_variant),
    );
}

fn group_header(ui: &mut Ui, tab: &mut Tab, gi: usize, rect: Rect, p: &M3Palette) {
    let group = tab.groups[gi].clone();
    let resp = ui.interact(rect, Id::unique(("group", &group.name)), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(vec2(0.0, 4.0)),
            CornerRadius::same(12),
            p.on_surface.gamma_multiply(0.05),
        );
    }
    let icon = if group.collapsed {
        ICON_CHEVRON_RIGHT
    } else {
        ICON_EXPAND_MORE
    };
    paint_icon(
        ui,
        icon,
        pos2(rect.left() + 16.0, rect.center().y),
        20.0,
        p.on_surface_variant,
    );
    let title = ui.painter().text(
        pos2(rect.left() + 34.0, rect.center().y),
        Align2::LEFT_CENTER,
        &group.name,
        widgets::title_font(15.0),
        p.on_surface,
    );
    // Count badge.
    let badge_text = group.total.to_string();
    let galley = ui.painter().layout_no_wrap(
        badge_text,
        FontId::proportional(11.5),
        p.on_secondary_container,
    );
    let badge = Rect::from_min_size(
        pos2(title.right() + 10.0, rect.center().y - 10.0),
        vec2(galley.size().x + 14.0, 20.0),
    );
    ui.painter()
        .rect_filled(badge, CornerRadius::same(10), p.secondary_container);
    ui.painter().galley(
        badge.center() - galley.size() / 2.0,
        galley,
        p.on_secondary_container,
    );
    ui.painter().hline(
        (badge.right() + 12.0)..=rect.right(),
        rect.center().y,
        Stroke::new(1.0, p.outline_variant.gamma_multiply(0.6)),
    );
    if resp
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        tab.toggle_group(&group.name);
    }
}

#[allow(clippy::too_many_arguments)]
fn item(
    ui: &mut Ui,
    tab: &mut Tab,
    thumbs: &mut Thumbnails,
    config: &Config,
    idx: usize,
    rect: Rect,
    grid: bool,
    cut: &HashSet<PathBuf>,
    has_clipboard: bool,
    actions: &mut Vec<Action>,
    p: &M3Palette,
) {
    let entry = tab.view[idx].clone();
    let id = Id::unique(("entry", &entry.path));
    let resp = ui.interact(rect, id, Sense::CLICK | Sense::DRAG);
    let selected = tab.selection.contains(&entry.path);
    let renaming = tab
        .rename
        .as_ref()
        .map(|r| r.path == entry.path)
        .unwrap_or(false);

    // ---- pointer interaction
    let mods = ui.input(|i| i.modifiers);
    if resp.clicked() {
        if mods.shift {
            tab.select_range(&entry.path, mods.command);
        } else if mods.command {
            tab.toggle(&entry.path);
        } else {
            tab.select_only(&entry.path);
        }
        if tab
            .rename
            .as_ref()
            .map(|r| r.path != entry.path)
            .unwrap_or(false)
        {
            commit_rename(tab, actions);
        }
    }
    if resp.double_clicked() && !renaming {
        actions.push(Action::Open(entry.path.clone()));
    }
    if resp.middle_clicked() && entry.is_dir {
        actions.push(Action::OpenInNewTab(Location::Folder(entry.path.clone())));
    }
    if resp.secondary_clicked() && !selected {
        tab.select_only(&entry.path);
    }
    if resp.drag_started() && !renaming {
        if !tab.selection.contains(&entry.path) {
            tab.select_only(&entry.path);
        }
        egui::DragAndDrop::set_payload(ui.ctx(), DragPaths(tab.selected_paths()));
    }
    let mut drop_hover = false;
    if entry.is_dir {
        if let Some(payload) = resp.dnd_hover_payload::<DragPaths>() {
            drop_hover = !payload.0.contains(&entry.path);
        }
        if let Some(payload) = resp.dnd_release_payload::<DragPaths>() {
            if !payload.0.contains(&entry.path) {
                actions.push(Action::Drop {
                    paths: payload.0.clone(),
                    target: entry.path.clone(),
                    copy: mods.command,
                });
            }
        }
    }
    let selected = tab.selection.contains(&entry.path);

    // ---- painting
    if ui.is_rect_visible(rect) {
        let hovered = resp.hovered() || resp.is_pointer_button_down_on();
        let radius = CornerRadius::same(if grid { 16 } else { 12 });
        let fill = if drop_hover {
            p.primary_container
        } else if selected {
            widgets::state_layer(p.secondary_container, p.on_secondary_container, &resp)
        } else if hovered {
            p.on_surface.gamma_multiply(0.06)
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, radius, fill);
        if drop_hover {
            ui.painter().rect_stroke(
                rect,
                radius,
                Stroke::new(2.0, p.primary),
                StrokeKind::Inside,
            );
        }
        if tab.cursor.as_ref() == Some(&entry.path) && selected && tab.selection.len() > 1 {
            ui.painter().rect_stroke(
                rect,
                radius,
                Stroke::new(1.0, p.on_secondary_container.gamma_multiply(0.5)),
                StrokeKind::Inside,
            );
        }
        let opacity = if cut.contains(&entry.path) { 0.45 } else { 1.0 };
        let text_color = if selected {
            p.on_secondary_container
        } else {
            p.on_surface
        };
        if grid {
            paint_tile(
                ui, &entry, rect, thumbs, config, text_color, opacity, renaming, p,
            );
        } else {
            paint_row(
                ui,
                &entry,
                rect,
                thumbs,
                config,
                text_color,
                opacity,
                renaming,
                tab.search.deep.is_some(),
                tab.folder(),
                p,
            );
        }
    }

    if renaming {
        let name_rect = if grid {
            let icon = config.ui.icon_size as f32;
            Rect::from_min_size(
                pos2(rect.left() + 4.0, rect.top() + icon + 18.0),
                vec2(rect.width() - 8.0, 28.0),
            )
        } else {
            let cols = Columns::new(rect);
            Rect::from_min_max(
                pos2(cols.name.0 - 6.0, rect.center().y - 14.0),
                pos2(cols.name.1, rect.center().y + 14.0),
            )
        };
        rename_editor(ui, tab, name_rect, actions, p);
    }

    let tooltip_entry = entry.clone();
    let resp = resp.on_hover_ui_at_pointer(|ui| {
        ui.label(&tooltip_entry.name);
        let detail = if tooltip_entry.is_dir {
            format_time(tooltip_entry.modified)
        } else {
            format!(
                "{} · {}",
                format_size(tooltip_entry.size),
                format_time(tooltip_entry.modified)
            )
        };
        ui.label(RichText::new(detail).size(12.0));
        if let Some(parent) = tooltip_entry.path.parent() {
            ui.label(RichText::new(parent.to_string_lossy()).size(11.5).weak());
        }
    });

    let targets: Vec<PathBuf> = if tab.selection.contains(&entry.path) {
        tab.selected_paths()
    } else {
        vec![entry.path.clone()]
    };
    resp.context_menu(|ui| item_menu(ui, &entry, &targets, has_clipboard, config, actions));
}

#[allow(clippy::too_many_arguments)]
fn paint_tile(
    ui: &Ui,
    entry: &FileEntry,
    rect: Rect,
    thumbs: &mut Thumbnails,
    config: &Config,
    text_color: Color32,
    opacity: f32,
    renaming: bool,
    p: &M3Palette,
) {
    let icon = config.ui.icon_size as f32;
    let icon_rect = Rect::from_center_size(
        pos2(rect.center().x, rect.top() + 10.0 + icon / 2.0),
        Vec2::splat(icon),
    );
    paint_entry_visual(ui, entry, icon_rect, thumbs, config, opacity, p);

    if !renaming {
        let galley = widgets::wrapped_galley(
            ui,
            &entry.name,
            FontId::proportional(13.0),
            text_color.gamma_multiply(opacity),
            rect.width() - 14.0,
            2,
        );
        // Center the (possibly two-line) label under the icon.
        let pos = pos2(
            rect.center().x - galley.rect.center().x,
            icon_rect.bottom() + 8.0,
        );
        ui.painter().galley(pos, galley, text_color);
    }
}

/// Icon or thumbnail for an entry, fit into `rect`.
fn paint_entry_visual(
    ui: &Ui,
    entry: &FileEntry,
    rect: Rect,
    thumbs: &mut Thumbnails,
    config: &Config,
    opacity: f32,
    p: &M3Palette,
) {
    let px = rect.width() * ui.ctx().pixels_per_point();
    if config.ui.show_thumbnails && entry.kind.has_thumbnail() && rect.width() >= 20.0 {
        match thumbs.get(&entry.path, px, entry.modified) {
            Thumb::Ready(tex) => {
                let size = tex.size_vec2();
                let scale = (rect.width() / size.x).min(rect.height() / size.y);
                let r = Rect::from_center_size(rect.center(), size * scale);
                egui::Image::new(&tex)
                    .corner_radius(CornerRadius::same(if rect.width() > 40.0 { 8 } else { 4 }))
                    .tint(Color32::WHITE.gamma_multiply(opacity))
                    .paint_at(ui, r);
                ui.painter().rect_stroke(
                    r,
                    CornerRadius::same(if rect.width() > 40.0 { 8 } else { 4 }),
                    Stroke::new(1.0, p.outline_variant.gamma_multiply(0.6)),
                    StrokeKind::Outside,
                );
                return;
            }
            Thumb::Loading => {
                ui.painter().rect_filled(
                    rect.shrink(rect.width() * 0.12),
                    CornerRadius::same(8),
                    p.surface_container_high.gamma_multiply(opacity),
                );
                return;
            }
            Thumb::Unavailable => {}
        }
    }
    let color = kind_color(entry.kind, p).gamma_multiply(opacity);
    let glyph = if entry.is_dir {
        rect.width()
    } else {
        rect.width() * 0.86
    };
    paint_icon(ui, entry_icon(entry), rect.center(), glyph, color);
    if entry.is_symlink {
        let badge = pos2(
            rect.right() - rect.width() * 0.12,
            rect.bottom() - rect.height() * 0.12,
        );
        let r = (rect.width() * 0.16).max(7.0);
        ui.painter()
            .circle_filled(badge, r, p.surface_container_highest);
        paint_icon(ui, ICON_LINK, badge, r * 1.4, p.on_surface_variant);
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_row(
    ui: &Ui,
    entry: &FileEntry,
    rect: Rect,
    thumbs: &mut Thumbnails,
    config: &Config,
    text_color: Color32,
    opacity: f32,
    renaming: bool,
    search: bool,
    folder: Option<&Path>,
    p: &M3Palette,
) {
    let cols = Columns::new(rect);
    let icon_rect = Rect::from_center_size(
        pos2(rect.left() + 24.0, rect.center().y),
        Vec2::splat(if search { 30.0 } else { 24.0 }),
    );
    paint_entry_visual(ui, entry, icon_rect, thumbs, config, opacity, p);
    let weak = p.on_surface_variant.gamma_multiply(opacity);
    let fg = text_color.gamma_multiply(opacity);
    if !renaming {
        let name_w = cols.name.1 - cols.name.0;
        let name =
            widgets::truncated_galley(ui, &entry.name, FontId::proportional(14.0), fg, name_w);
        if search {
            let parent = entry.path.parent().unwrap_or(Path::new("/"));
            let rel = folder
                .and_then(|f| parent.strip_prefix(f).ok())
                .map(|r| {
                    if r.as_os_str().is_empty() {
                        "This folder".to_string()
                    } else {
                        r.to_string_lossy().into_owned()
                    }
                })
                .unwrap_or_else(|| parent.to_string_lossy().into_owned());
            let sub = widgets::truncated_galley(ui, &rel, FontId::proportional(12.0), weak, name_w);
            ui.painter()
                .galley(pos2(cols.name.0, rect.center().y - name.size().y), name, fg);
            ui.painter()
                .galley(pos2(cols.name.0, rect.center().y + 1.0), sub, weak);
        } else {
            ui.painter().galley(
                pos2(cols.name.0, rect.center().y - name.size().y / 2.0),
                name,
                fg,
            );
        }
    }
    let small = FontId::proportional(13.0);
    if let Some(x) = cols.size {
        let text = if entry.is_dir {
            "—".to_string()
        } else {
            format_size(entry.size)
        };
        ui.painter().text(
            pos2(x + 80.0, rect.center().y),
            Align2::RIGHT_CENTER,
            text,
            small.clone(),
            weak,
        );
    }
    if let Some(x) = cols.kind {
        let g = widgets::truncated_galley(ui, &entry.type_label(), small.clone(), weak, 124.0);
        ui.painter()
            .galley(pos2(x + 12.0, rect.center().y - g.size().y / 2.0), g, weak);
    }
    if let Some(x) = cols.modified {
        ui.painter().text(
            pos2(x + 12.0, rect.center().y),
            Align2::LEFT_CENTER,
            format_time(entry.modified),
            small,
            weak,
        );
    }
}

fn rename_editor(ui: &mut Ui, tab: &mut Tab, rect: Rect, actions: &mut Vec<Action>, p: &M3Palette) {
    let Some(state) = tab.rename.as_mut() else {
        return;
    };
    let id = Id::unique(("rename", &state.path));
    ui.painter().rect_filled(
        rect.expand(2.0),
        CornerRadius::same(8),
        p.surface_container_highest,
    );
    ui.painter().rect_stroke(
        rect.expand(2.0),
        CornerRadius::same(8),
        Stroke::new(
            2.0,
            if state.error.is_some() {
                p.error
            } else {
                p.primary
            },
        ),
        StrokeKind::Outside,
    );
    let resp = ui.put(
        rect,
        egui::TextEdit::singleline(&mut state.text)
            .id(id)
            .frame(egui::Frame::NONE)
            .font(FontId::proportional(13.5))
            .align(egui::Align2::CENTER_CENTER),
    );
    if state.just_started {
        state.just_started = false;
        resp.request_focus();
        // Pre-select the name without its extension, like other file managers.
        let stem_chars = match state.text.rfind('.') {
            Some(i) if i > 0 => state.text[..i].chars().count(),
            _ => state.text.chars().count(),
        };
        if let Some(mut ts) = egui::TextEdit::load_state(ui.ctx(), id) {
            ts.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(stem_chars),
            )));
            ts.store(ui.ctx(), id);
        }
    }
    if resp.changed() {
        state.error = None;
    }
    if let Some(err) = &state.error {
        egui::Tooltip::always_open(
            ui.ctx().clone(),
            ui.layer_id(),
            id.with("err"),
            egui::PopupAnchor::Position(rect.left_bottom() + vec2(0.0, 6.0)),
        )
        .show(|ui| {
            ui.label(RichText::new(err).color(p.error));
        });
    }
    let (enter, escape) = ui.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
    if escape {
        tab.rename = None;
    } else if enter || (resp.lost_focus() && !state_has_error(tab)) {
        commit_rename(tab, actions);
    }
}

fn state_has_error(tab: &Tab) -> bool {
    tab.rename
        .as_ref()
        .map(|r| r.error.is_some())
        .unwrap_or(false)
}

fn commit_rename(tab: &mut Tab, actions: &mut Vec<Action>) {
    let Some(state) = &tab.rename else { return };
    let original = state
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if state.text.trim() == original || state.text.trim().is_empty() {
        tab.rename = None;
    } else {
        actions.push(Action::CommitRename {
            path: state.path.clone(),
            name: state.text.clone(),
        });
    }
}

fn rubber_band(
    ui: &mut Ui,
    bg: &egui::Response,
    tab: &mut Tab,
    layout: &Layout,
    origin: Pos2,
    p: &M3Palette,
) {
    let id = Id::unique("rubber-band");
    if bg.drag_started() {
        if let Some(pos) = bg.interact_pointer_pos() {
            let additive = ui.input(|i| i.modifiers.command || i.modifiers.shift);
            let base = if additive {
                tab.selection.clone()
            } else {
                HashSet::new()
            };
            ui.data_mut(|d| {
                d.insert_temp(
                    id,
                    std::sync::Arc::new(RubberBand {
                        start: pos - origin.to_vec2(),
                        base,
                    }),
                )
            });
        }
    }
    let Some(band) = ui.data(|d| d.get_temp::<std::sync::Arc<RubberBand>>(id)) else {
        return;
    };
    if !bg.dragged() {
        ui.data_mut(|d| d.remove::<std::sync::Arc<RubberBand>>(id));
        return;
    }
    let Some(pos) = ui.ctx().pointer_interact_pos() else {
        return;
    };
    let current = pos - origin.to_vec2();
    let sel = Rect::from_two_pos(band.start, current);

    // Only rows overlapping the band need testing.
    let mut selection = band.base.clone();
    let first = layout.row_at(sel.min.y);
    for row in first..layout.rows.len() {
        if layout.offsets[row] > sel.max.y {
            break;
        }
        if let Row::Items { start, end } = layout.rows[row] {
            for idx in start..end {
                if layout.slot_rect(row, idx - start).intersects(sel) {
                    selection.insert(tab.view[idx].path.clone());
                }
            }
        }
    }
    tab.selection = selection;

    let screen = sel.translate(origin.to_vec2());
    ui.painter().rect_filled(
        screen,
        CornerRadius::same(4),
        p.primary.gamma_multiply(0.12),
    );
    ui.painter().rect_stroke(
        screen,
        CornerRadius::same(4),
        Stroke::new(1.0, p.primary),
        StrokeKind::Inside,
    );
}

fn empty_state(ui: &mut Ui, tab: &Tab, folder: &Path, actions: &mut Vec<Action>, p: &M3Palette) {
    let rect = ui.available_rect_before_wrap();
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );
    let ui = &mut child;
    ui.add_space((rect.height() * 0.28).max(24.0));
    if tab.loading {
        ui.add(egui::Spinner::new().size(36.0).color(p.primary));
        ui.add_space(12.0);
        ui.label(RichText::new("Loading…").color(p.on_surface_variant));
        return;
    }
    let (icon, title, body) = if let Some(err) = &tab.error {
        (
            ICON_ERROR,
            "Can't open this folder".to_string(),
            err.clone(),
        )
    } else if tab.search.is_active() {
        if tab.search.is_running() {
            ui.add(egui::Spinner::new().size(36.0).color(p.primary));
            ui.add_space(12.0);
            ui.label(RichText::new("Searching…").color(p.on_surface_variant));
            return;
        }
        let hint = if tab.search.recursive {
            "Try different words, or search file contents too."
        } else {
            "Try including subfolders."
        };
        (
            ICON_SEARCH_OFF,
            format!("No results for “{}”", tab.search.query.trim()),
            hint.to_string(),
        )
    } else if !tab.groups.is_empty() {
        (
            ICON_FOLDER_OPEN,
            "Everything is collapsed".into(),
            String::new(),
        )
    } else {
        (
            ICON_FOLDER_OPEN,
            "This folder is empty".into(),
            "Drop files here, or create something new.".into(),
        )
    };
    let circle = ui.allocate_exact_size(Vec2::splat(96.0), Sense::hover()).0;
    ui.painter()
        .circle_filled(circle.center(), 48.0, p.secondary_container);
    paint_icon(ui, icon, circle.center(), 48.0, p.on_secondary_container);
    ui.add_space(16.0);
    ui.label(RichText::new(title).size(18.0).color(p.on_surface));
    if !body.is_empty() {
        ui.add_space(4.0);
        ui.label(RichText::new(body).color(p.on_surface_variant));
    }
    if tab.error.is_none() && !tab.search.is_active() && tab.groups.is_empty() {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            let w = 300.0;
            ui.add_space((ui.available_width() - w).max(0.0) / 2.0);
            if widgets::button(
                ui,
                ButtonKind::Tonal,
                Some(ICON_CREATE_NEW_FOLDER),
                "New folder",
            )
            .clicked()
            {
                actions.push(Action::NewFolder(folder.to_path_buf()));
            }
            if widgets::button(ui, ButtonKind::Text, Some(ICON_TERMINAL), "Open terminal").clicked()
            {
                actions.push(Action::OpenInTerminal(folder.to_path_buf()));
            }
        });
    } else if tab.error.is_some() {
        ui.add_space(16.0);
        if widgets::button(
            ui,
            ButtonKind::Tonal,
            Some(ICON_ARROW_UPWARD),
            "Go to parent folder",
        )
        .clicked()
        {
            actions.push(Action::Up);
        }
    }
}

// ----------------------------------------------------------------- menus

fn item_menu(
    ui: &mut Ui,
    entry: &FileEntry,
    targets: &[PathBuf],
    has_clipboard: bool,
    config: &Config,
    actions: &mut Vec<Action>,
) {
    let single = targets.len() == 1;
    let n = targets.len();
    if single {
        if menu_item(
            ui,
            if entry.is_dir {
                ICON_FOLDER_OPEN
            } else {
                ICON_OPEN_IN_NEW
            },
            "Open",
            "Enter",
        )
        .clicked()
        {
            actions.push(Action::Open(entry.path.clone()));
        }
        if entry.is_dir {
            if menu_item(ui, ICON_TAB, "Open in new tab", "Ctrl+Enter").clicked() {
                actions.push(Action::OpenInNewTab(Location::Folder(entry.path.clone())));
            }
            if menu_item(ui, ICON_TERMINAL, "Open in terminal", "").clicked() {
                actions.push(Action::OpenInTerminal(entry.path.clone()));
            }
        } else if menu_item(
            ui,
            ICON_APPS,
            if config.use_dms {
                "Open with DMS"
            } else {
                "Open with…"
            },
            "",
        )
        .clicked()
        {
            actions.push(Action::OpenWith(entry.path.clone()));
        }
        if menu_item(ui, ICON_CODE, "Open in editor", "").clicked() {
            actions.push(Action::OpenInEditor(entry.path.clone()));
        }
    } else {
        let files: Vec<PathBuf> = targets.to_vec();
        if menu_item(ui, ICON_OPEN_IN_NEW, &format!("Open {n} items"), "Enter").clicked() {
            actions.extend(files.into_iter().map(Action::Open));
        }
    }
    ui.separator();
    if menu_item(ui, ICON_CONTENT_CUT, "Cut", "Ctrl+X").clicked() {
        actions.push(Action::Cut(targets.to_vec()));
    }
    if menu_item(ui, ICON_CONTENT_COPY, "Copy", "Ctrl+C").clicked() {
        actions.push(Action::Copy(targets.to_vec()));
    }
    if single
        && entry.is_dir
        && has_clipboard
        && menu_item(ui, ICON_CONTENT_PASTE, "Paste into folder", "").clicked()
    {
        actions.push(Action::Paste(entry.path.clone()));
    }
    if menu_item(
        ui,
        ICON_LINK,
        if single { "Copy path" } else { "Copy paths" },
        "",
    )
    .clicked()
    {
        actions.push(Action::CopyPathText(targets.to_vec()));
    }
    ui.separator();
    if menu_item(ui, ICON_DRIVE_FILE_MOVE, "Move to…", "").clicked() {
        actions.push(Action::MoveTo(targets.to_vec()));
    }
    if menu_item(ui, ICON_FILE_COPY, "Copy to…", "").clicked() {
        actions.push(Action::CopyTo(targets.to_vec()));
    }
    if single && menu_item(ui, ICON_EDIT, "Rename", "F2").clicked() {
        actions.push(Action::StartRename(entry.path.clone()));
    }
    if single && entry.is_dir {
        let pinned = config.is_pinned(&entry.path);
        if menu_item(
            ui,
            if pinned { ICON_KEEP_OFF } else { ICON_KEEP },
            if pinned { "Unpin" } else { "Pin to sidebar" },
            "",
        )
        .clicked()
        {
            actions.push(if pinned {
                Action::Unpin(entry.path.clone())
            } else {
                Action::Pin(entry.path.clone())
            });
        }
    }
    ui.separator();
    if menu_item(ui, ICON_DELETE, "Move to trash", "Delete").clicked() {
        actions.push(Action::Trash(targets.to_vec()));
    }
    if menu_item(ui, ICON_DELETE_FOREVER, "Delete permanently", "Shift+Del").clicked() {
        actions.push(Action::DeleteForever(targets.to_vec()));
    }
    ui.separator();
    if single && menu_item(ui, ICON_INFO, "Properties", "Alt+Enter").clicked() {
        actions.push(Action::Properties(entry.path.clone()));
    }
}

fn background_menu(
    ui: &mut Ui,
    folder: &Path,
    has_clipboard: bool,
    config: &mut Config,
    actions: &mut Vec<Action>,
) {
    if menu_item(ui, ICON_CREATE_NEW_FOLDER, "New folder", "Ctrl+Shift+N").clicked() {
        actions.push(Action::NewFolder(folder.to_path_buf()));
    }
    if menu_item(ui, ICON_NOTE_ADD, "New empty file", "").clicked() {
        actions.push(Action::NewFile(folder.to_path_buf()));
    }
    if widgets::menu_item_enabled(ui, has_clipboard, ICON_CONTENT_PASTE, "Paste", "Ctrl+V")
        .clicked()
    {
        actions.push(Action::Paste(folder.to_path_buf()));
    }
    ui.separator();
    if menu_item(ui, ICON_SELECT_ALL, "Select all", "Ctrl+A").clicked() {
        actions.push(Action::SelectAll);
    }
    let hidden = config.ui.show_hidden;
    if menu_item(
        ui,
        if hidden {
            ICON_VISIBILITY_OFF
        } else {
            ICON_VISIBILITY
        },
        if hidden {
            "Hide hidden files"
        } else {
            "Show hidden files"
        },
        "Ctrl+H",
    )
    .clicked()
    {
        config.ui.show_hidden = !hidden;
        actions.push(Action::ConfigChanged);
    }
    if menu_item(ui, ICON_TERMINAL, "Open in terminal", "").clicked() {
        actions.push(Action::OpenInTerminal(folder.to_path_buf()));
    }
    let pinned = config.is_pinned(folder);
    if menu_item(
        ui,
        if pinned { ICON_KEEP_OFF } else { ICON_KEEP },
        if pinned { "Unpin folder" } else { "Pin folder" },
        "Ctrl+D",
    )
    .clicked()
    {
        actions.push(if pinned {
            Action::Unpin(folder.to_path_buf())
        } else {
            Action::Pin(folder.to_path_buf())
        });
    }
    ui.separator();
    if menu_item(ui, ICON_INFO, "Properties", "").clicked() {
        actions.push(Action::Properties(folder.to_path_buf()));
    }
}
