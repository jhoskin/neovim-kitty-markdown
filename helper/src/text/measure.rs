//! Text metrics in SVG mode and HTML mode.
//!
//! Chrome reports two kinds of numbers to mermaid. `getBBox()` on an SVG
//! `<text>` gives the summed glyph advances for the width and the font's
//! ascent plus descent for the height, each rounded to whole pixels the way
//! Blink's `SimpleFontData` rounds them. `getBoundingClientRect()` on an
//! HTML label div gives the same advance width and `line-height: 1.5`
//! times the font size per line. Both are reproduced here with rustybuzz
//! shaping (kerning and ligatures on, as HarfBuzz defaults in Chrome).
//! `getBBox` is the union of that advance box with the glyph ink, which
//! is what lets an italic overhang or a color emoji widen the box.
//!
//! Vertical metrics: CoreText, and so Chrome on macOS, takes ascent and
//! descent from `hhea` and ignores the OS/2 `USE_TYPO_METRICS` flag.
//! FreeType, and so Chrome on Linux, honors the flag. The oracle data in
//! `helper/tests/goldens/metrics.json` confirms the macOS rule for
//! Trebuchet MS, Arial, and Open Sans.

use std::cell::RefCell;
use std::collections::HashMap;

use super::fonts::{self, FontId, FontSystem, Style, Weight};
use super::markdown::{self, Line, Word, WordType};
use super::wrap;

/// Chrome's `SkScalarRoundToScalar`, which is also JavaScript's
/// `Math.round` for the positive values measured here.
pub fn round_half_up(x: f32) -> f32 {
    (x + 0.5).floor()
}

/// The CSS line height mermaid sets on HTML labels.
pub const HTML_LINE_HEIGHT: f32 = 1.5;
/// The tspan spacing `createFormattedText` uses for SVG labels, in em.
pub const SVG_LINE_HEIGHT_EM: f32 = 1.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontStyle {
    Normal,
    Italic,
}

impl From<FontStyle> for Style {
    fn from(style: FontStyle) -> Self {
        match style {
            FontStyle::Normal => Style::Normal,
            FontStyle::Italic => Style::Italic,
        }
    }
}

/// The CSS font a label is measured with.
#[derive(Clone, Debug, PartialEq)]
pub struct FontSpec {
    /// CSS `font-family` list, for example `"trebuchet ms", verdana, arial, sans-serif`.
    pub family: String,
    /// `font-size` in CSS pixels.
    pub size: f32,
    /// `font-weight` as a number; `bold` is 700.
    pub weight: u16,
    pub style: FontStyle,
}

impl FontSpec {
    pub fn new(family: impl Into<String>, size: f32) -> Self {
        FontSpec {
            family: family.into(),
            size,
            weight: 400,
            style: FontStyle::Normal,
        }
    }

    pub fn weight(mut self, weight: u16) -> Self {
        self.weight = weight;
        self
    }

    pub fn style(mut self, style: FontStyle) -> Self {
        self.style = style;
        self
    }

    pub fn family(&self, family: impl Into<String>) -> Self {
        FontSpec {
            family: family.into(),
            ..self.clone()
        }
    }

    /// The font an inner tspan or an HTML `<strong>`/`<em>` gets:
    /// `font-weight: bold` and `font-style: italic` are absolute.
    pub fn for_word(&self, kind: WordType) -> FontSpec {
        match kind {
            WordType::Normal => self.clone(),
            WordType::Strong => self.clone().weight(700),
            WordType::Em => self.clone().style(FontStyle::Italic),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

/// `calculateTextDimensions` result: whole pixels, like mermaid's.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct TextDimensions {
    pub width: f32,
    pub height: f32,
    pub line_height: f32,
}

/// Ink extents of shaped glyphs relative to the pen origin and the
/// baseline, in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ink {
    pub min_x: f32,
    pub max_x: f32,
    /// Highest point above the baseline.
    pub top: f32,
    /// Lowest point below the baseline, as a positive distance.
    pub bottom: f32,
}

impl Ink {
    fn union(a: Option<Ink>, b: Option<Ink>) -> Option<Ink> {
        match (a, b) {
            (Some(a), Some(b)) => Some(Ink {
                min_x: a.min_x.min(b.min_x),
                max_x: a.max_x.max(b.max_x),
                top: a.top.max(b.top),
                bottom: a.bottom.max(b.bottom),
            }),
            (a, None) => a,
            (None, b) => b,
        }
    }

