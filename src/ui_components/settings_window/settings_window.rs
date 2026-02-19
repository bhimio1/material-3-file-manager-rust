use super::events::SettingsEvent;
use crate::app_state::config::ConfigManager;
use crate::theme_engine::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

pub struct SettingsWindow;

impl SettingsWindow {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
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
            .on_click(|_, _, _| {
                // Stop propagation?
            })
            .text_color(palette.on_surface)
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .mb_4()
                    .child("Settings"),
            )
            .child(
                div().flex_grow().child(
                    div()
                        .flex_grow()
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .items_center()
                                .py_2()
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
                                    .on_click(cx.listener(
                                        move |_this, _, _, cx| {
                                            cx.update_global::<ConfigManager, _>(|manager, cx| {
                                                manager.config.ui.show_hidden =
                                                    !manager.config.ui.show_hidden;
                                                manager.save_config();
                                                cx.refresh_windows();
                                            });
                                            cx.emit(SettingsEvent::ConfigChanged);
                                        },
                                    )),
                                ),
                        )
                        // Separator
                        .child(div().h_px().bg(palette.outline_variant).my_2())
                        // View Mode
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .py_2()
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
                                            .on_click(
                                                cx.listener(move |_this, _, _, cx| {
                                                    cx.update_global::<ConfigManager, _>(
                                                        |manager, cx| {
                                                            manager.config.ui.view_mode =
                                                                "grid".to_string();
                                                            manager.save_config();
                                                            cx.refresh_windows();
                                                        },
                                                    );
                                                }),
                                            ),
                                        )
                                        .child(
                                            crate::ui_components::chips::Chip::new(
                                                "view_list_chip",
                                                "List",
                                            )
                                            .filter()
                                            .icon("list")
                                            .selected(ui_config.view_mode == "list")
                                            .on_click(
                                                cx.listener(move |_this, _, _, cx| {
                                                    cx.update_global::<ConfigManager, _>(
                                                        |manager, cx| {
                                                            manager.config.ui.view_mode =
                                                                "list".to_string();
                                                            manager.save_config();
                                                            cx.refresh_windows();
                                                        },
                                                    );
                                                }),
                                            ),
                                        ),
                                ),
                        )
                        // Icon Size
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .py_3()
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
                                                .child(crate::assets::icons::icon("image").size_5())
                                                .child("Icon Size"),
                                        )
                                        .child(
                                            // Read-only chip
                                            crate::ui_components::chips::Chip::new(
                                                "icon_size_chip",
                                                format!("{}px", ui_config.icon_size),
                                            ),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(palette.on_surface_variant)
                                        .child("To change icon size, edit the config file:"),
                                ),
                        )
                        // Use DMS Open
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .items_center()
                                .py_2()
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
                                    .on_click(cx.listener(
                                        move |_this, _, _, cx| {
                                            cx.update_global::<ConfigManager, _>(|manager, cx| {
                                                manager.config.use_dms = !manager.config.use_dms;
                                                manager.save_config();
                                                cx.refresh_windows();
                                            });
                                        },
                                    )),
                                ),
                        ),
                ),
            )
            // Theming Section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .py_2()
                    .child(div().h_px().bg(palette.outline_variant).my_2())
                    .child("Theming")
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                crate::ui_components::chips::Chip::new(
                                    "export_matugen_chip",
                                    "Export Matugen",
                                )
                                .icon("download")
                                .on_click(cx.listener(
                                    move |_this, _, _, cx| {
                                        match crate::theme_engine::matugen::generate_template() {
                                            Ok(path) => cx.emit(SettingsEvent::ShowToast(format!(
                                                "Template exported to {:?}",
                                                path
                                            ))),
                                            Err(e) => cx.emit(SettingsEvent::ShowToast(format!(
                                                "Error: {}",
                                                e
                                            ))),
                                        }
                                    },
                                )),
                            )
                            .child(
                                crate::ui_components::chips::Chip::new(
                                    "reload_theme_chip",
                                    "Reload Theme",
                                )
                                .icon("refresh")
                                .on_click(cx.listener(
                                    move |_this, _, _, cx| {
                                        cx.update_global::<crate::theme_engine::theme::Theme, _>(
                                            |theme, _| {
                                                theme.palette =
                                                    crate::theme_engine::theme::Theme::load_palette(
                                                    );
                                            },
                                        );
                                        cx.refresh_windows();
                                        cx.emit(SettingsEvent::ShowToast(
                                            "Theme reloaded".to_string(),
                                        ));
                                    },
                                )),
                            ),
                    )
                    .child(
                        div().mt_2().child(
                            div()
                                .flex()
                                .justify_between()
                                .items_center()
                                .child(div().text_xs().child("Matugen config example"))
                                .child(
                                    crate::ui_components::chips::Chip::new(
                                        "copy_matugen_chip",
                                        "Copy",
                                    )
                                    .icon("copy")
                                    .on_click(cx.listener(
                                        move |_this, _, _, cx| {
                                            let example = r#"[templates.material_3_file_manager]
input_path = "~/.config/material_3_file_manager/matugen_template.json"
output_path = "~/.config/material_3_file_manager/theme.json""#;
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                example.to_string(),
                                            ));
                                            cx.emit(SettingsEvent::ShowToast(
                                                "Copied to clipboard".to_string(),
                                            ));
                                        },
                                    )),
                                ),
                        ),
                    ),
            )
            // Done Button
            .child(
                div().mt_4().flex().justify_end().child(
                    crate::ui_components::chips::Chip::new("settings_done_chip", "Done")
                        .selected(true)
                        .on_click(cx.listener(move |_this, _, _, cx| {
                            cx.emit(SettingsEvent::Close);
                        })),
                ),
            )
    }
}
