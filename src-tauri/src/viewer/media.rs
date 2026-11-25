use std::path::Path;

use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

const MEDIA_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "svg", "avif", "mp4", "m4v", "webm", "mov", "ogv", "mkv", "mp3", "wav",
    "ogg", "oga", "m4a", "aac", "flac", "opus", "pdf",
];

pub fn is_media(p: &Path) -> bool {
    p.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|e| MEDIA_EXTS.contains(&e.as_str()))
}

pub fn allow(app: &AppHandle, p: &Path) -> AppResult<()> {
    if !is_media(p) {
        return Err(AppError::InvalidArgument(format!("no es un archivo multimedia: {}", p.display())));
    }
    app.asset_protocol_scope()
        .allow_file(p)
        .map_err(|e| AppError::Other(format!("asset scope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_extensions() {
        assert!(is_media(Path::new("/a/b.MP4")));
        assert!(is_media(Path::new("x.pdf")));
        assert!(!is_media(Path::new("x.rs")));
        assert!(!is_media(Path::new("noext")));
    }
}
