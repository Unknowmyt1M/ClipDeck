use std::path::{Path, PathBuf};

use base64::Engine;
use image::RgbaImage;
use image::imageops::FilterType;

/// Handles on-disk storage for captured clipboard images.
///
/// Images live under `<app_data>/images/` as PNG files named by item UUID,
/// with a small `<uuid>_thumb.png` for list rendering.
pub struct ImageService {
    images_dir: PathBuf,
}

impl ImageService {
    pub fn new(app_data_dir: PathBuf) -> Result<Self, String> {
        let images_dir = app_data_dir.join("images");
        std::fs::create_dir_all(&images_dir)
            .map_err(|e| format!("failed to create images directory: {e}"))?;
        Ok(Self { images_dir })
    }

    /// Save raw RGBA pixel data as a PNG plus a 320px-wide thumbnail.
    /// Returns (full_path, thumb_path).
    pub fn save_rgba(
        &self,
        uuid: &str,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<(PathBuf, PathBuf), String> {
        let img = RgbaImage::from_raw(width, height, rgba.to_vec())
            .ok_or_else(|| "invalid image buffer from clipboard".to_string())?;

        let full_path = self.path_for(uuid, false);
        img.save(&full_path)
            .map_err(|e| format!("failed to save image: {e}"))?;

        let thumb = if width > THUMB_MAX_WIDTH {
            let ratio = THUMB_MAX_WIDTH as f32 / width as f32;
            let h = (height as f32 * ratio).round().max(1.0) as u32;
            image::DynamicImage::ImageRgba8(img.clone()).resize(
                THUMB_MAX_WIDTH,
                h,
                FilterType::Triangle,
            )
        } else {
            image::DynamicImage::ImageRgba8(img.clone())
        };
        let thumb_path = self.path_for(uuid, true);
        thumb
            .save(&thumb_path)
            .map_err(|e| format!("failed to save thumbnail: {e}"))?;

        Ok((full_path, thumb_path))
    }

    /// Read an image file and return it as a base64 data URL.
    pub fn read_as_data_url(&self, path: &Path) -> Result<Option<String>, String> {
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(path).map_err(|e| format!("failed to read image: {e}"))?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        Ok(Some(format!("data:image/png;base64,{b64}")))
    }

    /// Full-size image for an item UUID.
    pub fn full_path(&self, uuid: &str) -> PathBuf {
        self.path_for(uuid, false)
    }

    /// Thumbnail image for an item UUID.
    pub fn thumb_path(&self, uuid: &str) -> PathBuf {
        self.path_for(uuid, true)
    }

    /// Delete both full and thumbnail files for an item (best effort).
    pub fn delete(&self, uuid: &str) {
        let _ = std::fs::remove_file(self.path_for(uuid, false));
        let _ = std::fs::remove_file(self.path_for(uuid, true));
    }

    /// Whether this item's thumbnail exists on disk (capture repair check).
    pub fn exists(&self, uuid: &str) -> bool {
        self.thumb_path(uuid).exists()
    }

    /// Delete files for many items (best effort).
    pub fn delete_many(&self, uuids: &[String]) {
        for uuid in uuids {
            self.delete(uuid);
        }
    }

    /// Remove image files whose item rows no longer exist (best effort).
    /// Returns how many files were deleted.
    pub fn cleanup_orphans(&self, keep: &[String]) -> usize {
        let keep: std::collections::HashSet<&str> = keep.iter().map(String::as_str).collect();
        let mut removed = 0;
        let Ok(entries) = std::fs::read_dir(&self.images_dir) else {
            return 0;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let base = name
                .strip_suffix("_thumb.png")
                .or_else(|| name.strip_suffix(".png"));
            if let Some(uuid) = base
                && !keep.contains(uuid)
                && std::fs::remove_file(entry.path()).is_ok()
            {
                removed += 1;
            }
        }
        removed
    }

    /// Remove every image file (privacy wipe).
    /// Returns how many files were actually removed (reported to the user).
    pub fn delete_all(&self) -> usize {
        let mut removed = 0;
        if let Ok(entries) = std::fs::read_dir(&self.images_dir) {
            for entry in entries.flatten() {
                if std::fs::remove_file(entry.path()).is_ok() {
                    removed += 1;
                }
            }
        }
        removed
    }

    fn path_for(&self, uuid: &str, thumb: bool) -> PathBuf {
        if thumb {
            self.images_dir.join(format!("{uuid}_thumb.png"))
        } else {
            self.images_dir.join(format!("{uuid}.png"))
        }
    }
}

const THUMB_MAX_WIDTH: u32 = 320;
