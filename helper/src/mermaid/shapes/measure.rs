//! Label measurement seam between the shapes and the text package.
//!
//! mermaid measures a label by inserting it into the DOM and reading
//! `getBoundingClientRect()` (HTML labels) or `getBBox()` (SVG text) in
//! `rendering-elements/shapes/util.ts` `labelHelper`. The shapes only need
//! the resulting box and the lines it holds, so that is the whole contract.
//! [`TextMetrics`] implements [`LabelMeasurer`] with rustybuzz (see
//! [`measure_with_metrics`]); the tests use [`FixedMeasurer`], which
//! charges a fixed advance per character.

use crate::mermaid::scene::{
    Color, DEFAULT_FONT_FAMILY, FontSpec, FontStyle as SceneFontStyle, FontWeight,
};
use crate::text::fonts::Weight;
use crate::text::markdown::{self, Line, WordType};
use crate::text::measure::{
    FontSpec as TextFont, FontStyle as TextStyle, HTML_LINE_HEIGHT, SVG_LINE_HEIGHT_EM, TextMetrics,
};

/// A width and a height in CSS pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Dims {
    pub width: f64,
    pub height: f64,
}

impl Dims {
    pub const fn new(width: f64, height: f64) -> Dims {
        Dims { width, height }
    }
}

/// One styled run inside a label line: mermaid's `MarkdownWord`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LabelRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
}

impl LabelRun {
    pub fn plain(text: impl Into<String>) -> LabelRun {
        LabelRun {
            text: text.into(),
            ..LabelRun::default()
        }
    }
}

/// Where an HTML label's lines sit in its box: the `text-align` of the
/// label span, which the `<p>` that `createText` wraps every HTML label in
/// inherits. `addHtmlSpan` centres the div, so `Center` is the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabelAlign {
    #[default]
    Center,
    Left,
    Right,
}

/// One line of a label after wrapping.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LabelLine {
    pub runs: Vec<LabelRun>,
    /// Advance width of the whole line.
    pub width: f64,
}

/// The measured label: the `DOMRect` mermaid reads plus the wrapped lines
/// the renderer has to draw.
///
/// `x` and `y` reproduce the difference between the two measuring modes
/// that `labelHelper` relies on. An HTML label's `getBoundingClientRect()`
/// has `left`, so `bbox.x - bbox.left` is `0`; SVG text's `getBBox()` is
/// centred on `x = 0`, so `x` is `-width / 2`. Shapes place labels with
/// `pos - (bbox.x - (bbox.left ?? 0))`, which is `pos - x` here.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LabelBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Line height in pixels, so the renderer can place each line.
    pub line_height: f64,
    /// HTML labels: the first line's baseline below the box top, where
    /// Chrome puts it (see [`html_first_baseline`]). Zero for SVG labels.
    pub baseline: f64,
    /// The display text: HTML labels decode entities through `innerHTML`
    /// and SVG labels through `decodeHTMLEntities`.
    pub lines: Vec<LabelLine>,
    /// The font the label was measured in, so it is drawn in the same one.
    pub font: FontSpec,
    /// The color of a `text-decoration: underline` that an ancestor of an
    /// HTML label propagates into it. The measurer never sets it; the shape
    /// that wraps the label in the decorating element does.
    pub underline: Option<Color>,
    /// The `text-align` of the label styles. The measurer leaves it
    /// centred; the label helpers read it off the node.
    pub align: LabelAlign,
}

impl LabelBox {
    pub fn dims(&self) -> Dims {
        Dims::new(self.width, self.height)
    }

    pub fn is_empty(&self) -> bool {
        self.lines
            .iter()
            .all(|l| l.runs.iter().all(|r| r.text.is_empty()))
    }
}

/// What `createText` was called with.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelOpts {
    pub font: FontSpec,
    /// `useHtmlLabels`: HTML measuring (`line-height: 1.5`) or SVG text
    /// (`getBBox`, `1.1em` tspans).
    pub html: bool,
    /// The wrapping width; `None` is `Number.POSITIVE_INFINITY`.
    pub width: Option<f64>,
    /// `minWidth` of `createText`; `0` disables it.
    pub min_width: f64,
    /// `labelType === 'markdown'`: `markdownToLines`, else
    /// `nonMarkdownToLines`.
    pub markdown: bool,
}

impl Default for LabelOpts {
    fn default() -> LabelOpts {
        LabelOpts {
            font: FontSpec::default(),
            html: true,
            width: None,
            min_width: 0.0,
            markdown: true,
        }
    }
}

