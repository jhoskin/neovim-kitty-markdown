//! Deterministic SVG serializer for the scene model.
//!
//! The same scene always produces the same bytes: attributes come out in a
//! fixed order, every number goes through [`fmt_num`], and child elements
//! keep their scene order. That is what makes insta snapshots of a scene
//! meaningful, and it also keeps resvg's input stable between runs.
//!
//! [`to_svg_with`] can also answer font questions the scene leaves open.
//! The raster step uses it for synthetic bold (see [`SvgOptions`]); the
//! snapshots use [`to_svg`], which asks none, so they do not depend on the
//! fonts a machine has.

use std::collections::BTreeSet;
use std::fmt::Write;

use super::path::fmt_num;
use super::{
    Baseline, BlendMode, ClipPath, Color, Common, Element, FontSpec, FontStyle, FontWeight,
    GradientUnits, Length, LineCap, LineJoin, LinearGradient, Marker, MarkerUnits, Orient, Paint,
    Point, Scene, Style, Text, TextAnchor, TransformOp, ViewBox, XmlSpace,
};
use crate::text::fonts::SYNTHETIC_BOLD_GROWTH;

const XMLNS: &str = "http://www.w3.org/2000/svg";
const XLINK: &str = "http://www.w3.org/1999/xlink";

/// Whether Chrome draws a `font-family` chain at a numeric weight and a
/// style with a synthetic bold.
pub type SyntheticBold<'a> = &'a dyn Fn(&str, u16, FontStyle) -> bool;

/// The font a text run is drawn in after the cascade: the `font-family`
/// chain, the size in pixels, the numeric weight, and the style.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunFont<'f> {
    pub family: &'f str,
    pub size: f64,
    pub weight: u16,
    pub style: FontStyle,
}

/// A stretch of a text run that Chrome draws in a face other than the
/// run's primary face (a fallback face for CJK, Hangul, or emoji).
#[derive(Debug, Clone, PartialEq)]
pub struct FacePiece {
    /// Byte range in the run's text, on grapheme boundaries.
    pub range: std::ops::Range<usize>,
    /// The `font-family` that names exactly that face for the raster
    /// step's font resolver. An own piece keeps the run's family.
    pub family: Option<String>,
    /// True when Chrome emboldens this face (a weight of 600 or more on a
    /// face lighter than 600).
    pub synthetic_bold: bool,
    /// Added after each cluster, so resvg advances the piece by the width
    /// the measurer gave it.
    pub letter_spacing: Option<f64>,
    /// Set when the raster step draws this grapheme itself, because resvg
    /// cannot draw its face as Chrome does: the piece is written as
    /// [`OWN_PLACEHOLDER`], tagged with this index on the [`OWN_AXIS`]
    /// variation axis and underlined until the glyphs exist, and hidden
    /// after.
    pub own: Option<usize>,
}

/// The text an own piece is written as: one character the primary face
/// has and that no whitespace rule removes.
pub const OWN_PLACEHOLDER: &str = "\u{A0}";

/// The `font-variation-settings` axis that carries an own piece's index
/// through usvg's layout. No font has it, so shaping ignores it.
pub const OWN_AXIS: &str = "kmpi";

/// The face pieces of one run of text in one font.
pub type FacePieces<'a> = &'a dyn Fn(&str, &RunFont<'_>) -> Vec<FacePiece>;

/// The glyphs the raster step draws for one text, as SVG elements in the
/// text's own user space, looked up by the text's id.
pub type OwnGlyphs<'a> = &'a dyn Fn(&str) -> Option<String>;

/// What [`to_svg_with`] may ask about fonts.
#[derive(Clone, Copy, Default)]
pub struct SvgOptions<'a> {
    /// When set, text that Chrome would embolden gets a stroke of its own
    /// fill, `2 * SYNTHETIC_BOLD_GROWTH * font-size` wide. Skia's fake bold
    /// strokes and fills the glyph outline in one pass; resvg draws the
    /// regular face as it is, so the stroke stands in for the emboldening.
    /// Text with a visible stroke of its own keeps it instead.
    pub synthetic_bold: Option<SyntheticBold<'a>>,
    /// When set, each stretch of text that Chrome draws in a fallback face
    /// becomes a `tspan` naming that face, so resvg draws the glyphs the
    /// measurer measured. A text with an own-drawing piece gets the id
    /// `kmt-<n>`, numbered in document order.
    pub face_pieces: Option<FacePieces<'a>>,
    /// When set, own-drawing pieces are hidden, and the elements this
    /// returns for a text's id follow the text in a group with the text's
    /// transform, opacity, and clip.
    pub own_glyphs: Option<OwnGlyphs<'a>>,
}

/// Serializes the scene. The root carries the natural size, the viewBox, and
/// the default font family; a background rect comes first when the scene
/// has a background color.
pub fn to_svg(scene: &Scene) -> String {
    to_svg_with(scene, &SvgOptions::default())
}

/// [`to_svg`] with the answers in `opts`.
pub fn to_svg_with(scene: &Scene, opts: &SvgOptions<'_>) -> String {
    let mut w = Writer::new(opts, Some(&scene.font_family));
    w.open("svg");
    w.attr("xmlns", XMLNS);
    w.attr("xmlns:xlink", XLINK);
    w.attr("width", &fmt_num(scene.width));
    w.attr("height", &fmt_num(scene.height));
    w.attr("viewBox", &view_box(&scene.view_box));
    w.attr("font-family", &scene.font_family);
    w.children_follow();

    if !scene.defs.is_empty() || !scene.linear_gradients.is_empty() {
        w.open("defs");
        w.children_follow();
        for element in &scene.defs {
            write_element(&mut w, element);
        }
        for gradient in &scene.linear_gradients {
            write_linear_gradient(&mut w, gradient);
        }
        w.close("defs");
    }

    if let Some(bg) = scene.background {
        w.open("rect");
        w.attr("x", &fmt_num(scene.view_box.x));
        w.attr("y", &fmt_num(scene.view_box.y));
        w.attr("width", &fmt_num(scene.view_box.width));
        w.attr("height", &fmt_num(scene.view_box.height));
        w.attr("fill", &color(bg));
        if !bg.is_opaque() {
            w.attr("fill-opacity", &fmt_num(bg.a));
        }
        w.close_empty();
    }

    write_group(&mut w, &scene.root.common, &scene.root.children);
    w.close("svg");
    w.finish()
}

