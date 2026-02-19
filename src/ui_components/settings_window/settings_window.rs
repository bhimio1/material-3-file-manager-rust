use super::events::SettingsEvent;
use crate::app_state::config::ConfigManager;
use crate::theme_engine::theme::ThemeContext;
use crate::theme_engine::palette::M3Palette;
use gpui::prelude::*;
use gpui::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    General,
    Appearance,
    Advanced,
}

pub struct SettingsWindow {
    active_tab: SettingsTab,
    terminal_input: String,
    editor_input: String,
    focus_handle: FocusHandle,
    terminal_focus: FocusHandle,
    editor_focus: FocusHandle,
}

impl SettingsWindow {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let config = cx.global::<ConfigManager>().config.clone();
        let terminal = config
            .commands
            .get("terminal")
            .cloned()
            .unwrap_or_else(|| "kitty".to_string());
        let editor = config
            .commands
            .get("editor")
            .cloned()
            .unwrap_or_else(|| "code".to_string());

        Self {
            active_tab: SettingsTab::General,
            terminal_input: terminal,
            editor_input: editor,
            focus_handle: cx.focus_handle(),
            terminal_focus: cx.focus_handle(),
            editor_focus: cx.focus_handle(),
        }
    }

    fn update_command(&mut self, key: &str, value: String, cx: &mut Context<Self>) {
        cx.update_global::<ConfigManager, _>(|manager, cx| {
            manager.config.commands.insert(key.to_string(), value);
            manager.save_config();
            cx.refresh_windows();
        });
    }

    fn render_sidebar(&self, palette: &M3Palette, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w_48()
            .gap_1()
            .pr_4()
            .border_r_1()
            .border_color(palette.outline_variant)
            .child(self.render_tab_button("General", SettingsTab::General, palette, cx))
            .child(self.render_tab_button("Appearance", SettingsTab::Appearance, palette, cx))
            .child(self.render_tab_button("Advanced", SettingsTab::Advanced, palette, cx))
    }

    fn render_tab_button(
        &self,
        label: &str,
        tab: SettingsTab,
        palette: &M3Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = self.active_tab == tab;
        let bg = if is_active {
            palette.secondary_container
        } else {
            gpui::rgba(0x00000000)
        };
        let color = if is_active {
            palette.on_secondary_container
        } else {
            palette.on_surface
        };

        let view = cx.entity().clone();
        // Create a unique ID for the tab button
        let id = format!("tab_btn_{}", label);

        div()
            .id(id)
            .px_4()
            .py_2()
            .rounded_full()
            .bg(bg)
            .text_color(color)
            .cursor_pointer()
            .hover(|s| s.bg(palette.surface_container_highest))
            .child(label.to_string())
            .on_click(move |_event, _window, cx| {
                view.update(cx, |this, cx| {
                    this.active_tab = tab;
                    cx.notify();
                });
            })
    }

    fn render_input_field(
        &self,
        label: &str,
        value: &str,
        focus_handle: FocusHandle,
        key_name: &'static str, // "terminal" or "editor"
        palette: &M3Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let focus_handle_clone = focus_handle.clone();
        let view = cx.entity().clone();

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(label.to_string())
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
                    .track_focus(&focus_handle)
                    .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                        window.focus(&focus_handle_clone, cx);
                    })
                    .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                         let key = &event.keystroke.key;
                         let char_str = &event.keystroke.key_char;
                         let modifiers = &event.keystroke.modifiers;

                         let key = key.clone();
                         let char_str = char_str.clone();
                         let modifiers = modifiers.clone();

                         view.update(cx, |this, cx| {
                            let mut handled = false;

                            if key_name == "terminal" {
                                if key == "backspace" {
                                    this.terminal_input.pop();
                                    handled = true;
                                } else if let Some(char_str) = &char_str {
                                    if !modifiers.control
                                        && !modifiers.alt
                                        && !modifiers.platform
                                        && char_str.len() == 1
                                    {
                                        this.terminal_input.push_str(char_str);
                                        handled = true;
                                    }
                                } else if key == "enter" {
                                    this.update_command(key_name, this.terminal_input.clone(), cx);
                                    cx.emit(SettingsEvent::ShowToast(format!("{} command saved", key_name)));
                                    handled = true;
                                }
                            } else {
                                if key == "backspace" {
                                    this.editor_input.pop();
                                    handled = true;
                                } else if let Some(char_str) = &char_str {
                                    if !modifiers.control
                                        && !modifiers.alt
                                        && !modifiers.platform
                                        && char_str.len() == 1
                                    {
                                        this.editor_input.push_str(char_str);
                                        handled = true;
                                    }
                                } else if key == "enter" {
                                    this.update_command(key_name, this.editor_input.clone(), cx);
                                    cx.emit(SettingsEvent::ShowToast(format!("{} command saved", key_name)));
                                    handled = true;
                                }
                            }

                            if handled {
                                cx.notify();
                            }
                         });
                    })
                    .child(if value.is_empty() {
                         div().text_color(palette.on_surface_variant).child("Type command...")
                    } else {
                         div().text_color(palette.on_surface).child(value.to_string())
                    })
            )
            .child(
                div().text_xs().text_color(palette.on_surface_variant).child("Press Enter to save")
            )
    }

    fn render_general_settings(
        &self,
        ui_config: &crate::app_state::config::UiConfig,
        palette: &M3Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            // Show Hidden Files
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(crate::assets::icons::icon("eye").size_5())
                            .child("Show Hidden Files"),
                    )
                    .child(
                        crate::ui_components::chips::Chip::new(
                            "toggle_hidden_chip",
                            "Hidden",
                        )
                        .filter()
                        .icon("check")
                        .selected(ui_config.show_hidden)
                        .on_click(cx.listener(move |_this, _, _, cx| {
                            cx.update_global::<ConfigManager, _>(|manager, cx| {
                                manager.config.ui.show_hidden = !manager.config.ui.show_hidden;
                                manager.save_config();
                                cx.refresh_windows();
                            });
                            cx.emit(SettingsEvent::ConfigChanged);
                        })),
                    ),
            )
            // View Mode
            .child(div().h_px().bg(palette.outline_variant))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child("View Mode")
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                crate::ui_components::chips::Chip::new(
                                    "view_grid_chip",
                                    "Grid",
                                )
                                .filter()
                                .icon("grid")
                                .selected(ui_config.view_mode == "grid")
                                .on_click(cx.listener(move |_this, _, _, cx| {
                                    cx.update_global::<ConfigManager, _>(|manager, cx| {
                                        manager.config.ui.view_mode = "grid".to_string();
                                        manager.save_config();
                                        cx.refresh_windows();
                                    });
                                })),
                            )
                            .child(
                                crate::ui_components::chips::Chip::new(
                                    "view_list_chip",
                                    "List",
                                )
                                .filter()
                                .icon("list")
                                .selected(ui_config.view_mode == "list")
                                .on_click(cx.listener(move |_this, _, _, cx| {
                                    cx.update_global::<ConfigManager, _>(|manager, cx| {
                                        manager.config.ui.view_mode = "list".to_string();
                                        manager.save_config();
                                        cx.refresh_windows();
                                    });
                                })),
                            ),
                    ),
            )
    }

    fn render_appearance_settings(
        &self,
        ui_config: &crate::app_state::config::UiConfig,
        palette: &M3Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let icon_sizes = [48, 64, 80, 96];

        div()
            .flex()
            .flex_col()
            .gap_4()
            // Icon Size
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(crate::assets::icons::icon("image").size_5())
                            .child("Icon Size"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .children(icon_sizes.iter().map(|&size| {
                                let label = format!("{}px", size);
                                let is_selected = ui_config.icon_size == size;
                                crate::ui_components::chips::Chip::new(
                                    format!("size_{}", size),
                                    label,
                                )
                                .filter()
                                .selected(is_selected)
                                .on_click(cx.listener(move |_this, _, _, cx| {
                                    cx.update_global::<ConfigManager, _>(move |manager, cx| {
                                        manager.config.ui.icon_size = size;
                                        manager.save_config();
                                        cx.refresh_windows();
                                    });
                                }))
                            })),
                    ),
            )
            // Theming
            .child(div().h_px().bg(palette.outline_variant))
            .child("Theming")
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        crate::ui_components::chips::Chip::new(
                            "export_matugen_chip",
                            "Export Matugen Template",
                        )
                        .icon("download")
                        .on_click(cx.listener(move |_this, _, _, cx| {
                            match crate::theme_engine::matugen::generate_template() {
                                Ok(path) => cx.emit(SettingsEvent::ShowToast(format!(
                                    "Template exported to {:?}",
                                    path
                                ))),
                                Err(e) => cx.emit(SettingsEvent::ShowToast(format!("Error: {}", e))),
                            }
                        })),
                    )
                    .child(
                        crate::ui_components::chips::Chip::new(
                            "reload_theme_chip",
                            "Reload Theme",
                        )
                        .icon("refresh")
                        .on_click(cx.listener(move |_this, _, _, cx| {
                            cx.update_global::<crate::theme_engine::theme::Theme, _>(|theme, _| {
                                theme.palette = crate::theme_engine::theme::Theme::load_palette();
                            });
                            cx.refresh_windows();
                            cx.emit(SettingsEvent::ShowToast("Theme reloaded".to_string()));
                        })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_xs().child("Matugen config example"))
                    .child(
                         crate::ui_components::chips::Chip::new("copy_matugen_chip", "Copy")
                            .icon("copy")
                            .on_click(cx.listener(move |_this, _, _, cx| {
                                let example = r#"[templates.material_3_file_manager]
input_path = "~/.config/material_3_file_manager/matugen_template.json"
output_path = "~/.config/material_3_file_manager/theme.json""#;
                                cx.write_to_clipboard(ClipboardItem::new_string(example.to_string()));
                                cx.emit(SettingsEvent::ShowToast("Copied to clipboard".to_string()));
                            }))
                    )
            )
    }

    fn render_advanced_settings(
        &self,
        config: &crate::app_state::config::Config,
        palette: &M3Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            // DMS Open
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(crate::assets::icons::icon("code").size_5())
                            .child("Use DMS Open"),
                    )
                    .child(
                        crate::ui_components::chips::Chip::new(
                            "toggle_dms_chip",
                            "Enabled",
                        )
                        .filter()
                        .icon("check")
                        .selected(config.use_dms)
                        .on_click(cx.listener(move |_this, _, _, cx| {
                            cx.update_global::<ConfigManager, _>(|manager, cx| {
                                manager.config.use_dms = !manager.config.use_dms;
                                manager.save_config();
                                cx.refresh_windows();
                            });
                        })),
                    ),
            )
            .child(div().h_px().bg(palette.outline_variant))
            // Commands
            .child(self.render_input_field(
                "Terminal Command",
                &self.terminal_input,
                self.terminal_focus.clone(),
                "terminal",
                palette,
                cx,
            ))
            .child(self.render_input_field(
                "Editor Command",
                &self.editor_input,
                self.editor_focus.clone(),
                "editor",
                palette,
                cx,
            ))
            .child(div().h_px().bg(palette.outline_variant))
            // Clear Pinned
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child("Reset Pinned Folders")
                    .child(
                        crate::ui_components::chips::Chip::new(
                            "reset_pinned_chip",
                            "Reset",
                        )
                        .icon("remove")
                        .on_click(cx.listener(move |_this, _, _, cx| {
                            cx.update_global::<ConfigManager, _>(|manager, cx| {
                                manager.config.pinned_folders = crate::app_state::config::default_pinned_folders();
                                manager.save_config();
                                cx.refresh_windows();
                            });
                             cx.emit(SettingsEvent::ShowToast("Pinned folders reset".to_string()));
                        })),
                    )
            )
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let palette = theme.palette.clone();

        let config = cx.global::<ConfigManager>().config.clone();
        let ui_config = config.ui.clone();

        div()
            .id("settings_window_root")
            .flex()
            .flex_col()
            .size_full()
            .rounded_3xl()
            .overflow_hidden()
            .p_6()
            .bg(palette.surface_container_low)
            .text_color(palette.on_surface)
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .mb_4()
                    .child("Settings"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_grow()
                    .gap_6()
                    // Sidebar
                    .child(self.render_sidebar(&palette, cx))
                    // Content
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_grow()
                            .child(match self.active_tab {
                                SettingsTab::General => {
                                    self.render_general_settings(&ui_config, &palette, cx).into_any_element()
                                }
                                SettingsTab::Appearance => {
                                    self.render_appearance_settings(&ui_config, &palette, cx).into_any_element()
                                }
                                SettingsTab::Advanced => {
                                    self.render_advanced_settings(&config, &palette, cx).into_any_element()
                                }
                            }),
                    ),
            )
            // Footer (Done button)
            .child(
                div()
                    .mt_4()
                    .flex()
                    .justify_end()
                    .child(
                        crate::ui_components::chips::Chip::new("settings_done_chip", "Done")
                            .selected(true)
                            .on_click(cx.listener(move |_this, _, _, cx| {
                                cx.emit(SettingsEvent::Close);
                            })),
                    ),
            )
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()) // Prevent closing if click inside
    }
}
