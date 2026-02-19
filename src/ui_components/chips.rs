use crate::theme_engine::theme::Theme;
use gpui::prelude::*;
use gpui::{
    div, App, ClickEvent, ElementId, InteractiveElement, IntoElement, RenderOnce, SharedString,
    Styled, Window,
};

#[derive(Clone, PartialEq)]
pub enum ChipType {
    Filter,
    Action,
}

#[derive(IntoElement)]
pub struct Chip {
    id: ElementId,
    label: SharedString,
    icon: Option<SharedString>,
    selected: bool,
    chip_type: ChipType,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + Send + Sync + 'static>>,
}

impl Chip {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            selected: false,
            chip_type: ChipType::Action,
            on_click: None,
        }
    }

    pub fn filter(mut self) -> Self {
        self.chip_type = ChipType::Filter;
        self
    }

    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Chip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.global::<Theme>();

        let (bg, fg, border) = if self.selected {
            (
                theme.palette.secondary_container,
                theme.palette.on_secondary_container,
                gpui::rgba(0x00000000),
            )
        } else {
            (
                gpui::rgba(0x00000000),
                theme.palette.on_surface_variant,
                theme.palette.outline,
            )
        };

        let mut el = div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .h_8()
            .rounded_lg()
            .border_1()
            .border_color(border)
            .bg(bg)
            .text_color(fg)
            .cursor_pointer()
            .hover(|s| {
                s.bg(if self.selected {
                    theme.palette.secondary_container
                } else {
                    theme.palette.surface_container_highest
                })
            })
            .id(self.id);

        if let Some(handler) = self.on_click {
            el = el.on_click(move |event, window, cx| handler(event, window, cx));
        }

        el.child(
            if self.selected && matches!(self.chip_type, ChipType::Filter) {
                crate::assets::icons::icon("check")
                    .size_4()
                    .text_color(fg)
                    .into_any_element()
            } else if let Some(icon) = &self.icon {
                crate::assets::icons::icon(icon)
                    .size_4()
                    .text_color(fg)
                    .into_any_element()
            } else {
                div().into_any_element()
            },
        )
        .child(div().text_sm().child(self.label))
    }
}
