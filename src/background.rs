//! What goes behind the person: nothing, a blur, or an image.

use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use image::imageops::FilterType;
use image::{ImageReader, Limits, RgbImage};

use crate::Error;

/// Decoder limits: background pictures come from users, so treat files as untrusted.
const MAX_DECODE_SIDE: u32 = 8192;
const MAX_DECODE_ALLOC_BYTES: u64 = 256 * 1024 * 1024;

/// The background effect to apply.
#[derive(Debug, Clone)]
pub enum Background {
    /// No effect: the model is skipped and frames pass unchanged.
    None,
    /// Blur the background.
    Blur {
        /// 0 (light) … 100 (strong); larger values count as 100.
        strength: u8,
    },
    /// Replace the background with a picture. Shared, so switching between a few pictures
    /// does not decode them again.
    Image(Arc<BackgroundImage>),
}

impl Background {
    /// Whether this is [`Background::None`].
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// A decoded background picture, kept at its own size and scaled to the frame (cover, no
/// letterboxing) when a [`Pipeline`](crate::Pipeline) is set up for a frame size.
#[derive(Debug)]
pub struct BackgroundImage {
    rgb: RgbImage,
}

impl BackgroundImage {
    /// Decodes a PNG, JPEG or WebP image. Pictures larger than 8192 pixels on a side are
    /// rejected.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let mut reader = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|err| Error::Image(err.to_string()))?;
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_DECODE_SIDE);
        limits.max_image_height = Some(MAX_DECODE_SIDE);
        limits.max_alloc = Some(MAX_DECODE_ALLOC_BYTES);
        reader.limits(limits);
        let image = reader
            .decode()
            .map_err(|err| Error::Image(err.to_string()))?;
        Ok(Self {
            rgb: image.into_rgb8(),
        })
    }

    /// Reads and decodes a PNG, JPEG or WebP file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let bytes = std::fs::read(path).map_err(|err| Error::Image(err.to_string()))?;
        Self::decode(&bytes)
    }

    /// Wraps an already decoded picture.
    pub fn from_rgb(rgb: RgbImage) -> Self {
        Self { rgb }
    }

    /// Scales and crops the image to cover `width × height` (no letterboxing).
    pub(crate) fn cover(&self, width: u32, height: u32) -> RgbImage {
        let (sw, sh) = self.rgb.dimensions();
        let scale = (width as f32 / sw as f32).max(height as f32 / sh as f32);
        let cw = ((width as f32 / scale).round() as u32).clamp(1, sw);
        let ch = ((height as f32 / scale).round() as u32).clamp(1, sh);
        let cropped =
            image::imageops::crop_imm(&self.rgb, (sw - cw) / 2, (sh - ch) / 2, cw, ch).to_image();
        image::imageops::resize(&cropped, width, height, FilterType::Triangle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32, colour: [u8; 3]) -> Vec<u8> {
        let image = RgbImage::from_pixel(width, height, image::Rgb(colour));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .expect("encode png");
        bytes
    }

    #[test]
    fn cover_fills_the_frame_without_bars() {
        let image = BackgroundImage::decode(&png(400, 400, [10, 200, 30])).expect("decode");
        let covered = image.cover(64, 36);
        assert_eq!(covered.dimensions(), (64, 36));
        assert!(covered.pixels().all(|p| p.0 == [10, 200, 30]));
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(
            BackgroundImage::decode(b"not an image"),
            Err(Error::Image(_))
        ));
    }

    #[test]
    fn opens_files_and_reports_missing_ones() {
        let path = std::env::temp_dir().join(format!("bgmatte-test-{}.png", std::process::id()));
        std::fs::write(&path, png(8, 8, [1, 2, 3])).expect("write");
        let opened = BackgroundImage::open(&path);
        std::fs::remove_file(&path).expect("remove");
        assert!(opened.is_ok());
        assert!(matches!(BackgroundImage::open(&path), Err(Error::Image(_))));
    }
}
