use crate::theme_engine::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;

pub struct Sidebar {
    active_path: PathBuf,
    is_dashboard: bool,
    drives: Vec<(String, PathBuf)>,
}

pub enum SidebarEvent {
    Navigate(PathBuf),
    OpenDashboard,
    OpenSettings,
}

impl EventEmitter<SidebarEvent> for Sidebar {}

impl Sidebar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".into());

        cx.spawn(move |this: WeakEntity<Sidebar>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                loop {
                    let disks = sysinfo::Disks::new_with_refreshed_list();
                    let mut new_drives = Vec::new();
                    for disk in disks.iter() {
                        let name = disk.name().to_string_lossy().to_string();
                        let mount = disk.mount_point().to_path_buf();
                        new_drives.push((
                            if name.is_empty() {
                                "Drive".to_string()
                            } else {
                                name
                            },
                            mount,
                        ));
                    }

                    let _ = this.update(&mut cx, |this, cx| {
                        if this.drives != new_drives {
                            this.drives = new_drives;
                            cx.notify();
                        }
                    });

                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(5))
                        .await;
                }
            }
        })
        .detach();

        Self {
            active_path: PathBuf::from(home),
            is_dashboard: true,
            drives: Vec::new(),
        }
    }

    pub fn set_state(&mut self, path: PathBuf, is_dashboard: bool, cx: &mut Context<Self>) {
        self.active_path = path;
        self.is_dashboard = is_dashboard;
        cx.notify();
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".into());

        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .w_64() // Increased width slightly
            .h_full()
            .bg(theme.palette.surface)
            .text_color(theme.palette.on_surface)
            .border_r_1()
            .border_color(theme.palette.outline_variant)
            .p_2()
            // Places header
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.palette.on_surface_variant)
                    .px_3()
                    .py_2()
                    .child("Places"),
            )
            .child(
                div()
                    .id("sidebar_dashboard")
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .rounded_full()
                    .text_color(if self.is_dashboard {
                        theme.palette.on_secondary_container
                    } else {
                        theme.palette.on_surface_variant
                    })
                    .bg(if self.is_dashboard {
                        theme.palette.secondary_container
                    } else {
                        gpui::rgba(0x00000000)
                    })
                    .font_weight(if self.is_dashboard {
                        FontWeight::BOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .cursor_pointer()
                    .hover(|s| {
                        if !self.is_dashboard {
                            s.bg(theme.palette.surface_container_highest)
                        } else {
                            s
                        }
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        if !this.is_dashboard {
                            cx.emit(SidebarEvent::OpenDashboard);
                        }
                    }))
                    .child(crate::assets::icons::icon("grid").size_5())
                    .child("Dashboard"),
            )
            .child(sidebar_item(
                "Home",
                &home,
                &self.active_path,
                self.is_dashboard,
                cx,
            ))
            .child(sidebar_item(
                "Desktop",
                &format!("{}/Desktop", home),
                &self.active_path,
                self.is_dashboard,
                cx,
            ))
            .child(sidebar_item(
                "Documents",
                &format!("{}/Documents", home),
                &self.active_path,
                self.is_dashboard,
                cx,
            ))
            .child(sidebar_item(
                "Downloads",
                &format!("{}/Downloads", home),
                &self.active_path,
                self.is_dashboard,
                cx,
            ))
            .child(sidebar_item(
                "Music",
                &format!("{}/Music", home),
                &self.active_path,
                self.is_dashboard,
                cx,
            ))
            .child(sidebar_item(
                "Pictures",
                &format!("{}/Pictures", home),
                &self.active_path,
                self.is_dashboard,
                cx,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .children(if !self.drives.is_empty() {
                        Some(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.palette.on_surface_variant)
                                .px_3()
                                .py_2()
                                .mt_2()
                                .child("Drives"),
                        )
                    } else {
                        None
                    })
                    .children(self.drives.iter().map(|(name, path)| {
                        sidebar_item(
                            name,
                            &path.to_string_lossy(),
                            &self.active_path,
                            self.is_dashboard,
                            cx,
                        )
                    })),
            )
            .child(
                div().mt_auto().child(
                    div()
                        .id("sidebar_settings")
                        .w_full()
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .text_color(theme.palette.on_surface_variant)
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.palette.surface_variant))
                        .on_click(cx.listener(|_, _, _, cx| {
                            cx.emit(SidebarEvent::OpenSettings);
                        }))
                        .child(crate::assets::icons::icon("settings").size_5())
                        .child("Settings"),
                ),
            )
    }
}

fn sidebar_item(
    label: &str,
    path: &str,
    active_path: &PathBuf,
    is_dashboard: bool,
    cx: &Context<Sidebar>,
) -> impl IntoElement {
    let theme = cx.theme();
    let path_buf = PathBuf::from(path);
    // Exact match for now
    let active = !is_dashboard && active_path == &path_buf;

    let bg = if active {
        theme.palette.secondary_container
    } else {
        gpui::rgba(0x00000000)
    };
    let color = if active {
        theme.palette.on_secondary_container
    } else {
        theme.palette.on_surface_variant
    };

    div()
        .id(SharedString::from(format!("sidebar_item_{}", label)))
        .w_full()
        .flex()
        .items_center()
        .gap_3()
        .px_4() // Increased horizontal padding
        .py_3() // Increased vertical padding for better touch target
        .rounded_full() // M3 Stadium shape
        .bg(bg)
        .text_color(color)
        .font_weight(if active {
            FontWeight::BOLD
        } else {
            FontWeight::MEDIUM
        }) // Emphasize active
        .cursor_pointer()
        .hover(|s| {
            if !active {
                s.bg(theme.palette.surface_container_highest)
            } else {
                s
            }
        })
        .on_click(cx.listener(move |_, _, _, cx| {
            cx.emit(SidebarEvent::Navigate(path_buf.clone()));
        }))
        .child(
            crate::assets::icons::icon(match label {
                "Home" => "home",
                "Downloads" => "download",
                "Desktop" => "folder",
                "Documents" => "description",
                "Music" => "audio",
                "Pictures" => "image",
                _ => "folder",
            })
            .size_5(),
        )
        .child(label.to_string())
}