/// The measurer the shapes draw with: [`TextMetrics`] in the helper,
/// [`FixedMeasurer`] in the snapshot tests.
pub trait LabelMeasurer {
    /// Measures `text` as `createText` would and returns the box with its
    /// wrapped lines. An empty `text` returns a zero box with no lines.
    fn measure(&self, text: &str, opts: &LabelOpts) -> LabelBox;

    /// `calculateTextWidth(text, { fontFamily, fontSize })` of `utils.ts`:
    /// the widest `<br>` line as SVG text in that font, rounded to whole
    /// pixels (`calculateTextDimensions`).
    fn text_width(&self, text: &str, family: &str, size: f64) -> f64;

    /// The thickness in pixels Blink gives `text-decoration: underline` in
    /// `font` (`text-decoration-thickness: auto`): the face's `post`
    /// underline thickness, rounded to whole pixels and at least 1. The
    /// default charges 1/18 em, Blink's value for a face without one.
    fn underline_thickness(&self, font: &FontSpec) -> f64 {
        blink_underline_thickness(font.size.unwrap_or(DEFAULT_FONT_SIZE) / 18.0)
    }
}

/// Blink snaps a decoration's thickness to whole device pixels, never
/// below one.
fn blink_underline_thickness(px: f64) -> f64 {
    px.round().max(1.0)
}

/// The `post` table's underline thickness of the face `family` resolves
/// to, in pixels.
fn post_underline_thickness(metrics: &TextMetrics<'_>, font: &FontSpec, family: &str) -> f64 {
    let spec = text_font(font, family);
    let fonts = metrics.fonts();
    let em = fonts
        .resolve(&spec.family, Weight(spec.weight), spec.style.into())
        .and_then(|id| {
            fonts.with_face(id, |face| {
                let upem = f64::from(face.units_per_em());
                face.underline_metrics()
                    .map(|m| f64::from(m.thickness) / upem)
            })
        })
        .flatten()
        .unwrap_or(1.0 / 18.0);
    blink_underline_thickness(em * f64::from(spec.size))
}

/// Test fake: every character advances `char_width`, every line is
/// `line_height` tall, `<br>` and newlines break lines, `**x**` and `*x*`
/// mark bold and italic runs, and no wrapping happens. Deterministic and
/// font-free, so snapshots do not depend on the installed fonts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FixedMeasurer {
    pub char_width: f64,
    pub line_height: f64,
}

impl FixedMeasurer {
    /// The fake the shape snapshots use: 8 px per character, 16 px lines.
    pub const SNAPSHOT: FixedMeasurer = FixedMeasurer {
        char_width: 8.0,
        line_height: 16.0,
    };
}

impl Default for FixedMeasurer {
    fn default() -> FixedMeasurer {
        FixedMeasurer::SNAPSHOT
    }
}

impl LabelMeasurer for FixedMeasurer {
    fn measure(&self, text: &str, opts: &LabelOpts) -> LabelBox {
        if text.is_empty() {
            return LabelBox {
                line_height: self.line_height,
                font: opts.font.clone(),
                ..LabelBox::default()
            };
        }
        let normalized = text
            .replace("<br/>", "\n")
            .replace("<br />", "\n")
            .replace("<br>", "\n")
            .replace("\\n", "\n");
        let lines: Vec<LabelLine> = normalized
            .split('\n')
            .map(|line| {
                let runs = if opts.markdown {
                    split_runs(line)
                } else {
                    vec![LabelRun::plain(line)]
                };
                let width = runs
                    .iter()
                    .map(|r| r.text.chars().count() as f64 * self.char_width)
                    .sum();
                LabelLine { runs, width }
            })
            .collect();
        let mut width = lines.iter().map(|l| l.width).fold(0.0, f64::max);
        if width < opts.min_width {
            width = opts.min_width;
        }
        let height = lines.len() as f64 * self.line_height;
        let x = if opts.html { 0.0 } else { -width / 2.0 };
        LabelBox {
            x,
            y: 0.0,
            width,
            height,
            line_height: self.line_height,
            // A face with ascent 0.7 and descent 0.3 of the line, centred.
            baseline: if opts.html {
                0.7 * self.line_height
            } else {
                0.0
            },
            lines,
            font: opts.font.clone(),
            underline: None,
            align: LabelAlign::Center,
        }
    }

