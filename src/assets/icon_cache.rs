use crate::assets::icons::IconPaths;
use gpui::*;
use gpui::{AppContext, AsyncApp, Context, Entity, Hsla, RenderImage};
use image::{Frame, RgbaImage};
use lru::LruCache;
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};
use smallvec::SmallVec;
use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum IconType {
    Path(PathBuf),
    System(String), // "folder", "audio", "video", etc.
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CacheKey {
    icon: IconType,
    color_hex: String,
}

pub struct IconCache {
    cache: LruCache<CacheKey, Arc<RenderImage>>,
    loading: HashSet<CacheKey>,
}

impl IconCache {
    pub fn new<T: AppContext>(cx: &mut T) -> Entity<Self> {
        cx.new(|_cx| Self {
            cache: LruCache::new(NonZeroUsize::new(256).unwrap()),
            loading: HashSet::new(),
        })
    }

    pub fn get(
        &mut self,
        icon_type: IconType,
        color: Hsla,
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        // Actually, let's use a simpler hex representation or just the debug string if it's stable.
        // For performance/correctness, converting to u32 Rgba is better.
        let rgba = color.to_rgb();
        let color_hex = format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            (rgba.r * 255.0) as u8,
            (rgba.g * 255.0) as u8,
            (rgba.b * 255.0) as u8,
            (rgba.a * 255.0) as u8
        );

        let key = CacheKey {
            icon: icon_type,
            color_hex: color_hex.clone(),
        };

        if let Some(img) = self.cache.get(&key) {
            return Some(img.clone());
        }

        if !self.loading.contains(&key) {
            self.loading.insert(key.clone());
            let key_clone = key.clone();
            let color_struct = rgba; // Copy Rgba (it's Copy)

            let this = cx.entity().downgrade();
            let cx = cx.to_async();
            let mut cx_clone = cx.clone();

            cx.spawn(move |_: &mut AsyncApp| async move {
                let image_data = Self::load_icon_data(&key_clone.icon, color_struct).await;

                if let Some(handle) = this.upgrade() {
                    cx_clone.update_entity(&handle, |this, cx| {
                        this.loading.remove(&key_clone);
                        if let Some(data) = image_data {
                            let render_image = Arc::new(RenderImage::new(SmallVec::from_elem(
                                Frame::new(data),
                                1,
                            )));
                            this.cache.put(key_clone, render_image);
                            cx.notify();
                        }
                    });
                }
            })
            .detach();
        }

        None
    }

    async fn load_icon_data(icon: &IconType, color: Rgba) -> Option<RgbaImage> {
        // 1. Get SVG content
        let svg_string = match icon {
            IconType::Path(p) => std::fs::read_to_string(p).ok()?,
            IconType::System(name) => {
                let path_d = match name.as_str() {
                    "folder" => IconPaths::FOLDER,
                    "audio" => IconPaths::AUDIO,
                    "video" => IconPaths::VIDEO,
                    "image" => IconPaths::IMAGE,
                    "archive" => IconPaths::ARCHIVE,
                    "settings" => IconPaths::SETTINGS,
                    "list" => IconPaths::LIST,
                    "grid" => IconPaths::GRID,
                    "back" => IconPaths::BACK,
                    "forward" => IconPaths::FORWARD,
                    "refresh" => IconPaths::REFRESH,
                    "close" => IconPaths::CLOSE,
                    "add" => IconPaths::ADD,
                    _ => IconPaths::FILE_GENERIC,
                };
                // Construct SVG with fill color
                let hex = format!(
                    "#{:02x}{:02x}{:02x}",
                    (color.r * 255.0) as u8,
                    (color.g * 255.0) as u8,
                    (color.b * 255.0) as u8
                );
                format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" height="24" viewBox="0 0 24 24" width="24" fill="{}"><path d="{}"/></svg>"#,
                    hex, path_d
                )
            }
        };

        // 2. Render SVG
        let opt = Options::default();
        let fontdb = resvg::usvg::fontdb::Database::new(); // Create default fontdb
        let tree = Tree::from_data(svg_string.as_bytes(), &opt, &fontdb).ok()?;

        let size = tree.size().to_int_size();
        let mut pixmap = Pixmap::new(size.width(), size.height())?;

        resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());

        // 3. Convert RGBA -> BGRA
        let mut data = pixmap.take();
        for chunk in data.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }

        RgbaImage::from_raw(size.width(), size.height(), data)
    }
}
