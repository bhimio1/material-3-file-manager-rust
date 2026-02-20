use crate::app_state::workspace::{ActiveOverlay, Workspace};
use crate::theme_engine::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;

pub struct ContextMenuOverlay;

impl ContextMenuOverlay {
    pub fn render<V: 'static>(
        position: Point<Pixels>,
        path: PathBuf,
        workspace: Entity<Workspace>,
        cx: &Context<V>,
    ) -> impl IntoElement {
        let theme = cx.theme();

        // Clones for closures
        let path_open = path.clone();
        let path_open_with = path.clone();
        let path_copy = path.clone();
        let path_delete = path.clone();
        let path_props = path.clone();

        // Helper to create menu items
        let menu_item = |label: &str,
                         icon_name: Option<&str>,
                         action: Box<
            dyn Fn(&mut Workspace, &mut Context<Workspace>) + Send + Sync + 'static,
        >,
                         cx: &Context<V>| {
            let theme = cx.theme();
            let workspace = workspace.clone();
            div()
                .id(label.to_string())
                .w_full()
                .flex()
                .items_center()
                .gap_3()
                .px_3()
                .py_2()
                .rounded_md() // M3 dropdown item rounding
                .bg(gpui::rgba(0x00000000)) // Transparent default
                .hover(|s| s.bg(theme.palette.surface_container_highest))
                .active(|s| s.bg(theme.palette.secondary_container).text_color(theme.palette.on_secondary_container))
                .cursor_pointer()
                .child(if let Some(name) = icon_name {
                    crate::assets::icons::icon(name).size_5().text_color(theme.palette.on_surface_variant).into_any_element()
                } else {
                    div().w_5().into_any_element() // Spacer
                })
                .child(div().text_sm().child(label.to_string()))
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    workspace.update(cx, |ws, cx| {
                        action(ws, cx);
                        if let Some(ActiveOverlay::ContextMenu) = ws.active_overlay {
                            ws.dismiss_overlay(cx);
                        }
                    });
                })
        };

        div()
            .id("context_menu_container")
            .on_click(|_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .absolute()
            .left(position.x)
            .top(position.y)
            .w_64()
            .bg(theme.palette.surface_container_high)
            .rounded_xl()
            .shadow_lg()
            .p_1()
            .flex()
            .flex_col()
            .gap_0()
            // Open
            .child(menu_item(
                "Open",
                Some("folder"),
                Box::new({
                    let path = path_open.clone();
                    move |ws, cx| {
                        ws.open(path.clone(), cx);
                    }
                }),
                cx,
            ))
            .child(menu_item(
                "Open in Terminal",
                Some("code"),
                Box::new({
                    let path = path_open.clone();
                    move |ws, cx| {
                        ws.open_in_terminal(path.clone(), cx);
                    }
                }),
                cx,
            ))
            // Separator
            .child(div().h_px().bg(theme.palette.outline_variant).my_1().mx_2())
            .child(menu_item(
                "Move To...",
                Some("arrow_forward"),
                Box::new(move |ws, cx| {
                    use crate::app_state::workspace::PickerAction;
                    ws.open_folder_picker(PickerAction::MoveSelection, cx);
                }),
                cx,
            ))
            .child(menu_item(
                "Open With...",
                Some("grid"),
                Box::new(move |ws, cx| {
                    ws.open_with(path_open_with.clone(), cx);
                }),
                cx,
            ))
            .child(div().h_px().bg(theme.palette.outline_variant).my_1().mx_2())
            // Cut
            .child(menu_item(
                "Cut",
                None,
                Box::new(move |ws, cx| {
                    ws.cut_selection(cx);
                }),
                cx,
            ))
            // Copy
            .child(menu_item(
                "Copy",
                Some("copy"),
                Box::new(move |ws, cx| {
                    ws.copy_selection(cx);
                }),
                cx,
            ))
            // Paste
            .child(menu_item(
                "Paste",
                None,
                Box::new(move |ws, cx| {
                    ws.paste_clipboard(cx);
                }),
                cx,
            ))
            .child(div().h_px().bg(theme.palette.outline_variant).my_1().mx_2())
            // Copy Path
            .child(menu_item(
                "Copy Path",
                Some("description"),
                Box::new(move |ws, cx| {
                    ws.copy_text_to_clipboard(path_copy.to_string_lossy().to_string(), cx);
                    ws.show_toast(
                        "Path copied to clipboard".to_string(),
                        crate::ui_components::toast::ToastKind::Info,
                        cx,
                    );
                }),
                cx,
            ))
            // Delete
            .child(menu_item(
                "Delete",
                Some("remove"),
                Box::new(move |ws, cx| {
                    ws.delete_path(path_delete.clone(), cx);
                }),
                cx,
            ))
            // Properties
            .child(div().h_px().bg(theme.palette.outline_variant).my_1().mx_2())
            .child(menu_item(
                "Properties",
                Some("settings"),
                Box::new(move |ws, cx| {
                    ws.active_overlay = Some(ActiveOverlay::DetailsDialog(path_props.clone()));
                    ws.details_metadata = None;
                    ws.load_details_metadata(path_props.clone(), cx);
                    cx.notify();
                }),
                cx,
            ))
    }
}
