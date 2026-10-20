//! Material 3 File Manager — an egui file manager themed with Material You.

mod app;
mod config;
mod fs;
mod tab;
mod theme;
mod thumbs;
mod ui;

use std::path::PathBuf;

const USAGE: &str = "Usage: m3fm [OPTIONS] [FOLDER]

Options:
  --portal     Serve xdg-desktop-portal FileChooser requests for other apps
  -h, --help   Show this help
  -V, --version";

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let mut location = None;
    let mut portal = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--portal" => portal = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            "-V" | "--version" => {
                println!("m3fm {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            other if other.starts_with('-') => {
                eprintln!("Unknown option: {other}\n\n{USAGE}");
                std::process::exit(2);
            }
            path => {
                let path = PathBuf::from(path);
                location = Some(std::fs::canonicalize(&path).unwrap_or(path));
            }
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Files")
            .with_app_id("material_3_file_manager")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "material_3_file_manager",
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::App::new(
                cc,
                app::Startup { location, portal },
            )))
        }),
    )
}