/// Serializes one element on its own, for tests and debugging.
pub fn element_to_svg(element: &Element) -> String {
    let mut w = Writer::new(&SvgOptions::default(), None);
    write_element(&mut w, element);
    w.finish()
}

/// The inherited values synthetic bold and face pieces depend on, as the
/// cascade hands them to the children of a group.
#[derive(Debug, Clone, Default)]
struct Inherited {
    font: FontSpec,
    fill: Option<Paint>,
    fill_opacity: Option<f64>,
    stroke: Option<Paint>,
    dashed: bool,
    anchor: Option<TextAnchor>,
}

impl Inherited {
    fn under(&self, style: &Style) -> Inherited {
        Inherited {
            font: style.font.merged_over(&self.font),
            fill: style.fill.clone().or_else(|| self.fill.clone()),
            fill_opacity: style.fill_opacity.or(self.fill_opacity),
            stroke: style.stroke.clone().or_else(|| self.stroke.clone()),
            dashed: style
                .stroke_dasharray
                .as_ref()
                .map_or(self.dashed, |d| !d.is_empty()),
            anchor: style.text_anchor.or(self.anchor),
        }
    }
}

struct Writer<'a> {
    out: String,
    depth: usize,
    /// True between `open` and the first child or the close of that tag.
    tag_open: bool,
    /// The `<clipPath>` ids already written, so a clip that several
    /// elements share is defined once.
    clips: BTreeSet<String>,
    synthetic_bold: Option<SyntheticBold<'a>>,
    face_pieces: Option<FacePieces<'a>>,
    own_glyphs: Option<OwnGlyphs<'a>>,
    /// The texts with own-drawing pieces written so far.
    own_texts: usize,
    /// One entry per open group, the root's values first.
    inherited: Vec<Inherited>,
}

impl<'a> Writer<'a> {
    fn new(opts: &SvgOptions<'a>, root_family: Option<&str>) -> Writer<'a> {
        let root = Inherited {
            font: FontSpec {
                family: root_family.map(str::to_string),
                ..FontSpec::default()
            },
            ..Inherited::default()
        };
        Writer {
            out: String::new(),
            depth: 0,
            tag_open: false,
            clips: BTreeSet::new(),
            synthetic_bold: opts.synthetic_bold,
            face_pieces: opts.face_pieces,
            own_glyphs: opts.own_glyphs,
            own_texts: 0,
            inherited: vec![root],
        }
    }

    fn inherited(&self) -> Inherited {
        self.inherited.last().cloned().unwrap_or_default()
    }

    /// The stroke that stands in for a synthetic bold of `font` in `fill`,
    /// or `None` when there is none to draw: no probe, a weight below 600,
    /// a face with a real bold, an invisible fill, or a visible stroke of
    /// the text's own.
    fn bold_stroke(
        &self,
        font: &FontSpec,
        fill: Option<&Paint>,
        fill_opacity: Option<f64>,
        stroke: Option<&Paint>,
        dashed: bool,
    ) -> Option<Style> {
        let probe = self.synthetic_bold?;
        let run = run_font(font);
        if !probe(run.family, run.weight, run.style) {
            return None;
        }
        emboldening_stroke(font, fill, fill_opacity, stroke, dashed)
    }

    fn indent(&mut self) {
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
    }

    fn open(&mut self, name: &str) {
        if self.tag_open {
            self.children_follow();
        }
        self.indent();
        self.out.push('<');
        self.out.push_str(name);
        self.tag_open = true;
    }

    fn attr(&mut self, name: &str, value: &str) {
        self.out.push(' ');
        self.out.push_str(name);
        self.out.push_str("=\"");
        escape_attr(&mut self.out, value);
        self.out.push('"');
    }

    /// Ends the start tag; children come next on their own lines.
    fn children_follow(&mut self) {
        if self.tag_open {
            self.out.push_str(">\n");
            self.tag_open = false;
            self.depth += 1;
        }
    }

    fn close_empty(&mut self) {
        self.out.push_str("/>\n");
        self.tag_open = false;
    }

    fn close(&mut self, name: &str) {
        if self.tag_open {
            self.close_empty();
            return;
        }
        self.depth = self.depth.saturating_sub(1);
        self.indent();
        let _ = writeln!(self.out, "</{name}>");
    }

    /// Writes character data and the end tag on the same line as the start
    /// tag, so text content keeps its leading and trailing spaces intact.
    fn text_close(&mut self, name: &str, text: &str) {
        self.out.push('>');
        escape_text(&mut self.out, text);
        let _ = writeln!(self.out, "</{name}>");
        self.tag_open = false;
    }

    /// Ends a start tag whose children stay on the same line. Used for
    /// `<text>` with tspans: whitespace between tspans is character data
    /// under the default `xml:space`, so an indented tspan would gain a
    /// trailing space and shift a centered or end-anchored line.
    fn inline_children(&mut self) {
        if self.tag_open {
            self.out.push('>');
            self.tag_open = false;
        }
    }

    fn open_inline(&mut self, name: &str) {
        self.out.push('<');
        self.out.push_str(name);
        self.tag_open = true;
    }

    fn text_close_inline(&mut self, name: &str, text: &str) {
        self.out.push('>');
        escape_text(&mut self.out, text);
        let _ = write!(self.out, "</{name}>");
        self.tag_open = false;
    }

    fn close_inline(&mut self, name: &str) {
        let _ = writeln!(self.out, "</{name}>");
    }

    /// Writes preformatted elements, one per line, at the current depth.
    fn raw_lines(&mut self, lines: &str) {
        for line in lines.lines() {
            self.indent();
            self.out.push_str(line);
            self.out.push('\n');
        }
    }

    fn finish(self) -> String {
        self.out
    }
}

/// The font of a run with the defaults a browser applies: the scene's
/// default chain, CSS `medium` (16 px), weight 400, and normal style.
fn run_font(font: &FontSpec) -> RunFont<'_> {
    RunFont {
        family: font.family.as_deref().unwrap_or(super::DEFAULT_FONT_FAMILY),
        size: font.size.unwrap_or(16.0),
        weight: match font.weight {
            None | Some(FontWeight::Normal) => 400,
            Some(FontWeight::Bold) => 700,
            Some(FontWeight::Number(n)) => n,
        },
        style: font.style.unwrap_or(FontStyle::Normal),
    }
}

