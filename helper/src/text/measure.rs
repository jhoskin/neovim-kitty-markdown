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

/// The horizontal extent of the bitmap glyphs of one run.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Bitmap {
    left: f32,
    right: f32,
}

/// One shaped run in one face.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
struct RunMetrics {
    advance: f32,
    ink: Option<Ink>,
    /// Horizontal extent of bitmap glyphs, which have no outline. Their
    /// vertical extent depends on the size alone ([`sbix_box`]), so `shape`
    /// adds it.
    bitmap: Option<Bitmap>,
}

/// Bitmap glyph boxes in Chrome's ink bounds extend one pixel past the
/// advance on either side.
const BITMAP_INK_PADDING: f32 = 1.0;

/// The advance of a 1 em glyph of an `sbix` font (Apple Color Emoji) and
/// how far its ink box reaches above and below the baseline, in whole
/// pixels, for the font sizes 6 to 64 px. The oracle's Chrome reports
/// these for `😀` under any family; they follow neither the `hmtx` value
/// nor CoreText's own advances and bounds (21 px and 16 px above the
/// baseline at 16 px, where Chrome has 20 and 17), so they are a table.
/// The advance is `max(size, min(round(1.25 * size), (size + 25) / 2))`.
const SBIX_BOXES: [(u8, u8, u8); 59] = [
    (8, 7, 3),
    (9, 8, 3),
    (10, 9, 3),
    (11, 10, 4),
    (13, 11, 4),
    (14, 12, 4),
    (15, 13, 4),
    (16, 14, 5),
    (18, 15, 5),
    (19, 16, 5),
    (20, 17, 5),
    (21, 18, 5),
    (21, 19, 5),
    (22, 19, 5),
    (22, 20, 5),
    (23, 21, 5),
    (23, 21, 5),
    (24, 22, 5),
    (24, 23, 5),
    (25, 23, 5),
    (26, 24, 6),
    (27, 25, 6),
    (28, 26, 6),
    (29, 27, 6),
    (30, 28, 6),
    (31, 29, 7),
    (32, 30, 7),
    (33, 30, 7),
    (34, 31, 7),
    (35, 32, 7),
    (36, 33, 8),
    (37, 34, 8),
    (38, 35, 8),
    (39, 36, 8),
    (40, 37, 8),
    (41, 37, 9),
    (42, 38, 9),
    (43, 39, 9),
    (44, 40, 9),
    (45, 41, 10),
    (46, 42, 10),
    (47, 43, 10),
    (48, 44, 10),
    (49, 44, 10),
    (50, 45, 11),
    (51, 46, 11),
    (52, 47, 11),
    (53, 48, 11),
    (54, 49, 11),
    (55, 50, 12),
    (56, 51, 12),
    (57, 52, 12),
    (58, 53, 12),
    (59, 53, 13),
    (60, 54, 13),
    (61, 55, 13),
    (62, 56, 13),
    (63, 57, 13),
    (64, 58, 14),
];

/// [`SBIX_BOXES`] at `size` pixels: the nearest whole size, and beyond the
/// table the advance of the size and the box proportions of 64 px.
fn sbix_box(size: f32) -> (f32, f32, f32) {
    let whole = round_half_up(size).max(6.0);
    match SBIX_BOXES.get(whole as usize - 6) {
        Some(&(advance, top, bottom)) => (f32::from(advance), f32::from(top), f32::from(bottom)),
        None => (
            whole,
            (whole * 58.0 / 64.0).round(),
            (whole * 14.0 / 64.0).round(),
        ),
    }
}

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

