#![allow(dead_code)]
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
    should_focus: bool,
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
            should_focus: true,
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

    fn update_search(&mut self, query: String, cx: &mut Context<Self>) {
        self.search_query = query;
        self.filter_apps(cx);
    }

    fn filter_apps(&mut self, cx: &mut Context<Self>) {
        let query = self.search_query.to_lowercase();
        if query.is_empty() {
            self.filtered_apps = self.apps.clone();
        } else {
            self.filtered_apps = self
                .apps
                .iter()
                .filter(|app| {
                    app.name.to_lowercase().contains(&query)
                        || app.exec.to_lowercase().contains(&query)
                })
                .cloned()
                .collect();
        }
        self.selected_index = None;
        cx.notify();
    }
}

impl Render for OpenWithDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.should_focus {
            self.should_focus = false;
            _window.focus(&self.focus_handle, cx);
        }

        let theme = cx.theme();
        let palette = theme.palette.clone();

        let file_name = self
            .file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let filtered_apps = self.filtered_apps.clone();
        let selected_index = self.selected_index;
        let item_count = filtered_apps.len();
        let list_id = ElementId::Name("app_list".into());
        let this_handle = cx.entity().clone();
        let palette_clone = palette.clone();

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
                    .track_focus(&self.focus_handle)
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        let key = &event.keystroke.key;
                        let mut handled = false;
                        if key == "backspace" {
                            let mut query = this.search_query.clone();
                            query.pop();
                            this.update_search(query, cx);
                            handled = true;
                        } else if let Some(char_str) = &event.keystroke.key_char {
                            // Simple text input logic
                            if !event.keystroke.modifiers.control
                                && !event.keystroke.modifiers.alt
                                && !event.keystroke.modifiers.platform
                                && char_str.len() == 1
                            {
                                let mut query = this.search_query.clone();
                                query.push_str(char_str);
                                this.update_search(query, cx);
                                handled = true;
                            }
                        } else if key == "escape" {
                            this.cancel(cx);
                            handled = true;
                        } else if key == "enter" {
                            this.confirm(cx);
                            handled = true;
                        } else if key == "up" {
                            if let Some(idx) = this.selected_index {
                                if idx > 0 {
                                    this.selected_index = Some(idx - 1);
                                }
                            } else if !this.filtered_apps.is_empty() {
                                this.selected_index = Some(this.filtered_apps.len() - 1);
                            }
                            cx.notify();
                            handled = true;
                        } else if key == "down" {
                            if let Some(idx) = this.selected_index {
                                if idx < this.filtered_apps.len() - 1 {
                                    this.selected_index = Some(idx + 1);
                                }
                            } else if !this.filtered_apps.is_empty() {
                                this.selected_index = Some(0);
                            }
                            cx.notify();
                            handled = true;
                        }

                        if handled {
                            cx.stop_propagation();
                        }
                    }))
                    .child(
                        div()
                            .p_4()
                            .border_b_1()
                            .border_color(palette.outline_variant)
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(palette.on_surface)
                                    .child(format!("Open \"{}\" with...", file_name)),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .h_10()
                                    .px_3()
                                    .flex()
                                    .items_center()
                                    .bg(palette.surface_container_high)
                                    .rounded_md()
                                    .border_1()
                                    .border_color(palette.outline)
                                    .child(if self.search_query.is_empty() {
                                        div()
                                            .text_color(palette.on_surface_variant)
                                            .child("Type to search...")
                                    } else {
                                        div()
                                            .text_color(palette.on_surface)
                                            .child(self.search_query.clone())
                                    }),
                            ),
                    )
                    .child(
                        div().flex_grow().size_full().child(
                            uniform_list(list_id, item_count, move |range, _window, _cx| {
                                let palette = palette_clone.clone();
                                let this_handle = this_handle.clone();
                                let filtered_apps = filtered_apps.clone();

                                range
                                    .map(|ix| {
                                        let app = &filtered_apps[ix];
                                        let is_selected = Some(ix) == selected_index;

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

                                        let handle_click = this_handle.clone();

                                        div()
                                            .id(ix)
                                            .h(px(72.0))
                                            .flex()
                                            .items_center()
                                            .gap_3()
                                            .px_4()
                                            .py_3()
                                            .bg(bg)
                                            .hover(|s| s.bg(palette.surface_container_high))
                                            .cursor_pointer()
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                handle_click.update(cx, |this, cx| {
                                                    this.select_app(ix, cx);
                                                });
                                            })
                                            .child(icon("folder").size_8().text_color(text))
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
                                            .into_any_element()
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .size_full(),
                        ),
                    )
                    .child(
                        div()
                            .flex_grow()
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
