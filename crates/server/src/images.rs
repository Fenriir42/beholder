//! Image uploads: header checks, bounded decoding, lossy WebP re-encoding and
//! content-addressed storage (ADR-0006).

use std::io::Cursor;
use std::path::{Path, PathBuf};

use beholder_core::{ImageHash, UnixMillis, UserId};
use beholder_protocol::http::{ImageError, MAX_IMAGE_PIXELS};
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, ImageReader, Limits};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::db::DbError;

/// Thumbnail width (library cards, mini covers).
pub const THUMB_WIDTH: u32 = 480;

/// Longest side of the display size.
pub const DISPLAY_MAX: u32 = 1920;

/// WebP quality: visually clean for covers at a fraction of JPEG size.
const QUALITY: f32 = 80.0;

/// Decoder allocation cap. A 40 MP RGB image needs 120 MB; RGBA PNGs at the
/// pixel limit need 160 MB. This keeps peak RSS under the 256 MB container limit.
const MAX_DECODE_BYTES: u64 = 170 * 1024 * 1024;

/// The two stored variants of an image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// ~480 px wide.
    Thumb,
    /// Longest side at most 1920 px.
    Display,
}

impl Variant {
    /// Parses the URL segment (`thumb.webp` / `display.webp`).
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

/// Re-encoded output of one upload.
pub struct Encoded {
    /// Original width.
    pub width: u32,
    /// Original height.
    pub height: u32,
    /// Thumbnail WebP bytes.
    pub thumb: Vec<u8>,
    /// Display-size WebP bytes.
    pub display: Vec<u8>,
}

/// Content hash of the uploaded bytes.
pub fn hash(bytes: &[u8]) -> ImageHash {
    ImageHash(Sha256::digest(bytes).into())
}

/// Checks the header, decodes within the memory cap, and re-encodes both
/// variants. CPU- and memory-heavy: run on the blocking pool, one at a time.
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

    // `thumbnail` averages areas: fast, low-memory and good for big downscales.
    // The full-size decode is dropped as soon as the display size exists.
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

/// Where a variant is stored: `images/<first byte>/<hash>-<variant>.webp`.
pub fn path(root: &Path, hash: &ImageHash, variant: Variant) -> PathBuf {
    let hex = hash.to_hex();
    root.join("images")
        .join(&hex[..2])
        .join(format!("{hex}-{}.webp", variant.suffix()))
}

/// Writes both variants atomically (temp file + rename), so a crash never
/// leaves a half-written file under a valid name.
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

/// Whether an image with this hash is already stored.
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

/// Records a stored image.
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
        // A well-formed PNG claiming 10000x10000 (100 MP) with no pixel data:
        // refused from the header alone, without attempting to decode.
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
