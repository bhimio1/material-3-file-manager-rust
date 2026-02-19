use crate::app_state::workspace::{ActiveOverlay, Workspace};
use crate::theme_engine::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;

pub struct ContextMenuOverlay;

impl ContextMenuOverlay {
    pub fn render<V: 'static>(
        position: Point<Pixels>,
        path: PathBuf, // The path the menu was opened on (unused for now but good for context)
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
                         action: Box<
            dyn Fn(&mut Workspace, &mut Context<Workspace>) + Send + Sync + 'static,
        >,
                         cx: &Context<V>| {
            let theme = cx.theme();
            let workspace = workspace.clone();
            div()
                .id(label.to_string()) // Add ID for interaction (owned string)
                .w_full()
                .px_4()
                .py_2()
                .bg(theme.palette.surface_container_high)
                .hover(|s| s.bg(theme.palette.surface_variant))
                .cursor_pointer()
                .child(label.to_string())
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    workspace.update(cx, |ws, cx| {
                        action(ws, cx);
                        // Only dismiss if it's still the context menu
                        // (Actions like "Properties" might have changed it to a dialog)
                        if let Some(ActiveOverlay::ContextMenu) = ws.active_overlay {
                            ws.dismiss_overlay(cx);
                        }
                    });
                })
        };

        div()
            .id("context_menu_container") // Add ID for interaction
            .on_click(|_, _, cx| cx.stop_propagation()) // Stop propagation to root
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .absolute()
            .left(position.x)
            .top(position.y)
            .w_64()
            .bg(theme.palette.surface_container_high)
            .rounded_lg()
            .shadow_md()
            .p_2()
            .flex()
            .flex_col()
            .gap_1()
            // Open
            .child(menu_item(
                "Open",
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
                Box::new({
                    let path = path_open.clone();
                    move |ws, cx| {
                        ws.open_in_terminal(path.clone(), cx);
                    }
                }),
                cx,
            ))
            // Separator
            .child(div().h_px().bg(theme.palette.outline_variant))
            .child(menu_item(
                "Move To...",
                Box::new(move |ws, cx| {
                    // Trigger picker with MoveSelection action
                    use crate::app_state::workspace::PickerAction;
                    ws.open_folder_picker(PickerAction::MoveSelection, cx);
                }),
                cx,
            ))
            .child(menu_item(
                "Open With...",
                Box::new(move |ws, cx| {
                    ws.open_with(path_open_with.clone(), cx);
                }),
                cx,
            ))
            .child(div().h_px().bg(theme.palette.outline_variant))
            // Cut
            .child(menu_item(
                "Cut",
                Box::new(move |ws, cx| {
                    ws.cut_selection(cx);
                }),
                cx,
            ))
            // Copy
            .child(menu_item(
                "Copy",
                Box::new(move |ws, cx| {
                    ws.copy_selection(cx);
                }),
                cx,
            ))
            // Paste
            .child(menu_item(
                "Paste",
                Box::new(move |ws, cx| {
                    ws.paste_clipboard(cx);
                }),
                cx,
            ))
            .child(div().h_px().bg(theme.palette.outline_variant))
            // Copy Path
            .child(menu_item(
                "Copy Path",
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
                Box::new(move |ws, cx| {
                    ws.delete_path(path_delete.clone(), cx);
                }),
                cx,
            ))
            // Properties
            .child(div().h_px().bg(theme.palette.outline_variant))
            .child(menu_item(
                "Properties",
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
