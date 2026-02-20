use crate::app_state::config::ConfigManager;
use crate::app_state::workspace::Workspace;
use crate::theme_engine::theme::ThemeContext;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use gpui::prelude::*;
use gpui::*;
use std::time::Instant;

use crate::assets::icon_cache::{IconCache, IconType};
use crate::ui_components::loader::ShapeShifterLoader;

pub struct FileList {
    workspace: Entity<Workspace>,
    icon_cache: Entity<IconCache>,
    focus_handle: FocusHandle,
    _subscription: Subscription,
    collapsed_categories: std::collections::HashSet<String>,
    loader: Entity<ShapeShifterLoader>,
    last_path_change: Instant,
    current_path_hash: u64,
}

impl FileList {
    pub fn new(
        workspace: Entity<Workspace>,
        icon_cache: Entity<IconCache>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.observe(&icon_cache, |_, _, cx| cx.notify());
        let loader = cx.new(|cx| ShapeShifterLoader::new(cx));

        let ws = workspace.read(cx);
        let hash = Self::hash_path(&ws.current_path);

        Self {
            workspace,
            icon_cache,
            focus_handle: cx.focus_handle(),
            _subscription: subscription,
            collapsed_categories: std::collections::HashSet::new(),
            loader,
            last_path_change: Instant::now(),
            current_path_hash: hash,
        }
    }

    fn hash_path(path: &std::path::Path) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        hasher.finish()
    }

    fn toggle_category(&mut self, category: String, cx: &mut Context<Self>) {
        if self.collapsed_categories.contains(&category) {
            self.collapsed_categories.remove(&category);
        } else {
            self.collapsed_categories.insert(category);
        }
        cx.notify();
    }
}