/// The Unicode script of `ch`, or `None` for the Common and Inherited
/// scripts (spaces, digits, punctuation, marks), which take the script of
/// the text around them. ASCII is answered without a lookup.
fn script_of(ch: char) -> Option<rustybuzz::Script> {
    if ch.is_ascii() {
        return ch.is_ascii_alphabetic().then_some(rustybuzz::script::LATIN);
    }
    // Private-use characters and noncharacters have the Unknown script,
    // which ICU's script runs keep apart from the Latin around them: the
    // oracle draws `U+E000` in `aU+E000b` under Open Sans as Arial's
    // `.notdef`, the face its own segment starts with.
    let cp = u32::from(ch);
    if matches!(cp, 0xE000..=0xF8FF | 0xF0000..=0x10FFFF | 0xFDD0..=0xFDEF) || cp & 0xFFFE == 0xFFFE
    {
        return Some(rustybuzz::script::UNKNOWN);
    }
    // HarfBuzz's segment guess skips Common and Inherited characters, so a
    // one-character buffer keeps no script for them.
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.add(ch, 0);
    buffer.guess_segment_properties();
    let script = buffer.script();
    // Blink's `ScriptRunIterator` collapses katakana into hiragana
    // (`GetScriptForOpenType`), so kana of both kinds share a segment: the
    // oracle draws `ゟ` in `ア𛀁ゟ` in the Hiragino Sans found for `ア`.
    if script == rustybuzz::script::KATAKANA {
        return Some(rustybuzz::script::HIRAGANA);
    }
    (script != rustybuzz::script::UNKNOWN).then_some(script)
}

/// The script segment of each grapheme: a new segment starts where a
/// grapheme with a script of its own differs from the segment's script.
/// Common and Inherited graphemes stay in the current segment, so leading
/// ones join the first script that follows, as in ICU's script runs.
fn script_segments(graphemes: &[String]) -> Vec<usize> {
    let mut current: Option<rustybuzz::Script> = None;
    let mut segment = 0;
    graphemes
        .iter()
        .map(|g| {
            if let Some(script) = g.chars().next().and_then(script_of) {
                match current {
                    Some(c) if c != script => {
                        segment += 1;
                        current = Some(script);
                    }
                    None => current = Some(script),
                    Some(_) => {}
                }
            }
            segment
        })
        .collect()
}

/// The script segments of [`script_segments`], cut again around every
/// emoji-presentation grapheme: Blink's `RunSegmenter` starts a run where
/// the fallback priority changes, and each emoji sequence is a run.
fn emoji_segments(graphemes: &[String], scripts: &[usize]) -> Vec<usize> {
    let mut segment = 0;
    let mut previous: Option<(usize, bool)> = None;
    graphemes
        .iter()
        .zip(scripts)
        .map(|(g, &script)| {
            let emoji = fonts::emoji_presentation(g);
            if let Some((last_script, last_emoji)) = previous
                && (script != last_script || emoji || last_emoji)
            {
                segment += 1;
            }
            previous = Some((script, emoji));
            segment
        })
        .collect()
}

/// Blink's `HanKerningCharType` after the font checks: the classes
/// `text-spacing-trim: normal` (Chrome's default) pairs up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HanType {
    Open,
    Close,
    OpenNarrow,
    CloseNarrow,
    Middle,
    Other,
}

/// The class of `ch` as Blink's `SetHanKerning`
/// (character_property_data_generator.cc) assigns it, with the
/// font-dependent classes left open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HanClass {
    Fixed(HanType),
    Dot,
    Colon,
    Semicolon,
    OpenQuote,
    CloseQuote,
}

fn han_class(ch: char) -> HanClass {
    match ch {
        '\u{2018}' | '\u{201C}' => HanClass::OpenQuote,
        '\u{2019}' | '\u{201D}' => HanClass::CloseQuote,
        '\u{3000}' | '\u{00B7}' | '\u{2027}' | '\u{30FB}' => HanClass::Fixed(HanType::Middle),
        '、' | '。' | '，' | '．' => HanClass::Dot,
        '：' => HanClass::Colon,
        '；' => HanClass::Semicolon,
        // `[[:blk=CJK_Symbols:][:ea=F:] & [:gc=Ps:]]` and its `Pe` twin.
        '〈' | '《' | '「' | '『' | '【' | '〔' | '〖' | '〘' | '〚' | '〝' | '（' | '［'
        | '｛' | '｟' => HanClass::Fixed(HanType::Open),
        '〉' | '》' | '」' | '』' | '】' | '〕' | '〗' | '〙' | '〛' | '〞' | '〟' | '）'
        | '］' | '｝' | '｠' => HanClass::Fixed(HanType::Close),
        // The other `Ps` and `Pe` characters; the ASCII ones are the ones
        // labels use.
        '(' | '[' | '{' => HanClass::Fixed(HanType::OpenNarrow),
        ')' | ']' | '}' => HanClass::Fixed(HanType::CloseNarrow),
        _ => HanClass::Fixed(HanType::Other),
    }
}

