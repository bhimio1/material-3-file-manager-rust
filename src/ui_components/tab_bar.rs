use crate::assets::icons::icon;
use crate::theme_engine::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

pub struct TabBar {
    pub tabs: Vec<String>,
    pub active_index: usize,
    scroll_handle: ScrollHandle,
}

pub enum TabEvent {
    Activate(usize),
    Close(usize),
    NewTab,
}

impl EventEmitter<TabEvent> for TabBar {}

impl TabBar {
    pub fn new(tabs: Vec<String>, active_index: usize) -> Self {
        Self {
            tabs,
            active_index,
            scroll_handle: ScrollHandle::new(),
        }
    }
}

impl Render for TabBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Extract copyable colors to avoid holding theme borrow
        let palette = &cx.theme().palette;
        let surface = palette.surface;
        let surface_variant = palette.surface_variant;
        let on_surface_variant = palette.on_surface_variant;

        let entity = cx.entity().clone();

        div()
            .flex()
            .flex_row()
            .w_full()
            .h_12()
            .bg(surface)
            .items_center()
            .child(
                div()
                    .id("tabs-scroll-view")
                    .flex_grow()
                    .overflow_x_scroll()
                    .track_scroll(&self.scroll_handle)
                    .child(
                        div().flex().flex_row().gap_2().children(
                            self.tabs.iter().enumerate().map(|(ix, title)| {
                                render_tab(ix, title, ix == self.active_index, cx)
                            }),
                        ),
                    ),
            )
            .child(
                div()
                    .id("new-tab-button")
                    .p_2()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .hover(|s| s.bg(surface_variant))
                    .cursor_pointer()
                    .on_click(move |_, _, cx| {
                        entity.update(cx, |_, cx| cx.emit(TabEvent::NewTab));
                    })
                    .child(icon("add").text_color(on_surface_variant)),
            )
    }
}

fn render_tab(ix: usize, title: &str, active: bool, cx: &mut Context<TabBar>) -> impl IntoElement {
    let theme = cx.theme();
    let entity = cx.entity().clone();
    let entity_close = entity.clone();

    // Use secondary_container for active (consistent with sidebar)
    let bg = if active {
        theme.palette.secondary_container
    } else {
        gpui::rgba(0x00000000)
    };
    let text = if active {
        theme.palette.on_secondary_container
    } else {
        theme.palette.on_surface_variant
    };

    let hover_bg = if active {
        theme.palette.secondary_container
    } else {
        theme.palette.surface_container_highest
    };

    div()
        .id(ElementId::Name(format!("tab-{}", ix).into()))
        .flex()
        .flex_row()
        .items_center()
        .p_2()
        .pl_4()
        .pr_2()
        .rounded_full()
        .bg(bg)
        .text_color(text)
        .cursor_pointer()
        .hover(|s| s.bg(hover_bg))
        .on_click(move |_, _, cx| {
            entity.update(cx, |_, cx| cx.emit(TabEvent::Activate(ix)));
        })
        .child(
            div()
                .mr_2()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child(title.to_string()),
        )
        .child(
            div()
                .id(format!("close-tab-{}", ix))
                .p_1()
                .rounded_full()
                .hover(|s| s.bg(theme.palette.surface_variant)) // Sudo-transparent feel
                .on_click(move |_, _, cx| {
                    entity_close.update(cx, |_, cx| cx.emit(TabEvent::Close(ix)));
                })
                .child(icon("close").size_4().text_color(text)), // Use text color (handles active/inactive)
        )
}
