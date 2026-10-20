//! Bottom status line: counts, selection size, free space, running jobs.

use super::widgets::{self, paint_icon};
use super::Action;
use crate::app::App;
use crate::fs::entry::format_size;
use crate::fs::places;
use egui::{pos2, vec2, RichText, Sense, Ui};
use egui_material_icons::icons::*;
use std::sync::atomic::Ordering;

pub fn show(ui: &mut Ui, app: &mut App) {
    let p = app.palette.clone();
    let mut actions = Vec::new();
    ui.horizontal(|ui| {
        ui.set_height(24.0);
        let tab = app.tab();
        let small = |s: String| RichText::new(s).size(12.5).color(p.on_surface_variant);
        let n = tab.view.len();
        let mut text = format!("{n} item{}", if n == 1 { "" } else { "s" });
        let selected = tab.selection_len();
        if selected > 0 {
            let bytes: u64 = tab
                .selected_entries()
                .filter(|e| !e.is_dir)
                .map(|e| e.size)
                .sum();
            text.push_str(&format!(" · {selected} selected"));
            if bytes > 0 {
                text.push_str(&format!(" ({})", format_size(bytes)));
            }
        }
        ui.label(small(text));
        if tab.loading {
            ui.add(egui::Spinner::new().size(12.0).color(p.primary));
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(drive) = tab.folder().and_then(|f| places::drive_for(&app.drives, f)) {
                ui.label(small(format!("{} free", format_size(drive.available))));
                widgets::icon(ui, drive.icon(), 14.0, p.on_surface_variant);
            }
            for job in &app.jobs {
                ui.add_space(12.0);
                let cancel =
                    widgets::icon_button_sized(ui, ICON_CLOSE, "Cancel", true, 22.0, false);
                if cancel.clicked() {
                    actions.push(Action::CancelJob(job.id));
                }
                let frac = job.progress.fraction();
                widgets::linear_progress(ui, frac, 120.0);
                let current = job
                    .progress
                    .current
                    .lock()
                    .map(|c| c.clone())
                    .unwrap_or_default();
                let mut label = job.label.clone();
                if let Some(f) = frac {
                    label.push_str(&format!(" — {:.0}%", f * 100.0));
                }
                let r = ui.label(small(label));
                if !current.is_empty() {
                    r.on_hover_text(current);
                }
                let done = job.progress.done_bytes.load(Ordering::Relaxed);
                let total = job.progress.total_bytes.load(Ordering::Relaxed);
                if total > 0 {
                    ui.label(small(format!(
                        "{} / {}",
                        format_size(done),
                        format_size(total)
                    )));
                }
            }
            if let Some(clip) = &app.clipboard {
                ui.add_space(12.0);
                let (rect, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                let icon = if clip.op == crate::app::ClipOp::Cut {
                    ICON_CONTENT_CUT
                } else {
                    ICON_CONTENT_COPY
                };
                paint_icon(
                    ui,
                    icon,
                    pos2(rect.center().x, rect.center().y),
                    14.0,
                    p.on_surface_variant,
                );
                let n = clip.paths.len();
                resp.on_hover_text(format!(
                    "{n} item{} on the clipboard",
                    if n == 1 { "" } else { "s" }
                ));
            }
        });
    });
    app.actions.extend(actions);
}