/// The stroke that stands in for an emboldening of `font` in `fill`, or
/// `None` for an invisible fill or a visible stroke of the text's own.
fn emboldening_stroke(
    font: &FontSpec,
    fill: Option<&Paint>,
    fill_opacity: Option<f64>,
    stroke: Option<&Paint>,
    dashed: bool,
) -> Option<Style> {
    if stroke.is_some_and(|s| *s != Paint::None) {
        return None;
    }
    // SVG's initial fill is black.
    let fill = fill.cloned().unwrap_or(Paint::Color(Color::BLACK));
    if fill == Paint::None {
        return None;
    }
    Some(Style {
        stroke: Some(fill),
        stroke_width: Some(2.0 * SYNTHETIC_BOLD_GROWTH * run_font(font).size),
        stroke_opacity: fill_opacity,
        // An inherited dash pattern would break the outline up.
        stroke_dasharray: dashed.then(Vec::new),
        ..Style::default()
    })
}

fn escape_attr(out: &mut String, value: &str) {
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            _ => out.push(c),
        }
    }
}

fn escape_text(out: &mut String, value: &str) {
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}

fn view_box(vb: &ViewBox) -> String {
    format!(
        "{} {} {} {}",
        fmt_num(vb.x),
        fmt_num(vb.y),
        fmt_num(vb.width),
        fmt_num(vb.height)
    )
}

fn color(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

fn paint(p: &Paint) -> String {
    match p {
        Paint::None => "none".to_string(),
        Paint::Color(c) => color(*c),
        Paint::Reference(id) => format!("url(#{id})"),
    }
}

/// A color's own alpha folds into the opacity attribute next to it, so a
/// translucent fill needs no second attribute from the caller.
fn paint_alpha(p: &Paint) -> Option<f64> {
    match p {
        Paint::Color(c) if !c.is_opaque() => Some(c.a),
        _ => None,
    }
}

fn points(points: &[Point]) -> String {
    let mut s = String::new();
    for (i, p) in points.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&fmt_num(p.x));
        s.push(',');
        s.push_str(&fmt_num(p.y));
    }
    s
}

fn transform(ops: &[TransformOp]) -> String {
    let mut s = String::new();
    for (i, op) in ops.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        match op {
            TransformOp::Translate(x, y) => {
                let _ = write!(s, "translate({}, {})", fmt_num(*x), fmt_num(*y));
            }
            TransformOp::Scale(x, y) => {
                let _ = write!(s, "scale({}, {})", fmt_num(*x), fmt_num(*y));
            }
            TransformOp::Rotate(a, cx, cy) => {
                let _ = write!(
                    s,
                    "rotate({}, {}, {})",
                    fmt_num(*a),
                    fmt_num(*cx),
                    fmt_num(*cy)
                );
            }
            TransformOp::Matrix(m) => {
                let parts: Vec<String> = m.iter().map(|v| fmt_num(*v)).collect();
                let _ = write!(s, "matrix({})", parts.join(", "));
            }
        }
    }
    s
}

fn length(l: Length) -> String {
    match l {
        Length::Px(v) => fmt_num(v),
        Length::Em(v) => format!("{}em", fmt_num(v)),
    }
}

fn font_weight(w: FontWeight) -> String {
    match w {
        FontWeight::Normal => "normal".to_string(),
        FontWeight::Bold => "bold".to_string(),
        FontWeight::Number(n) => n.to_string(),
    }
}

fn font_style(s: FontStyle) -> &'static str {
    match s {
        FontStyle::Normal => "normal",
        FontStyle::Italic => "italic",
        FontStyle::Oblique => "oblique",
    }
}

fn text_anchor(a: TextAnchor) -> &'static str {
    match a {
        TextAnchor::Start => "start",
        TextAnchor::Middle => "middle",
        TextAnchor::End => "end",
    }
}

fn baseline(b: Baseline) -> &'static str {
    match b {
        Baseline::Auto => "auto",
        Baseline::Middle => "middle",
        Baseline::Central => "central",
        Baseline::Hanging => "hanging",
        Baseline::TextBeforeEdge => "text-before-edge",
        Baseline::TextAfterEdge => "text-after-edge",
        Baseline::Alphabetic => "alphabetic",
    }
}

fn line_cap(c: LineCap) -> &'static str {
    match c {
        LineCap::Butt => "butt",
        LineCap::Round => "round",
        LineCap::Square => "square",
    }
}

fn line_join(j: LineJoin) -> &'static str {
    match j {
        LineJoin::Miter => "miter",
        LineJoin::Round => "round",
        LineJoin::Bevel => "bevel",
    }
}

fn write_font(w: &mut Writer, font: &FontSpec) {
    if let Some(family) = &font.family {
        w.attr("font-family", family);
    }
    if let Some(size) = font.size {
        w.attr("font-size", &fmt_num(size));
    }
    if let Some(weight) = font.weight {
        w.attr("font-weight", &font_weight(weight));
    }
    if let Some(style) = font.style {
        w.attr("font-style", font_style(style));
    }
}

