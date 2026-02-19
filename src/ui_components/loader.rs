use gpui::prelude::*;
use gpui::{canvas, div, fill, rgba, Bounds, Context, IntoElement, Point, Render, Window};
use std::time::Instant;

pub struct ShapeShifterLoader {
    start_time: Instant,
}

impl ShapeShifterLoader {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            start_time: Instant::now(),
        }
    }
}

impl Render for ShapeShifterLoader {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let elapsed = self.start_time.elapsed().as_secs_f32();

        // Loop animation - closure takes (view, window, context)
        cx.on_next_frame(window, |_this, _window, cx| {
            cx.notify();
        });

        div()
            .size_full()
            .flex()
            .justify_center()
            .items_center()
            .bg(rgba(0x00000000))
            .child(
                canvas(
                    move |_bounds, _window, _cx| {
                        let t = elapsed % 4.0;
                        let phase = t as u32;
                        let radius_factor = match phase {
                            0 => 0.1, // Square
                            1 => 0.5, // Pill
                            2 => 0.2, // Shape 3
                            3 => 0.8, // Flower
                            _ => 0.2,
                        };
                        radius_factor
                    },
                    move |bounds, radius_factor, window, cx| {
                        let theme = cx.global::<crate::theme_engine::theme::Theme>();
                        let center = bounds.center();
                        let max_size = bounds.size.width.min(bounds.size.height) * 0.6;
                        let current_size = max_size * (0.9 + 0.1 * (elapsed * 5.0).sin().abs());

                        let origin = Point {
                            x: center.x - current_size / 2.0,
                            y: center.y - current_size / 2.0,
                        };

                        let b = Bounds {
                            origin,
                            size: gpui::size(current_size, current_size).into(),
                        };

                        window.paint_quad(
                            fill(b, theme.palette.primary)
                                .corner_radii(current_size * radius_factor),
                        );
                    },
                )
                .size_12(),
            )
    }
}
