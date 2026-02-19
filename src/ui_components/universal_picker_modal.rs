use gpui::prelude::*;
use gpui::{InteractiveElement, Styled, *};
use std::path::PathBuf;

use crate::app_state::workspace::Workspace;
use crate::assets::icons::icon;
use crate::theme_engine::theme::ThemeContext;

#[derive(Clone, Debug)]
pub enum FilePickerEvent {
    PathsSelected(Vec<PathBuf>),
    Cancelled,
}

#[derive(Clone, Debug)]
pub struct Filter {
    pub name: String,
    pub patterns: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum PickerMode {
    OpenFile {
        filters: Vec<Filter>,
    },
    OpenFiles {
        filters: Vec<Filter>,
    },
    OpenFolder,
    SaveFile {
        current_name: String,
        filters: Vec<Filter>,
    },
}

pub struct UniversalPickerModal {
    workspace: Entity<Workspace>,
    mode: PickerMode,
    selected_filter_index: usize,
    save_filename: String,
}

impl EventEmitter<FilePickerEvent> for UniversalPickerModal {}

impl UniversalPickerModal {
    pub fn new(workspace: Entity<Workspace>, mode: PickerMode, _cx: &mut Context<Self>) -> Self {
        let save_filename = match &mode {
            PickerMode::SaveFile { current_name, .. } => current_name.clone(),
            _ => String::new(),
        };

        Self {
            workspace,
            mode,
            selected_filter_index: 0,
            save_filename,
        }
    }

    fn select_item(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if path.is_dir() {
            let _ = self.workspace.update(cx, |ws, cx| {
                ws.navigate(path, cx);
            });
        } else {
            match self.mode {
                PickerMode::OpenFile { .. }
                | PickerMode::SaveFile { .. }
                | PickerMode::OpenFiles { .. } => {
                    cx.emit(FilePickerEvent::PathsSelected(vec![path]));
                }
                _ => {}
            }
        }
    }

    fn select_current(&mut self, cx: &mut Context<Self>) {
        let current_path = self.workspace.read(cx).current_path.clone();
        match self.mode {
            PickerMode::OpenFolder => {
                cx.emit(FilePickerEvent::PathsSelected(vec![current_path]));
            }
            PickerMode::SaveFile { .. } => {
                let mut path = current_path;
                path.push(&self.save_filename);
                cx.emit(FilePickerEvent::PathsSelected(vec![path]));
            }
            _ => {}
        }
    }

    fn navigate_to_shortcut(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let _ = self.workspace.update(cx, |ws, cx| {
            ws.navigate(path, cx);
        });
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        cx.emit(FilePickerEvent::Cancelled);
    }

    fn render_shortcut_button(
        &self,
        icon_name: &'static str,
        label: &'static str,
        path: PathBuf,
        palette: &crate::theme_engine::palette::M3Palette,
        handle: Entity<Self>,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let handle = handle.downgrade();
        div()
            .id(SharedString::from(format!("shortcut_{}", label)))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .rounded_full()
            .bg(palette.surface_container)
            .border_1()
            .border_color(palette.outline_variant)
            .hover(|s| s.bg(palette.surface_container_high))
            .cursor_pointer()
            .on_click(move |_, _, cx| {
                if let Some(this) = handle.upgrade() {
                    let _ = this.update(cx, |this, cx| {
                        this.navigate_to_shortcut(path.clone(), cx);
                    });
                }
            })
            .child(icon(icon_name).size_5().text_color(palette.on_surface))
            .child(div().text_sm().text_color(palette.on_surface).child(label))
    }

    fn render_filter_dropdown(
        &self,
        palette: &crate::theme_engine::palette::M3Palette,
    ) -> impl IntoElement {
        let filter_text = match &self.mode {
            PickerMode::OpenFile { filters }
            | PickerMode::OpenFiles { filters }
            | PickerMode::SaveFile { filters, .. } => {
                if let Some(f) = filters.get(self.selected_filter_index) {
                    format!("{} ({})", f.name, f.patterns.join(", "))
                } else {
                    "All Files".to_string()
                }
            }
            _ => "All Folders".to_string(),
        };

        div()
            .px_3()
            .py_2()
            .bg(palette.surface_container_highest)
            .rounded_md()
            .child(filter_text)
    }

    fn matches_filter(&self, name: &str, is_dir: bool) -> bool {
        if is_dir {
            return true;
        }

        let filters = match &self.mode {
            PickerMode::OpenFile { filters }
            | PickerMode::OpenFiles { filters }
            | PickerMode::SaveFile { filters, .. } => filters,
            _ => return false,
        };

        if filters.is_empty() {
            return true;
        }

        if let Some(filter) = filters.get(self.selected_filter_index) {
            for pattern in &filter.patterns {
                if pattern == "*" {
                    return true;
                }
                let pat_clean = pattern.trim_start_matches('*');
                if name.to_lowercase().ends_with(&pat_clean.to_lowercase()) {
                    return true;
                }
            }
            return false;
        }
        true
    }
}

impl Render for UniversalPickerModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let palette = theme.palette.clone();
        let handle = cx.entity().clone();

        let workspace = self.workspace.read(cx);
        let current_path = workspace.current_path.clone();
        let items = workspace.items.clone();

        let filtered_items: Vec<_> = items
            .into_iter()
            .filter(|item| match &self.mode {
                PickerMode::OpenFolder => item.is_dir,
                _ => self.matches_filter(&item.name, item.is_dir),
            })
            .collect();

        let home = dirs::home_dir();
        let desktop = dirs::desktop_dir();
        let documents = dirs::document_dir();
        let downloads = dirs::download_dir();