/// Presentation attributes in one fixed order. `font` overrides the style's
/// font where set; `Text` passes its own.
fn write_style(w: &mut Writer, style: &Style, font: Option<&FontSpec>) {
    if let Some(fill) = &style.fill {
        w.attr("fill", &paint(fill));
    }
    let fill_opacity = style
        .fill_opacity
        .or_else(|| style.fill.as_ref().and_then(paint_alpha));
    if let Some(v) = fill_opacity {
        w.attr("fill-opacity", &fmt_num(v));
    }
    if let Some(stroke) = &style.stroke {
        w.attr("stroke", &paint(stroke));
    }
    if let Some(v) = style.stroke_width {
        w.attr("stroke-width", &fmt_num(v));
    }
    if let Some(dashes) = &style.stroke_dasharray {
        w.attr("stroke-dasharray", &dasharray(dashes));
    }
    if let Some(cap) = style.stroke_linecap {
        w.attr("stroke-linecap", line_cap(cap));
    }
    if let Some(join) = style.stroke_linejoin {
        w.attr("stroke-linejoin", line_join(join));
    }
    let stroke_opacity = style
        .stroke_opacity
        .or_else(|| style.stroke.as_ref().and_then(paint_alpha));
    if let Some(v) = stroke_opacity {
        w.attr("stroke-opacity", &fmt_num(v));
    }
    if let Some(v) = style.opacity {
        w.attr("opacity", &fmt_num(v));
    }
    let merged = font.map(|f| f.merged_over(&style.font));
    write_font(w, merged.as_ref().unwrap_or(&style.font));
    if let Some(anchor) = style.text_anchor {
        w.attr("text-anchor", text_anchor(anchor));
    }
    if let Some(id) = &style.marker_start {
        w.attr("marker-start", &format!("url(#{id})"));
    }
    if let Some(id) = &style.marker_end {
        w.attr("marker-end", &format!("url(#{id})"));
    }
    if let Some(clip) = &style.clip_path {
        w.attr("clip-path", &format!("url(#{})", clip.id));
    }
    if let Some(space) = style.xml_space {
        w.attr(
            "xml:space",
            match space {
                XmlSpace::Default => "default",
                XmlSpace::Preserve => "preserve",
            },
        );
    }
    // mermaid sets the blend mode with d3's `.style()`, so it is a `style`
    // declaration, which usvg reads like the attribute.
    if let Some(mode) = style.mix_blend_mode {
        w.attr("style", &format!("mix-blend-mode: {};", blend_mode(mode)));
    }
}

/// An empty list is `none`, which cancels an inherited pattern.
fn dasharray(dashes: &[f64]) -> String {
    if dashes.is_empty() {
        return "none".to_string();
    }
    let parts: Vec<String> = dashes.iter().map(|v| fmt_num(*v)).collect();
    parts.join(",")
}

/// The stroke attributes of a synthetic bold on a `tspan`.
fn write_bold_stroke(w: &mut Writer, bold: &Style) {
    if let Some(stroke) = &bold.stroke {
        w.attr("stroke", &paint(stroke));
    }
    if let Some(v) = bold.stroke_width {
        w.attr("stroke-width", &fmt_num(v));
    }
    if let Some(dashes) = &bold.stroke_dasharray {
        w.attr("stroke-dasharray", &dasharray(dashes));
    }
    let opacity = bold
        .stroke_opacity
        .or_else(|| bold.stroke.as_ref().and_then(paint_alpha));
    if let Some(v) = opacity {
        w.attr("stroke-opacity", &fmt_num(v));
    }
}

fn blend_mode(mode: BlendMode) -> &'static str {
    match mode {
        BlendMode::Normal => "normal",
        BlendMode::Multiply => "multiply",
    }
}

fn write_linear_gradient(w: &mut Writer, g: &LinearGradient) {
    w.open("linearGradient");
    w.attr("id", &g.id);
    w.attr(
        "gradientUnits",
        match g.units {
            GradientUnits::UserSpaceOnUse => "userSpaceOnUse",
            GradientUnits::ObjectBoundingBox => "objectBoundingBox",
        },
    );
    for (name, value) in [("x1", g.x1), ("y1", g.y1), ("x2", g.x2), ("y2", g.y2)] {
        if let Some(v) = value {
            w.attr(name, &fmt_num(v));
        }
    }
    if g.stops.is_empty() {
        w.close_empty();
        return;
    }
    w.children_follow();
    for stop in &g.stops {
        w.open("stop");
        w.attr("offset", &format!("{}%", fmt_num(stop.offset)));
        w.attr("stop-color", &color(stop.color));
        if !stop.color.is_opaque() {
            w.attr("stop-opacity", &fmt_num(stop.color.a));
        }
        w.close_empty();
    }
    w.close("linearGradient");
}

fn write_common_head(w: &mut Writer, common: &Common) {
    if let Some(id) = &common.id {
        w.attr("id", id);
    }
    if let Some(class) = &common.class {
        w.attr("class", class);
    }
    if !common.transform.is_empty() {
        w.attr("transform", &transform(&common.transform));
    }
}

/// The `<clipPath>` that `style` references, unless an earlier element
/// already wrote that id. It goes right before its first user, as mermaid's
/// treemap appends each one next to the text it clips.
fn write_clip_def(w: &mut Writer, style: &Style) {
    let Some(ClipPath { id, children }) = &style.clip_path else {
        return;
    };
    if !w.clips.insert(id.clone()) {
        return;
    }
    w.open("clipPath");
    w.attr("id", id);
    if children.is_empty() {
        w.close_empty();
        return;
    }
    w.children_follow();
    for child in children {
        write_element(w, child);
    }
    w.close("clipPath");
}

fn write_group(w: &mut Writer, common: &Common, children: &[Element]) {
    write_clip_def(w, &common.style);
    w.open("g");
    write_common_head(w, common);
    write_style(w, &common.style, None);
    if children.is_empty() {
        w.close_empty();
        return;
    }
    w.children_follow();
    let inherited = w.inherited().under(&common.style);
    w.inherited.push(inherited);
    for child in children {
        write_element(w, child);
    }
    w.inherited.pop();
    w.close("g");
}

