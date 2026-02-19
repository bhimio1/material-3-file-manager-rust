#![recursion_limit = "2048"]
mod app_state;
mod assets;
mod fs_ops;
mod theme_engine;
mod ui_components;

use gpui::prelude::*;
use gpui::*;
use std::env;
use std::path::PathBuf;

use crate::app_state::config::ConfigManager;
use crate::app_state::workspace::{Workspace, WorkspaceEvent};
use crate::assets::app_cache::AppCache;
use crate::assets::fonts;
use crate::assets::icon_cache::IconCache;
use crate::theme_engine::theme::{Theme, ThemeContext};
use crate::ui_components::settings_window::events::SettingsEvent;
use crate::ui_components::{
    dashboard::{Dashboard, DashboardEvent},
    file_list::FileList,
    navigation_toolbar::NavigationToolbar,
    preview_sidebar::PreviewSidebar,
    sidebar::{Sidebar, SidebarEvent},
    tab_bar::{TabBar, TabEvent},
};

// TabContent enum for type-safe tab management
#[derive(Clone)]
pub enum TabContent {
    Workspace {
        model: Entity<Workspace>,
        file_list: Entity<FileList>,
        dashboard: Entity<Dashboard>,
        preview_sidebar: Entity<PreviewSidebar>,
    },
    Settings(Entity<crate::ui_components::settings_window::SettingsWindow>),
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let palette = cx.theme().palette.clone();
        let _search_focus = self.search_focus_handle.clone();

        if self.active_tab_index >= self.tabs.len() {
            self.active_tab_index = self.tabs.len().saturating_sub(1);
        }

        // Pattern match on the active tab to determine what to render
        let active_tab = &self.tabs[self.active_tab_index];

        match active_tab {
            TabContent::Workspace {
                model,
                file_list,
                dashboard,
                preview_sidebar,
            } => {
                // Render workspace tab (file browser)
                self.render_workspace_tab(
                    model.clone(),
                    file_list.clone(),
                    dashboard.clone(),
                    preview_sidebar.clone(),
                    palette,
                    window,
                    cx,
                )
            }
            TabContent::Settings(settings_entity) => {
                // Render settings tab
                self.render_settings_tab(settings_entity.clone(), palette, cx)
            }
        }
    }
}

