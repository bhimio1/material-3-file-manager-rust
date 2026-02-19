use crate::app_state::config::ConfigContext;
use crate::theme_engine::theme::ThemeContext;
use chrono::Timelike;
use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;
use sysinfo::Disks;

pub struct Dashboard {
    disks: Disks,
    focus_handle: FocusHandle,
}

impl Dashboard {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let disks = Disks::new_with_refreshed_list();
        Self {
            disks,
            focus_handle: cx.focus_handle(),
        }
    }

    fn get_greeting() -> &'static str {
        let hour = chrono::Local::now().hour();
        match hour {
            5..=11 => "Good Morning",
            12..=16 => "Good Afternoon",
            17..=20 => "Good Evening",
            _ => "Good Night",
        }
    }

    fn render_header(&self, palette: &crate::theme_engine::palette::M3Palette) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .mb_6()
            .child(
                div()
                    .text_3xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(palette.on_surface)
                    .child(Self::get_greeting()),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(palette.on_surface_variant)
                    .child("Welcome back to your command center."),
            )
    }

    fn render_storage_widget(
        &self,
        palette: &crate::theme_engine::palette::M3Palette,
    ) -> impl IntoElement {
        // Get the root disk ("/")
        let root_disk = self
            .disks
            .iter()
            .find(|d| d.mount_point() == std::path::Path::new("/"));

        let (used_space, total_space, usage_percent) = if let Some(disk) = root_disk {
            let total = disk.total_space();
            let available = disk.available_space();
            let used = total - available;
            let percent = if total > 0 {
                (used as f64 / total as f64) as f32
            } else {
                0.0
            };
            (used, total, percent)
        } else {
            (0, 0, 0.0)
        };

        let used_gb = used_space as f64 / 1_073_741_824.0; // Convert to GB
        let total_gb = total_space as f64 / 1_073_741_824.0;

        div()
            .p_5()
            .bg(palette.surface_container_low)
            .rounded_2xl()
            .border_1()
            .border_color(palette.outline_variant)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .mb_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                crate::assets::icons::icon("hard_drive")
                                    .size_5()
                                    .text_color(palette.primary),
                            )
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(palette.on_surface)
                                    .child("System Storage"),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(palette.on_surface_variant)
                            .child(format!(
                                "{:.1} GB used of {:.1} GB total",
                                used_gb, total_gb
                            )),
                    ),
            )
            .child(
                div()
                    .h(px(12.0))
                    .w_full()
                    .bg(palette.surface_container_highest)
                    .rounded_full()
                    .overflow_hidden()
                    .child(
                        div()
                            .h_full()
                            .bg(palette.primary)
                            .rounded_full()
                            .w(relative(usage_percent)),
                    ),
            )
    }

    fn render_pinned_section(
        &self,
        palette: &crate::theme_engine::palette::M3Palette,
        pinned_folders: &[PathBuf],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(crate::assets::icons::icon("star").size_5())
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(palette.on_surface)
                            .child("Pinned"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .children(pinned_folders.iter().take(3).map(|folder| {
                        let name = folder
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Folder")
                            .to_string();
                        let icon_name = match name.as_str() {
                            "Downloads" => "download",
                            "Documents" => "description",
                            _ => "home",
                        };
                        let folder_clone = folder.clone();

                        div()
                            .id(SharedString::from(format!("pinned_{}", folder.display())))
                            .w(px(110.0))
                            .h(px(100.0))
                            .p_3()
                            .bg(palette.surface_container)
                            .rounded_xl()
                            .border_1()
                            .border_color(palette.outline_variant)
                            .hover(|s| s.bg(palette.surface_container_high))
                            .cursor_pointer()
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(DashboardEvent::OpenPath(folder_clone.clone()));
                            }))
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(
                                crate::assets::icons::icon(icon_name)
                                    .size_8()
                                    .text_color(palette.primary),
                            )
                            .child(div().text_xs().text_color(palette.on_surface).child(name))
                    }))
                    .child(
                        // Add button
                        div()
                            .id("add_pinned")
                            .w(px(110.0))
                            .h(px(100.0))
                            .p_3()
                            .bg(palette.surface_container)
                            .rounded_xl()
                            .border_1()
                            .border_color(palette.outline_variant)
                            .hover(|s| s.bg(palette.surface_container_high))
                            .cursor_pointer()
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(DashboardEvent::ShowAddPinned);
                            }))
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(
                                crate::assets::icons::icon("add")
                                    .size_8()
                                    .text_color(palette.on_surface_variant),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(palette.on_surface_variant)
                                    .child("Add"),
                            ),
                    ),
            )
    }

    fn render_recent_section(
        &self,
        palette: &crate::theme_engine::palette::M3Palette,
        recent_folders: &std::collections::VecDeque<PathBuf>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(crate::assets::icons::icon("schedule").size_5())
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(palette.on_surface)
                            .child("Recent"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(recent_folders.iter().take(8).map(|folder| {
                        let name = folder
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Unknown")
                            .to_string();
                        let path_str = folder.display().to_string();

                        div()
                            .id(SharedString::from(format!("recent_{}", folder.display())))
                            .flex()
                            .items_center()
                            .gap_3()
                            .p_2()
                            .rounded_lg()
                            .hover(|s| s.bg(palette.surface_container))
                            .cursor_pointer()
                            .child(
                                crate::assets::icons::icon("folder")
                                    .size_5()
                                    .text_color(palette.primary),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div().text_sm().text_color(palette.on_surface).child(name),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(palette.on_surface_variant)
                                            .child(path_str),
                                    ),
                            )
                    })),
            )
    }
}

impl Render for Dashboard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let palette = theme.palette.clone();
        let config = cx.config();
        let pinned_folders = config.pinned_folders.clone();
        let recent_folders = config.recent_folders.clone();

        div()
            .id("dashboard")
            .flex()
            .flex_col()
            .size_full()
            .bg(palette.background)
            .p_8()
            .overflow_y_scroll()
            .track_focus(&self.focus_handle)
            .child(self.render_header(&palette))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(self.render_storage_widget(&palette))
                    .child(
                        div()
                            .flex()
                            .gap_6()
                            .child(
                                div()
                                    .flex_1()
                                    .p_5()
                                    .bg(palette.surface_container_low)
                                    .rounded_2xl()
                                    .border_1()
                                    .border_color(palette.outline_variant)
                                    .child(self.render_pinned_section(
                                        &palette,
                                        &pinned_folders,
                                        cx,
                                    )),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .p_5()
                                    .bg(palette.surface_container_low)
                                    .rounded_2xl()
                                    .border_1()
                                    .border_color(palette.outline_variant)
                                    .child(self.render_recent_section(&palette, &recent_folders)),
                            ),
                    ),
            )
    }
}

pub enum DashboardEvent {
    OpenPath(PathBuf),
    ShowAddPinned,
}

impl EventEmitter<DashboardEvent> for Dashboard {}