/// How a face draws the font-dependent classes (Blink's
/// `HanKerning::FontData`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HanFontData {
    dot: HanType,
    colon: HanType,
    semicolon: HanType,
    quotes_fullwidth: bool,
}

/// `halt` (OpenType alternate half widths), the feature Blink applies to
/// the punctuation it trims in horizontal text.
const HALT: rustybuzz::ttf_parser::Tag = rustybuzz::ttf_parser::Tag::from_bytes(b"halt");

/// Blink's `HanKerning::ShouldKern`: an opening mark after another
/// opening, middle, or closing mark loses its leading half.
fn should_kern(kind: HanType, last: HanType) -> bool {
    kind == HanType::Open
        && matches!(
            last,
            HanType::Open | HanType::Middle | HanType::Close | HanType::OpenNarrow
        )
}

/// Blink's `HanKerning::ShouldKernLast`: a closing mark before another
/// closing or middle mark loses its trailing half.
fn should_kern_last(kind: HanType, last: HanType) -> bool {
    last == HanType::Close
        && matches!(
            kind,
            HanType::Close | HanType::Middle | HanType::CloseNarrow
        )
}

/// One shaping run: text in one face, with the byte ranges of `text`
/// that get `halt`.
struct Run {
    face: FontId,
    text: String,
    halt: Vec<(usize, usize)>,
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
    halt: Vec<(usize, usize)>,
}

pub struct TextMetrics<'f> {
    fonts: &'f FontSystem,
    runs: RefCell<HashMap<ShapeKey, RunMetrics>>,
    han_fonts: RefCell<HashMap<FontId, HanFontData>>,
}

