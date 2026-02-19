use crate::app_state::workspace::Workspace;
use crate::theme_engine::theme::ThemeContext;
use crate::ui_components::breadcrumb::Breadcrumb;
use crate::ui_components::chips::Chip;
use gpui::prelude::*;
use gpui::*;

pub struct NavigationToolbar;

impl NavigationToolbar {
    pub fn render<V: 'static>(
        workspace: Entity<Workspace>,
        search_focus: FocusHandle,
        cx: &mut Context<V>,
        window: &Window,
    ) -> impl IntoElement {
        let palette = cx.theme().palette.clone();

        let (can_go_back, can_go_forward, current_path, filter_query, is_grouped, search_options) = {
            let ws = workspace.read(cx);
            (
                ws.can_go_back(),
                ws.can_go_forward(),
                ws.current_path.clone(),
                ws.filter_query.clone(),
                ws.group_by_type,
                ws.search_options.clone(),
            )
        };
        let is_searching = search_focus.is_focused(window);

        let ws_back = workspace.clone();
        let ws_fwd = workspace.clone();
        let ws_input = workspace.clone();

        let ws_recursive = workspace.clone();
        let ws_content = workspace.clone();

        div()
            .flex()
            .flex_row()
            .w_full()
            .h_12()
            .items_center()
            .px_4()
            .gap_2()
            .bg(palette.background) // Main background
            // Back Button
            .child({
                let btn = div().p_1().rounded_full().text_color(palette.on_surface);

                if can_go_back {
                    btn.cursor_pointer()
                        .hover(|s| s.bg(palette.surface_variant))
                        .id("back_btn")
                        .on_click(move |_event, _phase, cx| {
                            ws_back.update(cx, |ws, cx| ws.go_back(cx));
                        })
                } else {
                    btn.opacity(0.3).id("back_btn_disabled")
                }
                .child(crate::assets::icons::icon("arrow_left").text_color(palette.on_surface))
            })
            // Forward Button
            .child({
                let btn = div().p_1().rounded_full().text_color(palette.on_surface);

                if can_go_forward {
                    btn.cursor_pointer()
                        .hover(|s| s.bg(palette.surface_variant))
                        .id("fwd_btn")
                        .on_click(move |_event, _phase, cx| {
                            ws_fwd.update(cx, |ws, cx| ws.go_forward(cx));
                        })
                } else {
                    btn.opacity(0.3).id("fwd_btn_disabled")
                }
                .child(crate::assets::icons::icon("arrow_right").text_color(palette.on_surface))
            })
            // Breadcrumb (Flex Grow)
            .child(
                div()
                    .flex_grow()
                    .child(Breadcrumb::render(current_path, workspace.clone(), cx)),
            )
            // Search Input Area
            .child(
                div().flex().items_center().gap_2().child(
                    div()
                        .w_64()
                        .h_8()
                        .flex()
                        .items_center()
                        .px_3()
                        .rounded_full()
                        .bg(if is_searching {
                            palette.surface_container_high
                        } else {
                            gpui::rgba(0x00000000)
                        })
                        .border_1()
                        .border_color(if is_searching {
                            palette.primary
                        } else {
                            palette.outline
                        })
                        .track_focus(&search_focus)
                        .on_mouse_down(MouseButton::Left, {
                            let search_focus = search_focus.clone();
                            move |_, window, cx| {
                                window.focus(&search_focus, cx);
                            }
                        })
                        .on_key_down(move |event: &KeyDownEvent, _phase, cx| {
                            let key = &event.keystroke.key;
                            let mut handled = false;

                            ws_input.update(cx, |ws, cx| {
                                if key == "backspace" {
                                    let mut query = ws.filter_query.clone();
                                    query.pop();
                                    ws.set_filter_query(query, cx);
                                    handled = true;
                                } else if key == "enter" || key == "return" {
                                    // println!(
                                    //     "NavigationToolbar: triggering search for query: {}",
                                    //     ws.filter_query
                                    // );
                                    let query = ws.filter_query.clone();
                                    ws.perform_search(query, cx);
                                    handled = true;
                                } else if key == "escape" {
                                    ws.clear_search(cx);
                                    handled = true;
                                } else if let Some(char_str) = &event.keystroke.key_char {
                                    if let Some(char) = char_str.chars().next() {
                                        if char.is_alphanumeric()
                                            || char.is_ascii_punctuation()
                                            || char == ' '
                                        {
                                            let mut query = ws.filter_query.clone();
                                            query.push(char);
                                            ws.set_filter_query(query, cx);
                                            handled = true;
                                        }
                                    }
                                }
                            });

                            if handled {
                                cx.stop_propagation();
                            }
                        })
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    crate::assets::icons::icon("search")
                                        .size_4()
                                        .text_color(palette.on_surface_variant),
                                )
                                .child(if filter_query.is_empty() {
                                    div()
                                        .text_sm()
                                        .text_color(palette.on_surface_variant)
                                        .child("Search...")
                                } else {
                                    div()
                                        .text_sm()
                                        .text_color(palette.on_surface)
                                        .child(filter_query.clone())
                                }),
                        ),
                ),
            )
            // Search Options Chips (Only show if searching or focused?)
            // Let's show them always for now for visibility, or only when query is not empty?
            // Actually instructions say: "Add 'Toggle Chips' to the toolbar (next to the input)"
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        Chip::new("opt_recursive", "Recursive")
                            .filter()
                            .selected(search_options.recursive)
                            .on_click(cx.listener(move |_this, _, _, cx| {
                                ws_recursive.update(cx, |ws, cx| ws.toggle_search_recursive(cx));
                            })),
                    )
                    .child(
                        Chip::new("opt_content", "Content")
                            .filter()
                            .selected(search_options.content_search)
                            .on_click(cx.listener(move |_this, _, _, cx| {
                                ws_content.update(cx, |ws, cx| ws.toggle_search_content(cx));
                            })),
                    ),
            )
            // Grouping Toggle
            .child({
                // is_grouped is already defined above
                let ws_toggle = workspace.clone();
                let palette = palette.clone();

                div()
                    .id("group_toggle_btn")
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_2()
                    .rounded_full()
                    .bg(if is_grouped {
                        palette.secondary_container
                    } else {
                        gpui::rgba(0x00000000)
                    })
                    .hover(|s| {
                        s.bg(if is_grouped {
                            palette.secondary_container
                        } else {
                            palette.surface_container_highest
                        })
                    })
                    .cursor_pointer()
                    .on_click(move |_event, _phase, cx| {
                        ws_toggle.update(cx, |ws, cx| {
                            ws.toggle_grouping(cx);
                        });
                    })
                    .child(
                        crate::assets::icons::icon("category") // Using "category" which maps to our CATEGORY path
                            .size_5()
                            .text_color(if is_grouped {
                                palette.on_secondary_container
                            } else {
                                palette.on_surface_variant
                            }),
                    )
            })
    }
}