    fn text_width(&self, text: &str, _family: &str, _size: f64) -> f64 {
        let widest = markdown::split_breaks(text)
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0);
        (widest as f64 * self.char_width + 0.5).floor()
    }
}

/// Splits `**bold**` and `*italic*` spans into runs. Enough markdown for
/// the fake; the text package has the real `markdownToLines` port.
fn split_runs(line: &str) -> Vec<LabelRun> {
    let mut runs = Vec::new();
    let mut rest = line;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("**")
            && let Some(end) = after.find("**")
        {
            runs.push(LabelRun {
                text: after[..end].to_string(),
                bold: true,
                italic: false,
            });
            rest = &after[end + 2..];
            continue;
        }
        if let Some(after) = rest.strip_prefix('*')
            && let Some(end) = after.find('*')
        {
            runs.push(LabelRun {
                text: after[..end].to_string(),
                bold: false,
                italic: true,
            });
            rest = &after[end + 1..];
            continue;
        }
        let first_len = rest.chars().next().map_or(1, char::len_utf8);
        let next = rest[first_len..]
            .find('*')
            .map(|i| i + first_len)
            .unwrap_or(rest.len());
        runs.push(LabelRun::plain(&rest[..next]));
        rest = &rest[next..];
    }
    if runs.is_empty() {
        runs.push(LabelRun::plain(""));
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_measurer_counts_characters_and_lines() {
        let m = FixedMeasurer::SNAPSHOT;
        let b = m.measure("Hello<br/>World!", &LabelOpts::default());
        assert_eq!(b.width, 48.0);
        assert_eq!(b.height, 32.0);
        assert_eq!(b.lines.len(), 2);
        assert_eq!(b.x, 0.0);
    }

    #[test]
    fn fixed_measurer_splits_emphasis() {
        let m = FixedMeasurer::SNAPSHOT;
        let b = m.measure("a **b** *c*", &LabelOpts::default());
        let runs = &b.lines[0].runs;
        assert_eq!(runs.len(), 4);
        assert!(runs[1].bold);
        assert!(runs[3].italic);
        assert_eq!(b.width, 5.0 * 8.0);
    }

    #[test]
    fn svg_mode_centres_the_box() {
        let m = FixedMeasurer::SNAPSHOT;
        let opts = LabelOpts {
            html: false,
            ..LabelOpts::default()
        };
        let b = m.measure("abcd", &opts);
        assert_eq!(b.x, -16.0);
    }
}

// ---------------------------------------------------------------------------
// The real measurer

/// The root `font-size` of mermaid's default theme, for a font with none.
const DEFAULT_FONT_SIZE: f64 = 16.0;

/// The browser font a label is measured with. `ShapeCtx::font()` carries
/// the theme's size and leaves the family unset so the drawn text inherits
/// it from the root; the measurer needs it, so `family` fills that gap.
fn text_font(font: &FontSpec, family: &str) -> TextFont {
    let weight = match font.weight {
        Some(FontWeight::Bold) => 700,
        Some(FontWeight::Number(n)) => n,
        Some(FontWeight::Normal) | None => 400,
    };
    let style = match font.style {
        Some(SceneFontStyle::Italic | SceneFontStyle::Oblique) => TextStyle::Italic,
        Some(SceneFontStyle::Normal) | None => TextStyle::Normal,
    };
    TextFont::new(
        font.family.as_deref().unwrap_or(family),
        font.size.unwrap_or(DEFAULT_FONT_SIZE) as f32,
    )
    .weight(weight)
    .style(style)
}

/// The entities `decodeHTMLEntities` of `createText.ts` decodes.
const ENTITY: [(&str, &str); 3] = [("&lt;", "<"), ("&gt;", ">"), ("&amp;", "&")];

