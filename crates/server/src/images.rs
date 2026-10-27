use std::io::Cursor;
use std::path::{Path, PathBuf};

use beholder_core::{ImageHash, UnixMillis, UserId};
use beholder_protocol::http::{ImageError, MAX_IMAGE_PIXELS};
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, ImageReader, Limits};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::db::DbError;

pub const THUMB_WIDTH: u32 = 480;

pub const DISPLAY_MAX: u32 = 1920;

const QUALITY: f32 = 80.0;

const MAX_DECODE_BYTES: u64 = 170 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Thumb,
    Display,
}

impl Variant {
    pub fn from_file_name(s: &str) -> Option<Self> {
        match s {
            "thumb.webp" => Some(Self::Thumb),
            "display.webp" => Some(Self::Display),
            _ => None,
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Self::Thumb => "thumb",
            Self::Display => "display",
        }
    }
}

pub struct Encoded {
    pub width: u32,
    pub height: u32,
    pub thumb: Vec<u8>,
    pub display: Vec<u8>,
}

pub fn hash(bytes: &[u8]) -> ImageHash {
    ImageHash(Sha256::digest(bytes).into())
}

pub fn process(bytes: &[u8]) -> Result<Encoded, ImageError> {
    let reader = || {
        ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|_| ImageError::Unsupported)
    };
    let r = reader()?;
    if !matches!(
        r.format(),
        Some(ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP)
    ) {
        return Err(ImageError::Unsupported);
    }
    // Reads only the header, so oversized images are refused before any decoding.
    let (width, height) = r.into_dimensions().map_err(|_| ImageError::Unsupported)?;
    if u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS {
        return Err(ImageError::TooManyPixels);
    }
    let mut r = reader()?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    r.limits(limits);
    let full = r.decode().map_err(|_| ImageError::Unsupported)?;

    let display = if width > DISPLAY_MAX || height > DISPLAY_MAX {
        full.thumbnail(DISPLAY_MAX, DISPLAY_MAX)
    } else {
        full
    };
    let thumb = if display.width() > THUMB_WIDTH {
        display.resize(THUMB_WIDTH, u32::MAX, FilterType::Triangle)
    } else {
        display.clone()
    };
    Ok(Encoded {
        width,
        height,
        thumb: webp(&thumb),
        display: webp(&display),
    })
}

fn webp(img: &DynamicImage) -> Vec<u8> {
    let (w, h) = (img.width(), img.height());
    if img.color().has_alpha() {
        webp::Encoder::from_rgba(&img.to_rgba8(), w, h)
            .encode(QUALITY)
            .to_vec()
    } else {
        webp::Encoder::from_rgb(&img.to_rgb8(), w, h)
            .encode(QUALITY)
            .to_vec()
    }
}

pub fn path(root: &Path, hash: &ImageHash, variant: Variant) -> PathBuf {
    let hex = hash.to_hex();
    root.join("images")
        .join(&hex[..2])
        .join(format!("{hex}-{}.webp", variant.suffix()))
}

pub fn store_files(root: &Path, hash: &ImageHash, enc: &Encoded) -> std::io::Result<()> {
    for (variant, bytes) in [
        (Variant::Thumb, &enc.thumb),
        (Variant::Display, &enc.display),
    ] {
        let dest = path(root, hash, variant);
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = dest.with_extension("tmp");
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &dest)?;
    }
    Ok(())
}

pub fn exists(conn: &Connection, hash: &ImageHash) -> Result<bool, DbError> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM images WHERE hash = ?1",
            [hash.0.as_slice()],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

pub fn record(
    conn: &Connection,
    hash: &ImageHash,
    enc: &Encoded,
    uploader: UserId,
    now: UnixMillis,
) -> Result<(), DbError> {
    conn.execute(
        "INSERT OR IGNORE INTO images (hash, width, height, thumb_bytes, display_bytes, uploader, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            hash.0.as_slice(),
            enc.width,
            enc.height,
            enc.thumb.len(),
            enc.display.len(),
            uploader.as_bytes(),
            now.0
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageEncoder, RgbImage};

    fn jpeg(w: u32, h: u32) -> Vec<u8> {
        let img = RgbImage::from_fn(w, h, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        });
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut out)
            .write_image(img.as_raw(), w, h, image::ExtendedColorType::Rgb8)
            .unwrap();
        out
    }

    fn decoded_size(webp: &[u8]) -> (u32, u32) {
        let img = image::load_from_memory_with_format(webp, ImageFormat::WebP).unwrap();
        (img.width(), img.height())
    }

    #[test]
    fn large_image_gets_both_variants() {
        let enc = process(&jpeg(2400, 1800)).unwrap();
        assert_eq!((enc.width, enc.height), (2400, 1800));
        assert_eq!(decoded_size(&enc.display), (1920, 1440));
        assert_eq!(decoded_size(&enc.thumb), (480, 360));
        assert_eq!(&enc.thumb[..4], b"RIFF");
    }

    #[test]
    fn small_image_is_not_upscaled() {
        let enc = process(&jpeg(300, 200)).unwrap();
        assert_eq!(decoded_size(&enc.display), (300, 200));
        assert_eq!(decoded_size(&enc.thumb), (300, 200));
    }

    #[test]
    fn png_with_alpha_is_supported() {
        let img = image::RgbaImage::from_pixel(64, 64, image::Rgba([10, 20, 30, 40]));
        let mut out = Vec::new();
        image::codecs::png::PngEncoder::new(&mut out)
            .write_image(img.as_raw(), 64, 64, image::ExtendedColorType::Rgba8)
            .unwrap();
        assert!(process(&out).is_ok());
    }

    #[test]
    fn rejects_garbage_and_too_many_pixels() {
        assert_eq!(
            process(b"not an image").err(),
            Some(ImageError::Unsupported)
        );
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&10_000u32.to_be_bytes());
        ihdr.extend_from_slice(&10_000u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        for (kind, data) in [(b"IHDR", ihdr.as_slice()), (b"IDAT", &[]), (b"IEND", &[])] {
            png.extend_from_slice(&(data.len() as u32).to_be_bytes());
            let mut chunk = kind.to_vec();
            chunk.extend_from_slice(data);
            png.extend_from_slice(&chunk);
            png.extend_from_slice(&crc32(&chunk).to_be_bytes());
        }
        assert_eq!(process(&png).err(), Some(ImageError::TooManyPixels));
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xffff_ffffu32;
        for &b in data {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    #[test]
    fn variant_paths_are_content_addressed() {
        let h = ImageHash([0xab; 32]);
        let p = path(Path::new("/data"), &h, Variant::Thumb);
        assert_eq!(
            p,
            Path::new("/data/images/ab").join(format!("{}-thumb.webp", h.to_hex()))
        );
        assert_eq!(
            Variant::from_file_name("display.webp"),
            Some(Variant::Display)
        );
        assert_eq!(Variant::from_file_name("x.webp"), None);
    }
}
