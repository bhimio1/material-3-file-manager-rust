//! Opt-in performance logging: run with `M3FM_PERF=1` to print frame
//! timing once a second and the cost of notable operations to stderr.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("M3FM_PERF").is_some_and(|v| v != "0"))
}

/// Run `f`, logging how long it took when perf logging is on.
pub fn time<T>(label: &str, f: impl FnOnce() -> T) -> T {
    if !enabled() {
        return f();
    }
    let start = Instant::now();
    let out = f();
    eprintln!(
        "[perf] {label}: {:.2} ms",
        start.elapsed().as_secs_f64() * 1e3
    );
    out
}

/// Rolling per-second frame statistics.
#[derive(Default)]
pub struct FrameStats {
    ui_ms: Vec<f32>,
    cpu_ms: Vec<f32>,
    causes: std::collections::HashMap<String, usize>,
    window_start: Option<Instant>,
}

impl FrameStats {
    /// `ui_ms`: time spent building this frame's UI. `cpu_ms`: eframe's
    /// measurement of the whole previous frame (UI + tessellation + paint).
    pub fn record(&mut self, ctx: &egui::Context, ui_ms: f32, cpu_ms: Option<f32>) {
        if !enabled() {
            return;
        }
        for cause in ctx.repaint_causes() {
            *self
                .causes
                .entry(format!("{}:{} {}", cause.file, cause.line, cause.reason))
                .or_default() += 1;
        }
        self.ui_ms.push(ui_ms);
        self.cpu_ms.extend(cpu_ms);
        let start = *self.window_start.get_or_insert_with(Instant::now);
        if start.elapsed() >= Duration::from_secs(1) {
            let stats = |v: &mut Vec<f32>| -> String {
                if v.is_empty() {
                    return "-".into();
                }
                v.sort_by(|a, b| a.total_cmp(b));
                let avg = v.iter().sum::<f32>() / v.len() as f32;
                let p95 = v[((v.len() as f32 * 0.95) as usize).min(v.len() - 1)];
                format!("avg {avg:.2} p95 {p95:.2} max {:.2}", v[v.len() - 1])
            };
            eprintln!(
                "[perf] {} frames | ui ms: {} | frame cpu ms: {}",
                self.ui_ms.len(),
                stats(&mut self.ui_ms),
                stats(&mut self.cpu_ms)
            );
            let mut causes: Vec<_> = self.causes.drain().collect();
            causes.sort_by_key(|c| std::cmp::Reverse(c.1));
            for (cause, n) in causes.iter().take(3) {
                eprintln!("[perf]   repaint x{n}: {cause}");
            }
            self.ui_ms.clear();
            self.cpu_ms.clear();
            self.window_start = Some(Instant::now());
        }
    }
}