/// `decodeHTMLEntities(text)` of `createText.ts`: SVG labels write each
/// word with `textContent`, so the entities `sanitizeText` produced would
/// show as typed; mermaid decodes these three in one pass, so `&amp;lt;`
/// becomes `&lt;`.
pub fn decode_html_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        match ENTITY.iter().find(|(e, _)| rest.starts_with(e)) {
            Some((e, c)) => {
                out.push_str(c);
                rest = &rest[e.len()..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn label_lines(lines: &[Line], width: impl Fn(&Line) -> f32) -> Vec<LabelLine> {
    lines
        .iter()
        .map(|line| LabelLine {
            runs: line
                .iter()
                .map(|word| LabelRun {
                    text: word.content.clone(),
                    bold: word.kind == WordType::Strong,
                    italic: word.kind == WordType::Em,
                })
                .collect(),
            width: f64::from(width(line)),
        })
        .collect()
}

/// `createText` followed by the read `labelHelper` makes, through the
/// browser-equivalent [`TextMetrics`].
///
/// HTML labels (`useHtmlLabels`): `markdownToHTML` or `nonMarkdownToHTML`,
/// then `addHtmlSpan` lays the HTML out as a `table-cell` with `nowrap`
/// and `line-height: 1.5`, switching to a wrapping `table` of `width` when
/// the text reaches it, and `getBoundingClientRect()` is read. SVG labels:
/// `<br>` tags are normalised, `markdownToLines` or `nonMarkdownToLines`
/// split the text, `createFormattedText` puts every line in a tspan 1.1 em
/// below the previous one, and `getBBox()` of the `<text>` is read; its
/// `x` is `-width / 2` because `.node .label text { text-anchor: middle }`
/// centres each tspan on `x = 0`, and its `y` is the first baseline minus
/// the first line's ascent (the tspan's `y="-0.1em"` and `dy="1.1em"`
/// put that baseline at 1 em). `minWidth` widens the box as `withMinWidth`
/// does: an HTML rect keeps its `left`, an SVG rect stays centred.
pub fn measure_with_metrics(
    metrics: &TextMetrics<'_>,
    text: &str,
    opts: &LabelOpts,
    family: &str,
) -> LabelBox {
    let spec = text_font(&opts.font, family);
    let size = f64::from(spec.size);
    let line_height = if opts.html {
        f64::from(HTML_LINE_HEIGHT) * size
    } else {
        f64::from(SVG_LINE_HEIGHT_EM) * size
    };
    if text.is_empty() {
        return LabelBox {
            line_height,
            font: opts.font.clone(),
            ..LabelBox::default()
        };
    }
    let mut baseline = 0.0;
    let (y, mut width, height, lines) = if opts.html {
        let html = if opts.markdown {
            markdown::markdown_to_html(text)
        } else {
            markdown::non_markdown_to_html(text)
        };
        let structured = markdown::html_to_lines(&html);
        let label = metrics.html_label(&structured, &spec, opts.width.map(|w| w as f32));
        let lines = label_lines(&label.lines, |line| metrics.words_width(line, &spec))
            .into_iter()
            .map(|line| LabelLine {
                width: layout_unit_ceil(line.width),
                ..line
            })
            .collect();
        baseline = html_first_baseline(metrics, &spec, line_height);
        (
            0.0,
            layout_unit_ceil(f64::from(label.width)),
            f64::from(label.height),
            lines,
        )
    } else {
        let normalized = markdown::normalize_breaks(text);
        let mut structured = if opts.markdown {
            markdown::markdown_to_lines(&normalized)
        } else {
            markdown::non_markdown_to_lines(&normalized)
        };
        // `updateTextContentAndStyles` decodes each word before it writes
        // it, and the browser measures the decoded text.
        for word in structured.iter_mut().flatten() {
            word.content = decode_html_entities(&word.content);
        }
        let max_width = opts.width.map_or(f32::INFINITY, |w| w as f32);
        let label = metrics.svg_label(&structured, &spec, max_width);
        let ascent = label
            .lines
            .iter()
            .find(|line| !line.is_empty())
            .map_or(0.0, |line| {
                line.iter()
                    .map(|word| {
                        metrics
                            .shape(&word.content, &spec.for_word(word.kind))
                            .ascent
                    })
                    .fold(0.0, f32::max)
            });
        let y = if label.height > 0.0 {
            size - f64::from(ascent)
        } else {
            0.0
        };
        let lines = label_lines(&label.lines, |line| {
            metrics.compute_width_of_text(line, &spec)
        });
        (y, f64::from(label.width), f64::from(label.height), lines)
    };
    if width < opts.min_width {
        width = opts.min_width;
    }
    LabelBox {
        x: if opts.html { 0.0 } else { -width / 2.0 },
        y,
        width,
        height,
        line_height,
        baseline,
        lines,
        font: opts.font.clone(),
        underline: None,
        align: LabelAlign::Center,
    }
}

/// Rounds a width up to Blink's `LayoutUnit` of 1/64 px. Chrome lays the
/// shrink-to-fit label `div` out in LayoutUnits and rounds the text width
/// up, so `getBoundingClientRect()` reports a multiple of 1/64; dagre's
/// alignment choice can turn on that last fraction. The epsilon keeps an
/// exact multiple from rounding up once more.
pub fn layout_unit_ceil(v: f64) -> f64 {
    (v * 64.0 - 1e-6).ceil() / 64.0
}

/// Where Chrome puts the first baseline of an HTML label line: the line box
/// is `line_height` tall, the primary face's rounded ascent `A` and
/// descent `D` sit in it with the half-leading floored, so the baseline is
/// `floor((line_height - (A + D)) / 2) + A` below the line top. Trebuchet
/// MS at 16 px gives 17, not the 17.5 an unrounded centre would.
pub fn html_first_baseline(metrics: &TextMetrics<'_>, spec: &TextFont, line_height: f64) -> f64 {
    let shaped = metrics.shape(" ", spec);
    let (ascent, descent) = (f64::from(shaped.ascent), f64::from(shaped.descent));
    ((line_height - (ascent + descent)) / 2.0).floor() + ascent
}

/// Labels measured in the default theme's family (`"trebuchet ms",
/// verdana, arial, sans-serif`) when the font leaves it unset. A diagram
/// with another `fontFamily` theme variable measures through
/// [`ThemedMetrics`].
impl LabelMeasurer for TextMetrics<'_> {
    fn measure(&self, text: &str, opts: &LabelOpts) -> LabelBox {
        measure_with_metrics(self, text, opts, DEFAULT_FONT_FAMILY)
    }

    /// The rounding (`(w + 0.5).floor()`) and the `sans-serif` comparison
    /// happen in [`TextMetrics::calculate_text_dimensions`], which also
    /// swaps an invalid family for the page default as Chrome does.
    fn text_width(&self, text: &str, family: &str, size: f64) -> f64 {
        f64::from(self.calculate_text_width(text, &TextFont::new(family, size as f32)))
    }

    fn underline_thickness(&self, font: &FontSpec) -> f64 {
        post_underline_thickness(self, font, DEFAULT_FONT_FAMILY)
    }
}

/// [`TextMetrics`] bound to the theme's `fontFamily`, the family the
/// `.label { font-family }` rule gives every label in the browser.
pub struct ThemedMetrics<'m, 'f> {
    pub metrics: &'m TextMetrics<'f>,
    pub family: String,
}

impl<'m, 'f> ThemedMetrics<'m, 'f> {
    pub fn new(metrics: &'m TextMetrics<'f>, family: impl Into<String>) -> ThemedMetrics<'m, 'f> {
        ThemedMetrics {
            metrics,
            family: family.into(),
        }
    }
}

impl LabelMeasurer for ThemedMetrics<'_, '_> {
    fn measure(&self, text: &str, opts: &LabelOpts) -> LabelBox {
        measure_with_metrics(self.metrics, text, opts, &self.family)
    }

    fn text_width(&self, text: &str, family: &str, size: f64) -> f64 {
        self.metrics.text_width(text, family, size)
    }

    fn underline_thickness(&self, font: &FontSpec) -> f64 {
        post_underline_thickness(self.metrics, font, &self.family)
    }
}

#[cfg(test)]
mod metrics_tests {
    use super::*;
    use crate::text::font_system;

    fn open_sans_opts(html: bool) -> LabelOpts {
        LabelOpts {
            font: FontSpec {
                family: Some("\"Open Sans\", sans-serif".to_string()),
                size: Some(16.0),
                ..FontSpec::default()
            },
            html,
            width: Some(120.0),
            min_width: 0.0,
            markdown: true,
        }
    }

    #[test]
    fn html_label_is_one_and_a_half_em_per_line() {
        let metrics = TextMetrics::new(font_system());
        let b = metrics.measure("Hello", &open_sans_opts(true));
        assert_eq!(b.height, 24.0);
        assert_eq!(b.line_height, 24.0);
        assert_eq!(b.x, 0.0);
        assert_eq!(b.lines.len(), 1);
        assert_eq!(b.lines[0].runs[0].text, "Hello");
        assert_eq!(b.lines[0].width, b.width);
        let two = metrics.measure("Hello<br>World", &open_sans_opts(true));
        assert_eq!(two.height, 48.0);
        assert_eq!(two.lines.len(), 2);
    }

    #[test]
    fn html_widths_are_layout_units() {
        let metrics = TextMetrics::new(font_system());
        let b = metrics.measure("Hello world", &open_sans_opts(true));
        assert_eq!(b.width * 64.0, (b.width * 64.0).round());
        assert_eq!(b.lines[0].width, b.width);
        assert_eq!(layout_unit_ceil(2.0), 2.0);
        assert_eq!(layout_unit_ceil(2.001), 2.015625);
    }

    #[test]
    fn svg_label_is_centred_with_the_bbox_y() {
        let metrics = TextMetrics::new(font_system());
        let b = metrics.measure("Hello", &open_sans_opts(false));
        // Open Sans at 16 px: rounded hhea ascent 17, descent 5.
        assert_eq!(b.height, 22.0);
        assert_eq!(b.x, -b.width / 2.0);
        assert_eq!(b.y, -1.0);
        assert!((b.line_height - 17.6).abs() < 1e-6);
    }

    #[test]
    fn min_width_widens_both_modes() {
        let metrics = TextMetrics::new(font_system());
        let mut opts = open_sans_opts(true);
        opts.min_width = 120.0;
        let html = metrics.measure("Hi", &opts);
        assert_eq!((html.x, html.width), (0.0, 120.0));
        opts.html = false;
        let svg = metrics.measure("Hi", &opts);
        assert_eq!((svg.x, svg.width), (-60.0, 120.0));
    }

    #[test]
    fn markdown_runs_carry_their_style() {
        let metrics = TextMetrics::new(font_system());
        let b = metrics.measure("a **b** *c*", &open_sans_opts(true));
        let runs = &b.lines[0].runs;
        assert!(runs.iter().any(|r| r.bold && r.text.trim() == "b"));
        assert!(runs.iter().any(|r| r.italic && r.text.trim() == "c"));
        let plain = metrics.measure(
            "a **b**",
            &LabelOpts {
                markdown: false,
                ..open_sans_opts(true)
            },
        );
        assert!(plain.lines[0].runs.iter().all(|r| !r.bold));
    }

    #[test]
    fn svg_labels_decode_entities_per_word() {
        let metrics = TextMetrics::new(font_system());
        let svg = metrics.measure("&lt;&lt;Element&gt;&gt; a&amp;lt;b", &open_sans_opts(false));
        let words: Vec<&str> = svg
            .lines
            .iter()
            .flat_map(|l| l.runs.iter().map(|r| r.text.as_str()))
            .collect();
        assert_eq!(words, ["<<Element>>", "a&lt;b"]);
        // The decoded SVG words advance as far as the HTML label, which the
        // browser decodes through `innerHTML` (up to its 1/64 px rounding).
        let html = metrics.measure("&lt;&lt;Element&gt;&gt;", &open_sans_opts(true));
        let decoded = metrics.measure("&lt;&lt;Element&gt;&gt;", &open_sans_opts(false));
        assert_eq!(html.lines[0].runs[0].text.trim(), "<<Element>>");
        assert!((decoded.lines[0].width - html.lines[0].width).abs() < 1.0 / 64.0);
    }

    #[test]
    fn empty_text_is_a_zero_box() {
        let metrics = TextMetrics::new(font_system());
        let b = metrics.measure("", &open_sans_opts(true));
        assert_eq!((b.width, b.height), (0.0, 0.0));
        assert!(b.lines.is_empty());
        assert_eq!(b.line_height, 24.0);
    }

    #[test]
    fn text_width_falls_back_to_the_page_default_family() {
        let metrics = TextMetrics::new(font_system());
        let text = "A fairly long label";
        let invalid = format!("{DEFAULT_FONT_FAMILY};");
        let page_default = crate::text::measure::css_family_or_page_default(&invalid);
        let via_config = metrics.text_width(text, &invalid, 16.0);
        assert_eq!(via_config, metrics.text_width(text, page_default, 16.0));
        assert_eq!(via_config, via_config.round());
    }

    #[test]
    fn fixed_text_width_takes_the_widest_line() {
        let m = FixedMeasurer::SNAPSHOT;
        assert_eq!(m.text_width("ab<br>abcd", "x", 16.0), 32.0);
        assert_eq!(m.text_width("", "x", 16.0), 0.0);
    }

    #[test]
    fn themed_metrics_fill_the_family() {
        let metrics = TextMetrics::new(font_system());
        let themed = ThemedMetrics::new(&metrics, "\"Open Sans\", sans-serif");
        let mut opts = open_sans_opts(true);
        opts.font.family = None;
        let via_theme = themed.measure("Hello", &opts);
        let direct = metrics.measure("Hello", &open_sans_opts(true));
        assert_eq!(via_theme.width, direct.width);
    }
}