fn write_text(w: &mut Writer, text: &Text) {
    // The text's own anchor attribute is written below; the style's copy
    // would duplicate it, so the style is written with it cleared.
    let mut style = text.common.style.clone();
    if text.anchor.is_some() {
        style.text_anchor = None;
    }
    let cascade = w.inherited().under(&style);
    let font = text.font.merged_over(&cascade.font);
    // The pieces come first: an own-drawing piece decides the text's id.
    let pieces: Vec<Vec<FacePiece>> = text
        .runs
        .iter()
        .map(|run| match w.face_pieces {
            Some(find) => find(&run.text, &run_font(&run.font.merged_over(&font))),
            None => Vec::new(),
        })
        .collect();
    let own_id = pieces.iter().flatten().any(|p| p.own.is_some()).then(|| {
        w.own_texts += 1;
        format!("kmt-{}", w.own_texts)
    });
    let shifts = chunk_shifts(text, &pieces, text.anchor.or(cascade.anchor));

    w.open("text");
    match &own_id {
        Some(id) => write_common_head(
            w,
            &Common {
                id: Some(id.clone()),
                ..text.common.clone()
            },
        ),
        None => write_common_head(w, &text.common),
    }
    w.attr("x", &fmt_num(text.x));
    w.attr("y", &fmt_num(text.y));
    if let Some(anchor) = text.anchor {
        w.attr("text-anchor", text_anchor(anchor));
    }
    if let Some(b) = text.baseline {
        w.attr("dominant-baseline", baseline(b));
    }
    let bold = w.bold_stroke(
        &font,
        cascade.fill.as_ref(),
        cascade.fill_opacity,
        cascade.stroke.as_ref(),
        cascade.dashed,
    );
    if let Some(bold) = &bold {
        style.stroke = bold.stroke.clone();
        style.stroke_width = bold.stroke_width;
        style.stroke_opacity = bold.stroke_opacity;
        style.stroke_dasharray = bold.stroke_dasharray.clone();
    }
    write_style(w, &style, Some(&text.font));

    let plain = text.runs.len() == 1 && pieces[0].is_empty() && {
        let run = &text.runs[0];
        run.x.is_none() && run.dy.is_none() && run.font == FontSpec::default() && run.fill.is_none()
    };
    if plain {
        w.text_close("text", &text.runs[0].text);
    } else {
        w.inline_children();
        let single = text.runs.len() == 1 && {
            let run = &text.runs[0];
            run.x.is_none()
                && run.dy.is_none()
                && run.font == FontSpec::default()
                && run.fill.is_none()
        };
        for ((run, pieces), shift) in text.runs.iter().zip(&pieces).zip(&shifts) {
            if single {
                if let Some(dx) = shift {
                    w.open_inline("tspan");
                    w.attr("dx", &fmt_num(*dx));
                    w.inline_children();
                }
                write_pieces(
                    w,
                    &run.text,
                    pieces,
                    &font,
                    None,
                    &cascade,
                    bold.is_some(),
                    text.baseline,
                );
                if shift.is_some() {
                    w.out.push_str("</tspan>");
                }
                continue;
            }
            w.open_inline("tspan");
            if let Some(x) = run.x {
                w.attr("x", &fmt_num(x));
            }
            if let Some(dx) = shift {
                w.attr("dx", &fmt_num(*dx));
            }
            if let Some(dy) = run.dy {
                w.attr("dy", &length(dy));
            }
            if let Some(fill) = &run.fill {
                w.attr("fill", &paint(fill));
                if let Some(a) = paint_alpha(fill) {
                    w.attr("fill-opacity", &fmt_num(a));
                }
            }
            write_font(w, &run.font);
            // A run that differs from its text in weight, size, or fill
            // needs its own stroke, or none where the text's does not
            // apply.
            let run_font = run.font.merged_over(&font);
            let run_fill = run.fill.as_ref().or(cascade.fill.as_ref());
            let run_bold = w.bold_stroke(
                &run_font,
                run_fill,
                cascade.fill_opacity,
                cascade.stroke.as_ref(),
                cascade.dashed,
            );
            match (&bold, &run_bold) {
                (Some(_), Some(own)) if run.font.size.is_some() || run.fill.is_some() => {
                    write_bold_stroke(w, own);
                }
                (None, Some(own)) => write_bold_stroke(w, own),
                (Some(_), None) => w.attr("stroke", "none"),
                _ => {}
            }
            if pieces.is_empty() {
                w.text_close_inline("tspan", &run.text);
                continue;
            }
            w.inline_children();
            write_pieces(
                w,
                &run.text,
                pieces,
                &run_font,
                run_fill,
                &cascade,
                run_bold.is_some(),
                text.baseline,
            );
            w.out.push_str("</tspan>");
        }
        w.close_inline("text");
    }

    let fragment = own_id
        .as_deref()
        .zip(w.own_glyphs)
        .and_then(|(id, own)| own(id));
    if let Some(fragment) = fragment {
        w.open("g");
        if !text.common.transform.is_empty() {
            w.attr("transform", &transform(&text.common.transform));
        }
        if let Some(v) = text.common.style.opacity {
            w.attr("opacity", &fmt_num(v));
        }
        if let Some(clip) = &text.common.style.clip_path {
            w.attr("clip-path", &format!("url(#{})", clip.id));
        }
        w.children_follow();
        w.raw_lines(&fragment);
        w.close("g");
    }
}

/// The `dx` that each run starting a text chunk needs, so the chunk keeps
/// the place Chrome gives it. usvg adds no letter-spacing after the last
/// cluster of a chunk, while Chrome's advance of a fallback piece includes
/// its correction; a chunk that ends in such a piece is that much too
/// narrow, which moves a centered chunk right by half of it and an
/// end-anchored one by all of it. A chunk starts at the first run and at
/// every run with its own `x`.
fn chunk_shifts(
    text: &Text,
    pieces: &[Vec<FacePiece>],
    anchor: Option<TextAnchor>,
) -> Vec<Option<f64>> {
    let mut shifts = vec![None; text.runs.len()];
    let share = match anchor {
        None | Some(TextAnchor::Start) => return shifts,
        Some(TextAnchor::Middle) => 0.5,
        Some(TextAnchor::End) => 1.0,
    };
    let mut start = 0;
    for (i, (run, run_pieces)) in text.runs.iter().zip(pieces).enumerate() {
        let ends_chunk = text.runs.get(i + 1).is_none_or(|next| next.x.is_some());
        if !ends_chunk {
            continue;
        }
        let last = run_pieces
            .last()
            .filter(|p| p.range.end == run.text.trim_end().len());
        if let Some(spacing) = last.and_then(|p| p.letter_spacing) {
            shifts[start] = Some(-share * spacing);
        }
        start = i + 1;
    }
    shifts
}

