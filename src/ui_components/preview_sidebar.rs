use crate::app_state::workspace::Workspace;
use crate::theme_engine::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

pub struct PreviewSidebar {
    workspace: Entity<Workspace>,
}

impl PreviewSidebar {
    pub fn new(workspace: Entity<Workspace>, _cx: &mut Context<Self>) -> Self {
        Self { workspace }
    }
}

impl Render for PreviewSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ws = self.workspace.read(cx);
        let selected_path = ws.last_selected.clone();
        let palette = theme.palette.clone();

        // Check if selected file exists and get metadata
        let file_info = if let Some(path) = &selected_path {
            if let Some(item) = ws.items.iter().find(|i| i.path == *path) {
                Some(item.clone())
            } else {
                None
            }
        } else {
            None
        };

        let content = if let Some(item) = file_info {
            let is_large = item.size > 10 * 1024 * 1024; // 10MB
            let ext = item
                .path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let is_image =
                matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif") && !is_large;

            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_4()
                .p_4()
                .w_full()
                .child(if is_image {
                    // Reverting file:// prefix to see if raw path works better or if encoding was issue
                    // Also adding debug print
                    let path_str = format!("file://{}", item.path.to_string_lossy());
                    println!("PreviewSidebar rendering image: {}", path_str);
                    img(path_str)
                        .w(px(200.0))
                        .h(px(200.0))
                        .object_fit(ObjectFit::Contain)
                        .into_any_element()
                } else {
                    crate::assets::icons::icon(if item.is_dir {
                        "folder"
                    } else {
                        match ext.as_str() {
                            "mp4" | "mkv" | "webm" => "video",
                            "mp3" | "wav" | "ogg" => "audio",
                            "zip" | "tar" | "gz" | "7z" => "archive",
                            _ => "file",
                        }
                    })
                    .size(px(128.0))
                    .text_color(palette.primary)
                    .into_any_element()
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_2()
                        .w_full()
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_center()
                                .child(item.name.clone()),
                        )
                        .child(div().h_px().w_full().bg(palette.outline_variant))
                        .child(
                            div()
                                .w_full()
                                .flex()
                                .justify_between()
                                .text_sm()
                                .text_color(palette.on_surface_variant)
                                .child("Size")
                                .child(item.formatted_size.clone()),
                        )
                        .child(
                            div()
                                .w_full()
                                .flex()
                                .justify_between()
                                .text_sm()
                                .text_color(palette.on_surface_variant)
                                .child("Modified")
                                .child(item.formatted_date.clone()),
                        ),
                )
        } else {
            div()
                .flex()
                .items_center()
                .justify_center()
                .h_full()
                .text_color(palette.on_surface_variant)
                .child("No selection")
        };

        div()
            .w_64()
            .h_full()
            .bg(palette.surface)
            .border_l_1()
            .border_color(palette.outline_variant)
            .child(content)
    }
}
