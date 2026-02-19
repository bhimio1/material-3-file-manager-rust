use mime_guess::Mime;
use std::path::Path;

pub struct MimeResolver;

impl MimeResolver {
    pub fn get_mime(path: &Path) -> Mime {
        mime_guess::from_path(path).first_or_octet_stream()
    }

    pub fn get_icon_name(path: &Path, is_dir: bool) -> String {
        if is_dir {
            return "folder".to_string();
        }

        // This is a simplified mapping. Real implementation would use xdg-mime query if needed
        // or freedesktop_icons crate logic details.
        let mime = Self::get_mime(path);
        let subtype = mime.subtype().as_str();

        match (mime.type_().as_str(), subtype) {
            ("image", _) => "image-x-generic".to_string(),
            ("video", _) => "video-x-generic".to_string(),
            ("audio", _) => "audio-x-generic".to_string(),
            ("text", "rust") => "text-rust".to_string(), // specialized if available
            ("text", _) => "text-x-generic".to_string(),
            ("application", "pdf") => "application-pdf".to_string(),
            _ => "unknown".to_string(),
        }
    }
}
