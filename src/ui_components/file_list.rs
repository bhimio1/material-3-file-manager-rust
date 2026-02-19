use crate::app_state::config::ConfigManager;
use crate::app_state::workspace::Workspace;
use crate::theme_engine::theme::ThemeContext;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use gpui::prelude::*;
use gpui::InteractiveElement;
use gpui::*;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::assets::icon_cache::IconCache;
use crate::ui_components::loader::ShapeShifterLoader;

pub struct FileList {
    workspace: Entity<Workspace>,
    icon_cache: Entity<IconCache>,
    focus_handle: FocusHandle,
    _subscription: Subscription,
    collapsed_categories: std::collections::HashSet<String>,
    pending_thumbnails: HashSet<PathBuf>,
    loader: Entity<ShapeShifterLoader>,
}

impl FileList {
    pub fn new(
        workspace: Entity<Workspace>,
        icon_cache: Entity<IconCache>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.observe(&icon_cache, |_, _, cx| cx.notify());
        let loader = cx.new(|cx| ShapeShifterLoader::new(cx));
        Self {
            workspace,
            icon_cache,
            focus_handle: cx.focus_handle(),
            _subscription: subscription,
            collapsed_categories: std::collections::HashSet::new(),
            pending_thumbnails: HashSet::new(),
            loader,
        }
    }
}