    fn shifted(self, dx: f32) -> Ink {
        Ink {
            min_x: self.min_x + dx,
            max_x: self.max_x + dx,
            ..self
        }
    }
}

/// One shaped run in one face.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
struct RunMetrics {
    advance: f32,
    ink: Option<Ink>,
    /// Horizontal extent of bitmap glyphs, which have no outline. Their
    /// vertical extent depends on the line, so `shape` adds it.
    bitmap: Option<(f32, f32)>,
}

/// CoreText reports the advances of an `sbix` bitmap font (Apple Color
/// Emoji) at 1.25 times the `hmtx` value, rounded to whole pixels; the
/// oracle's emoji cases show 15, 18, and 20 px per glyph at 12, 14, and
/// 16 px against 1.0 em in the table.
const SBIX_ADVANCE_SCALE: f32 = 1.25;
/// Bitmap glyph boxes in Chrome's ink bounds extend one pixel past the
/// advance square on every side.
const BITMAP_INK_PADDING: f32 = 1.0;

/// A shaped line as Blink's SVG text layout sees it: the advance box of the
/// primary font plus the ink of every glyph.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Shaped {
    /// Sum of glyph advances, what `getComputedTextLength()` returns.
    pub width: f32,
    /// Rounded ascent of the primary face.
    pub ascent: f32,
    /// Rounded descent of the primary face.
    pub descent: f32,
    /// Ink of the outline glyphs and the bitmap boxes, relative to the pen
    /// origin and the baseline.
    pub ink: Option<Ink>,
}

impl Shaped {
    /// `getBBox()` of a `<text>` holding this line: the advance box joined
    /// with the ink bounds, the ink placed with the baseline at the rounded
    /// ascent. The oracle confirms both axes: an italic overhang widens the
    /// box, and an Arial descender at 16 px makes it 17.367 px tall, the
    /// rounded ascent 14 plus the glyph's 3.367 px below the baseline.
    pub fn bbox(&self) -> Size {
        let rect_height = self.ascent + self.descent;
        match self.ink {
            None => Size {
                width: self.width,
                height: rect_height,
            },
            Some(ink) => Size {
                width: ink.max_x.max(self.width) - ink.min_x.min(0.0),
                height: (self.ascent + ink.bottom).max(rect_height)
                    - (self.ascent - ink.top).min(0.0),
            },
        }
    }
}

/// The horizontal `trak` value at `size` pixels, in font units: the
/// normal track (value 0) when the table has one, else its only track,
/// interpolated linearly between the listed sizes and held at the ends,
/// as HarfBuzz's `TrackData::get_tracking` does. Chrome 156 on macOS
/// shapes PingFang SC with its single track (value 1): the oracle's Han
/// strings advance 21, 20, and 20 units per glyph more than `hmtx` at 12,
/// 14, and 16 px, the table's entries at those sizes. rustybuzz applies
/// only a track of value 0, so the measurer adds this itself.
fn trak_tracking(face: &rustybuzz::Face<'_>, size: f32) -> f32 {
    let Some(trak) = face.tables().trak else {
        return 0.0;
    };
    let data = trak.horizontal;
    let tracks: Vec<_> = (0..data.tracks.len())
        .filter_map(|i| data.tracks.get(i))
        .collect();
    let track = tracks
        .iter()
        .find(|t| t.value == 0.0)
        .or_else(|| (tracks.len() == 1).then(|| &tracks[0]));
    let Some(track) = track else {
        return 0.0;
    };
    let points: Vec<(f32, f32)> = (0..data.sizes.len())
        .filter_map(|i| Some((data.sizes.get(i)?.0, f32::from(track.values.get(i)?))))
        .collect();
    let (Some(&first), Some(&last)) = (points.first(), points.last()) else {
        return 0.0;
    };
    if size <= first.0 {
        return first.1;
    }
    if size >= last.0 {
        return last.1;
    }
    points
        .windows(2)
        .find(|pair| size <= pair[1].0)
        .map_or(last.1, |pair| {
            let (a, b) = (pair[0], pair[1]);
            a.1 + (b.1 - a.1) * (size - a.0) / (b.0 - a.0)
        })
}

/// The ink box of one glyph in font units, `[x_min, y_min, x_max, y_max]`
/// with y up. Under a synthetic oblique (`skew` > 0) the outline is
/// skewed first (x' = x + skew * y) and the box is the tight box of the
/// skewed curves, as CoreText reports for the skewed font Chrome draws.
/// Skewing the corners of the upright box instead overshoots wherever a
/// glyph's top right or bottom left corner is empty, which the oracle
/// shows as up to 1.4 px too wide on a Han string at 16 px.
fn glyph_box(
    face: &rustybuzz::Face<'_>,
    glyph: rustybuzz::ttf_parser::GlyphId,
    skew: f32,
) -> Option<[f32; 4]> {
    if skew == 0.0 {
        let r = face.glyph_bounding_box(glyph)?;
        return Some([r.x_min, r.y_min, r.x_max, r.y_max].map(f32::from));
    }
    let mut bounds = SkewedBounds::new(skew);
    face.outline_glyph(glyph, &mut bounds)?;
    bounds.finish()
}

