//! SVG rasterization with resvg at the target width (R-088).
//!
//! `rasterize_with` takes the font database so the text package can hand
//! over the shared one with the bundled Open Sans; `rasterize` falls back to
//! the system fonts loaded once per process.

pub mod handler;

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use resvg::tiny_skia;
use resvg::usvg;
use usvg::fontdb;

/// A rasterized SVG as PNG bytes with its pixel size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// usvg rejected the document. The protocol reports it as an
    /// unsupported format with `format: "svg"` (R-089).
    #[error("cannot parse SVG: {0}")]
    Parse(String),
    #[error("cannot allocate a {width}x{height} pixmap")]
    Pixmap { width: u32, height: u32 },
    #[error("cannot encode PNG: {0}")]
    Encode(String),
    #[error("cannot read file {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid params: {0}")]
    InvalidParams(String),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Error::Parse(_) => "unsupported_format",
            Error::Pixmap { .. } | Error::Encode(_) => "internal",
            Error::Io { .. } => "io",
            Error::InvalidParams(_) => "invalid_params",
        }
    }

    pub fn data(&self) -> Option<serde_json::Value> {
        match self {
            Error::Parse(_) => Some(serde_json::json!({ "format": "svg" })),
            Error::Io { path, .. } => Some(serde_json::json!({ "path": path })),
            Error::Pixmap { .. } | Error::Encode(_) | Error::InvalidParams(_) => None,
        }
    }
}

/// System fonts, loaded on first use. Loading takes tens of milliseconds and
/// the result never changes while the helper runs.
pub fn default_fontdb() -> Arc<fontdb::Database> {
    static DB: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        Arc::new(db)
    })
    .clone()
}

/// Rasterize with the process-wide font database.
pub fn rasterize(bytes: &[u8], width_px: u32) -> Result<Prepared, Error> {
    rasterize_with(bytes, width_px, default_fontdb())
}

/// Rasterize at `min(width_px, intrinsic width)`; the height follows the
/// aspect ratio (R-088, R-091). The canvas is transparent.
pub fn rasterize_with(
    bytes: &[u8],
    width_px: u32,
    fontdb: Arc<fontdb::Database>,
) -> Result<Prepared, Error> {
    let options = usvg::Options {
        fontdb,
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(bytes, &options).map_err(|e| Error::Parse(e.to_string()))?;
    let size = tree.size();
    let (width, height) = target_size(size.width(), size.height(), width_px);
    let mut pixmap =
        tiny_skia::Pixmap::new(width, height).ok_or(Error::Pixmap { width, height })?;
    let scale = width as f32 / size.width();
    let transform = tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let png = pixmap
        .encode_png()
        .map_err(|e| Error::Encode(e.to_string()))?;
    Ok(Prepared { png, width, height })
}

/// Read `input`, rasterize at `width`, and write the PNG to `output`
/// (R-145, R-200). Returns the output pixel size.
pub fn run_render_svg(input: &Path, width: u32, output: &Path) -> Result<(u32, u32), Error> {
    let bytes = std::fs::read(input).map_err(|source| Error::Io {
        path: input.to_path_buf(),
        source,
    })?;
    let prepared = rasterize(&bytes, width)?;
    std::fs::write(output, &prepared.png).map_err(|source| Error::Io {
        path: output.to_path_buf(),
        source,
    })?;
    Ok((prepared.width, prepared.height))
}

/// Pixel size for an intrinsic `w` by `h` at most `width_px` wide. usvg
/// guarantees a positive intrinsic size, so the division is safe.
pub fn target_size(w: f32, h: f32, width_px: u32) -> (u32, u32) {
    let intrinsic_w = w.round().max(1.0);
    let width = (width_px.max(1) as f32).min(intrinsic_w);
    let height = (h * width / w).round().max(1.0);
    (width as u32, height as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_size_caps_at_intrinsic_and_keeps_aspect() {
        assert_eq!(target_size(200.0, 100.0, 50), (50, 25));
        assert_eq!(target_size(200.0, 100.0, 500), (200, 100));
        assert_eq!(target_size(100.4, 50.2, 500), (100, 50));
        assert_eq!(target_size(1.0, 1000.0, 0), (1, 1000));
    }
}
