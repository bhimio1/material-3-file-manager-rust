use super::events::SettingsEvent;
use crate::app_state::config::ConfigManager;
use crate::theme_engine::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use std::time::Instant;

pub struct SettingsWindow {
    active_tab: usize,
    scroll_state: ListState,
    start_time: Instant,
}

impl SettingsWindow {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            active_tab: 0,
            scroll_state: ListState::new(0, ListAlignment::Top, px(1000.0)),
            start_time: Instant::now(),
        }
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let tabs = ["General", "Appearance", "Advanced", "About"];

        // Capture colors needed for the loop to avoid holding reference to theme if needed
        // But since this compiled before, we assume it's fine or we'll fix it if it pops up.
        // Actually, to be safe, let's clone the palette if possible, or just be careful.
        // ThemeContext returns &Theme. Theme struct usually owns its data or is refcounted?
        // Let's rely on the previous check not complaining about render_sidebar.

        div()
            .w_64()
            .h_full()
            .bg(theme.palette.surface_container_low)
            .border_r_1()
            .border_color(theme.palette.outline_variant)
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .children(tabs.iter().enumerate().map(|(i, tab)| {
                let is_active = self.active_tab == i;

                div()
                    .id(SharedString::from(format!("tab-{}", i)))
                    .w_full()
                    .px_4()
                    .py_3()
                    .rounded_full() // Stadium shape
                    .flex()
                    .items_center()
                    .justify_start()
                    .gap_3()
                    .cursor_pointer()
                    .bg(if is_active {
                        theme.palette.secondary_container
                    } else {
                        gpui::rgba(0x00000000)
                    })
                    .text_color(if is_active {
                        theme.palette.on_secondary_container
                    } else {
                        theme.palette.on_surface_variant
                    })
                    .hover(|s| {
                        if !is_active {
                            s.bg(theme.palette.surface_container_highest)
                        } else {
                            s
                        }
                    })
                    .child(
                         div()
                            .text_sm()
                            .font_weight(if is_active { FontWeight::BOLD } else { FontWeight::NORMAL })
                            .child(tab.to_string())
                    )
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.active_tab = i;
                        cx.notify();
                    }))
            }))
    }

    fn render_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Extract only what we need to avoid holding immutable borrow of cx
        let surface_color = cx.theme().palette.surface;

        // Entrance animation: Slide up + Fade in
        let duration = 0.4; // seconds
        let elapsed = self.start_time.elapsed().as_secs_f32();
        let t = (elapsed / duration).clamp(0.0, 1.0);

        // Ease out quint
        let ease = 1.0 - (1.0 - t).powi(5);

        let offset_y = 20.0 * (1.0 - ease); // Slide up 20px
        let opacity = ease;

        let content = match self.active_tab {
            0 => self.render_general_tab(cx).into_any_element(),
            1 => self.render_appearance_tab(cx).into_any_element(),
            _ => div().child("Not implemented").into_any_element(),
        };

        div()
            .flex_1()
            .h_full()
            .bg(surface_color)
            .p_8()
            // .overflow_y_scroll() // Removed due to compilation error
            .child(
                div()
                    .mt(px(offset_y))
                    .opacity(opacity)
                    .child(content)
            )
    }

    fn render_general_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.palette.on_surface)
                    .child("General Settings")
            )
            .child(
                div()
                    .p_4()
                    .rounded_xl()
                    .bg(theme.palette.surface_container)
                    .border_1()
                    .border_color(theme.palette.outline_variant)
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(div().text_base().text_color(theme.palette.on_surface).child("Show Hidden Files"))
                                    .child(div().text_sm().text_color(theme.palette.on_surface_variant).child("Toggle visibility of dotfiles"))
                            )
                            .child(div().text_sm().child("Switch Placeholder"))
                    )
            )
    }

    fn render_appearance_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.palette.on_surface)
                    .child("Appearance")
            )
             .child(
                div()
                    .p_4()
                    .rounded_xl()
                    .bg(theme.palette.surface_container)
                    .border_1()
                    .border_color(theme.palette.outline_variant)
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(div().text_base().text_color(theme.palette.on_surface).child("Theme Mode"))
                                    .child(div().text_sm().text_color(theme.palette.on_surface_variant).child("Choose between Light and Dark mode"))
                            )
                             .child(div().text_sm().child("Theme Toggle Placeholder"))
                    )
            )
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Same here: avoid holding theme across mutable method calls if any
        let bg_color = cx.theme().palette.surface;
        let text_color = cx.theme().palette.on_surface;

        if self.start_time.elapsed().as_secs_f32() < 0.5 {
            cx.on_next_frame(window, move |_, _, cx| {
                cx.notify();
            });
        }

        div()
            .w_full()
            .h_full()
            .flex()
            .bg(bg_color)
            .text_color(text_color)
            .child(self.render_sidebar(cx))
            .child(self.render_content(cx))
    }
}