impl MainWindow {
    fn render_workspace_tab(
        &mut self,
        workspace_entity: Entity<Workspace>,
        file_list: Entity<FileList>,
        dashboard: Entity<Dashboard>,
        preview_sidebar: Entity<PreviewSidebar>,
        palette: crate::theme_engine::palette::M3Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let _search_focus = self.search_focus_handle.clone();
        let (
            active_overlay,
            context_menu_loc,
            context_menu_path,
            current_path,
            details_size,
            details_modified,
            details_mime,
            details_dim,
            is_loading,
            toasts,
        ) = {
            let workspace = workspace_entity.read(cx);
            let active_overlay = workspace.active_overlay.clone();
            let context_menu_loc = workspace.context_menu_location;
            let context_menu_path = workspace.context_menu_path.clone();
            let current_path = workspace.current_path.clone();
            let is_loading = workspace.is_loading;
            let details_file =
                if let Some(crate::app_state::workspace::ActiveOverlay::DetailsDialog(path)) =
                    &active_overlay
                {
                    workspace.items.iter().find(|i| i.path == *path).cloned()
                } else {
                    None
                };
            let (details_size, details_modified, details_mime, details_dim) = {
                if let Some(m) = &workspace.details_metadata {
                    (
                        m.size,
                        m.modified,
                        Some(m.mime_type.clone()),
                        m.image_dimensions,
                    )
                } else if let Some(f) = &details_file {
                    (f.size, f.modified, None, None)
                } else if let Some(crate::app_state::workspace::ActiveOverlay::DetailsDialog(
                    path,
                )) = &active_overlay
                {
                    if let Ok(meta) = std::fs::metadata(path) {
                        (
                            meta.len(),
                            meta.modified().unwrap_or(std::time::SystemTime::now()),
                            None,
                            None,
                        )
                    } else {
                        (0, std::time::SystemTime::now(), None, None)
                    }
                } else {
                    (0, std::time::SystemTime::now(), None, None)
                }
            };
            let toasts = workspace.toasts.clone();
            (
                active_overlay,
                context_menu_loc,
                context_menu_path,
                current_path,
                details_size,
                details_modified,
                details_mime,
                details_dim,
                is_loading,
                toasts,
            )
        };

        let search_focus = self.search_focus_handle.clone();
        let ws_entity_key = workspace_entity.clone();
        let ws_entity_click = workspace_entity.clone();

        div()
            .id("root")
            .size_full()
            .flex()
            .font_family("Inter")
            .on_key_down(move |event, window, cx| {
                let search_focus = search_focus.clone();
                let workspace_entity = ws_entity_key.clone();
                if event.keystroke.modifiers.control && event.keystroke.key == "f" {
                    window.focus(&search_focus, cx);
                } else if event.keystroke.modifiers.control && event.keystroke.key == "r" {
                    workspace_entity.update(cx, |ws, cx| {
                        ws.reload(cx);
                    });
                } else if event.keystroke.modifiers.alt && event.keystroke.key == "arrow left" {
                    workspace_entity.update(cx, |ws, cx| ws.go_back(cx));
                } else if event.keystroke.modifiers.alt && event.keystroke.key == "arrow right" {
                    workspace_entity.update(cx, |ws, cx| ws.go_forward(cx));
                } else if event.keystroke.modifiers.control && event.keystroke.key == "," {
                    workspace_entity.update(cx, |ws, cx| {
                        ws.active_overlay = Some(crate::app_state::workspace::ActiveOverlay::Settings);
                        cx.notify();
                    });
                } else if event.keystroke.modifiers.control && event.keystroke.key == "g" {
                    workspace_entity.update(cx, |ws, cx| {
                        ws.toggle_grouping(cx);
                    });
                } else if event.keystroke.key == "backspace" {
                    if !search_focus.is_focused(window) {
                        workspace_entity.update(cx, |ws, cx| ws.go_back(cx));
                    }
                }
            })
            .bg(palette.background)
            .child(
                div()
                    .id("dismissal_layer")
                    .size_full()
                    .on_click(move |_, _, cx| {
                        ws_entity_click.update(cx, |ws, cx| {
                            if ws.active_overlay.is_some() {
                                ws.dismiss_overlay(cx);
                            }
                        });
                    })
                    .child(
                        div().flex().size_full().child(self.sidebar.clone()).child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .size_full()
                                .child(self.tab_bar.clone())
                                .child(NavigationToolbar::render(
                                    workspace_entity.clone(),
                                    self.search_focus_handle.clone(),
                                    cx,
                                    window,
                                ))
                                .child(
                                    div()
                                        .when(is_loading, |d| {
                                            d.child(crate::ui_components::progress::LinearProgress::indeterminate().render(cx.theme()))
                                        })
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_1()
                                        .child(
                                            div()
                                                .flex()
                                                .flex_1()
                                                .h_full()
                                                .min_w_0()
                                                .m_2()
                                                .bg(palette.surface_container_low)
                                                .rounded_xl()
                                                .overflow_hidden()
                                                .child(
                                                    if workspace_entity.read(cx).is_dashboard {
                                                        dashboard.clone().into_any_element()
                                                    } else {
                                                        file_list.clone().into_any_element()
                                                    }
                                                )
                                        )
                                        .child(preview_sidebar.into_any_element())
                                ),
                        ),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .size_full()
                    .top_0()
                    .left_0()
                    .children(
                        if let Some(crate::app_state::workspace::ActiveOverlay::ContextMenu) =
                            &active_overlay
                        {
                            Some(
                                crate::ui_components::context_menu::ContextMenuOverlay::render(
                                    context_menu_loc,
                                    context_menu_path.unwrap_or(current_path.clone()),
                                    workspace_entity.clone(),
                                    cx,
                                ),
                            )
                        } else {
                            None
                        },
                    )
                    .children(
                        if let Some(crate::app_state::workspace::ActiveOverlay::DetailsDialog(
                            ref path,
                        )) = &active_overlay
                        {
                            Some(crate::ui_components::details_dialog::DetailsDialog::render(
                                path,
                                details_size,
                                details_modified,
                                details_mime,
                                details_dim,
                                workspace_entity.clone(),
                                cx,
                            ))
                        } else {
                            None
                        },
                    )
                    .children(
                        if let Some(crate::app_state::workspace::ActiveOverlay::Settings) =
                            &active_overlay
                        {
                            Some(
                                div()
                                    .id("settings_scrim")
                                    .absolute()
                                    .size_full()
                                    .bg(palette.scrim)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .on_mouse_down(gpui::MouseButton::Left, |_, _, _| {})
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let TabContent::Workspace { model, .. } = &this.tabs[this.active_tab_index] {
                                            model.update(cx, |ws, cx| {
                                                ws.dismiss_overlay(cx);
                                            });
                                        }
                                    }))
                                    .child(
                                        div()
                                            .id("settings_card")
                                            .on_click(|_, _, cx| {
                                                cx.stop_propagation();
                                            })
                                            .w(px(512.0))
                                            .h(px(384.0))
                                            .bg(palette.surface)
                                            .rounded_3xl()
                                            .shadow_xl()
                                            .overflow_hidden()
                                            // Settings view removed - now uses tab-based rendering
                                    ),
                            )
                        } else {
                            None
                        },
                    )
                    .children(
                        if let Some(crate::app_state::workspace::ActiveOverlay::FolderPicker) =
                            &active_overlay
                        {
                            let ws = workspace_entity.read(cx);
                            if let Some(picker) = ws.folder_picker.clone() {
                                Some(picker.into_any_element())
                            } else {
                                None
                            }
                        } else {
                            None
                        },
                    )
                    .children(
                        if let Some(crate::app_state::workspace::ActiveOverlay::OpenWith(_)) =
                            &active_overlay
                        {
                            let ws = workspace_entity.read(cx);
                            if let Some(dialog) = ws.open_with_dialog.clone() {
                                Some(dialog.into_any_element())
                            } else {
                                None
                            }
                        } else {
                            None
                        },
                    ),
            )
            .child(
                div()
                    .absolute()
                    .bottom_8()
                    .right_8()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(toasts.iter().map(|t| t.render(cx.theme()))),
            )
            .into_any_element()
    }

    fn render_settings_tab(
        &mut self,
        settings_entity: Entity<crate::ui_components::settings_window::SettingsWindow>,
        palette: crate::theme_engine::palette::M3Palette,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .flex()
            .size_full()
            .bg(palette.surface)
            .child(self.sidebar.clone())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_grow()
                    .child(self.tab_bar.clone())
                    .child(settings_entity.clone()),
            )
            .into_any_element()
    }
}

