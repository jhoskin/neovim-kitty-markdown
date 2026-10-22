//! Mermaid rendering pipeline: preprocess, detect, parse, layout, scene,
//! raster. [`render`] is the one entry point; the server handler and the
//! CLI both call it.

use std::sync::{Arc, OnceLock};

use usvg::fontdb;

use crate::error::HelperError;
use crate::text::{self, TextMetrics};

pub mod config;
pub mod detect;
pub mod diagrams;
pub mod directive;
pub mod edges;
pub mod jison;
pub mod layout;
pub mod preprocess;
pub mod raster;
pub mod scene;
pub mod shapes;
pub mod styles;
pub mod theme;

use config::BaseConfig;
use diagrams::RenderCtx;
use raster::{Background, RasterOpts};

pub struct RenderRequest<'a> {
    pub source: &'a str,
    pub width_px: u32,
    /// A built-in theme name; the plugin picks it from its palette.
    pub theme: &'a str,
    pub background: Background,
}

pub struct RenderOutput {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub diagram_type: &'static str,
}

/// The font database resvg reads. It is the measurer's database, shared so
/// text is shaped and rasterized from the same faces.
pub fn fontdb() -> Arc<fontdb::Database> {
    static DB: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| Arc::new(text::font_system().db().clone()))
        .clone()
}

/// Parses the `background` request parameter: `transparent`, a hex color,
/// or white for anything else (mermaid-cli's default).
pub fn parse_background(value: &str) -> Background {
    if value.eq_ignore_ascii_case("transparent") {
        return Background::Transparent;
    }
    scene::Color::from_hex(value)
        .map(Background::Color)
        .unwrap_or_default()
}

pub fn render(req: &RenderRequest<'_>) -> Result<RenderOutput, HelperError> {
    let resolved = config::resolve(req.source, &BaseConfig::new(req.theme))?;
    let unsupported = || HelperError::UnsupportedDiagramType {
        diagram_type: resolved.diagram_type.to_string(),
    };
    if detect::UNSUPPORTED_TYPES.contains(&resolved.diagram_type) {
        return Err(unsupported());
    }
    let diagram = diagrams::find(resolved.diagram_type).ok_or_else(unsupported)?;
    let fonts = text::font_system();
    let ctx = RenderCtx {
        fonts,
        metrics: TextMetrics::new(fonts),
        target_width: req.width_px,
    };
    let scene = diagram.render(&resolved, &ctx)?;
    let opts = RasterOpts {
        target_width: Some(req.width_px),
        background: Some(req.background),
    };
    let out = raster::render_png(&scene, fontdb(), &opts)
        .map_err(|e| HelperError::internal(e.to_string()))?;
    Ok(RenderOutput {
        png: out.png,
        width: out.width,
        height: out.height,
        diagram_type: resolved.diagram_type,
    })
}