impl Render for FileList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.theme().palette.clone();
        let ws = self.workspace.read(cx);

        // Detect path change to trigger animation
        let new_hash = Self::hash_path(&ws.current_path);
        if new_hash != self.current_path_hash {
            self.current_path_hash = new_hash;
            self.last_path_change = Instant::now();
        }

        let items = ws.items.clone();
        let selection = ws.selection.clone();
        let is_loading = ws.is_loading;
        let icon_cache = self.icon_cache.clone();

        let config = cx.global::<ConfigManager>().config.clone();
        let show_hidden = config.ui.show_hidden;
        let view_mode = config.ui.view_mode.clone();
        let is_grid = view_mode == "grid";
        let icon_size_px = px(config.ui.icon_size as f32);

        // Filter items
        let matcher = SkimMatcherV2::default();
        let filter_query = ws.filter_query.clone();

        let filtered_items: Vec<_> = if let Some(global_results) = &ws.search_results {
            global_results.clone()
        } else {
            items
                .into_iter()
                .filter(|item| {
                    let is_hidden = item.name.starts_with('.');
                    if is_hidden && !show_hidden {
                        return false;
                    }
                    if !filter_query.is_empty() {
                        return matcher.fuzzy_match(&item.name, &filter_query).is_some();
                    }
                    true
                })
                .collect()
        };

        let item_count = filtered_items.len();
        let workspace_handle = self.workspace.clone();

        // Animation params
        let elapsed = self.last_path_change.elapsed().as_secs_f32();
        let animation_duration = 0.3; // Total staggered entrance time

        // If still animating, request frames
        if elapsed < animation_duration + 0.1 {
             cx.on_next_frame(window, |_this, _window, cx| {
                cx.notify();
            });
        }

        // Removed overflow_y_scroll() as uniform_list handles it
        let list_view = div()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                 // Basic keyboard navigation placeholders
            }))
            .child({
                let workspace = workspace_handle.clone();
                let filtered_items = filtered_items.clone();

                gpui::uniform_list(
                    view_mode,
                    item_count,
                    move |range, _window, cx| {
                        if is_grid {
                            // GRID VIEW
                            let items_slice = &filtered_items[range];
                            items_slice
                            .iter()
                            .enumerate()
                            .map(|(i, item)| {
                                let is_selected = selection.contains(&item.path);
                                let item_path = item.path.clone();
                                let is_dir = item.is_dir;
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

                                // Staggered Animation Calculation
                                let stagger_delay = (i as f32) * 0.03;
                                let item_time = (elapsed - stagger_delay).max(0.0);
                                let item_t = (item_time / 0.2).min(1.0); // Each item takes 0.2s to enter
                                let item_ease = 1.0 - (1.0 - item_t).powi(3);
                                let opacity = item_ease;
                                let translate_y = px(10.0 * (1.0 - item_ease));

                                div()
                                    .id(i)
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .p_2()
                                    .m_1()
                                    .w_32()
                                    .h_40()
                                    .rounded_lg()
                                    .hover(|s| s.bg(palette.surface_container_highest))
                                    .bg(if is_selected {
                                        palette.secondary_container
                                    } else {
                                        gpui::rgba(0x00000000)
                                    })
                                    // Apply Animation
                                    .opacity(opacity)
                                    .mt(translate_y)
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
                                            .w(icon_size_px)
                                            .h(icon_size_px)
                                            .flex()
                                            .justify_center()
                                            .items_center()
                                            .child(
                                                if let Some(thumb) = icon_cache.update(cx, |cache, cx| {
                                                    cache.get(IconType::Path(item.path.clone()), palette.primary.into(), cx)
                                                }) {
                                                    div().size_full().children(vec![
                                                        img(thumb)
                                                            .object_fit(ObjectFit::Cover)
                                                            .rounded_md()
                                                            .into_any_element()
                                                    ]).into_any_element()
                                                } else {
                                                    crate::assets::icons::icon(icon_name).size_12().into_any_element()
                                                }
                                            )
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

                                // Staggered Animation Calculation (List View)
                                let stagger_delay = (i as f32) * 0.02;
                                let item_time = (elapsed - stagger_delay).max(0.0);
                                let item_t = (item_time / 0.2).min(1.0);
                                let item_ease = 1.0 - (1.0 - item_t).powi(3);
                                let opacity = item_ease;
                                let translate_x = px(10.0 * (1.0 - item_ease));

                                div()
                                    .id(i)
                                    .h_10()
                                    .flex()
                                    .items_center()
                                    .w_full()
                                    .px_3()
                                    .border_b_1()
                                    .border_color(Hsla::from(palette.outline_variant).opacity(0.1))
                                    .bg(if is_selected {
                                        Hsla::from(palette.secondary_container)
                                    } else {
                                        gpui::hsla(0., 0., 0., 0.)
                                    })
                                    .text_color(if is_selected {
                                        palette.on_secondary_container
                                    } else {
                                        palette.on_surface
                                    })
                                    .hover(|s| s.bg(palette.surface_container_highest))
                                    // Apply Animation
                                    .opacity(opacity)
                                    .ml(translate_x)
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
                                            .child(crate::assets::icons::icon(icon_name).size_5()),
                                    )
                                    .child(div().ml_3().flex_grow().min_w_0().child(
                                        div().text_ellipsis().child(item.name.clone()),
                                    ))
                                    .child(
                                        div()
                                            .w_24()
                                            .text_sm()
                                            .text_color(if is_selected {
                                                palette.on_secondary_container
                                            } else {
                                                palette.on_surface_variant
                                            })
                                            .child(item.formatted_date.clone()),
                                    )
                                    .child(
                                        div()
                                            .w_20()
                                            .text_sm()
                                            .text_right()
                                            .text_color(if is_selected {
                                                palette.on_secondary_container
                                            } else {
                                                palette.on_surface_variant
                                            })
                                            .child(item.formatted_size.clone()),
                                    )
                                    .into_any_element()
                            })
                            .collect::<Vec<_>>()
                        }
                    }
                )
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
