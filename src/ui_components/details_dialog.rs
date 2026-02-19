use crate::app_state::workspace::Workspace;
use crate::theme_engine::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;
use std::time::SystemTime;

pub struct DetailsDialog;

impl DetailsDialog {
    pub fn render<V: 'static>(
        path: &PathBuf,
        size: u64,
        modified: SystemTime,
        mime_type: Option<String>,
        image_dimensions: Option<(u32, u32)>,
        workspace: Entity<Workspace>,
        cx: &Context<V>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Lazy Kind resolution
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let kind = if path.is_dir() {
            "Directory".to_string()
        } else if let Some(mime) = &mime_type {
            mime.clone()
        } else {
            match extension.as_str() {
                "rs" => "Rust Source code".to_string(),
                "toml" => "TOML Configuration".to_string(),
                "md" => "Markdown Document".to_string(),
                "png" | "jpg" | "jpeg" | "gif" => "Image File".to_string(),
                "mp4" | "mkv" | "webm" => "Video File".to_string(),
                "mp3" | "wav" | "ogg" => "Audio File".to_string(),
                "zip" | "tar" | "gz" | "7z" => "Archive File".to_string(),
                "txt" => "Text File".to_string(),
                _ => format!("{} File", extension.to_uppercase()),
            }
        };

        let workspace_scrim = workspace.clone();
        let workspace_close = workspace.clone();

        div()
            .id("details_scrim")
            .absolute()
            .flex()
            .items_center()
            .justify_center()
            .size_full()
            .bg(gpui::rgba(0x00000080)) // Scrim
            // Close on click outside
            .on_click(move |_, _, cx| {
                workspace_scrim.update(cx, |ws, cx| {
                    ws.active_overlay = None;
                    cx.notify();
                });
            })
            .child(
                div()
                    .id("details_card")
                    // Stop propagation so clicking dialog doesn't close it
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                    .w_96()
                    .bg(theme.palette.surface_container_high)
                    .rounded_xl()
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .shadow_lg()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            // Icon would go here
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(theme.palette.on_surface)
                                    .child(name),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(div().flex().justify_between().child("Kind:").child(kind))
                            .children(mime_type.clone().map(|mime| {
                                div().flex().justify_between().child("MIME:").child(mime)
                            }))
                            .children(image_dimensions.map(|(w, h)| {
                                div()
                                    .flex()
                                    .justify_between()
                                    .child("Dimensions:")
                                    .child(format!("{}x{}", w, h))
                            }))
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .child("Size:")
                                    .child(human_bytes::human_bytes(size as f64)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .child("Location:")
                                    .child(path.to_string_lossy().to_string())
                                    .text_ellipsis() // truncate path if long
                                    .overflow_hidden(),
                            )
                            .child(
                                div().flex().justify_between().child("Modified:").child(
                                    chrono::DateTime::<chrono::Local>::from(modified)
                                        .format("%Y-%m-%d %H:%M")
                                        .to_string(),
                                ),
                            ),
                    )
                    .child(
                        div().flex().justify_end().child(
                            div()
                                .id("details_close_btn")
                                .px_4()
                                .py_2()
                                .rounded_full()
                                .bg(theme.palette.primary)
                                .text_color(theme.palette.on_primary)
                                .cursor_pointer()
                                .hover(|s| s.bg(theme.palette.primary_container)) // Use a solid variant for safety
                                .child("Close")
                                .on_click(move |_, _, cx| {
                                    workspace_close.update(cx, |ws, cx| {
                                        ws.active_overlay = None;
                                        cx.notify();
                                    });
                                }),
                        ),
                    ),
            )
    }
}
