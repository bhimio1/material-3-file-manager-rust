//! Asynchronous image thumbnails (and app icons) as egui textures.
//!
//! Requests go onto a LIFO stack served by a few worker threads, so the
//! tiles the user is looking at right now are decoded first when scrolling
//! through a large folder.

use egui::{ColorImage, TextureHandle, TextureOptions};
use lru::LruCache;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Instant, SystemTime};

const WORKERS: usize = 3;
const MAX_QUEUE: usize = 256;
const CACHE_ENTRIES: usize = 600;
/// Raster images larger than this get their thumbnail cached on disk.
const DISK_CACHE_MIN_BYTES: u64 = 512 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    path: PathBuf,
    size: u32,
    mtime: Option<SystemTime>,
}

enum Slot {
    Pending,
    Ready(TextureHandle),
    Failed,
}

enum Outcome {
    Loaded(ColorImage),
    Failed,
    /// Evicted from the queue before a worker got to it.
    Dropped,
}

pub enum Thumb {
    Loading,
    Ready(TextureHandle),
    Unavailable,
}

struct Queue {
    stack: Mutex<Vec<Key>>,
    ready: Condvar,
}

pub struct Thumbnails {
    queue: Arc<Queue>,
    results: flume::Receiver<(Key, Outcome)>,
    results_tx: flume::Sender<(Key, Outcome)>,
    cache: LruCache<Key, Slot>,
    ctx: egui::Context,
}

/// Round to a few texture sizes so tiles of similar size share textures.
fn bucket(size: f32) -> u32 {
    match size.ceil() as u32 {
        0..=96 => 96,
        97..=192 => 192,
        193..=384 => 384,
        _ => 768,
    }
}

fn disk_cache_path(key: &Key) -> Option<PathBuf> {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut h);
    dirs::cache_dir().map(|d| {
        d.join("material-3-file-manager/thumbnails")
            .join(format!("{:016x}.png", h.finish()))
    })
}

fn to_color_image(img: image::DynamicImage) -> ColorImage {
    let rgba = img.to_rgba8();
    ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    )
}

fn decode(key: &Key) -> Option<ColorImage> {
    let is_svg = key
        .path
        .extension()
        .map(|e| e.eq_ignore_ascii_case("svg") || e.eq_ignore_ascii_case("svgz"))
        .unwrap_or(false);
    if is_svg {
        let bytes = std::fs::read(&key.path).ok()?;
        return egui_extras::image::load_svg_bytes_with_size(
            &bytes,
            egui::SizeHint::Size {
                width: key.size,
                height: key.size,
                maintain_aspect_ratio: true,
            },
            &resvg::usvg::Options::default(),
        )
        .ok();
    }

    let file_len = std::fs::metadata(&key.path).map(|m| m.len()).unwrap_or(0);
    let cache_path = (file_len >= DISK_CACHE_MIN_BYTES)
        .then(|| disk_cache_path(key))
        .flatten();
    if let Some(cached) = cache_path.as_ref().and_then(|p| image::open(p).ok()) {
        return Some(to_color_image(cached));
    }

    use image::ImageDecoder;
    let reader = image::ImageReader::open(&key.path)
        .ok()?
        .with_guessed_format()
        .ok()?;
    let mut decoder = reader.into_decoder().ok()?;
    let orientation = decoder.orientation().ok();
    let mut img = image::DynamicImage::from_decoder(decoder).ok()?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    let thumb = if img.width() > key.size || img.height() > key.size {
        img.thumbnail(key.size, key.size)
    } else {
        img
    };
    if let Some(p) = cache_path {
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = thumb.save_with_format(&p, image::ImageFormat::Png);
    }
    Some(to_color_image(thumb))
}

impl Thumbnails {
    pub fn new(ctx: egui::Context) -> Self {
        let queue = Arc::new(Queue {
            stack: Mutex::new(Vec::new()),
            ready: Condvar::new(),
        });
        let (tx, rx) = flume::unbounded();
        for i in 0..WORKERS {
            let queue = queue.clone();
            let tx = tx.clone();
            let ctx = ctx.clone();
            std::thread::Builder::new()
                .name(format!("m3fm-thumb-{i}"))
                .spawn(move || loop {
                    let key = {
                        let mut stack = queue.stack.lock().unwrap();
                        loop {
                            if let Some(k) = stack.pop() {
                                break k;
                            }
                            stack = queue.ready.wait(stack).unwrap();
                        }
                    };
                    let outcome = match decode(&key) {
                        Some(img) => Outcome::Loaded(img),
                        None => Outcome::Failed,
                    };
                    if tx.send((key, outcome)).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                })
                .expect("spawn thumbnail worker");
        }
        Self {
            queue,
            results: rx,
            results_tx: tx,
            cache: LruCache::new(NonZeroUsize::new(CACHE_ENTRIES).unwrap()),
            ctx,
        }
    }

    /// Thumbnail of `path` at least `size` pixels on its long edge.
    pub fn get(&mut self, path: &Path, size: f32, mtime: Option<SystemTime>) -> Thumb {
        let key = Key {
            path: path.to_path_buf(),
            size: bucket(size),
            mtime,
        };
        match self.cache.get(&key) {
            Some(Slot::Ready(t)) => return Thumb::Ready(t.clone()),
            Some(Slot::Pending) => return Thumb::Loading,
            Some(Slot::Failed) => return Thumb::Unavailable,
            None => {}
        }
        self.cache.put(key.clone(), Slot::Pending);
        let mut stack = self.queue.stack.lock().unwrap();
        stack.push(key);
        if stack.len() > MAX_QUEUE {
            let dropped = stack.remove(0);
            let _ = self.results_tx.send((dropped, Outcome::Dropped));
        }
        drop(stack);
        self.queue.ready.notify_one();
        Thumb::Loading
    }

    /// Upload finished decodes as textures, within a small time budget so
    /// scrolling stays smooth.
    pub fn poll(&mut self) {
        let start = Instant::now();
        while start.elapsed().as_millis() < 6 {
            let Ok((key, outcome)) = self.results.try_recv() else {
                break;
            };
            match outcome {
                Outcome::Loaded(img) => {
                    let name = format!("thumb:{}:{}", key.path.display(), key.size);
                    let tex = self.ctx.load_texture(name, img, TextureOptions::LINEAR);
                    self.cache.put(key, Slot::Ready(tex));
                }
                Outcome::Failed => {
                    self.cache.put(key, Slot::Failed);
                }
                Outcome::Dropped => {
                    self.cache.pop(&key);
                }
            }
        }
        if !self.results.is_empty() {
            self.ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets() {
        assert_eq!(bucket(10.0), 96);
        assert_eq!(bucket(96.5), 192);
        assert_eq!(bucket(1000.0), 768);
    }

    #[test]
    fn decodes_png_and_svg() {
        let tmp = tempfile::tempdir().unwrap();
        let png = tmp.path().join("a.png");
        image::RgbaImage::from_pixel(400, 200, image::Rgba([255, 0, 0, 255]))
            .save(&png)
            .unwrap();
        let img = decode(&Key {
            path: png,
            size: 96,
            mtime: None,
        })
        .unwrap();
        assert_eq!(img.size, [96, 48]);

        let svg = tmp.path().join("b.svg");
        std::fs::write(&svg, r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10"/></svg>"#).unwrap();
        let img = decode(&Key {
            path: svg,
            size: 96,
            mtime: None,
        })
        .unwrap();
        assert_eq!(img.size, [96, 96]);

        assert!(decode(&Key {
            path: tmp.path().join("missing.png"),
            size: 96,
            mtime: None
        })
        .is_none());
    }
}
