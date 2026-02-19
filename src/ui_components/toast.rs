use gpui::*;

#[derive(Clone, PartialEq)]
pub enum ToastKind {
    Info,
    Error,
    Success,
}

#[derive(Clone)]
pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
    pub id: u64,
}

use crate::theme_engine::theme::Theme;

impl Toast {
    pub fn render(&self, theme: &Theme) -> impl IntoElement {
        let (bg, text) = match self.kind {
            ToastKind::Info => (
                theme.palette.inverse_surface,
                theme.palette.inverse_on_surface,
            ),
            ToastKind::Error => (
                theme.palette.error_container,
                theme.palette.on_error_container,
            ),
            ToastKind::Success => (
                theme.palette.primary_container,
                theme.palette.on_primary_container,
            ),
        };

        div()
            .flex()
            .items_center()
            .py_2()
            .px_4()
            .rounded_lg()
            .bg(bg)
            .text_color(text)
            .shadow_md()
            .child(self.message.clone())
    }
}