        div()
            .id("universal_picker_modal_overlay")
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x00000080))
            .child(
                div()
                    .id("universal_picker_modal_container")
                    .w(px(800.0))
                    .h(px(600.0))
                    .bg(palette.surface)
                    .rounded_3xl()
                    .shadow_2xl()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        // Header
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .p_5()
                            .border_b_1()
                            .border_color(palette.outline_variant)
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(palette.on_surface)
                                    .child(match self.mode {
                                        PickerMode::OpenFolder => "Select Folder",
                                        PickerMode::SaveFile { .. } => "Save File",
                                        _ => "Open File",
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .size_10()
                                    .items_center()
                                    .justify_center()
                                    .rounded_full()
                                    .hover(|s| s.bg(palette.surface_container_high))
                                    .cursor_pointer()
                                    .id("close_button")
                                    .on_click({
                                        let handle = handle.clone();
                                        move |_, _, cx| {
                                            let _ = handle.update(cx, |this, cx| {
                                                this.cancel(cx);
                                            });
                                        }
                                    })
                                    .child(icon("close").size_6().text_color(palette.on_surface)),
                            ),
                    )
                    .child(
                        // Shortcuts Row
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .p_4()
                            .border_b_1()
                            .border_color(palette.outline_variant)
                            .children(home.map(|path| {
                                self.render_shortcut_button(
                                    "home",
                                    "Home",
                                    path,
                                    &palette,
                                    handle.clone(),
                                    cx,
                                )
                            }))
                            .children(desktop.map(|path| {
                                self.render_shortcut_button(
                                    "folder",
                                    "Desktop",
                                    path,
                                    &palette,
                                    handle.clone(),
                                    cx,
                                )
                            }))
                            .children(documents.map(|path| {
                                self.render_shortcut_button(
                                    "description",
                                    "Documents",
                                    path,
                                    &palette,
                                    handle.clone(),
                                    cx,
                                )
                            }))
                            .children(downloads.map(|path| {
                                self.render_shortcut_button(
                                    "download",
                                    "Downloads",
                                    path,
                                    &palette,
                                    handle.clone(),
                                    cx,
                                )
                            })),
                    )
                    .child(
                        // Path Breadcrumb / Current Path info
                        div().px_4().py_2().bg(palette.surface_container_low).child(
                            div()
                                .text_sm()
                                .text_color(palette.on_surface_variant)
                                .child(current_path.to_string_lossy().to_string()),
                        ),
                    )
                    .child(
                        // Content List
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .overflow_hidden()
                            .bg(palette.surface)
                            .when(filtered_items.is_empty(), |el| {
                                el.flex()
                                    .items_center()
                                    .justify_center()
                                    .child("No items found")
                            })
                            .when(!filtered_items.is_empty(), |el| {
                                el.child(div().flex().flex_col().children(
                                    filtered_items.into_iter().map(|entry| {
                                        let path = entry.path.clone();
                                        let name = entry.name.clone();
                                        let is_dir = entry.is_dir;
                                        let handle = handle.clone();

                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_3()
                                            .px_4()
                                            .py_2()
                                            .hover(|s| s.bg(palette.surface_container))
                                            .cursor_pointer()
                                            .id(SharedString::from(format!("item_{}", name)))
                                            .on_click(move |_, _, cx| {
                                                let _ = handle.update(cx, |this, cx| {
                                                    this.select_item(path.clone(), cx);
                                                });
                                            })
                                            .child(
                                                icon(if is_dir { "folder" } else { "description" })
                                                    .size_6()
                                                    .text_color(if is_dir {
                                                        palette.primary
                                                    } else {
                                                        palette.on_surface_variant
                                                    }),
                                            )
                                            .child(name)
                                    }),
                                ))
                            }),
                    )
                    .child(
                        // Footer
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .p_4()
                            .border_t_1()
                            .border_color(palette.outline_variant)
                            .child(match &self.mode {
                                PickerMode::SaveFile { .. } => div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .items_center()
                                    .child("Name:")
                                    .child(
                                        div()
                                            .flex_1()
                                            .px_3()
                                            .py_2()
                                            .bg(palette.surface_container_highest)
                                            .rounded_md()
                                            .child(self.save_filename.clone()),
                                    ),
                                _ => div(),
                            })
                            .child(
                                // Action Row
                                div()
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .items_center()
                                    .child(self.render_filter_dropdown(&palette))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap_3()
                                            .child({
                                                let handle = handle.clone();
                                                div()
                                                    .px_6()
                                                    .py_3()
                                                    .rounded_full()
                                                    .border_1()
                                                    .border_color(palette.outline)
                                                    .cursor_pointer()
                                                    .id("cancel_button")
                                                    .on_click(move |_, _, cx| {
                                                        let _ = handle.update(cx, |this, cx| {
                                                            this.cancel(cx);
                                                        });
                                                    })
                                                    .child("Cancel")
                                            })
                                            .child({
                                                let handle = handle.clone();
                                                div()
                                                    .px_6()
                                                    .py_3()
                                                    .rounded_full()
                                                    .bg(palette.primary)
                                                    .text_color(palette.on_primary)
                                                    .cursor_pointer()
                                                    .id("select_button")
                                                    .on_click(move |_, _, cx| {
                                                        let _ = handle.update(cx, |this, cx| {
                                                            this.select_current(cx);
                                                        });
                                                    })
                                                    .child(match self.mode {
                                                        PickerMode::SaveFile { .. } => "Save",
                                                        _ => "Select",
                                                    })
                                            }),
                                    ),
                            ),
                    ),
            )
    }
}