/// Tight bounds of an outline under a horizontal skew. A quadratic or
/// cubic segment stays a Bezier curve of the same degree after the skew,
/// so its extremes are at the end points or where the derivative of each
/// coordinate is zero.
struct SkewedBounds {
    skew: f32,
    last: (f32, f32),
    min: (f32, f32),
    max: (f32, f32),
}

impl SkewedBounds {
    fn new(skew: f32) -> Self {
        SkewedBounds {
            skew,
            last: (0.0, 0.0),
            min: (f32::INFINITY, f32::INFINITY),
            max: (f32::NEG_INFINITY, f32::NEG_INFINITY),
        }
    }

    fn skewed(&self, x: f32, y: f32) -> (f32, f32) {
        (x + self.skew * y, y)
    }

    fn add(&mut self, p: (f32, f32)) {
        self.min = (self.min.0.min(p.0), self.min.1.min(p.1));
        self.max = (self.max.0.max(p.0), self.max.1.max(p.1));
    }

    fn finish(&self) -> Option<[f32; 4]> {
        (self.min.0 <= self.max.0).then_some([self.min.0, self.min.1, self.max.0, self.max.1])
    }

    /// Adds the extremes of one coordinate of a Bezier curve between its
    /// end points: `points` are that coordinate's control values.
    fn extremes(points: &[f32]) -> Vec<f32> {
        match *points {
            [p0, p1, p2] => {
                let d = p0 - 2.0 * p1 + p2;
                if d == 0.0 {
                    return Vec::new();
                }
                vec![(p0 - p1) / d]
            }
            [p0, p1, p2, p3] => {
                // B'(t)/3 = a t^2 + b t + c
                let a = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
                let b = 2.0 * (p0 - 2.0 * p1 + p2);
                let c = p1 - p0;
                if a.abs() < 1e-6 {
                    return if b == 0.0 { Vec::new() } else { vec![-c / b] };
                }
                let disc = b * b - 4.0 * a * c;
                if disc < 0.0 {
                    return Vec::new();
                }
                let root = disc.sqrt();
                vec![(-b + root) / (2.0 * a), (-b - root) / (2.0 * a)]
            }
            _ => Vec::new(),
        }
    }

    fn curve(&mut self, points: &[(f32, f32)]) {
        let xs: Vec<f32> = points.iter().map(|p| p.0).collect();
        let ys: Vec<f32> = points.iter().map(|p| p.1).collect();
        let mut ts = Self::extremes(&xs);
        ts.extend(Self::extremes(&ys));
        for t in ts.into_iter().filter(|t| *t > 0.0 && *t < 1.0) {
            let at = |v: &[f32]| -> f32 {
                let u = 1.0 - t;
                match *v {
                    [a, b, c] => u * u * a + 2.0 * u * t * b + t * t * c,
                    [a, b, c, d] => {
                        u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
                    }
                    _ => 0.0,
                }
            };
            self.add((at(&xs), at(&ys)));
        }
        if let Some(&end) = points.last() {
            self.add(end);
            self.last = end;
        }
    }
}

impl rustybuzz::ttf_parser::OutlineBuilder for SkewedBounds {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.skewed(x, y);
        self.add(p);
        self.last = p;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.skewed(x, y);
        self.add(p);
        self.last = p;
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let points = [self.last, self.skewed(x1, y1), self.skewed(x, y)];
        self.curve(&points);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let points = [
            self.last,
            self.skewed(x1, y1),
            self.skewed(x2, y2),
            self.skewed(x, y),
        ];
        self.curve(&points);
    }

    fn close(&mut self) {}
}

/// Skia's fake-italic skew, applied when an italic request lands on an
/// upright face: x' = x + y / 4.
const SYNTHETIC_OBLIQUE_SKEW: f32 = 0.25;
/// How far a synthetic bold pushes the ink out on each side, as a fraction
/// of the font size. The raster step draws the same growth, so the measured
/// box and the drawn ink come from one constant.
const SYNTHETIC_BOLD_INK_GROWTH: f32 = crate::text::fonts::SYNTHETIC_BOLD_GROWTH as f32;

/// What Chrome fakes when the matched face lacks the requested style.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
struct Synthetic {
    oblique: bool,
    bold: bool,
}

/// The SVG label `createFormattedText` builds, as `getBBox` sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct SvgLabel {
    pub width: f32,
    pub height: f32,
    pub lines: Vec<Line>,
}

