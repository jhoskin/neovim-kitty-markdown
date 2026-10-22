//! Raster image decode, downscale, and PNG encode (R-086, R-087, R-090, R-091).
//!
//! `prepare` is pure: bytes in, PNG bytes out. The protocol glue that reads
//! the file and talks to the cache lives in `handler.rs`, and the CLI entry
//! point `run_render_image` skips the cache altogether (R-200).

pub mod handler;

use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::codecs::gif::GifDecoder;
use image::imageops::FilterType;
use image::{AnimationDecoder, DynamicImage, ImageFormat};

use crate::svg;

/// A decoded image re-encoded as PNG at its final pixel size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Image errors. `code` and `data` follow the protocol table in AGENTS.md so
/// the server can serialize them without knowing the variants.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported image format: {format}")]
    UnsupportedFormat { format: String },
    #[error("cannot decode {format} image: {message}")]
    Decode { format: String, message: String },
    #[error("cannot encode PNG: {0}")]
    Encode(String),
    #[error("cannot read file {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Svg(#[from] svg::Error),
    #[error("invalid params: {0}")]
    InvalidParams(String),
}

impl Error {
    /// The stable protocol error code.
    pub fn code(&self) -> &'static str {
        match self {
            Error::UnsupportedFormat { .. } | Error::Decode { .. } => "unsupported_format",
            Error::Encode(_) => "internal",
            Error::Io { .. } => "io",
            Error::Svg(e) => e.code(),
            Error::InvalidParams(_) => "invalid_params",
        }
    }

    /// The protocol `data` object, when the code has one.
    pub fn data(&self) -> Option<serde_json::Value> {
        match self {
            Error::UnsupportedFormat { format } | Error::Decode { format, .. } => {
                Some(serde_json::json!({ "format": format }))
            }
            Error::Io { path, .. } => Some(serde_json::json!({ "path": path })),
            Error::Svg(e) => e.data(),
            Error::Encode(_) | Error::InvalidParams(_) => None,
        }
    }
}

/// Size limits from an `<img>` tag (R-092). `None` means no limit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Limits {
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
}

/// Decode `bytes`, scale down to fit `width_px` and the limits, and encode
/// PNG. SVG bytes are routed to the rasterizer so callers can hand over any
/// image file without sniffing first.
pub fn prepare(
    bytes: &[u8],
    width_px: u32,
    max_width: Option<u32>,
    max_height: Option<u32>,
) -> Result<Prepared, Error> {
    if looks_like_svg(bytes) {
        let limit = match max_width {
            Some(max_w) => width_px.min(max_w),
            None => width_px,
        };
        let rendered = svg::rasterize(bytes, limit)?;
        return Ok(Prepared {
            png: rendered.png,
            width: rendered.width,
            height: rendered.height,
        });
    }

    let format = sniff(bytes)?;
    let decoded = decode(bytes, format)?;
    let (width, height) = target_size(
        decoded.width(),
        decoded.height(),
        width_px,
        Limits {
            max_width,
            max_height,
        },
    );
    let scaled = if (width, height) == (decoded.width(), decoded.height()) {
        decoded
    } else {
        decoded.resize_exact(width, height, FilterType::Lanczos3)
    };
    let png = encode_png(&scaled)?;
    Ok(Prepared { png, width, height })
}

/// Read `input`, prepare it at `width` with no `<img>` limits, and write the
/// PNG to `output` (R-145, R-200). Returns the output pixel size.
pub fn run_render_image(input: &Path, width: u32, output: &Path) -> Result<(u32, u32), Error> {
    let bytes = std::fs::read(input).map_err(|source| Error::Io {
        path: input.to_path_buf(),
        source,
    })?;
    let prepared = prepare(&bytes, width, None, None)?;
    std::fs::write(output, &prepared.png).map_err(|source| Error::Io {
        path: output.to_path_buf(),
        source,
    })?;
    Ok((prepared.width, prepared.height))
}

