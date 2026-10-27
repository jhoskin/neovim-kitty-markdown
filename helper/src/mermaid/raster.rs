//! Scene to PNG with resvg; background fill; output size rule (R-180).
//!
//! # How the oracle's pixel size arises
//!
//! mermaid-cli 12.0.0 (`src/index.js`) renders a PNG like this:
//!
//! 1. The page viewport is `size + 16` square (Puppeteer's body has an 8 px
//!    margin on each side). mermaid writes the SVG with `width="100%"`, a
//!    `viewBox` of the natural size, and `style="max-width: <natural>px"`.
//! 2. With `--size`, the CLI overwrites `max-width` and `max-height` with
//!    `size` px and removes the `height` attribute, so the SVG fills the
//!    container width (`size`) and takes its height from the viewBox aspect
//!    ratio.
//! 3. For a diagram taller than wide (`viewBox.width / viewBox.height < 1`)
//!    the CLI sets `max-width` to `Math.round(size * aspectRatio)` so the
//!    height, not the width, equals `size`.
//! 4. The screenshot clip is the SVG's bounding rect with `floor` on the
//!    offset and `ceil` on width and height; the PNG has exactly the clip's
//!    pixel size, and the fractional remainder is page background.
//!
//! So an oracle PNG is `size` wide (or tall) and `ceil(size / aspect)` on the
//! other axis. The helper's [`OutputSize::for_target`] produces that same
//! size for a target width: when the natural width fits (R-114, R-180) the
//! scale is `ceil(natural_w) / natural_w`, otherwise `target_w / natural_w`;
//! the height is `ceil(natural_h * scale)`. The harness then passes
//! `--size max(width, height)` to the oracle (R-181). For a tall diagram the
//! oracle rounds the width from the height, which can differ from the
//! helper's width by one pixel; that is the 1 px tolerance in R-181.

use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, Mutex};

use resvg::tiny_skia;
use usvg::fontdb;

use crate::mermaid::scene::svg::{OwnGlyphs, RunFont, SvgOptions, to_svg_with};
use crate::mermaid::scene::{Color, FontStyle, Scene};
use crate::text::FontSystem;
use crate::text::fonts::{FontId, Style as FaceStyle, Weight};

#[cfg(target_os = "macos")]
mod coregraphics;
mod faces;
#[cfg(target_os = "macos")]
mod own;

use faces::FaceTable;

/// The pixel size of a render and the scale from CSS pixels to pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputSize {
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

impl OutputSize {
    /// The rule in the module documentation. A non-positive natural size
    /// yields a 1 by 1 canvas so the caller still gets a valid PNG.
    pub fn for_target(natural_w: f64, natural_h: f64, target_w: u32) -> OutputSize {
        let positive = |v: f64| v.is_finite() && v > 0.0;
        if !positive(natural_w) || !positive(natural_h) {
            return OutputSize {
                width: 1,
                height: 1,
                scale: 1.0,
            };
        }
        let target = f64::from(target_w.max(1));
        let width = if natural_w <= target {
            natural_w.ceil()
        } else {
            target
        };
        let scale = width / natural_w;
        let height = (natural_h * scale).ceil().max(1.0);
        OutputSize {
            width: width as u32,
            height: height as u32,
            scale,
        }
    }

    /// The natural size with no target: `ceil` of both dimensions.
    pub fn natural(natural_w: f64, natural_h: f64) -> OutputSize {
        OutputSize::for_target(natural_w, natural_h, u32::MAX)
    }

    /// The `--size` value the oracle needs to produce this size (R-181).
    pub fn oracle_size(self) -> u32 {
        self.width.max(self.height)
    }
}

/// What goes under the diagram.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Background {
    Transparent,
    Color(Color),
}

impl Default for Background {
    fn default() -> Background {
        Background::Color(Color::WHITE)
    }
}

#[derive(Debug, Clone, Default)]
pub struct RasterOpts {
    /// The target width in pixels. `None` renders at the natural size.
    pub target_width: Option<u32>,
    /// Overrides [`Scene::background`]; the default is white.
    pub background: Option<Background>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterOut {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub enum RasterError {
    /// usvg rejected the serialized scene; a bug in the serializer.
    Svg(usvg::Error),
    /// The pixmap could not be allocated (zero or overflowing size).
    Canvas {
        width: u32,
        height: u32,
    },
    Png(String),
}

impl RasterError {
    /// The protocol error code. Every raster failure is a helper bug, not a
    /// user input problem, so it maps to `internal`.
    pub fn code(&self) -> &'static str {
        "internal"
    }
}

impl fmt::Display for RasterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RasterError::Svg(e) => write!(f, "scene SVG rejected by usvg: {e}"),
            RasterError::Canvas { width, height } => {
                write!(f, "cannot allocate a {width}x{height} canvas")
            }
            RasterError::Png(e) => write!(f, "PNG encoding failed: {e}"),
        }
    }
}