struct MainWindow {
    sidebar: Entity<Sidebar>,
    tab_bar: Entity<TabBar>,
    tabs: Vec<TabContent>,
    active_tab_index: usize,
    icon_cache: Entity<IconCache>,
    search_focus_handle: FocusHandle,
    app_cache: Entity<AppCache>,
}

impl MainWindow {
    fn new(
        cx: &mut Context<Self>,
        icon_cache: Entity<IconCache>,
        app_cache: Entity<AppCache>,
    ) -> Self {
        let initial_path = env::var("HOME")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/"));
        let home_dir = initial_path.to_string_lossy().to_string();

        // Workspace::new already returns Entity<Workspace> - don't wrap
        let workspace = Workspace::new(cx, initial_path, app_cache.clone());
        cx.subscribe(&workspace, Self::handle_workspace_event)
            .detach();

        // FileList, Dashboard, Sidebar need cx.new() wrapper
        let file_list = cx.new(|cx| FileList::new(workspace.clone(), icon_cache.clone(), cx));
        let dashboard = cx.new(|cx| Dashboard::new(cx));
        cx.subscribe(&dashboard, Self::handle_dashboard_event)
            .detach();

        let sidebar = cx.new(|cx| Sidebar::new(cx));
        cx.subscribe(&sidebar, Self::handle_sidebar_event).detach();

        let tabs = vec![TabContent::Workspace {
            model: workspace.clone(),
            file_list,
            dashboard,
            preview_sidebar: cx.new(|cx| PreviewSidebar::new(workspace.clone(), cx)),
        }];
        let tab_bar = cx.new(|_cx| TabBar::new(vec![home_dir.clone()], 0));

        cx.subscribe(&tab_bar, Self::handle_tab_event).detach();

        let mut this = Self {
            sidebar,
            tab_bar,
            tabs,
            active_tab_index: 0,
            icon_cache: icon_cache.clone(),
            search_focus_handle: cx.focus_handle(),
            app_cache,
        };
        this.update_tab_bar(cx);
        this
    }