impl<'f> TextMetrics<'f> {
    pub fn new(fonts: &'f FontSystem) -> Self {
        TextMetrics {
            fonts,
            runs: RefCell::new(HashMap::new()),
            han_fonts: RefCell::new(HashMap::new()),
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

    /// Splits `text` into shaping runs: one face and one script segment
    /// each ([`Self::assign_faces`]), with the byte ranges that get `halt`
    /// ([`Self::han_kerning`]). HarfBuzz in Chrome shapes each segment on
    /// its own, so no kerning pair crosses from Han to kana or from text
    /// to an emoji. Marks, joiners, and variation selectors stay with
    /// their base.
    fn runs(
        &self,
        text: &str,
        chain: &str,
        primary: FontId,
        weight: Weight,
        style: Style,
    ) -> Vec<Run> {
        let assigned = self.assign_faces(text, chain, primary, weight, style);
        let kerned = self.han_kerning(&assigned);
        let mut runs: Vec<(usize, Run)> = Vec::new();
        for ((grapheme, face, segment), kern) in assigned.into_iter().zip(kerned) {
            let run = match runs.last_mut() {
                Some((seg, run)) if run.face == face && *seg == segment => run,
                _ => {
                    runs.push((
                        segment,
                        Run {
                            face,
                            text: String::new(),
                            halt: Vec::new(),
                        },
                    ));
                    &mut runs.last_mut().expect("just pushed").1
                }
            };
            if kern {
                run.halt
                    .push((run.text.len(), run.text.len() + grapheme.len()));
            }
            run.text.push_str(&grapheme);
        }
        runs.into_iter().map(|(_, run)| run).collect()
    }

    /// The graphemes `text-spacing-trim: normal` sets in half width, as
    /// Blink's `HanKerning` picks them: in each adjacent pair, a closing
    /// mark before a closing or middle mark (`」、`), and an opening mark
    /// after an opening, middle, or closing mark (`」「`). The oracle
    /// measures `「テスト」、バグ。` 8 px (half an em at 16 px) narrower than
    /// the sum of its glyphs. The pairs are found across face boundaries,
    /// and each grapheme is classed in its own face. The line-start and
    /// line-end trims of that value apply only to wrapped lines after the
    /// first and to lines that overflow, which the helper leaves out.
    fn han_kerning(&self, assigned: &[(String, FontId, usize)]) -> Vec<bool> {
        let types: Vec<HanType> = assigned
            .iter()
            .map(|(grapheme, face, _)| {
                let ch = grapheme.chars().next().unwrap_or(' ');
                let class = han_class(ch);
                if class == HanClass::Fixed(HanType::Other) {
                    return HanType::Other;
                }
                let data = self.han_font_data(*face);
                match class {
                    HanClass::Fixed(kind) => kind,
                    HanClass::Dot => data.dot,
                    HanClass::Colon => data.colon,
                    HanClass::Semicolon => data.semicolon,
                    HanClass::OpenQuote if data.quotes_fullwidth => HanType::Open,
                    HanClass::OpenQuote => HanType::OpenNarrow,
                    HanClass::CloseQuote if data.quotes_fullwidth => HanType::Close,
                    HanClass::CloseQuote => HanType::CloseNarrow,
                }
            })
            .collect();
        let mut kerned = vec![false; types.len()];
        for i in 1..types.len() {
            if should_kern_last(types[i], types[i - 1]) {
                kerned[i - 1] = true;
            }
            if should_kern(types[i], types[i - 1]) {
                kerned[i] = true;
            }
        }
        kerned
    }

    /// Blink's `HanKerning::FontData`: a dot (`、。，．`) whose ink ends
    /// left of the em's middle is set like a closing mark, one whose ink
    /// starts right of it like an opening mark, and anything else is left
    /// alone; the four dots must share one advance and one class. The
    /// colon and semicolon are classed one by one, and only when the dots
    /// pass. Quotes count as fullwidth when their advance is a full em.
    fn han_font_data(&self, face: FontId) -> HanFontData {
        if let Some(found) = self.han_fonts.borrow().get(&face) {
            return *found;
        }
        let other = HanFontData {
            dot: HanType::Other,
            colon: HanType::Other,
            semicolon: HanType::Other,
            quotes_fullwidth: false,
        };
        let data = self
            .fonts
            .with_face(face, |f| {
                let upem = f.units_per_em() as f32;
                let half = upem / 2.0;
                let tables = f.tables();
                let readable =
                    tables.glyf.is_some() || tables.cff.is_some() || tables.cff2.is_some();
                let glyph = |ch: char| f.glyph_index(ch);
                let boxes = |glyphs: &[rustybuzz::ttf_parser::GlyphId]| -> Vec<Option<[f32; 4]>> {
                    if readable {
                        glyphs.iter().map(|&g| glyph_box(f, g, 0.0)).collect()
                    } else {
                        let ids: Vec<u16> = glyphs.iter().map(|g| g.0).collect();
                        let found = self.fonts.platform_glyph_bounds(face, &ids, 0.0);
                        (0..glyphs.len())
                            .map(|i| found.get(i).copied().flatten().map(|b| b.map(|v| v as f32)))
                            .collect()
                    }
                };
                let classify = |b: Option<[f32; 4]>| match b {
                    Some([_, _, x_max, _]) if x_max <= half => HanType::Close,
                    Some([x_min, _, _, _]) if x_min >= half => HanType::Open,
                    _ => HanType::Other,
                };
                let dots: Option<Vec<_>> =
                    ['、', '。', '，', '．'].into_iter().map(glyph).collect();
                let Some(dots) = dots else {
                    return other;
                };
                let advance0 = f.glyph_hor_advance(dots[0]);
                let dot_boxes = boxes(&dots);
                let dot = classify(dot_boxes[0]);
                let consistent = dots
                    .iter()
                    .zip(&dot_boxes)
                    .all(|(&g, &b)| f.glyph_hor_advance(g) == advance0 && classify(b) == dot);
                let quotes_fullwidth = ['\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}']
                    .into_iter()
                    .all(|ch| {
                        glyph(ch)
                            .and_then(|g| f.glyph_hor_advance(g))
                            .is_some_and(|a| f32::from(a) >= upem)
                    });
                if !consistent || dot == HanType::Other {
                    return HanFontData {
                        quotes_fullwidth,
                        ..other
                    };
                }
                let single =
                    |ch: char| glyph(ch).map_or(HanType::Other, |g| classify(boxes(&[g])[0]));
                HanFontData {
                    dot,
                    colon: single('：'),
                    semicolon: single('；'),
                    quotes_fullwidth,
                }
            })
            .unwrap_or(other);
        self.han_fonts.borrow_mut().insert(face, data);
        data
    }
    /// The face and segment of each grapheme of `text`, as Blink's
    /// `HarfBuzzShaper` assigns them.
    ///
    /// The text is cut into segments (`RunSegmenter`): by script, where a
    /// grapheme of the Common or Inherited script (a space, a digit, `「`)
    /// joins the segment before it, or the one after it at the start; and
    /// around every emoji-presentation grapheme, which `SymbolsIterator`
    /// gives a fallback priority of its own. So in `✂ ✍ ✎` the `✎` is a
    /// segment apart from the `✂`, and the oracle draws it in Lucida
    /// Grande, the face CoreText picks for it, not in the Zapf Dingbats
    /// picked for `✂`. In each segment the primary face takes what it
    /// covers ([`FontSystem::covers`]), then each listed face in turn
    /// ([`FontSystem::listed_face`]). What is left goes to the platform
    /// with one hint character (`HarfBuzzShaper::CollectFallbackHintChars`
    /// stops at the first character with a script of its own, and
    /// `ChooseHintIndex` takes it; with none, the first), and every
    /// leftover grapheme the answer covers is drawn in it; the rest go
    /// round again with a new hint. So under `monospace` the oracle asks
    /// with `Ⅰ` and draws it in Monaco, and `Ⅱ` in Helvetica. So in
    /// `「テスト」` the `「` is drawn in Hiragino Sans, the face Chrome finds
    /// for `テ`, while a lone `「` gets PingFang SC, and the Han segment of
    /// `ログイン画面` asks with `面` and gets PingFang SC though Hiragino
    /// Sans has the glyphs too. When the platform has no face, or answers
    /// with a face the segment already tried, or the hint was asked before,
    /// Blink stops asking: the rest is drawn in Times where Times covers
    /// it, and as `.notdef` in the first face tried
    /// ([`FontSystem::notdef_face`]) otherwise. Emoji-presentation
    /// graphemes take the listed faces and then the color emoji face.
    fn assign_faces(
        &self,
        text: &str,
        chain: &str,
        primary: FontId,
        weight: Weight,
        style: Style,
    ) -> Vec<(String, FontId, usize)> {
        let graphemes = wrap::split_text_to_chars(text);
        let base = |g: &str| g.chars().next().unwrap_or(' ');
        let segments = emoji_segments(&graphemes, &script_segments(&graphemes));
        let mut faces: Vec<Option<FontId>> = graphemes
            .iter()
            .map(|g| {
                if fonts::emoji_presentation(g) {
                    Some(
                        self.fonts
                            .fallback_in_chain(chain, g, weight, style)
                            .unwrap_or(primary),
                    )
                } else if base(g).is_whitespace() || self.fonts.covers(primary, g) {
                    Some(primary)
                } else {
                    self.fonts.listed_face(chain, g, weight, style)
                }
            })
            .collect();
        let segment_count = segments.last().map_or(0, |s| s + 1);
        for segment in 0..segment_count {
            let members: Vec<usize> = (0..graphemes.len())
                .filter(|&i| segments[i] == segment)
                .collect();
            let mut asked: Vec<char> = Vec::new();
            let mut tried = self.fonts.listed_faces(chain, weight, style);
            loop {
                let holes: Vec<usize> = members
                    .iter()
                    .copied()
                    .filter(|&i| faces[i].is_none())
                    .collect();
                let Some(&first) = holes.first() else {
                    break;
                };
                let hint = holes
                    .iter()
                    .find(|&&i| script_of(base(&graphemes[i])).is_some())
                    .copied()
                    .unwrap_or(first);
                let hint_char = base(&graphemes[hint]);
                if asked.contains(&hint_char) {
                    break;
                }
                asked.push(hint_char);
                let Some(face) = self
                    .fonts
                    .platform_face_in_chain(chain, &graphemes[hint], weight, style)
                    .filter(|face| !tried.contains(face))
                else {
                    break;
                };
                tried.push(face);
                for &i in &holes {
                    if self.fonts.covers(face, &graphemes[i]) {
                        faces[i] = Some(face);
                    }
                }
            }
            if members.iter().all(|&i| faces[i].is_some()) {
                continue;
            }
            let segment_text: String = members.iter().map(|&i| graphemes[i].as_str()).collect();
            let notdef = self
                .fonts
                .notdef_face(chain, &segment_text, weight, style)
                .unwrap_or(primary);
            for &i in &members {
                if faces[i].is_none() {
                    faces[i] = Some(
                        self.fonts
                            .last_resort_face(&graphemes[i], weight, style)
                            .unwrap_or(notdef),
                    );
                }
            }
        }
        graphemes
            .into_iter()
            .zip(faces)
            .zip(segments)
            .map(|((g, face), segment)| (g, face.unwrap_or(primary), segment))
            .collect()
    }

    /// splits it. The calibration test compares these with the faces
    /// Chrome reports through `CSS.getPlatformFontsForNode`.
    pub fn grapheme_faces(&self, text: &str, spec: &FontSpec) -> Vec<(String, FontId)> {
        let weight = Weight(spec.weight);
        let style: Style = spec.style.into();
        let Some(primary) = self.primary_face(spec) else {
            return Vec::new();
        };
        self.assign_faces(text, &spec.family, primary, weight, style)
            .into_iter()
            .map(|(grapheme, face, _)| (grapheme, face))
            .collect()
    }

    /// Advance width of `text` shaped in one face at `size`, in pixels.
    pub fn run_advance(&self, text: &str, face: FontId, size: f32) -> f32 {
        self.run_metrics(text, face, size, Synthetic::default(), &[])
            .advance
    }

    /// Shapes `text` in one face: summed advances and the union of the
    /// glyph outlines' bounding boxes, skewed or grown when Chrome would
    /// synthesize an oblique or a bold. Bitmap-only glyphs (color emoji)
    /// have no outline; their boxes are collected apart. `halt` is
    /// applied to the byte ranges given.
    fn run_metrics(
        &self,
        text: &str,
        face: FontId,
        size: f32,
        synthetic: Synthetic,
        halt: &[(usize, usize)],
    ) -> RunMetrics {
        let key = ShapeKey {
            text: text.to_string(),
            face,
            size_bits: size.to_bits(),
            synthetic,
            halt: halt.to_vec(),
        };
        if let Some(found) = self.runs.borrow().get(&key) {
            return *found;
        }
        // Blink shapes with the glyphs CoreText maps the characters to.
        let platform = self.fonts.platform_text(face, text);
        let (text, halt): (&str, Vec<(usize, usize)>) = match &platform {
            Some((mapped, offsets)) => (
                mapped,
                halt.iter()
                    .map(|&(start, end)| (offsets[start], offsets[end]))
                    .collect(),
            ),
            None => (text, halt.to_vec()),
        };
        let metrics = self
            .fonts
            .with_face_at_size(face, size, |f| {
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
                let features: Vec<rustybuzz::Feature> = halt
                    .iter()
                    .map(|&(start, end)| rustybuzz::Feature::new(HALT, 1, start..end))
                    .collect();
                let glyphs = rustybuzz::shape(f, &features, buffer);
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
                let mut bitmap: Option<Bitmap> = None;
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
                        round_half_up(pos.x_advance as f32 * scale / size * sbix_box(size).0)
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
                        let glyph = Bitmap {
                            left: x - BITMAP_INK_PADDING,
                            right: x + advance + BITMAP_INK_PADDING,
                        };
                        bitmap = Some(match bitmap {
                            Some(b) => Bitmap {
                                left: b.left.min(glyph.left),
                                right: b.right.max(glyph.right),
                            },
                            None => glyph,
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
        for Run {
            face,
            text: run,
            halt,
        } in self.runs(text, &spec.family, primary, weight, style)
        {
            let synthetic = Synthetic {
                oblique: spec.style == FontStyle::Italic
                    && self.fonts.face_style(face) == Some(Style::Normal),
                bold: spec.weight >= 600 && self.fonts.face_weight(face).is_some_and(|w| w.0 < 600),
            };
            let oblique = synthetic.oblique;
            let metrics = self.run_metrics(&run, face, spec.size, synthetic, &halt);
            ink = Ink::union(ink, metrics.ink.map(|i| i.shifted(width)));
            if let Some(Bitmap { left, right }) = metrics.bitmap {
                // The bitmap box's height depends on the size alone
                // ([`sbix_box`]), not on the primary face: under Trebuchet
                // MS and Open Sans alike the oracle's 12, 14, and 16 px
                // emoji reach 13, 15, and 17 px above the baseline, and a
                // row of 12 emoji is as tall as one. A synthetic oblique
                // moves the top right corner by the skew, and Skia rounds
                // the glyph box out to whole pixels.
                let (_, top, bottom) = sbix_box(spec.size);
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
                        bottom,
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

    /// Width of a line of styled words laid end to end. Adjacent words of
    /// one style are shaped as one run, as Chrome shapes a text node whole
    /// and measures a line as a range of it: a wrapped CJK line is a word
    /// per character, and shaping them one by one would drop the kerning
    /// between them (Hiragino Sans kerns `グイ` by 0.1 em). An inline style
    /// boundary ends a shaping run in Chrome, so each style is shaped in
    /// its own font.
    pub fn words_width(&self, words: &[Word], spec: &FontSpec) -> f32 {
        let mut runs: Vec<(WordType, String)> = Vec::new();
        for word in words {
            match runs.last_mut() {
                Some((kind, text)) if *kind == word.kind => text.push_str(&word.content),
                _ => runs.push((word.kind, word.content.clone())),
            }
        }
        runs.iter()
            .map(|(kind, text)| self.shape(text, &spec.for_word(*kind)).width)
            .sum()
    }

    /// `drawSimpleText` followed by `getBBox()`: `<br>` tags become spaces,
    /// the `<text>` collapses its spaces, tabs, and line breaks
    /// ([`wrap::collapse_text`]), and an empty string measures as a
    /// zero-width space.
    pub fn svg_bbox(&self, text: &str, spec: &FontSpec) -> Size {
        let joined = wrap::collapse_text(&markdown::split_breaks(text).join(" "));
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

/// Fallback runs as Chrome on macOS shapes them (R-214, R-302).
#[cfg(test)]
mod cjk_tests {
    use super::*;
    #[cfg(target_os = "macos")]
    use crate::text::font_system;

    #[cfg(target_os = "macos")]
    const MERMAID_CHAIN: &str = "\"trebuchet ms\", verdana, arial, sans-serif";

    #[cfg(target_os = "macos")]
    fn postscript(fonts: &FontSystem, face: FontId) -> String {
        fonts
            .db()
            .face(face)
            .map(|f| f.post_script_name.clone())
            .unwrap_or_default()
    }

    /// Chrome 156 on macOS, `getBoundingClientRect` of each string at
    /// 16 px under mermaid's chain: Hiragino Sans W3 kerns `グイ` by
    /// 1.594 px and `バグ` by 0.641 px; W4 (the OS/2 400 face) kerns less.
    #[cfg(target_os = "macos")]
    #[test]
    fn kana_pairs_kern_like_chrome() {
        let fonts = font_system();
        let metrics = TextMetrics::new(fonts);
        let spec = FontSpec::new(MERMAID_CHAIN, 16.0);
        for (text, chrome) in [
            ("グイ", 30.406),
            ("バグ", 31.359),
            ("ドキュメント", 96.0),
            ("ログイン画面のバグを直す", 190.734),
            ("「テスト」、バグ。", 132.0),
        ] {
            let width = metrics.shape(text, &spec).width;
            assert!((width - chrome).abs() < 0.05, "{text}: {width} vs {chrome}");
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn fallback_faces_follow_the_script_segment_hint() {
        let fonts = font_system();
        let metrics = TextMetrics::new(fonts);
        let faces = |text: &str, weight: u16| -> Vec<String> {
            let spec = FontSpec::new(MERMAID_CHAIN, 16.0).weight(weight);
            metrics
                .grapheme_faces(text, &spec)
                .into_iter()
                .map(|(_, face)| postscript(fonts, face))
                .collect()
        };
        // `「` takes the face the kana after it get; alone it gets PingFang.
        assert_eq!(faces("「テ", 400), ["HiraginoSans-W3", "HiraginoSans-W3"]);
        assert_eq!(faces("「", 400), ["PingFangSC-Regular"]);
        // Han between kana is a segment of its own and asks for itself.
        assert_eq!(
            faces("ン画の", 400),
            ["HiraginoSans-W3", "PingFangSC-Regular", "HiraginoSans-W3"]
        );
        assert_eq!(faces("カ", 500), ["HiraginoSans-W4"]);
        assert_eq!(faces("カ", 700), ["HiraginoSans-W6"]);
        assert_eq!(faces("한", 700), ["AppleSDGothicNeo-Bold"]);
    }

    /// A wrapped line is measured from the words of the line; shaped
    /// together they keep the kerning a per-word sum would drop, in CJK
    /// lines (a word per character) and across Latin spaces alike.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_line_of_words_is_shaped_as_one_run() {
        let fonts = font_system();
        let metrics = TextMetrics::new(fonts);
        let spec = FontSpec::new(MERMAID_CHAIN, 16.0);
        for text in ["ログイン画面のバグを直", "A database with ", "AVAWAY Type"] {
            let words: Vec<Word> = wrap::html_segments(&[Word::normal(text)])
                .into_iter()
                .map(|segment| segment.word)
                .collect();
            assert!(words.len() > 1, "{text}");
            let whole = metrics.shape(text, &spec).width;
            assert!(
                (metrics.words_width(&words, &spec) - whole).abs() < 1e-3,
                "{text}"
            );
        }
        // A style change still ends the run.
        let styled = [
            Word::normal("AV"),
            Word::new("A".to_string(), WordType::Strong),
        ];
        let expected = metrics.shape("AV", &spec).width
            + metrics.shape("A", &spec.for_word(WordType::Strong)).width;
        assert!((metrics.words_width(&styled, &spec) - expected).abs() < 1e-3);
    }

    #[test]
    fn script_segments_absorb_common_characters() {
        let graphemes = |s: &str| wrap::split_text_to_chars(s);
        assert_eq!(script_segments(&graphemes("「テ」")), [0, 0, 0]);
        assert_eq!(
            script_segments(&graphemes("ID番号42の")),
            [0, 0, 1, 1, 1, 1, 2]
        );
        assert_eq!(script_segments(&graphemes("a b")), [0, 0, 0]);
        // Katakana and hiragana are one script; private use is its own.
        assert_eq!(script_segments(&graphemes("アあ漢")), [0, 0, 1]);
        assert_eq!(script_segments(&graphemes("a\u{E000}b")), [0, 1, 2]);
    }
}
