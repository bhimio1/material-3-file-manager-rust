use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;

use crate::app_state::workspace::Workspace;
use crate::assets::icons::icon;
use crate::fs_ops::applications::AppEntry;
use crate::theme_engine::theme::ThemeContext;

pub struct OpenWithDialog {
    workspace: Entity<Workspace>,
    file_path: PathBuf,
    apps: Vec<AppEntry>,
    filtered_apps: Vec<AppEntry>,
    selected_index: Option<usize>,
    focus_handle: FocusHandle,
    search_query: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OpenWithEvent {
    Open(String),
    Close,
}

impl EventEmitter<OpenWithEvent> for OpenWithDialog {}

impl OpenWithDialog {
    pub fn new(
        workspace: Entity<Workspace>,
        file_path: PathBuf,
        apps: Vec<AppEntry>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            workspace,
            file_path,
            apps: apps.clone(),
            filtered_apps: apps,
            selected_index: None,
            focus_handle: cx.focus_handle(),
            search_query: String::new(),
        }
    }

    fn select_app(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected_index = Some(index);
        cx.notify();
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        if let Some(idx) = self.selected_index {
            if let Some(app) = self.filtered_apps.get(idx) {
                cx.emit(OpenWithEvent::Open(app.exec.clone()));
            }
        }
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        cx.emit(OpenWithEvent::Close);
    }
}

impl Render for OpenWithDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let palette = theme.palette.clone();

        let file_name = self
            .file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        div()
            .id("open_with_overlay")
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .w_full()
            .h_full()
            .bg(rgba(0x00000080))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.cancel(cx)),
            )
            .child(
                div()
                    .w(px(500.0))
                    .h(px(600.0))
                    .bg(palette.surface)
                    .rounded_xl()
                    .shadow_xl()
                    .flex()
                    .flex_col()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .p_4()
                            .border_b_1()
                            .border_color(palette.outline_variant)
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(palette.on_surface)
                                    .child(format!("Open \"{}\" with...", file_name)),
                            ),
                    )
                    .child(
                        div()
                            .flex_grow()
                            // .overflow_y_scroll() // Temporarily commented out
                            .child(div().flex().flex_col().children(
                                self.filtered_apps.iter().enumerate().map(|(ix, app)| {
                                    let is_selected = Some(ix) == self.selected_index;
                                    let bg = if is_selected {
                                        palette.secondary_container
                                    } else {
                                        palette.surface
                                    };
                                    let text = if is_selected {
                                        palette.on_secondary_container
                                    } else {
                                        palette.on_surface
                                    };
                                    let app_name = app.name.clone();
                                    let app_exec = app.exec.clone();

                                    div()
                                        .id(SharedString::from(format!("app_{}", ix)))
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .px_4()
                                        .py_3()
                                        .bg(bg)
                                        .hover(|s| s.bg(palette.surface_container_high))
                                        .cursor_pointer()
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                this.select_app(ix, cx);
                                            }),
                                        )
                                        .child(icon("folder").size_6().text_color(text))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(text)
                                                        .child(app_name),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(palette.on_surface_variant)
                                                        .child(app_exec),
                                                ),
                                        )
                                }),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .p_4()
                            .border_t_1()
                            .border_color(palette.outline_variant)
                            .child(
                                div()
                                    .px_4()
                                    .py_2()
                                    .rounded_full()
                                    .border_1()
                                    .border_color(palette.outline)
                                    .text_color(palette.primary)
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| this.cancel(cx)),
                                    )
                                    .child("Cancel"),
                            )
                            .child(
                                div()
                                    .px_4()
                                    .py_2()
                                    .rounded_full()
                                    .bg(palette.primary)
                                    .text_color(palette.on_primary)
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| this.confirm(cx)),
                                    )
                                    .child("Open"),
                            ),
                    ),
            )
    }
}