    fn handle_sidebar_event(
        &mut self,
        _sidebar: Entity<Sidebar>,
        event: &SidebarEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            SidebarEvent::Navigate(path) => {
                if let TabContent::Workspace { model, .. } = &self.tabs[self.active_tab_index] {
                    model.update(cx, |ws, cx| {
                        ws.is_dashboard = false;
                        ws.navigate(path.clone(), cx);
                    });
                    self.update_tab_titles(cx);
                }
            }
            SidebarEvent::OpenDashboard => {
                if let TabContent::Workspace { model, .. } = &self.tabs[self.active_tab_index] {
                    model.update(cx, |ws, cx| {
                        ws.is_dashboard = true;
                        cx.notify();
                    });
                    self.update_tab_bar(cx);
                }
            }
            SidebarEvent::OpenSettings => {
                self.new_settings_tab(cx);
            }
        }
    }

    fn handle_dashboard_event(
        &mut self,
        _dashboard: Entity<Dashboard>,
        event: &DashboardEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            DashboardEvent::OpenPath(path) => {
                if let TabContent::Workspace { model, .. } = &self.tabs[self.active_tab_index] {
                    model.update(cx, |ws, cx| {
                        ws.is_dashboard = false;
                        ws.navigate(path.clone(), cx);
                    });
                    self.update_tab_bar(cx);
                }
            }
            DashboardEvent::ShowAddPinned => {
                if let TabContent::Workspace { model, .. } = &self.tabs[self.active_tab_index] {
                    model.update(cx, |ws, cx| {
                        ws.active_overlay =
                            Some(crate::app_state::workspace::ActiveOverlay::FolderPicker);
                        cx.notify();
                    });
                }
            }
        }
    }

    fn handle_tab_event(
        &mut self,
        _tab_bar: Entity<TabBar>,
        event: &TabEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            TabEvent::Activate(ix) => {
                self.active_tab_index = *ix;
                self.update_tab_bar(cx);
            }
            TabEvent::Close(ix) => {
                self.close_tab(*ix, cx);
            }
            TabEvent::NewTab => {
                self.new_tab(cx);
            }
        }
    }

    fn handle_workspace_event(
        &mut self,
        _workspace: Entity<Workspace>,
        event: &WorkspaceEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            WorkspaceEvent::PathChanged(_path) => {
                self.update_tab_bar(cx);
            }
        }
    }

    fn handle_settings_event(
        &mut self,
        _view: Entity<crate::ui_components::settings_window::SettingsWindow>,
        event: &SettingsEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            SettingsEvent::Close => {
                if let TabContent::Workspace { model, .. } = &self.tabs[self.active_tab_index] {
                    model.update(cx, |ws, cx| {
                        ws.dismiss_overlay(cx);
                    });
                }
            }
            SettingsEvent::ConfigChanged => {
                // Refresh ALL workspace tabs to apply new settings
                // (can't use active_tab_index since that points to the settings tab itself)
                for tab in &self.tabs {
                    if let TabContent::Workspace { model, .. } = tab {
                        model.update(cx, |ws, cx| {
                            // Re-navigate to current path to reload with new settings
                            let current_path = ws.current_path.clone();
                            ws.navigate(current_path, cx);
                        });
                    }
                }
            }
            SettingsEvent::ShowToast(message) => {
                for tab in &self.tabs {
                    if let TabContent::Workspace { model, .. } = tab {
                        model.update(cx, |ws, cx| {
                            ws.show_toast(
                                message.clone(),
                                crate::ui_components::toast::ToastKind::Info,
                                cx,
                            );
                        });
                        break;
                    }
                }
            }
        }
    }

    fn new_tab(&mut self, cx: &mut Context<Self>) {
        let initial_path = env::var("HOME")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/"));

        // Workspace::new already returns Entity<Workspace>, don't double-wrap
        let workspace = Workspace::new(cx, initial_path.clone(), self.app_cache.clone());
        cx.subscribe(&workspace, Self::handle_workspace_event)
            .detach();

        let file_list = cx.new(|cx| FileList::new(workspace.clone(), self.icon_cache.clone(), cx));
        let dashboard = cx.new(|cx| Dashboard::new(cx));
        cx.subscribe(&dashboard, Self::handle_dashboard_event)
            .detach();

        let preview_sidebar = cx.new(|cx| PreviewSidebar::new(workspace.clone(), cx));

        self.tabs.push(TabContent::Workspace {
            model: workspace,
            file_list,
            dashboard,
            preview_sidebar,
        });
        self.active_tab_index = self.tabs.len() - 1;

        self.update_tab_bar(cx);
        self.update_sidebar(cx);
    }

    fn new_settings_tab(&mut self, cx: &mut Context<Self>) {
        // Check if settings tab already exists (singleton pattern)
        for (index, tab) in self.tabs.iter().enumerate() {
            if matches!(tab, TabContent::Settings(_)) {
                // Settings tab exists, just switch to it
                self.active_tab_index = index;
                self.update_tab_bar(cx);
                return;
            }
        }

        // Create new settings tab
        let settings = cx.new(|cx| crate::ui_components::settings_window::SettingsWindow::new(cx));
        cx.subscribe(&settings, Self::handle_settings_event)
            .detach();

        self.tabs.push(TabContent::Settings(settings));
        self.active_tab_index = self.tabs.len() - 1;

        self.update_tab_bar(cx);
    }

    fn close_tab(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.tabs.len() > 1 && ix < self.tabs.len() {
            self.tabs.remove(ix);

            if self.active_tab_index >= self.tabs.len() {
                self.active_tab_index = self.tabs.len().saturating_sub(1);
            } else if self.active_tab_index > ix {
                self.active_tab_index = self.active_tab_index.saturating_sub(1);
            }
            self.update_tab_bar(cx);
            self.update_sidebar(cx);
        }
    }

    fn update_tab_bar(&mut self, cx: &mut Context<Self>) {
        let titles: Vec<String> = self
            .tabs
            .iter()
            .map(|tab| match tab {
                TabContent::Workspace { model, .. } => {
                    let workspace = model.read(cx);
                    if workspace.is_dashboard {
                        "Dashboard".to_string()
                    } else {
                        workspace
                            .current_path
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_else(|| {
                                let path = &workspace.current_path;
                                if path.as_os_str().is_empty() || path == std::path::Path::new("/")
                                {
                                    "/".to_string()
                                } else {
                                    path.to_string_lossy().to_string()
                                }
                            })
                    }
                }
                TabContent::Settings(_) => "Settings".to_string(),
            })
            .collect();
        let active_index = self.active_tab_index;

        self.tab_bar.update(cx, |bar, cx| {
            bar.tabs = titles;
            bar.active_index = active_index;
            cx.notify();
        });

        self.update_sidebar(cx);
        cx.notify();
    }

    fn update_sidebar(&mut self, cx: &mut Context<Self>) {
        if self.active_tab_index < self.tabs.len() {
            // Pattern match to extract workspace from TabContent
            if let TabContent::Workspace { model, .. } = &self.tabs[self.active_tab_index] {
                let workspace = model.read(cx);
                let current_path = workspace.current_path.clone();
                let is_dashboard = workspace.is_dashboard;

                self.sidebar.update(cx, |sidebar, cx| {
                    sidebar.set_state(current_path, is_dashboard, cx);
                });
            }
        }
    }

    fn update_tab_titles(&mut self, cx: &mut Context<Self>) {
        self.update_tab_bar(cx);
    }
}

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _guard = runtime.enter();

    Application::new().run(|cx: &mut App| {
        Theme::init(cx);
        Theme::watch(cx);
        fonts::load_fonts(cx);
        ConfigManager::init(cx);

        let icon_cache = IconCache::new(cx);
        let app_cache = AppCache::new(cx);
        let _ = cx.open_window(WindowOptions::default(), move |_, cx| {
            cx.new(|cx| MainWindow::new(cx, icon_cache.clone(), app_cache.clone()))
        });
    });
}