impl std::error::Error for RasterError {}

fn face_style(style: usvg::FontStyle) -> FaceStyle {
    match style {
        usvg::FontStyle::Normal => FaceStyle::Normal,
        usvg::FontStyle::Italic => FaceStyle::Italic,
        usvg::FontStyle::Oblique => FaceStyle::Oblique,
    }
}

/// True when `id` names the same face in `db` as in the font system's
/// database, which holds for every database cloned from it (ids survive a
/// clone). A test that rasterizes with a database of its own gets resvg's
/// choice instead.
fn same_face(fonts: &FontSystem, db: &fontdb::Database, id: FontId) -> bool {
    match (fonts.db().face(id), db.face(id)) {
        (Some(a), Some(b)) => a.post_script_name == b.post_script_name && a.index == b.index,
        _ => false,
    }
}

/// The font choice resvg draws with, taken from the font system so the
/// drawn face is the measured face: a face token names the fallback face
/// of a piece ([`faces`]), a generic family maps to Chrome's platform
/// default (resvg's own `monospace` is Courier New, Chrome's on macOS is
/// Menlo), names match in any case, and Open Sans is the bundled pair.
/// Whatever the font system cannot place goes to resvg's default query.
///
/// A face the font system placed never falls back. usvg shapes the whole
/// text chunk in the face of each span and keeps only the span's own
/// clusters, but when a fallback face covers the whole chunk it takes all
/// of that face's glyphs: under a Hangul label, the `→` span (Arial) and
/// the spaces of the primary span (Trebuchet MS) were drawn in Arial
/// Unicode MS, the first face with a Hangul glyph, 30 % narrower than the
/// Arial arrow Chrome draws. The raster SVG gives every grapheme the
/// measurer did not leave to the primary face a piece of its own, so such
/// a span has a glyph for each of its own characters, or the `.notdef`
/// Chrome draws as well.
fn font_resolver(fonts: &'static FontSystem, faces: Vec<FontId>) -> usvg::FontResolver<'static> {
    let default_font = usvg::FontResolver::default_font_selector();
    let default_fallback = usvg::FontResolver::default_fallback_selector();
    let placed: Arc<Mutex<HashSet<FontId>>> = Arc::default();
    let placed_by_resolver = placed.clone();
    usvg::FontResolver {
        select_font: Box::new(move |font, db| {
            let weight = Weight(font.weight());
            let style = face_style(font.style());
            font.families()
                .iter()
                .find_map(|family| {
                    let name = match family {
                        usvg::FontFamily::Serif => "serif",
                        usvg::FontFamily::SansSerif => "sans-serif",
                        usvg::FontFamily::Cursive => "cursive",
                        usvg::FontFamily::Fantasy => "fantasy",
                        usvg::FontFamily::Monospace => "monospace",
                        usvg::FontFamily::Named(name) => name.as_str(),
                    };
                    faces::face_for_family(&faces, name)
                        .or_else(|| fonts.resolve_family(name, weight, style))
                        .filter(|&id| same_face(fonts, db, id))
                })
                .inspect(|&id| {
                    if let Ok(mut placed) = placed_by_resolver.lock() {
                        placed.insert(id);
                    }
                })
                .or_else(|| default_font(font, db))
        }),
        select_fallback: Box::new(move |c, used, db| {
            let base_placed = used
                .first()
                .is_some_and(|id| placed.lock().is_ok_and(|p| p.contains(id)));
            if base_placed {
                return None;
            }
            default_fallback(c, used, db)
        }),
    }
}

/// The SVG resvg reads: the scene with Chrome's synthetic bold and a
/// `tspan` per fallback face, which the font system decides because it
/// resolves families and fallbacks the way the measurer does. With
/// `own_glyphs`, the pieces the raster step draws itself are hidden and
/// its elements follow their texts.
fn raster_svg(scene: &Scene, table: &FaceTable<'_>, own_glyphs: Option<OwnGlyphs<'_>>) -> String {
    let fonts = crate::text::font_system();
    let probe = |chain: &str, weight: u16, style: FontStyle| {
        let style = match style {
            FontStyle::Normal => FaceStyle::Normal,
            FontStyle::Italic => FaceStyle::Italic,
            FontStyle::Oblique => FaceStyle::Oblique,
        };
        fonts.synthesizes_bold(chain, Weight(weight), style)
    };
    let pieces = |text: &str, font: &RunFont<'_>| table.pieces(text, font);
    let opts = SvgOptions {
        synthetic_bold: Some(&probe),
        face_pieces: Some(&pieces),
        own_glyphs,
    };
    to_svg_with(scene, &opts)
}

/// Parses the raster SVG. Where a fallback face is one resvg cannot draw
/// as Chrome does, the first parse lays the text out and a second one
/// draws the glyphs the raster step made for it (macOS only).
fn raster_tree(
    scene: &Scene,
    fontdb: Arc<fontdb::Database>,
    scale: f32,
) -> Result<usvg::Tree, RasterError> {
    let fonts = crate::text::font_system();
    let table = FaceTable::new(fonts);
    // The first serialization names every face; the second names the
    // same ones, so the resolver's copy of the table stays complete.
    let svg = raster_svg(scene, &table, None);
    let usvg_opts = usvg::Options {
        dpi: 96.0,
        font_family: scene.font_family.clone(),
        font_resolver: font_resolver(fonts, table.faces()),
        fontdb,
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(&svg, &usvg_opts).map_err(RasterError::Svg)?;
    #[cfg(target_os = "macos")]
    {
        let drawn = own::own_glyphs(&tree, scale, &table, fonts);
        if !drawn.is_empty() {
            let lookup = |id: &str| drawn.get(id).cloned();
            let svg = raster_svg(scene, &table, Some(&lookup));
            return usvg::Tree::from_str(&svg, &usvg_opts).map_err(RasterError::Svg);
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = scale;
    Ok(tree)
}

/// Rasterizes the scene into a premultiplied RGBA pixmap at the size the
/// output rule gives. `fontdb` is the database the text package builds
/// (bundled Open Sans first, then the system fonts).
pub fn render_pixmap(
    scene: &Scene,
    fontdb: Arc<fontdb::Database>,
    opts: &RasterOpts,
) -> Result<(tiny_skia::Pixmap, OutputSize), RasterError> {
    let size = match opts.target_width {
        Some(target) => OutputSize::for_target(scene.width, scene.height, target),
        None => OutputSize::natural(scene.width, scene.height),
    };
    let scale = size.scale as f32;
    let tree = raster_tree(scene, fontdb, scale)?;

    let mut pixmap =
        tiny_skia::Pixmap::new(size.width, size.height).ok_or(RasterError::Canvas {
            width: size.width,
            height: size.height,
        })?;
    let background = opts.background.unwrap_or(match scene.background {
        Some(color) => Background::Color(color),
        None => Background::default(),
    });
    if let Background::Color(c) = background {
        pixmap.fill(tiny_skia::Color::from_rgba8(
            c.r,
            c.g,
            c.b,
            (c.a.clamp(0.0, 1.0) * 255.0).round() as u8,
        ));
    }
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok((pixmap, size))
}

/// Renders the scene to PNG bytes.
pub fn render_png(
    scene: &Scene,
    fontdb: Arc<fontdb::Database>,
    opts: &RasterOpts,
) -> Result<RasterOut, RasterError> {
    let (pixmap, size) = render_pixmap(scene, fontdb, opts)?;
    let png = pixmap
        .encode_png()
        .map_err(|e| RasterError::Png(e.to_string()))?;
    Ok(RasterOut {
        png,
        width: size.width,
        height: size.height,
    })
}

/// Renders the scene to a straight-alpha RGBA image, for tests and SSIM.
pub fn render_rgba(
    scene: &Scene,
    fontdb: Arc<fontdb::Database>,
    opts: &RasterOpts,
) -> Result<image::RgbaImage, RasterError> {
    let (pixmap, size) = render_pixmap(scene, fontdb, opts)?;
    let mut data = Vec::with_capacity((size.width * size.height * 4) as usize);
    for px in pixmap.pixels() {
        let c = px.demultiply();
        data.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    image::RgbaImage::from_raw(size.width, size.height, data).ok_or(RasterError::Canvas {
        width: size.width,
        height: size.height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The PostScript name of the face of every glyph usvg laid out.
    fn glyph_faces(tree: &usvg::Tree) -> Vec<(String, String)> {
        fn walk(group: &usvg::Group, db: &fontdb::Database, out: &mut Vec<(String, String)>) {
            for node in group.children() {
                match node {
                    usvg::Node::Group(g) => walk(g, db, out),
                    usvg::Node::Text(t) => {
                        for glyph in t.layouted().iter().flat_map(|s| &s.positioned_glyphs) {
                            let face = db.face(glyph.font).map(|f| f.post_script_name.clone());
                            out.push((glyph.text.clone(), face.unwrap_or_default()));
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(tree.root(), tree.fontdb(), &mut out);
        out
    }

    /// A face the font system placed keeps its own glyphs. usvg shaped
    /// the whole Hangul line in the Arial of the arrow's piece, fell back
    /// to Arial Unicode MS, which covers the line, and drew the arrow and
    /// the spaces in it; Chrome draws them in Arial and Trebuchet MS.
    #[test]
    fn placed_faces_do_not_fall_back_to_a_face_covering_the_line() {
        use crate::mermaid::scene::{
            Baseline, Common, DEFAULT_FONT_FAMILY, Element, FontSpec, Style, Text, TextAnchor,
            TextRun,
        };
        let fonts = crate::text::font_system();
        let present = ["Trebuchet MS", "Arial", "Apple SD Gothic Neo"]
            .iter()
            .all(|f| {
                fonts
                    .resolve_family(f, Weight(400), FaceStyle::Normal)
                    .is_some()
            });
        if !cfg!(target_os = "macos") || !present {
            return;
        }
        let mut scene = Scene::new(200.0, 40.0);
        scene.root.children.push(Element::Text(Text {
            common: Common::with_style(Style::fill(Color::BLACK)),
            x: 100.0,
            y: 20.0,
            anchor: Some(TextAnchor::Middle),
            baseline: Some(Baseline::Middle),
            font: FontSpec {
                family: Some(DEFAULT_FONT_FAMILY.to_string()),
                size: Some(16.0),
                ..FontSpec::default()
            },
            runs: vec![TextRun::new("한국어 ★ → ok")],
        }));
        let tree = raster_tree(&scene, crate::text::fontdb_arc(), 1.0).expect("tree");
        let faces = glyph_faces(&tree);
        let face_of = |text: &str| -> Vec<&str> {
            faces
                .iter()
                .filter(|(t, _)| t == text)
                .map(|(_, f)| f.as_str())
                .collect()
        };
        assert_eq!(face_of("→"), ["ArialMT"], "{faces:?}");
        assert!(
            face_of(" ").iter().all(|f| *f == "TrebuchetMS"),
            "{faces:?}"
        );
        assert_eq!(face_of("o"), ["TrebuchetMS"], "{faces:?}");
        assert_eq!(face_of("한"), ["AppleSDGothicNeo-Regular"], "{faces:?}");
    }

    #[test]
    fn natural_width_fits_target() {
        let s = OutputSize::for_target(300.4, 120.2, 960);
        assert_eq!((s.width, s.height), (301, 121));
        assert!((s.scale - 301.0 / 300.4).abs() < 1e-12);
        assert_eq!(s.oracle_size(), 301);
    }

    #[test]
    fn wider_than_target_scales_down() {
        let s = OutputSize::for_target(1920.0, 500.0, 960);
        assert_eq!((s.width, s.height), (960, 250));
        assert!((s.scale - 0.5).abs() < 1e-12);
    }

    #[test]
    fn height_rounds_up_like_the_clip() {
        let s = OutputSize::for_target(1000.0, 333.0, 500);
        assert_eq!((s.width, s.height), (500, 167));
        assert_eq!(s.oracle_size(), 500);
        let tall = OutputSize::for_target(200.0, 800.0, 960);
        assert_eq!(tall.oracle_size(), 800);
    }

    #[test]
    fn degenerate_sizes_stay_valid() {
        assert_eq!(
            OutputSize::for_target(0.0, 10.0, 100),
            OutputSize {
                width: 1,
                height: 1,
                scale: 1.0
            }
        );
        assert_eq!(OutputSize::for_target(10.0, 0.5, 100).height, 1);
    }
}
