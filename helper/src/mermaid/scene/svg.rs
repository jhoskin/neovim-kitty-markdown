//! Deterministic SVG serializer for the scene model.
//!
//! The same scene always produces the same bytes: attributes come out in a
//! fixed order, every number goes through [`fmt_num`], and child elements
//! keep their scene order. That is what makes insta snapshots of a scene
//! meaningful, and it also keeps resvg's input stable between runs.

use std::fmt::Write;

use super::path::fmt_num;
use super::{
    Baseline, Color, Common, Element, FontSpec, FontStyle, FontWeight, Length, LineCap, LineJoin,
    Marker, MarkerUnits, Orient, Paint, Point, Scene, Style, Text, TextAnchor, TransformOp,
    ViewBox,
};

const XMLNS: &str = "http://www.w3.org/2000/svg";
const XLINK: &str = "http://www.w3.org/1999/xlink";

/// Serializes the scene. The root carries the natural size, the viewBox, and
/// the default font family; a background rect comes first when the scene
/// has a background color.
pub fn to_svg(scene: &Scene) -> String {
    let mut w = Writer::new();
    w.open("svg");
    w.attr("xmlns", XMLNS);
    w.attr("xmlns:xlink", XLINK);
    w.attr("width", &fmt_num(scene.width));
    w.attr("height", &fmt_num(scene.height));
    w.attr("viewBox", &view_box(&scene.view_box));
    w.attr("font-family", &scene.font_family);
    w.children_follow();

    if !scene.defs.is_empty() {
        w.open("defs");
        w.children_follow();
        for element in &scene.defs {
            write_element(&mut w, element);
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
    let mut w = Writer::new();
    write_element(&mut w, element);
    w.finish()
}

struct Writer {
    out: String,
    depth: usize,
    /// True between `open` and the first child or the close of that tag.
    tag_open: bool,
}

impl Writer {
    fn new() -> Writer {
        Writer {
            out: String::new(),
            depth: 0,
            tag_open: false,
        }
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

    fn finish(self) -> String {
        self.out
    }
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
        let parts: Vec<String> = dashes.iter().map(|v| fmt_num(*v)).collect();
        w.attr("stroke-dasharray", &parts.join(","));
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

fn write_group(w: &mut Writer, common: &Common, children: &[Element]) {
    w.open("g");
    write_common_head(w, common);
    write_style(w, &common.style, None);
    if children.is_empty() {
        w.close_empty();
        return;
    }
    w.children_follow();
    for child in children {
        write_element(w, child);
    }
    w.close("g");
}

fn write_text(w: &mut Writer, text: &Text) {
    w.open("text");
    write_common_head(w, &text.common);
    w.attr("x", &fmt_num(text.x));
    w.attr("y", &fmt_num(text.y));
    if let Some(anchor) = text.anchor {
        w.attr("text-anchor", text_anchor(anchor));
    }
    if let Some(b) = text.baseline {
        w.attr("dominant-baseline", baseline(b));
    }
    // The text's own anchor attribute was written above; the style's copy
    // would duplicate it, so the style is written with it cleared.
    let mut style = text.common.style.clone();
    if text.anchor.is_some() {
        style.text_anchor = None;
    }
    write_style(w, &style, Some(&text.font));

    let plain = text.runs.len() == 1 && {
        let run = &text.runs[0];
        run.x.is_none() && run.dy.is_none() && run.font == FontSpec::default() && run.fill.is_none()
    };
    if plain {
        w.text_close("text", &text.runs[0].text);
        return;
    }
    w.children_follow();
    for run in &text.runs {
        w.open("tspan");
        if let Some(x) = run.x {
            w.attr("x", &fmt_num(x));
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
        w.text_close("tspan", &run.text);
    }
    w.close("text");
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
    use crate::mermaid::scene::{Rect, TextRun};

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
