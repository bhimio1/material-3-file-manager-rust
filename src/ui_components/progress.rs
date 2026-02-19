use crate::theme_engine::theme::Theme;
use gpui::prelude::*;
use gpui::*;

pub struct LinearProgress {
    indeterminate: bool,
    value: f32, // 0.0 to 1.0
}

impl LinearProgress {
    pub fn indeterminate() -> Self {
        Self {
            indeterminate: true,
            value: 0.0,
        }
    }

    pub fn determinate(value: f32) -> Self {
        Self {
            indeterminate: false,
            value: value.clamp(0.0, 1.0),
        }
    }

    pub fn render(self, theme: &Theme) -> impl IntoElement {
        let is_indeterminate = self.indeterminate;
        let value = self.value;

        div()
            .h_1()
            .w_full()
            .bg(theme.palette.surface_container_high) // Track color
            .overflow_hidden()
            .child(
                div()
                    .h_full()
                    .bg(theme.palette.primary)
                    .when(is_indeterminate, |d| d.w_1_2().mx_auto())
                    .when(!is_indeterminate, |d| {
                        d.w(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                            value,
                        )))
                    }),
            )
    }
}
