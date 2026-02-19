use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

pub struct ThumbnailWorker;

impl ThumbnailWorker {
    pub fn new() -> Self {
        Self
    }

    fn cache_path(path: &PathBuf) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        let hash = hasher.finish();
        dirs::cache_dir()
            .unwrap_or(PathBuf::from("/tmp"))
            .join("material-3-file-manager/thumbnails")
            .join(format!("{:x}.jpg", hash))
    }

    pub fn get_cached_path(path: &PathBuf) -> Option<PathBuf> {
        let p = Self::cache_path(path);
        if p.exists() {
            Some(p)
        } else {
            None
        }
    }

    pub fn generate_thumbnail(path: PathBuf) -> Option<PathBuf> {
        let cache_path = Self::cache_path(&path);
        // println!("ThumbnailWorker: Check cache for {:?} -> {:?}", path, cache_path);

        if cache_path.exists() {
            return Some(cache_path);
        }

        println!("ThumbnailWorker: Generating for {:?}", path);

        if let Some(parent) = cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        match image::open(&path) {
            Ok(img) => {
                let thumb = img.thumbnail(256, 256);
                if let Err(e) = thumb.save(&cache_path) {
                    println!("ThumbnailWorker: Failed to save thumb: {}", e);
                    return None;
                }
                println!("ThumbnailWorker: Generated {:?}", cache_path);
                Some(cache_path)
            }
            Err(e) => {
                println!("ThumbnailWorker: Failed to open image {:?}: {}", path, e);
                None
            }
        }
    }
}
