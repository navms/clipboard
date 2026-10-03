use std::path::Path;

use image::{imageops::FilterType, DynamicImage, ImageFormat, RgbaImage};

use crate::error::Result;

/// Longest edge of a generated thumbnail, in pixels.
pub const THUMB_WIDTH: u32 = 320;

pub struct StoredImage {
    pub hash: String,
    /// Paths relative to the app data dir, as persisted in the database.
    pub image_rel: String,
    pub thumb_rel: String,
    pub width: u32,
    pub height: u32,
    pub byte_size: i64,
}

/// Writes a captured frame under `<data_dir>/images` plus a thumbnail under
/// `<data_dir>/thumbs`, and returns a fingerprint derived from the *pixels*,
/// so the same picture re-encoded by a different app still dedupes.
///
/// Takes the frame already decoded rather than a PNG bitstream. The previous
/// shape forced a round trip nobody wanted: `clipboard-rs` encoded pixels into
/// a PNG for this function to decode them back before doing anything at all —
/// and their encoder builds that PNG from `to_rgba8()`, so the old call could
/// not even be described as one wasted copy.
///
/// No stored fingerprint moves as a result. What reaches the hasher is still
/// dimensions followed by RGBA pixels, and if anything this reaches them more
/// directly than before: it used to arrive by way of a PNG round trip through
/// `image`, which is lossless for this colour type and therefore a very
/// expensive no-op.
pub fn persist(data_dir: &Path, rgba: RgbaImage) -> Result<StoredImage> {
    let (width, height) = rgba.dimensions();

    // Hashed straight out of the buffer we were handed. A `to_rgba8()` used to
    // stand here as well, and it could only ever be a full-frame copy: every
    // caller arrives here already holding RGBA8.
    let mut hasher = blake3::Hasher::new();
    hasher.update(&width.to_le_bytes());
    hasher.update(&height.to_le_bytes());
    hasher.update(rgba.as_raw());
    let hash = hasher.finalize().to_hex().to_string();

    // Moving the buffer rather than copying it, and moving it once: the
    // original is encoded and the thumbnail resized off this same allocation.
    let img = DynamicImage::ImageRgba8(rgba);

    let images_dir = data_dir.join("images");
    let thumbs_dir = data_dir.join("thumbs");
    std::fs::create_dir_all(&images_dir)?;
    std::fs::create_dir_all(&thumbs_dir)?;

    let image_rel = format!("images/{hash}.png");
    let thumb_rel = format!("thumbs/{hash}.png");
    let image_abs = data_dir.join(&image_rel);
    let thumb_abs = data_dir.join(&thumb_rel);

    if !image_abs.exists() {
        img.save_with_format(&image_abs, ImageFormat::Png)?;
    }

    if !thumb_abs.exists() {
        write_thumb(&img, width, height, &thumb_abs)?;
    }

    let byte_size = std::fs::metadata(&image_abs)
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    Ok(StoredImage {
        hash,
        image_rel,
        thumb_rel,
        width,
        height,
        byte_size,
    })
}

/// Encodes the thumbnail for a frame of `width` × `height` that fits within
/// [`THUMB_WIDTH`], or copies it through unchanged when it already does.
fn write_thumb(img: &DynamicImage, width: u32, height: u32, path: &Path) -> Result<()> {
    if width <= THUMB_WIDTH {
        // Already thumbnail-sized: encode the frame straight out. This used to
        // be `img.clone()` — a deep copy of the entire pixel buffer whose only
        // purpose was to feed an encoder that never mutates its input.
        return img
            .save_with_format(path, ImageFormat::Png)
            .map_err(Into::into);
    }

    let ratio = THUMB_WIDTH as f64 / width as f64;
    // `max(1.0)` rather than trusting the arithmetic: an extreme aspect ratio
    // rounds its target height down to nothing, and the encoder quite rightly
    // treats a zero-height image as an error rather than as a thumbnail.
    let target_h = ((height as f64) * ratio).round().max(1.0) as u32;

    img.resize_exact(THUMB_WIDTH, target_h, FilterType::Triangle)
        .save_with_format(path, ImageFormat::Png)?;
    Ok(())
}

/// Best-effort unlink; missing files are not an error.
pub fn remove(data_dir: &Path, rel: &str) {
    let _ = std::fs::remove_file(data_dir.join(rel));
}
