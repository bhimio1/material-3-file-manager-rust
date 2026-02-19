use crate::app_state::workspace::Workspace;
use crate::assets::icons;
use crate::theme_engine::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;

pub struct Breadcrumb;

impl Breadcrumb {
    pub fn render<V: 'static>(
        path: PathBuf,
        workspace: Entity<Workspace>,
        cx: &mut Context<V>,
    ) -> impl IntoElement {
        let mut segments: Vec<(String, PathBuf, bool)> = Vec::new();
        let mut path_acc = PathBuf::new();

        let components: Vec<_> = path.components().collect();
        let total = components.len();

        for (i, component) in components.iter().enumerate() {
            match component {
                std::path::Component::RootDir => {
                    path_acc.push("/");
                    segments.push(("/".to_string(), path_acc.clone(), i == total - 1));
                }
                std::path::Component::Normal(name) => {
                    path_acc.push(name);
                    segments.push((
                        name.to_string_lossy().into_owned(),
                        path_acc.clone(),
                        i == total - 1,
                    ));
                }
                _ => {}
            }
        }

        // Hack: if empty (root only sometimes tricky), ensure something
        if segments.is_empty() {
            segments.push(("/".to_string(), PathBuf::from("/"), true));
        }

        let menu_item = |label: &str, target: PathBuf, is_last: bool, cx: &mut Context<V>| {
            let theme = cx.theme();
            let workspace = workspace.clone();

            let base_div = div()
                .flex()
                .items_center()
                .px_2()
                .py_1()
                .rounded_md()
                .text_sm();

            if is_last {
                base_div
                    .bg(theme.palette.surface_variant)
                    .text_color(theme.palette.on_surface)
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(label.to_string())
                    .id(SharedString::from(format!(
                        "crumb_active_{}",
                        target.display()
                    )))
            } else {
                base_div
                    .cursor_pointer()
                    .text_color(theme.palette.on_surface_variant)
                    .hover(|s| s.bg(theme.palette.surface_variant))
                    .id(SharedString::from(format!("crumb_{}", target.display())))
                    .on_click(move |_event, _phase, cx| {
                        workspace.update(cx, |ws, cx| {
                            ws.open(target.clone(), cx);
                        });
                    })
                    .child(label.to_string())
            }
        };

        div()
            .w_full()
            .h_10() // Fixed height for bar
            .flex()
            .items_center()
            .items_center()
            // .overflow_x_scroll() // Removed due to error
            .child(
                div()
                    .flex()
                    .items_center()
                    .children(segments.into_iter().map(|(label, target, is_last)| {
                        div()
                            .flex()
                            .items_center()
                            .child(menu_item(&label, target, is_last, cx))
                            .child(if !is_last {
                                div()
                                    .child(
                                        icons::icon("chevron_right")
                                            .text_color(cx.theme().palette.on_surface_variant),
                                    )
                                    .mr_1()
                            } else {
                                div()
                            })
                    })),
            )
    }
}