/// The character data of one run, with a `tspan` per face piece. `fill`
/// is the run's own fill when it has one; `emboldened` says whether the
/// enclosing element already carries a synthetic bold stroke.
///
/// Each piece repeats the text's `dominant-baseline`: Chrome inherits it
/// (SVG 2), while usvg reads it from the element and its direct parent
/// only, so a piece inside a run's `tspan` sat on the alphabetic baseline,
/// 4 px above the rest of a 16 px `middle` message text.
#[allow(clippy::too_many_arguments)]
fn write_pieces(
    w: &mut Writer,
    text: &str,
    pieces: &[FacePiece],
    font: &FontSpec,
    fill: Option<&Paint>,
    cascade: &Inherited,
    emboldened: bool,
    dominant: Option<Baseline>,
) {
    let mut at = 0;
    for piece in pieces {
        escape_text(&mut w.out, &text[at..piece.range.start]);
        w.open_inline("tspan");
        if let Some(b) = dominant {
            w.attr("dominant-baseline", baseline(b));
        }
        if let Some(family) = &piece.family {
            w.attr("font-family", family);
        }
        if let Some(spacing) = piece.letter_spacing {
            w.attr("letter-spacing", &fmt_num(spacing));
        }
        match piece.own {
            Some(_) if w.own_glyphs.is_some() => w.attr("visibility", "hidden"),
            Some(index) => {
                w.attr("font-variation-settings", &format!("'{OWN_AXIS}' {index}"));
                // usvg drops a text that draws nothing, and the
                // placeholder has no ink; an underline gives the layout
                // pass something to draw without moving a glyph.
                w.attr("text-decoration", "underline");
            }
            None => {}
        }
        let stroke = if piece.synthetic_bold {
            emboldening_stroke(
                font,
                fill.or(cascade.fill.as_ref()),
                cascade.fill_opacity,
                cascade.stroke.as_ref(),
                cascade.dashed,
            )
        } else {
            None
        };
        match (&stroke, emboldened) {
            (Some(own), false) => write_bold_stroke(w, own),
            (None, true) => w.attr("stroke", "none"),
            _ => {}
        }
        let content = match piece.own {
            Some(_) => OWN_PLACEHOLDER,
            None => &text[piece.range.clone()],
        };
        w.text_close_inline("tspan", content);
        at = piece.range.end;
    }
    escape_text(&mut w.out, &text[at..]);
}

fn write_marker(w: &mut Writer, m: &Marker) {
    w.open("marker");
    w.attr("id", &m.id);
    if let Some(class) = &m.class {
        w.attr("class", class);
    }
    if let Some(vb) = &m.view_box {
        w.attr("viewBox", &view_box(vb));
    }
    w.attr("refX", &fmt_num(m.ref_x));
    w.attr("refY", &fmt_num(m.ref_y));
    w.attr("markerWidth", &fmt_num(m.marker_width));
    w.attr("markerHeight", &fmt_num(m.marker_height));
    w.attr(
        "orient",
        match m.orient {
            Orient::Auto => "auto",
            Orient::AutoStartReverse => "auto-start-reverse",
        },
    );
    w.attr(
        "markerUnits",
        match m.marker_units {
            MarkerUnits::StrokeWidth => "strokeWidth",
            MarkerUnits::UserSpaceOnUse => "userSpaceOnUse",
        },
    );
    write_style(w, &m.style, None);
    if m.children.is_empty() {
        w.close_empty();
        return;
    }
    w.children_follow();
    for child in &m.children {
        write_element(w, child);
    }
    w.close("marker");
}

