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

        // Loop animation
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
                        // Animation State Calculation
                        let speed = 2.0;
                        let t = elapsed * speed;

                        // 1. Morph Factor: 0.0 (square) -> 1.0 (circle)
                        // Uses sine wave to oscillate smoothly
                        let morph = (t.sin() * 0.5 + 0.5).clamp(0.0, 1.0);

                        // 2. Twist Factor (Aspect Ratio Oscillation)
                        // Creates an elliptical "breathing" or "twisting" effect
                        // Phase offset by PI/2 creates a circular/elliptical motion in size space
                        let twist_w = 1.0 + 0.2 * t.sin();
                        let twist_h = 1.0 + 0.2 * (t + std::f32::consts::PI / 2.0).sin();

                        (morph, twist_w, twist_h)
                    },
                    move |bounds, (morph, twist_w, twist_h), window, cx| {
                        let theme = cx.global::<crate::theme_engine::theme::Theme>();
                        let center = bounds.center();

                        // Base size relative to container
                        let base_size = bounds.size.width.min(bounds.size.height) * 0.5;

                        // Calculate actual dimensions based on twist factors
                        let width = base_size * twist_w;
                        let height = base_size * twist_h;

                        // Center the shape
                        let origin = Point {
                            x: center.x - width / 2.0,
                            y: center.y - height / 2.0,
                        };

                        let b = Bounds {
                            origin,
                            size: gpui::size(width, height).into(),
                        };

                        // Calculate corner radius based on morph factor
                        // Square (morph=0) -> slightly rounded (0.1 * min_dim)
                        // Circle (morph=1) -> fully rounded (0.5 * min_dim)
                        let min_dim = width.min(height);
                        let corner_radius = min_dim * (0.1 + 0.4 * morph);

                        // Draw Main Shape (Primary Color)
                        window
                            .paint_quad(fill(b, theme.palette.primary).corner_radii(corner_radius));

                        // Draw Inner Shape (Secondary Color, Offset Phase)
                        // This adds complexity and the "Material" layering look
                        let inner_scale = 0.6;
                        let inner_width = width * inner_scale;
                        let inner_height = height * inner_scale;
                        let inner_origin = Point {
                            x: center.x - inner_width / 2.0,
                            y: center.y - inner_height / 2.0,
                        };
                        let inner_b = Bounds {
                            origin: inner_origin,
                            size: gpui::size(inner_width, inner_height).into(),
                        };

                        // Inner shape morphs opposite to outer shape for "squishy" feel
                        // Also pulse opacity or color if desired, but surface_container_highest provides good contrast
                        let inner_radius = inner_width.min(inner_height) * (0.5 - 0.4 * morph);

                        window.paint_quad(
                            fill(inner_b, theme.palette.surface_container_highest)
                                .corner_radii(inner_radius),
                        );
                    },
                )
                .size_12(), // Keep container size
            )
    }
}