impl Render for FileList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.theme().palette.clone();
        let ws = self.workspace.read(cx);

        let items = ws.items.clone();
        let selection = ws.selection.clone();
        let is_loading = ws.is_loading;
        let _icon_cache = self.icon_cache.clone();

        let config = cx.global::<ConfigManager>().config.clone();
        let show_hidden = config.ui.show_hidden;
        let view_mode = config.ui.view_mode.clone();
        let is_grid = view_mode == "grid";
        let _icon_size_px = px(config.ui.icon_size as f32);

        // Filter items
        let filtered_items = ws.filtered_items.clone();

        let item_count = filtered_items.len();
        let workspace_handle = self.workspace.clone();
        let ws_handle_click = workspace_handle.clone();
        let ws_handle_key = workspace_handle.clone();
        let ws_handle_list = workspace_handle.clone();

        let file_list_entity = cx.entity().clone();
        let list_id = ElementId::Name("file_list_virtual".into());

        let list_view = div()
            .size_full()
            .bg(palette.surface_container_low)
            .track_focus(&self.focus_handle)
            .on_key_down(move |event: &KeyDownEvent, _phase, cx| {
                if event.keystroke.modifiers.control {
                    ws_handle_key.update(cx, |ws, cx| match event.keystroke.key.as_str() {
                        "c" => ws.copy_selection(cx),
                        "x" => ws.cut_selection(cx),
                        "v" => ws.paste_clipboard(cx),
                        _ => {}
                    });
                }
            })
            .on_mouse_down(MouseButton::Right, move |event, _phase, cx| {
                ws_handle_click.update(cx, |ws, cx| {
                    ws.open_context_menu(event.position, None, cx);
                });
            })
            .child({
                let palette = cx.theme().palette.clone();
                // TODO: Find correct API for window size in ViewContext
                let viewport_width = px(1000.0);

                // Estimate grid item width including margins/padding (pixel value)
                let item_width_px = 120.0;
                let cols = if is_grid {
                    let w = f32::from(viewport_width);
                    (w / item_width_px).floor() as usize
                } else {
                    1
                };
                let cols = std::cmp::max(1, cols);

                let list_count = if is_grid {
                    (item_count + cols - 1) / cols
                } else {
                    item_count
                };

                uniform_list(list_id, list_count, move |range, _window, cx| {
                    let workspace = ws_handle_list.clone();
                    let selection = selection.clone();

                    if is_grid {
                        range
                            .map(|row_index| {
                                let start_index = row_index * cols;
                                let end_index = std::cmp::min(start_index + cols, item_count);
                                let row_items = &filtered_items[start_index..end_index];

                                div()
                                    .id(row_index)
                                    .flex()
                                    .w_full()
                                    .h(px(130.0)) // Row height for Grid
                                    .items_start() // Top align
                                    .children(row_items.iter().enumerate().map(
                                        |(col_idx, item)| {
                                            let item_idx = start_index + col_idx; // Global index if needed for ID
                                            let is_selected = selection.contains(&item.path);
                                            let item_path = item.path.clone();
                                            let is_dir = item.is_dir;

                                            let bg_color = if is_selected {
                                                Hsla::from(palette.secondary_container)
                                            } else {
                                                gpui::hsla(0., 0., 0., 0.)
                                            };
                                            let text_color = if is_selected {
                                                palette.on_secondary_container
                                            } else {
                                                palette.on_surface
                                            };

                                            // Icon / Thumbnail logic
                                            let ext = item_path
                                                .extension()
                                                .and_then(|e| e.to_str())
                                                .unwrap_or("")
                                                .to_lowercase();
                                            
                                            let is_image = !is_dir && matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp");
                                            
                                            let thumbnail_path = if is_image {
                                                crate::assets::thumbnail_worker::ThumbnailWorker::get_cached_path(&item_path)
                                            } else {
                                                None
                                            };

                                            if is_image && thumbnail_path.is_none() {
                                                let is_pending = file_list_entity.read(cx).pending_thumbnails.contains(&item_path);
                                                if !is_pending {

                                                    let path_for_task = item_path.clone();
                                                    let entity_for_task = file_list_entity.clone();
                                                    cx.background_executor().spawn(async move {
                                                        crate::assets::thumbnail_worker::ThumbnailWorker::generate_thumbnail(path_for_task.clone());
                                                    }).detach();
                                                }
                                            }
                                            let icon_name = if is_dir {
                                                "folder"
                                            } else {
                                                match ext.as_str() {
                                                    "png" | "jpg" | "jpeg" | "webp" => "image",
                                                    "mp4" | "mkv" | "webm" => "video",
                                                    "mp3" | "wav" | "ogg" => "audio",
                                                    _ => "file",
                                                }
                                            };

                                            let ws_click = workspace.clone();
                                            let path_click = item_path.clone();
                                            let ws_dbl = workspace.clone();
                                            let path_dbl = item_path.clone();
                                            let ws_right = workspace.clone();
                                            let path_right = item_path.clone();

                                            div()
                                                .id(item_idx)
                                                .w(px(item_width_px))
                                                .h_full()
                                                .flex()
                                                .flex_col()
                                                .items_center()
                                                .p_2()
                                                .m_1()
                                                .rounded_md()
                                                .bg(bg_color)
                                                .text_color(text_color)
                                                .hover(|s| s.bg(palette.surface_container_highest))
                                                .on_click(move |event, _, cx| {
                                                    if event.click_count() >= 2 {
                                                        ws_dbl.update(cx, |ws, cx| {
                                                            ws.open(path_dbl.clone(), cx);
                                                        });
                                                    }
                                                })
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    move |event, _, cx| {
                                                        cx.stop_propagation();
                                                        ws_click.update(cx, |ws, cx| {
                                                            if event.modifiers.control {
                                                                ws.toggle_selection(
                                                                    path_click.clone(),
                                                                    cx,
                                                                );
                                                            } else if event.modifiers.shift {
                                                                ws.select_range(
                                                                    path_click.clone(),
                                                                    cx,
                                                                );
                                                            } else {
                                                                ws.set_selection(
                                                                    path_click.clone(),
                                                                    cx,
                                                                );
                                                            }
                                                        });
                                                    },
                                                )
                                                .on_mouse_down(
                                                    MouseButton::Right,
                                                    move |event, _, cx| {
                                                        cx.stop_propagation();
                                                        ws_right.update(cx, |ws, cx| {
                                                            if !ws.selection.contains(&path_right) {
                                                                ws.set_selection(
                                                                    path_right.clone(),
                                                                    cx,
                                                                );
                                                            }
                                                            ws.open_context_menu(
                                                                event.position,
                                                                Some(path_right.clone()),
                                                                cx,
                                                            );
                                                        });
                                                    },
                                                )
                                                .child(
                                                    if let Some(thumb) = thumbnail_path {
                                                        let path_str = format!("file://{}", thumb.to_string_lossy());
                                                        div().flex().children(vec![
                                                            div().w(px(4.0)).h(px(4.0)).bg(gpui::blue()).rounded_full().into_any_element(),
                                                            img(path_str)
                                                                .w(px(64.0))
                                                                .h(px(64.0))
                                                                .object_fit(ObjectFit::Cover)
                                                                .rounded_md()
                                                                .into_any_element()
                                                        ]).into_any_element()
                                                    } else {
                                                        div().flex().items_center().justify_center().children(vec![
                                                            // Revert to svg(), explicit size
                                                            icon(icon_name).size_12().into_any_element()
                                                        ]).into_any_element()
                                                    }
                                                )
                                                .child(
                                                    div()
                                                        .mt_2()
                                                        .text_sm()
                                                        .text_center()
                                                        .text_ellipsis()
                                                        .max_w_full()
                                                        .child(item.name.clone()),
                                                )
                                        },
                                    ))
                                    .into_any_element()
                            })
                            .collect::<Vec<_>>()
                    } else {
                        // LIST VIEW
                        let items_slice = &filtered_items[range];

                        items_slice
                            .iter()
                            .enumerate()
                            .map(|(i, item)| {
                                let is_selected = selection.contains(&item.path);
                                let item_path = item.path.clone();
                                let is_dir = item.is_dir;

                                let bg_color = if is_selected {
                                    Hsla::from(palette.secondary_container)
                                } else {
                                    gpui::hsla(0., 0., 0., 0.)
                                };
                                let text_color = if is_selected {
                                    palette.on_secondary_container
                                } else {
                                    palette.on_surface
                                };
                                let sub_text_color = if is_selected {
                                    palette.on_secondary_container
                                } else {
                                    palette.on_surface_variant
                                };
                                let ext = item_path
                                    .extension()
                                    .and_then(|e| e.to_str())
                                    .unwrap_or("")
                                    .to_lowercase();

                                let is_image = !is_dir && matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp");

                                let thumbnail_path = if is_image {
                                    crate::assets::thumbnail_worker::ThumbnailWorker::get_cached_path(&item_path)
                                } else {
                                    None
                                };

                                            if is_image && thumbnail_path.is_none() {
                                                let is_pending = file_list_entity.read(cx).pending_thumbnails.contains(&item_path);
                                                if !is_pending {

                                                    let path_for_task = item_path.clone();
                                                    let entity_for_task = file_list_entity.clone();
                                                    cx.background_executor().spawn(async move {
                                                        crate::assets::thumbnail_worker::ThumbnailWorker::generate_thumbnail(path_for_task.clone());
                                                    }).detach();
                                                }
                                            }
                                let icon_name = if is_dir {
                                    "folder"
                                } else {
                                    match item_path
                                        .extension()
                                        .and_then(|e| e.to_str())
                                        .unwrap_or("")
                                    {
                                        "png" | "jpg" | "jpeg" | "webp" => "image",
                                        "mp4" | "mkv" | "webm" => "video",
                                        "mp3" | "wav" | "ogg" => "audio",
                                        _ => "file",
                                    }
                                };

                                let ws_click = workspace.clone();
                                let path_click = item_path.clone();
                                let ws_dbl = workspace.clone();
                                let path_dbl = item_path.clone();
                                let ws_right = workspace.clone();
                                let path_right = item_path.clone();

                                div()
                                    .id(i)
                                    .h_10()
                                    .flex()
                                    .items_center()
                                    .w_full()
                                    .px_3()
                                    .border_b_1()
                                    .border_color(Hsla::from(palette.outline_variant).opacity(0.1))
                                    .bg(bg_color)
                                    .text_color(text_color)
                                    .hover(|s| s.bg(palette.surface_container_highest))
                                    .on_click(move |event, _, cx| {
                                        if event.click_count() >= 2 {
                                            ws_dbl.update(cx, |ws, cx| {
                                                ws.open(path_dbl.clone(), cx);
                                            });
                                        }
                                    })
                                    .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                                        cx.stop_propagation();
                                        ws_click.update(cx, |ws, cx| {
                                            if event.modifiers.control {
                                                ws.toggle_selection(path_click.clone(), cx);
                                            } else if event.modifiers.shift {
                                                ws.select_range(path_click.clone(), cx);
                                            } else {
                                                ws.set_selection(path_click.clone(), cx);
                                            }
                                        });
                                    })
                                    .on_mouse_down(MouseButton::Right, move |event, _, cx| {
                                        cx.stop_propagation();
                                        ws_right.update(cx, |ws, cx| {
                                            if !ws.selection.contains(&path_right) {
                                                ws.set_selection(path_right.clone(), cx);
                                            }
                                            ws.open_context_menu(
                                                event.position,
                                                Some(path_right.clone()),
                                                cx,
                                            );
                                        });
                                    })
                                    .child(
                                        div()
                                            .w(px(24.0))
                                            .flex()
                                            .justify_center()
                                            .child(icon(icon_name).size_5()),
                                    )
                                    .child(div().ml_3().flex_grow().min_w_0().child(
                                        div().text_ellipsis().child(item.name.clone()),
                                    ))
                                    .child(
                                        div()
                                            .w_24()
                                            .text_sm()
                                            .text_color(sub_text_color)
                                            .child(item.formatted_date.clone()),
                                    )
                                    .child(
                                        div()
                                            .w_20()
                                            .text_sm()
                                            .text_right()
                                            .text_color(sub_text_color)
                                            .child(item.formatted_size.clone()),
                                    )
                                    .into_any_element()
                            })
                            .collect::<Vec<_>>()
                    }
                })
                .size_full()
            })
            .child(if is_loading {
                div()
                    .absolute()
                    .size_full()
                    .bg(gpui::rgba(0x00000080))
                    .child(self.loader.clone())
            } else if item_count == 0 {
                div()
                    .absolute()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child("No items")
            } else {
                div()
            });

        list_view
    }
}