/// The format name the protocol reports: the `image` crate's first
/// extension (`png`, `jpg`, `gif`, `webp`, `bmp`, `tiff`, ...).
fn format_name(format: ImageFormat) -> String {
    format
        .extensions_str()
        .first()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Identify the container by its magic bytes. The sniff never trusts a file
/// extension, so a PNG named `.jpg` still decodes (R-086) and a text file
/// named `.png` is `unsupported_format` with `format: "unknown"` (R-089).
fn sniff(bytes: &[u8]) -> Result<ImageFormat, Error> {
    let format = image::guess_format(bytes).map_err(|_| Error::UnsupportedFormat {
        format: "unknown".to_string(),
    })?;
    match format {
        ImageFormat::Png
        | ImageFormat::Jpeg
        | ImageFormat::Gif
        | ImageFormat::WebP
        | ImageFormat::Bmp => Ok(format),
        other => Err(Error::UnsupportedFormat {
            format: format_name(other),
        }),
    }
}

fn decode(bytes: &[u8], format: ImageFormat) -> Result<DynamicImage, Error> {
    let decode_error = |e: image::ImageError| Error::Decode {
        format: format_name(format),
        message: e.to_string(),
    };
    if format == ImageFormat::Gif {
        // The frame iterator composites each frame onto the full canvas, so
        // the first frame has the screen size even when it is a sub-rectangle
        // (R-087). `load_from_memory` would do the same, but the intent is
        // clearer here.
        let decoder = GifDecoder::new(Cursor::new(bytes)).map_err(decode_error)?;
        let first = decoder
            .into_frames()
            .next()
            .ok_or_else(|| Error::Decode {
                format: "gif".to_string(),
                message: "the GIF has no frames".to_string(),
            })?
            .map_err(decode_error)?;
        return Ok(DynamicImage::ImageRgba8(first.into_buffer()));
    }
    image::load_from_memory_with_format(bytes, format).map_err(decode_error)
}

/// Fit `(w, h)` into the target width and the limits, keep the aspect ratio,
/// never grow (R-090, R-091, R-092). Every dimension stays at least 1.
pub fn target_size(w: u32, h: u32, width_px: u32, limits: Limits) -> (u32, u32) {
    if w == 0 || h == 0 {
        return (w.max(1), h.max(1));
    }
    let mut target_w = w.min(width_px.max(1));
    if let Some(max_w) = limits.max_width {
        target_w = target_w.min(max_w.max(1));
    }
    let mut target_h = scale_dim(h, target_w, w);
    if let Some(max_h) = limits.max_height
        && target_h > max_h.max(1)
    {
        target_h = max_h.max(1);
        target_w = scale_dim(w, target_h, h);
    }
    (target_w.max(1), target_h.max(1))
}

/// `dim * num / den`, rounded to nearest, at least 1.
fn scale_dim(dim: u32, num: u32, den: u32) -> u32 {
    let scaled = (u64::from(dim) * u64::from(num) + u64::from(den) / 2) / u64::from(den);
    u32::try_from(scaled).unwrap_or(u32::MAX).max(1)
}

/// Encode as RGBA8 when the image has alpha, RGB8 otherwise. Narrowing
/// 16-bit and gray images keeps the PNG a format kitty decodes quickly.
fn encode_png(img: &DynamicImage) -> Result<Vec<u8>, Error> {
    let mut out = Cursor::new(Vec::new());
    let result = if img.color().has_alpha() {
        match img {
            DynamicImage::ImageRgba8(buf) => buf.write_to(&mut out, ImageFormat::Png),
            other => other.to_rgba8().write_to(&mut out, ImageFormat::Png),
        }
    } else {
        match img {
            DynamicImage::ImageRgb8(buf) => buf.write_to(&mut out, ImageFormat::Png),
            other => other.to_rgb8().write_to(&mut out, ImageFormat::Png),
        }
    };
    result.map_err(|e| Error::Encode(e.to_string()))?;
    Ok(out.into_inner())
}

/// SVG has no magic bytes. Accept an optional BOM, whitespace, an XML
/// prolog, comments, and a doctype before the `<svg` root, within the first
/// kilobyte.
pub fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(1024)];
    let text = String::from_utf8_lossy(head);
    let trimmed = text.trim_start_matches('\u{feff}').trim_start();
    trimmed.starts_with('<') && trimmed.contains("<svg")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_size_keeps_aspect_and_never_grows() {
        assert_eq!(target_size(400, 200, 100, Limits::default()), (100, 50));
        assert_eq!(target_size(40, 20, 100, Limits::default()), (40, 20));
        assert_eq!(
            target_size(
                400,
                200,
                1000,
                Limits {
                    max_width: Some(200),
                    max_height: None
                }
            ),
            (200, 100)
        );
        assert_eq!(
            target_size(
                400,
                200,
                1000,
                Limits {
                    max_width: None,
                    max_height: Some(50)
                }
            ),
            (100, 50)
        );
        assert_eq!(
            target_size(
                400,
                200,
                1000,
                Limits {
                    max_width: Some(800),
                    max_height: Some(800)
                }
            ),
            (400, 200)
        );
    }

    #[test]
    fn target_size_stays_positive() {
        assert_eq!(target_size(1000, 1, 10, Limits::default()), (10, 1));
        assert_eq!(target_size(1, 1000, 10, Limits::default()), (1, 1000));
    }

    #[test]
    fn svg_sniff() {
        assert!(looks_like_svg(b"<svg xmlns='http://www.w3.org/2000/svg'/>"));
        assert!(looks_like_svg(
            b"\xef\xbb\xbf<?xml version=\"1.0\"?>\n<!-- c -->\n<svg/>"
        ));
        assert!(!looks_like_svg(b"<html><body>svg</body></html>"));
        assert!(!looks_like_svg(b"\x89PNG\r\n\x1a\n"));
    }
}