/// The HTML label `addHtmlSpan` builds, as `getBoundingClientRect` sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct HtmlLabel {
    pub width: f32,
    pub height: f32,
    pub lines: Vec<Line>,
    /// True when the browser switched to `display: table` and wrapped.
    pub wrapped: bool,
}

#[derive(Hash, PartialEq, Eq)]
struct ShapeKey {
    text: String,
    face: FontId,
    size_bits: u32,
    synthetic: Synthetic,
}

pub struct TextMetrics<'f> {
    fonts: &'f FontSystem,
    runs: RefCell<HashMap<ShapeKey, RunMetrics>>,
}

impl<'f> TextMetrics<'f> {
    pub fn new(fonts: &'f FontSystem) -> Self {
        TextMetrics {
            fonts,
            runs: RefCell::new(HashMap::new()),
        }
    }

    pub fn fonts(&self) -> &'f FontSystem {
        self.fonts
    }

    fn primary_face(&self, spec: &FontSpec) -> Option<FontId> {
        self.fonts
            .resolve(&spec.family, Weight(spec.weight), spec.style.into())
    }

    /// Unrounded ascent and descent in pixels from the tables Chrome reads
    /// on this platform.
    pub fn face_float_metrics(&self, face: FontId, size: f32) -> (f32, f32) {
        self.fonts
            .with_face(face, |f| {
                let upem = f.units_per_em() as f32;
                let (ascender, descender) = if cfg!(target_os = "macos") {
                    (f.ascender(), f.descender())
                } else {
                    let typo = f.tables().os2.filter(|os2| os2.use_typographic_metrics());
                    match typo {
                        Some(os2) => (os2.typographic_ascender(), os2.typographic_descender()),
                        None => (f.ascender(), f.descender()),
                    }
                };
                (
                    ascender as f32 / upem * size,
                    -(descender as f32) / upem * size,
                )
            })
            .unwrap_or((0.0, 0.0))
    }

    /// Rounded ascent and descent in pixels, as Chrome reports them.
    pub fn face_ascent_descent(&self, face: FontId, size: f32) -> (f32, f32) {
        let (ascent, descent) = self.face_float_metrics(face, size);
        (round_half_up(ascent), round_half_up(descent))
    }

    /// Splits `text` into runs by the face that has each character:
    /// the primary face when it covers the character, else the next family
    /// of `chain` that does, else the platform fallback
    /// ([`FontSystem::fallback_in_chain`]). Marks, joiners, and variation
    /// selectors stay with their base.
    fn runs(
        &self,
        text: &str,
        chain: &str,
        primary: FontId,
        weight: Weight,
        style: Style,
    ) -> Vec<(FontId, String)> {
        let mut runs: Vec<(FontId, String)> = Vec::new();
        for grapheme in wrap::split_text_to_chars(text) {
            let face = self.grapheme_face(&grapheme, chain, primary, weight, style);
            match runs.last_mut() {
                Some((last, run)) if *last == face => run.push_str(&grapheme),
                _ => runs.push((face, grapheme)),
            }
        }
        runs
    }

    fn grapheme_face(
        &self,
        grapheme: &str,
        chain: &str,
        primary: FontId,
        weight: Weight,
        style: Style,
    ) -> FontId {
        let base = grapheme.chars().next().unwrap_or(' ');
        let in_primary =
            self.fonts.has_glyph(primary, base) && !fonts::emoji_presentation(grapheme);
        if in_primary || base.is_whitespace() {
            return primary;
        }
        self.fonts
            .fallback_in_chain(chain, grapheme, weight, style)
            .unwrap_or(primary)
    }

    /// The face each grapheme of `text` is drawn in, as [`Self::shape`]
    /// splits it. The calibration test compares these with the faces
    /// Chrome reports through `CSS.getPlatformFontsForNode`.
    pub fn grapheme_faces(&self, text: &str, spec: &FontSpec) -> Vec<(String, FontId)> {
        let weight = Weight(spec.weight);
        let style: Style = spec.style.into();
        let Some(primary) = self.primary_face(spec) else {
            return Vec::new();
        };
        wrap::split_text_to_chars(text)
            .into_iter()
            .map(|grapheme| {
                let face = self.grapheme_face(&grapheme, &spec.family, primary, weight, style);
                (grapheme, face)
            })
            .collect()
    }

    /// Advance width of `text` shaped in one face at `size`, in pixels.
    pub fn run_advance(&self, text: &str, face: FontId, size: f32) -> f32 {
        self.run_metrics(text, face, size, Synthetic::default())
            .advance
    }

    /// Shapes `text` in one face: summed advances and the union of the
    /// glyph outlines' bounding boxes, skewed or grown when Chrome would
    /// synthesize an oblique or a bold. Bitmap-only glyphs (color emoji)
    /// have no outline; their boxes are collected apart.
    fn run_metrics(&self, text: &str, face: FontId, size: f32, synthetic: Synthetic) -> RunMetrics {
        let key = ShapeKey {
            text: text.to_string(),
            face,
            size_bits: size.to_bits(),
            synthetic,
        };
        if let Some(found) = self.runs.borrow().get(&key) {
            return *found;
        }
        let metrics = self
            .fonts
            .with_face(face, |f| {
                let sbix = f
                    .raw_face()
                    .table(rustybuzz::ttf_parser::Tag::from_bytes(b"sbix"))
                    .is_some();
                let scale = size / f.units_per_em() as f32;
                let skew = if synthetic.oblique {
                    SYNTHETIC_OBLIQUE_SKEW
                } else {
                    0.0
                };
                let grow = if synthetic.bold {
                    size * SYNTHETIC_BOLD_INK_GROWTH
                } else {
                    0.0
                };
                let mut buffer = rustybuzz::UnicodeBuffer::new();
                buffer.push_str(text);
                let glyphs = rustybuzz::shape(f, &[], buffer);
                let tables = f.tables();
                let readable =
                    tables.glyf.is_some() || tables.cff.is_some() || tables.cff2.is_some();
                let platform_boxes = if sbix || readable {
                    Vec::new()
                } else {
                    let ids: Vec<u16> = glyphs
                        .glyph_infos()
                        .iter()
                        .map(|info| info.glyph_id as u16)
                        .collect();
                    self.fonts.platform_glyph_bounds(face, &ids, skew)
                };
                let tracking = if sbix {
                    0.0
                } else {
                    trak_tracking(f, size) * scale
                };
                // The pen runs in pixels: bitmap advances round per glyph.
                let mut pen = 0f32;
                let mut ink: Option<Ink> = None;
                let mut bitmap: Option<(f32, f32)> = None;
                let mut last_cluster = None;
                for (index, (info, pos)) in glyphs
                    .glyph_infos()
                    .iter()
                    .zip(glyphs.glyph_positions())
                    .enumerate()
                {
                    let glyph = rustybuzz::ttf_parser::GlyphId(info.glyph_id as u16);
                    // The tracking widens the first glyph of each cluster.
                    // HarfBuzz would also move that glyph right by half of
                    // it; Chrome does not, as the italic Han ink shows.
                    let track = if last_cluster == Some(info.cluster) {
                        0.0
                    } else {
                        tracking
                    };
                    last_cluster = Some(info.cluster);
                    let x = pen + pos.x_offset as f32 * scale;
                    let advance = if sbix {
                        round_half_up(pos.x_advance as f32 * scale * SBIX_ADVANCE_SCALE)
                    } else {
                        pos.x_advance as f32 * scale + track
                    };
                    // An sbix font carries placeholder outlines; only its
                    // bitmaps count.
                    let outline = if sbix {
                        None
                    } else if readable {
                        glyph_box(f, glyph, skew)
                    } else {
                        platform_boxes
                            .get(index)
                            .copied()
                            .flatten()
                            .map(|b| b.map(|v| v as f32))
                    };
                    if let Some([x_min, y_min, x_max, y_max]) = outline {
                        let y_offset = pos.y_offset as f32;
                        let dx = x + skew * y_offset * scale;
                        let glyph_ink = Ink {
                            min_x: dx + x_min * scale - grow,
                            max_x: dx + x_max * scale + grow,
                            top: (y_max + y_offset) * scale + grow,
                            bottom: -(y_min + y_offset) * scale + grow,
                        };
                        ink = Ink::union(ink, Some(glyph_ink));
                    } else if sbix && advance > 0.0 {
                        let left = x - BITMAP_INK_PADDING;
                        let right = x + advance + BITMAP_INK_PADDING;
                        bitmap = Some(match bitmap {
                            Some((l, r)) => (l.min(left), r.max(right)),
                            None => (left, right),
                        });
                    }
                    pen += advance;
                }
                RunMetrics {
                    advance: pen,
                    ink,
                    bitmap,
                }
            })
            .unwrap_or_default();
        self.runs.borrow_mut().insert(key, metrics);
        metrics
    }

    /// One line of text in one font, shaped run by run. The advance box
    /// comes from the primary face alone, as in Blink; fallback glyphs only
    /// contribute ink.
    pub fn shape(&self, text: &str, spec: &FontSpec) -> Shaped {
        let weight = Weight(spec.weight);
        let style: Style = spec.style.into();
        let Some(primary) = self.primary_face(spec).or_else(|| {
            text.chars()
                .find_map(|ch| self.fonts.fallback_for(ch, weight, style))
        }) else {
            return Shaped::default();
        };
        let (ascent, descent) = self.face_ascent_descent(primary, spec.size);
        let mut width = 0.0;
        let mut ink: Option<Ink> = None;
        for (face, run) in self.runs(text, &spec.family, primary, weight, style) {
            let synthetic = Synthetic {
                oblique: spec.style == FontStyle::Italic
                    && self.fonts.face_style(face) == Some(Style::Normal),
                bold: spec.weight >= 600 && self.fonts.face_weight(face).is_some_and(|w| w.0 < 600),
            };
            let oblique = synthetic.oblique;
            let metrics = self.run_metrics(&run, face, spec.size, synthetic);
            ink = Ink::union(ink, metrics.ink.map(|i| i.shifted(width)));
            if let Some((left, right)) = metrics.bitmap {
                // A bitmap strike box is square: as tall as its advance,
                // from one pixel above the em box, padded like the width.
                // Its place does not depend on the primary face: under
                // Trebuchet MS and Open Sans alike the oracle's 12, 14, and
                // 16 px emoji reach 13, 15, and 17 px above the baseline.
                // A synthetic oblique moves the top right corner by the
                // skew, and Skia rounds the glyph box out to whole pixels.
                let side = right - left;
                let top = spec.size + BITMAP_INK_PADDING;
                let lean = if oblique {
                    (SYNTHETIC_OBLIQUE_SKEW * top).ceil()
                } else {
                    0.0
                };
                ink = Ink::union(
                    ink,
                    Some(Ink {
                        min_x: width + left,
                        max_x: width + right + lean,
                        top,
                        bottom: side - top,
                    }),
                );
            }
            width += metrics.advance;
        }
        Shaped {
            width,
            ascent,
            descent,
            ink,
        }
    }

    /// Width of a line of styled words laid end to end, each word shaped in
    /// its own font (an inline style boundary ends a shaping run in Chrome).
    pub fn words_width(&self, words: &[Word], spec: &FontSpec) -> f32 {
        words
            .iter()
            .map(|word| self.shape(&word.content, &spec.for_word(word.kind)).width)
            .sum()
    }

    /// `drawSimpleText` followed by `getBBox()`: `<br>` tags become spaces,
    /// an empty string measures as a zero-width space.
    pub fn svg_bbox(&self, text: &str, spec: &FontSpec) -> Size {
        let joined: String = markdown::split_breaks(text).join(" ");
        self.shape(&joined, spec).bbox()
    }

    /// `calculateTextDimensions`: each `<br>` line measured on its own,
    /// widths and heights rounded, and the `sans-serif` measurement taken
    /// instead of the configured family's only when it is larger in width,
    /// height, and line height at once.
    pub fn calculate_text_dimensions(&self, text: &str, spec: &FontSpec) -> TextDimensions {
        if text.is_empty() {
            return TextDimensions::default();
        }
        let lines = markdown::split_breaks(text);
        let families = ["sans-serif", css_family_or_page_default(&spec.family)];
        let dims: Vec<TextDimensions> = families
            .iter()
            .map(|family| {
                let family_spec = spec.family(*family);
                let mut dim = TextDimensions::default();
                for line in &lines {
                    let bbox = self.svg_bbox(line, &family_spec);
                    dim.width = round_half_up(dim.width.max(bbox.width));
                    let line_height = round_half_up(bbox.height);
                    dim.height += line_height;
                    dim.line_height = round_half_up(dim.line_height.max(line_height));
                }
                dim
            })
            .collect();
        let fallback_wins = dims[0].height > dims[1].height
            && dims[0].width > dims[1].width
            && dims[0].line_height > dims[1].line_height;
        if fallback_wins { dims[0] } else { dims[1] }
    }

    pub fn calculate_text_width(&self, text: &str, spec: &FontSpec) -> f32 {
        self.calculate_text_dimensions(text, spec).width
    }

    pub fn calculate_text_height(&self, text: &str, spec: &FontSpec) -> f32 {
        self.calculate_text_dimensions(text, spec).height
    }

    /// `computeWidthOfText`: the outer tspan's `getComputedTextLength()`
    /// after `updateTextContentAndStyles` put each word in an inner tspan
    /// with a leading space on all but the first.
    pub fn compute_width_of_text(&self, line: &[Word], spec: &FontSpec) -> f32 {
        line.iter()
            .enumerate()
            .map(|(index, word)| {
                let content = if index == 0 {
                    word.content.clone()
                } else {
                    format!(" {}", word.content)
                };
                self.shape(&content, &spec.for_word(word.kind)).width
            })
            .sum()
    }

    /// `createFormattedText` for an SVG label: lines that do not fit are
    /// split with `splitLineToFitWidth`, every line becomes a tspan 1.1 em
    /// below the previous one, and the `<text>` element's `getBBox()` is
    /// the union of the glyph boxes.
    pub fn svg_label(&self, structured: &[Line], spec: &FontSpec, max_width: f32) -> SvgLabel {
        let check_fit = |line: &[Word]| self.compute_width_of_text(line, spec) <= max_width;
        let mut lines: Vec<Line> = Vec::new();
        for line in structured {
            if check_fit(line) {
                lines.push(line.clone());
            } else {
                lines.extend(wrap::split_line_to_fit_width(line, &check_fit));
            }
        }
        let has_text = lines.iter().any(|line| !line.is_empty());
        if !has_text {
            return SvgLabel {
                width: 0.0,
                height: 0.0,
                lines,
            };
        }
        let width = lines
            .iter()
            .map(|line| self.compute_width_of_text(line, spec))
            .fold(0.0, f32::max);
        let vertical = |line: &Line| -> (f32, f32) {
            line.iter().fold((0.0f32, 0.0f32), |(a, d), word| {
                let shaped = self.shape(&word.content, &spec.for_word(word.kind));
                (a.max(shaped.ascent), d.max(shaped.descent))
            })
        };
        let first = lines.iter().find(|line| !line.is_empty()).map(vertical);
        let last = lines
            .iter()
            .rev()
            .find(|line| !line.is_empty())
            .map(vertical);
        let ascent = first.map_or(0.0, |(a, _)| a);
        let descent = last.map_or(0.0, |(_, d)| d);
        let height = (lines.len().saturating_sub(1)) as f32 * SVG_LINE_HEIGHT_EM * spec.size
            + ascent
            + descent;
        SvgLabel {
            width,
            height,
            lines,
        }
    }

    /// `addHtmlSpan` for an HTML label: a `table-cell` with `nowrap` and
    /// `line-height: 1.5`. When the text reaches `max_width`, mermaid turns
    /// the div into a `table` of that width with `break-spaces`, and the
    /// browser wraps it; a word wider than `max_width` widens the table.
    pub fn html_label(&self, lines: &[Line], spec: &FontSpec, max_width: Option<f32>) -> HtmlLabel {
        let collapsed: Vec<Line> = lines
            .iter()
            .map(|line| wrap::collapse_whitespace(line))
            .collect();
        let has_text = collapsed.iter().any(|line| !line.is_empty());
        if !has_text {
            return HtmlLabel {
                width: 0.0,
                height: 0.0,
                lines: collapsed,
                wrapped: false,
            };
        }
        let width_of = |line: &[Word]| self.words_width(line, spec);
        let nowrap_width = collapsed
            .iter()
            .map(|line| width_of(line))
            .fold(0.0, f32::max);
        let line_height = HTML_LINE_HEIGHT * spec.size;
        match max_width {
            Some(max) if nowrap_width >= max => {
                let min_content = lines
                    .iter()
                    .map(|line| wrap::min_content_width(line, &width_of))
                    .fold(0.0, f32::max);
                let avail = max.max(min_content);
                let wrapped: Vec<Line> = lines
                    .iter()
                    .flat_map(|line| wrap::wrap_break_spaces(line, avail, &width_of))
                    .collect();
                HtmlLabel {
                    width: avail,
                    height: line_height * wrapped.len() as f32,
                    lines: wrapped,
                    wrapped: true,
                }
            }
            _ => HtmlLabel {
                width: nowrap_width,
                height: line_height * collapsed.len() as f32,
                lines: collapsed,
                wrapped: false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::font_system;

    fn open_sans(size: f32) -> FontSpec {
        FontSpec::new("\"Open Sans\", sans-serif", size)
    }

    #[test]
    fn rounding_matches_chrome() {
        assert_eq!(round_half_up(11.268), 11.0);
        assert_eq!(round_half_up(2.666), 3.0);
        assert_eq!(round_half_up(2.5), 3.0);
        assert_eq!(round_half_up(0.0), 0.0);
    }

    #[test]
    fn open_sans_height_is_rounded_hhea_metrics() {
        // hhea ascender 2189, descender -600, 2048 units per em.
        let fonts = font_system();
        let metrics = TextMetrics::new(fonts);
        assert_eq!(metrics.svg_bbox("Hello", &open_sans(12.0)).height, 17.0);
        assert_eq!(metrics.svg_bbox("Hello", &open_sans(14.0)).height, 19.0);
        assert_eq!(metrics.svg_bbox("Hello", &open_sans(16.0)).height, 22.0);
        assert_eq!(metrics.svg_bbox("", &open_sans(16.0)).width, 0.0);
    }

    #[test]
    fn kerning_changes_the_advance() {
        // The bundled Open Sans carries no kerning (GPOS has only mark and
        // mkmk), so this needs a system font with a kern table.
        let fonts = font_system();
        if fonts
            .resolve(
                "Trebuchet MS",
                crate::text::fonts::Weight::NORMAL,
                Style::Normal,
            )
            .is_none()
        {
            eprintln!("skipping kerning test: Trebuchet MS is not installed");
            return;
        }
        let metrics = TextMetrics::new(fonts);
        let spec = FontSpec::new("Trebuchet MS", 16.0);
        let pair = metrics.shape("AV", &spec).width;
        let separate = metrics.shape("A", &spec).width + metrics.shape("V", &spec).width;
        assert!(pair < separate, "{pair} should be less than {separate}");
    }

    #[test]
    fn synthetic_bold_keeps_regular_advances() {
        let fonts = font_system();
        let metrics = TextMetrics::new(fonts);
        let regular = metrics.svg_bbox("Hello", &open_sans(16.0)).width;
        let bold = metrics
            .svg_bbox("Hello", &open_sans(16.0).weight(700))
            .width;
        assert_eq!(regular, bold);
        let italic = metrics
            .svg_bbox("Hello", &open_sans(16.0).style(FontStyle::Italic))
            .width;
        assert_ne!(regular, italic);
    }

    #[test]
    fn calculate_text_dimensions_rounds_and_sums_lines() {
        let fonts = font_system();
        let metrics = TextMetrics::new(fonts);
        let spec = open_sans(16.0);
        assert_eq!(
            metrics.calculate_text_dimensions("", &spec),
            TextDimensions::default()
        );
        let one = metrics.calculate_text_dimensions("Hello", &spec);
        assert_eq!(one.width, one.width.round());
        assert_eq!(one.height, one.line_height);
        let two = metrics.calculate_text_dimensions("Hello<br>Hello world", &spec);
        assert_eq!(two.height, 2.0 * one.line_height);
        assert!(two.width > one.width);
    }

    #[test]
    fn html_label_nowrap_and_wrapped() {
        let fonts = font_system();
        let metrics = TextMetrics::new(fonts);
        let spec = open_sans(16.0);
        let lines = markdown::html_to_lines("<p>Hello world</p>");
        let plain = metrics.html_label(&lines, &spec, Some(200.0));
        assert!(!plain.wrapped);
        assert_eq!(plain.height, 24.0);
        let long = markdown::html_to_lines(&markdown::markdown_to_html(
            "This is a fairly long node label that the browser has to wrap onto several lines",
        ));
        let wrapped = metrics.html_label(&long, &spec, Some(200.0));
        assert!(wrapped.wrapped);
        assert_eq!(wrapped.width, 200.0);
        assert_eq!(wrapped.height, 24.0 * wrapped.lines.len() as f32);
        assert!(wrapped.lines.len() >= 3);
        let empty = metrics.html_label(&markdown::html_to_lines(""), &spec, Some(200.0));
        assert_eq!(empty.height, 0.0);
    }

    #[test]
    fn svg_label_splits_long_lines() {
        let fonts = font_system();
        let metrics = TextMetrics::new(fonts);
        let spec = open_sans(16.0);
        let structured = markdown::markdown_to_lines("The quick brown fox jumps over the lazy dog");
        let label = metrics.svg_label(&structured, &spec, 100.0);
        assert!(label.lines.len() > 1);
        assert!(label.width <= 100.0 + 1e-3);
        let one_line = metrics.svg_label(&structured, &spec, 1000.0);
        assert_eq!(one_line.lines.len(), 1);
        assert_eq!(one_line.height, 22.0);
        assert_eq!(
            label.height,
            (label.lines.len() - 1) as f32 * SVG_LINE_HEIGHT_EM * 16.0 + 22.0
        );
    }
}

/// Chrome's CSS parser rejects a `font-family` value it cannot read, and
/// `style.setProperty` then leaves the property unset. mermaid's default
/// top-level `fontFamily` ends with a `;`, so `calculateTextDimensions`
/// measures in the page default of mermaid-cli's headless page,
/// `"Times New Roman", serif`, while the drawn text inherits the
/// stylesheet's family (the theme variable, which has no `;`).
pub fn css_family_or_page_default(family: &str) -> &str {
    if family.contains(';') {
        "\"Times New Roman\", serif"
    } else {
        family
    }
}

#[cfg(test)]
mod css_family_tests {
    use super::css_family_or_page_default;

    #[test]
    fn a_trailing_semicolon_falls_back_to_the_page_default() {
        assert_eq!(
            css_family_or_page_default("\"trebuchet ms\", verdana, arial, sans-serif;"),
            "\"Times New Roman\", serif"
        );
        assert_eq!(
            css_family_or_page_default("\"trebuchet ms\", verdana, arial, sans-serif"),
            "\"trebuchet ms\", verdana, arial, sans-serif"
        );
    }
}