fn write_element(w: &mut Writer, element: &Element) {
    let style = match element {
        // `write_group` writes its own, so the root group gets one too.
        Element::Group(_) => None,
        Element::Rect(e) => Some(&e.common.style),
        Element::Circle(e) => Some(&e.common.style),
        Element::Ellipse(e) => Some(&e.common.style),
        Element::Line(e) => Some(&e.common.style),
        Element::Polygon(e) => Some(&e.common.style),
        Element::Polyline(e) => Some(&e.common.style),
        Element::Path(e) => Some(&e.common.style),
        Element::Text(e) => Some(&e.common.style),
        Element::Image(e) => Some(&e.common.style),
        Element::Use(e) => Some(&e.common.style),
        Element::Marker(m) => Some(&m.style),
    };
    if let Some(style) = style {
        write_clip_def(w, style);
    }
    match element {
        Element::Group(g) => write_group(w, &g.common, &g.children),
        Element::Rect(r) => {
            w.open("rect");
            write_common_head(w, &r.common);
            w.attr("x", &fmt_num(r.x));
            w.attr("y", &fmt_num(r.y));
            w.attr("width", &fmt_num(r.width));
            w.attr("height", &fmt_num(r.height));
            if let Some(rx) = r.rx {
                w.attr("rx", &fmt_num(rx));
            }
            if let Some(ry) = r.ry {
                w.attr("ry", &fmt_num(ry));
            }
            write_style(w, &r.common.style, None);
            w.close_empty();
        }
        Element::Circle(c) => {
            w.open("circle");
            write_common_head(w, &c.common);
            w.attr("cx", &fmt_num(c.cx));
            w.attr("cy", &fmt_num(c.cy));
            w.attr("r", &fmt_num(c.r));
            write_style(w, &c.common.style, None);
            w.close_empty();
        }
        Element::Ellipse(e) => {
            w.open("ellipse");
            write_common_head(w, &e.common);
            w.attr("cx", &fmt_num(e.cx));
            w.attr("cy", &fmt_num(e.cy));
            w.attr("rx", &fmt_num(e.rx));
            w.attr("ry", &fmt_num(e.ry));
            write_style(w, &e.common.style, None);
            w.close_empty();
        }
        Element::Line(l) => {
            w.open("line");
            write_common_head(w, &l.common);
            w.attr("x1", &fmt_num(l.x1));
            w.attr("y1", &fmt_num(l.y1));
            w.attr("x2", &fmt_num(l.x2));
            w.attr("y2", &fmt_num(l.y2));
            write_style(w, &l.common.style, None);
            w.close_empty();
        }
        Element::Polygon(p) => {
            w.open("polygon");
            write_common_head(w, &p.common);
            w.attr("points", &points(&p.points));
            write_style(w, &p.common.style, None);
            w.close_empty();
        }
        Element::Polyline(p) => {
            w.open("polyline");
            write_common_head(w, &p.common);
            w.attr("points", &points(&p.points));
            write_style(w, &p.common.style, None);
            w.close_empty();
        }
        Element::Path(p) => {
            w.open("path");
            write_common_head(w, &p.common);
            w.attr("d", &p.d.to_svg());
            write_style(w, &p.common.style, None);
            w.close_empty();
        }
        Element::Text(t) => write_text(w, t),
        Element::Image(i) => {
            w.open("image");
            write_common_head(w, &i.common);
            w.attr("x", &fmt_num(i.x));
            w.attr("y", &fmt_num(i.y));
            w.attr("width", &fmt_num(i.width));
            w.attr("height", &fmt_num(i.height));
            w.attr("href", &i.href);
            write_style(w, &i.common.style, None);
            w.close_empty();
        }
        Element::Use(u) => {
            w.open("use");
            write_common_head(w, &u.common);
            w.attr("href", &format!("#{}", u.href));
            w.attr("x", &fmt_num(u.x));
            w.attr("y", &fmt_num(u.y));
            write_style(w, &u.common.style, None);
            w.close_empty();
        }
        Element::Marker(m) => write_marker(w, m),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::scene::{Group, Rect, TextRun};

    fn bold_text(runs: Vec<TextRun>, weight: Option<FontWeight>) -> Element {
        Element::Text(Text {
            common: Common::with_style(Style::fill(Color::rgb(1, 2, 3))),
            x: 0.0,
            y: 0.0,
            anchor: None,
            baseline: None,
            font: FontSpec {
                size: Some(10.0),
                weight,
                ..FontSpec::default()
            },
            runs,
        })
    }

    fn with_probe(element: Element) -> String {
        let probe = |_: &str, weight: u16, _: FontStyle| weight >= 600;
        let opts = SvgOptions {
            synthetic_bold: Some(&probe),
            ..SvgOptions::default()
        };
        let mut scene = Scene::new(1.0, 1.0);
        scene.root.children.push(element);
        to_svg_with(&scene, &opts)
    }

    /// A piece for every `X` (a fallback face) and an own piece for every
    /// `E` (an emoji), with a spacing of 2 each.
    fn fake_pieces(text: &str, _: &RunFont<'_>) -> Vec<FacePiece> {
        let mut own = 0;
        text.char_indices()
            .filter(|(_, c)| *c == 'X' || *c == 'E')
            .map(|(i, c)| FacePiece {
                range: i..i + 1,
                family: (c == 'X').then(|| "face-x".to_string()),
                synthetic_bold: false,
                letter_spacing: Some(2.0),
                own: (c == 'E').then(|| {
                    own += 1;
                    own - 1
                }),
            })
            .collect()
    }

    fn with_pieces(element: Element, own_glyphs: Option<OwnGlyphs<'_>>) -> String {
        let opts = SvgOptions {
            face_pieces: Some(&fake_pieces),
            own_glyphs,
            ..SvgOptions::default()
        };
        let mut scene = Scene::new(1.0, 1.0);
        scene.root.children.push(element);
        to_svg_with(&scene, &opts)
    }

    fn label(runs: Vec<TextRun>, anchor: Option<TextAnchor>) -> Element {
        Element::Text(Text {
            common: Common {
                transform: vec![TransformOp::Translate(5.0, 6.0)],
                ..Common::default()
            },
            x: 0.0,
            y: 0.0,
            anchor,
            baseline: None,
            font: FontSpec::default(),
            runs,
        })
    }

    #[test]
    fn fallback_pieces_become_tspans_naming_their_face() {
        let svg = with_pieces(label(vec![TextRun::new("aXb")], None), None);
        assert!(
            svg.contains(">a<tspan font-family=\"face-x\" letter-spacing=\"2\">X</tspan>b</text>"),
            "{svg}"
        );
        // A run of its own keeps its attributes and nests the pieces.
        let svg = with_pieces(
            label(vec![TextRun::line("aX", 0.0, Length::Em(1.1))], None),
            None,
        );
        assert!(
            svg.contains("<tspan x=\"0\" dy=\"1.1em\">a<tspan font-family=\"face-x\""),
            "{svg}"
        );
        // Without the hook, or with no piece, the text is unchanged.
        let mut scene = Scene::new(1.0, 1.0);
        scene
            .root
            .children
            .push(label(vec![TextRun::new("aXb")], None));
        assert!(scene.to_svg().contains(">aXb</text>"));
        assert!(with_pieces(label(vec![TextRun::new("ab")], None), None).contains(">ab</text>"));
    }

    /// usvg reads `dominant-baseline` from an element and its direct parent
    /// only, so every piece repeats the text's value, as Chrome inherits it.
    #[test]
    fn pieces_repeat_the_text_dominant_baseline() {
        let middle = |runs: Vec<TextRun>| {
            let mut element = label(runs, Some(TextAnchor::Middle));
            if let Element::Text(text) = &mut element {
                text.baseline = Some(Baseline::Middle);
            }
            element
        };
        let svg = with_pieces(
            middle(vec![TextRun::line("aXbE", 0.0, Length::Em(1.0))]),
            None,
        );
        assert_eq!(
            svg.matches("dominant-baseline=\"middle\"").count(),
            3,
            "{svg}"
        );
        assert!(
            svg.contains("<tspan dominant-baseline=\"middle\" font-family=\"face-x\""),
            "{svg}"
        );
        let svg = with_pieces(middle(vec![TextRun::new("aXb")]), None);
        assert_eq!(
            svg.matches("dominant-baseline=\"middle\"").count(),
            2,
            "{svg}"
        );
        // The scene SVG has no pieces and so no repeats.
        let mut scene = Scene::new(1.0, 1.0);
        scene
            .root
            .children
            .push(middle(vec![TextRun::line("aXbE", 0.0, Length::Em(1.0))]));
        assert_eq!(
            scene.to_svg().matches("dominant-baseline").count(),
            1,
            "{}",
            scene.to_svg()
        );
    }

    #[test]
    fn own_pieces_are_placeholders_tagged_then_hidden() {
        let svg = with_pieces(label(vec![TextRun::new("aEbE")], None), None);
        assert!(svg.contains("<text id=\"kmt-1\""), "{svg}");
        assert!(
            svg.contains(
                "<tspan letter-spacing=\"2\" font-variation-settings=\"'kmpi' 0\" text-decoration=\"underline\">\u{A0}</tspan>b"
            ),
            "{svg}"
        );
        assert!(svg.contains("'kmpi' 1"), "{svg}");
        assert!(!svg.contains('E'), "{svg}");
        let drawn = |id: &str| (id == "kmt-1").then(|| "<path d=\"M0 0\"/>".to_string());
        let svg = with_pieces(label(vec![TextRun::new("aEbE")], None), Some(&drawn));
        assert!(
            svg.contains("<tspan letter-spacing=\"2\" visibility=\"hidden\">\u{A0}</tspan>"),
            "{svg}"
        );
        assert!(!svg.contains("kmpi"), "{svg}");
        // The drawn glyphs follow the text in its user space.
        assert!(
            svg.contains(
                "</text>\n    <g transform=\"translate(5, 6)\">\n      <path d=\"M0 0\"/>\n    </g>"
            ),
            "{svg}"
        );
    }

    #[test]
    fn a_chunk_ending_in_a_spaced_piece_moves_by_its_anchor_share() {
        // usvg drops the spacing after a chunk's last cluster.
        let middle = with_pieces(
            label(vec![TextRun::new("aX")], Some(TextAnchor::Middle)),
            None,
        );
        assert!(middle.contains("<tspan dx=\"-1\">a<tspan"), "{middle}");
        let end = with_pieces(label(vec![TextRun::new("aX")], Some(TextAnchor::End)), None);
        assert!(end.contains("<tspan dx=\"-2\">a<tspan"), "{end}");
        let start = with_pieces(label(vec![TextRun::new("aX")], None), None);
        assert!(!start.contains("dx="), "{start}");
        // A piece inside the chunk keeps its spacing; only the last counts.
        let inside = with_pieces(
            label(vec![TextRun::new("Xa")], Some(TextAnchor::Middle)),
            None,
        );
        assert!(!inside.contains("dx="), "{inside}");
        // Each line is a chunk of its own.
        let lines = with_pieces(
            label(
                vec![
                    TextRun::line("aX", 0.0, Length::Em(0.0)),
                    TextRun::line("b", 0.0, Length::Em(1.1)),
                ],
                Some(TextAnchor::End),
            ),
            None,
        );
        assert!(
            lines.contains("<tspan x=\"0\" dx=\"-2\" dy=\"0em\">"),
            "{lines}"
        );
        assert!(
            lines.contains("<tspan x=\"0\" dy=\"1.1em\">b</tspan>"),
            "{lines}"
        );
    }

    #[test]
    fn synthetic_bold_strokes_the_text_in_its_fill() {
        let svg = with_probe(bold_text(vec![TextRun::new("a")], Some(FontWeight::Bold)));
        assert!(
            svg.contains("fill=\"#010203\" stroke=\"#010203\" stroke-width=\"0.467\""),
            "{svg}"
        );
        // Without a probe the scene is written as it is.
        let mut scene = Scene::new(1.0, 1.0);
        scene
            .root
            .children
            .push(bold_text(vec![TextRun::new("a")], Some(FontWeight::Bold)));
        assert!(!scene.to_svg().contains("stroke"));
    }

    #[test]
    fn synthetic_bold_follows_the_runs() {
        let bold_run = TextRun {
            font: FontSpec {
                weight: Some(FontWeight::Bold),
                ..FontSpec::default()
            },
            ..TextRun::new("b")
        };
        let svg = with_probe(bold_text(vec![TextRun::new("a"), bold_run.clone()], None));
        assert!(svg.contains("<tspan>a</tspan>"), "{svg}");
        assert!(
            svg.contains("<tspan font-weight=\"bold\" stroke=\"#010203\" stroke-width=\"0.467\">b"),
            "{svg}"
        );
        let regular_run = TextRun {
            font: FontSpec {
                weight: Some(FontWeight::Normal),
                ..FontSpec::default()
            },
            ..TextRun::new("c")
        };
        let svg = with_probe(bold_text(
            vec![bold_run, regular_run],
            Some(FontWeight::Bold),
        ));
        assert!(
            svg.contains("<tspan font-weight=\"normal\" stroke=\"none\">c"),
            "{svg}"
        );
    }

    #[test]
    fn escapes_text_and_attributes() {
        let text = Text {
            common: Common {
                id: Some("a\"b".to_string()),
                ..Common::default()
            },
            x: 0.0,
            y: 0.0,
            anchor: None,
            baseline: None,
            font: FontSpec::default(),
            runs: vec![TextRun::new("<a & b>")],
        };
        let svg = element_to_svg(&Element::Text(text));
        assert_eq!(
            svg,
            "<text id=\"a&quot;b\" x=\"0\" y=\"0\">&lt;a &amp; b&gt;</text>\n"
        );
    }

    #[test]
    fn a_shared_clip_is_defined_once_before_its_first_user() {
        let clip = ClipPath::rect("c0", 0.0, 0.0, 8.0, 4.0);
        let text = |s: &str| {
            Element::Text(Text {
                common: Common::with_style(Style {
                    clip_path: Some(clip.clone()),
                    ..Style::default()
                }),
                x: 1.0,
                y: 2.0,
                anchor: None,
                baseline: None,
                font: FontSpec::default(),
                runs: vec![TextRun::new(s)],
            })
        };
        let group = Element::Group(Group {
            common: Common::default(),
            children: vec![text("a"), text("b")],
        });
        assert_eq!(
            element_to_svg(&group),
            "<g>\n  <clipPath id=\"c0\">\n    <rect x=\"0\" y=\"0\" width=\"8\" height=\"4\"/>\n  \
             </clipPath>\n  <text x=\"1\" y=\"2\" clip-path=\"url(#c0)\">a</text>\n  \
             <text x=\"1\" y=\"2\" clip-path=\"url(#c0)\">b</text>\n</g>\n"
        );
    }

    #[test]
    fn preserved_space_reaches_the_text() {
        let text = Text {
            common: Common::with_style(Style {
                xml_space: Some(XmlSpace::Preserve),
                ..Style::default()
            }),
            x: 0.0,
            y: 0.0,
            anchor: None,
            baseline: None,
            font: FontSpec::default(),
            runs: vec![TextRun::new("  a  b ")],
        };
        assert_eq!(
            element_to_svg(&Element::Text(text)),
            "<text x=\"0\" y=\"0\" xml:space=\"preserve\">  a  b </text>\n"
        );
    }

    #[test]
    fn same_scene_same_bytes() {
        let mut scene = Scene::new(10.0, 10.0);
        scene.root.children.push(Element::Rect(Rect {
            common: Common::with_style(Style::fill(Color::rgba(255, 0, 0, 0.5))),
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 5.0,
            rx: None,
            ry: None,
        }));
        assert_eq!(scene.to_svg(), scene.clone().to_svg());
        assert!(
            scene
                .to_svg()
                .contains("fill=\"#ff0000\" fill-opacity=\"0.5\"")
        );
    }
}
