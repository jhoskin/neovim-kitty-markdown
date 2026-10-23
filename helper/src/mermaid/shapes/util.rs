//! Port of `rendering-elements/shapes/util.ts`, `insertPolygonShape.ts`,
//! `roundedRectPath.ts`, and `handDrawnShapeStyles.ts` (`compileStyles`,
//! `styles2String`), plus the scene-building helpers every shape shares.
//!
//! mermaid measures elements with `getBBox()`; without a DOM the helpers
//! here compute the same boxes from the geometry ([`points_bbox`],
//! [`path_bbox`]).

use indexmap::IndexMap;

use super::measure::{LabelAlign, LabelBox, LabelOpts};
use super::{Node, NodeStyleVars, ShapeCtx, ShapeOut};
use crate::mermaid::scene::path::Command;
use crate::mermaid::scene::{
    Baseline, Circle, Color, Common, Element, FontSpec, FontStyle, FontWeight, Group, Length, Line,
    Paint, Path, PathData, Point, Polygon, Rect, Style, Text, TextAnchor, TextRun, TransformOp,
};

/// `translate(x, y)` as a transform list.
pub fn translate(x: f64, y: f64) -> Vec<TransformOp> {
    vec![TransformOp::Translate(x, y)]
}

/// `getNodeClasses(node, extra)`: `node <cssClasses> <extra>`. The
/// handDrawn `rough-node` spelling is unreachable here.
pub fn get_node_classes(node: &Node, extra: Option<&str>) -> String {
    format!("node {} {}", node.css_classes, extra.unwrap_or(""))
}

// ---------------------------------------------------------------------------
// Styles

/// CSS declarations in order, later keys replacing earlier values in place
/// (a JS `Map.set` on an existing key keeps its position).
pub type Declarations = IndexMap<String, String>;

/// `compileStyles(node)`: `cssCompiledStyles`, then `cssStyles`, then the
/// `labelStyle` string split on `;`, as a key-to-value map.
pub fn compile_styles(node: &Node) -> Declarations {
    let mut map = Declarations::new();
    let label_style = node
        .label_style
        .as_deref()
        .map(|s| {
            s.split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for decl in node
        .css_compiled_styles
        .iter()
        .chain(node.css_styles.iter())
        .chain(label_style.iter())
    {
        let (key, value) = match decl.split_once(':') {
            Some((k, v)) => (k.trim().to_string(), v.trim().to_string()),
            None => (decl.trim().to_string(), String::new()),
        };
        map.insert(key, value);
    }
    map
}

/// `isLabelStyle(key)`.
pub fn is_label_style(key: &str) -> bool {
    matches!(
        key,
        "color"
            | "font-size"
            | "font-family"
            | "font-weight"
            | "font-style"
            | "text-decoration"
            | "text-align"
            | "text-transform"
            | "line-height"
            | "letter-spacing"
            | "word-spacing"
            | "text-shadow"
            | "text-overflow"
            | "white-space"
            | "word-wrap"
            | "word-break"
            | "overflow-wrap"
            | "hyphens"
    )
}

/// The two halves `styles2String(node)` returns, without the `!important`
/// suffixes (the scene has no cascade to win).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeStyles {
    pub label_styles: Declarations,
    pub node_styles: Declarations,
}

/// `styles2String(node)`.
pub fn styles2string(node: &Node) -> NodeStyles {
    let mut out = NodeStyles::default();
    for (key, value) in compile_styles(node) {
        if is_label_style(&key) {
            out.label_styles.insert(key, value);
        } else {
            out.node_styles.insert(key, value);
        }
    }
    out
}

/// The CSS named colors mermaid diagrams use in practice, plus the basic
/// sixteen. Anything else is left to the theme package's khroma port.
fn named_color(name: &str) -> Option<Color> {
    let rgb = match name.to_ascii_lowercase().as_str() {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "red" => (255, 0, 0),
        "lime" => (0, 255, 0),
        "blue" => (0, 0, 255),
        "yellow" => (255, 255, 0),
        "cyan" | "aqua" => (0, 255, 255),
        "magenta" | "fuchsia" => (255, 0, 255),
        "silver" => (192, 192, 192),
        "gray" | "grey" => (128, 128, 128),
        "maroon" => (128, 0, 0),
        "olive" => (128, 128, 0),
        "green" => (0, 128, 0),
        "purple" => (128, 0, 128),
        "teal" => (0, 128, 128),
        "navy" => (0, 0, 128),
        "orange" => (255, 165, 0),
        "pink" => (255, 192, 203),
        "brown" => (165, 42, 42),
        "lightblue" => (173, 216, 230),
        "lightgrey" | "lightgray" => (211, 211, 211),
        "darkgrey" | "darkgray" => (169, 169, 169),
        "lightgreen" => (144, 238, 144),
        "lightyellow" => (255, 255, 224),
        "gold" => (255, 215, 0),
        "coral" => (255, 127, 80),
        "salmon" => (250, 128, 114),
        "tomato" => (255, 99, 71),
        "violet" => (238, 130, 238),
        "indigo" => (75, 0, 130),
        "khaki" => (240, 230, 140),
        "beige" => (245, 245, 220),
        "ivory" => (255, 255, 240),
        "lavender" => (230, 230, 250),
        "skyblue" => (135, 206, 235),
        "steelblue" => (70, 130, 180),
        "crimson" => (220, 20, 60),
        "darkblue" => (0, 0, 139),
        "darkgreen" => (0, 100, 0),
        "darkred" => (139, 0, 0),
        "whitesmoke" => (245, 245, 245),
        "gainsboro" => (220, 220, 220),
        _ => return None,
    };
    Some(Color::rgb(rgb.0, rgb.1, rgb.2))
}

/// Parses a CSS color value into a paint: `none`/`transparent`, hex,
/// `rgb()`/`rgba()`, `url(#id)`, and the named colors above.
pub fn parse_css_color(value: &str) -> Option<Paint> {
    let v = value.trim();
    if v.eq_ignore_ascii_case("none") || v.eq_ignore_ascii_case("transparent") {
        return Some(Paint::None);
    }
    if let Some(c) = Color::from_hex(v) {
        return Some(Paint::Color(c));
    }
    if let Some(inner) = v.strip_prefix("url(").and_then(|s| s.strip_suffix(')')) {
        return Some(Paint::Reference(inner.trim_start_matches('#').to_string()));
    }
    let lower = v.to_ascii_lowercase();
    if let Some(inner) = lower
        .strip_prefix("rgba(")
        .or_else(|| lower.strip_prefix("rgb("))
        .and_then(|s| s.strip_suffix(')'))
    {
        let parts: Vec<f64> = inner
            .split([',', ' ', '/'])
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.trim().parse::<f64>().ok())
            .collect();
        if parts.len() >= 3 {
            let a = parts.get(3).copied().unwrap_or(1.0);
            let ch = |f: f64| f.round().clamp(0.0, 255.0) as u8;
            return Some(Paint::Color(Color::rgba(
                ch(parts[0]),
                ch(parts[1]),
                ch(parts[2]),
                a,
            )));
        }
        return None;
    }
    named_color(v).map(Paint::Color)
}

fn parse_px(value: &str) -> Option<f64> {
    value
        .trim()
        .trim_end_matches("px")
        .trim()
        .parse::<f64>()
        .ok()
}

/// Applies CSS declarations to a scene style the way an inline `style`
/// attribute would: only the presentation properties the scene knows.
pub fn apply_declarations(style: &mut Style, decls: &Declarations) {
    for (key, value) in decls {
        match key.as_str() {
            "fill" => {
                if let Some(p) = parse_css_color(value) {
                    style.fill = Some(p);
                }
            }
            "stroke" => {
                if let Some(p) = parse_css_color(value) {
                    style.stroke = Some(p);
                }
            }
            "stroke-width" => {
                if let Some(w) = parse_px(value) {
                    style.stroke_width = Some(w);
                }
            }
            "stroke-dasharray" => {
                let dashes: Vec<f64> = value
                    .split([',', ' '])
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse::<f64>().ok())
                    .collect();
                if !dashes.is_empty() {
                    style.stroke_dasharray = Some(dashes);
                }
            }
            "opacity" => {
                if let Ok(o) = value.trim().parse::<f64>() {
                    style.opacity = Some(o);
                }
            }
            "fill-opacity" => {
                if let Ok(o) = value.trim().parse::<f64>() {
                    style.fill_opacity = Some(o);
                }
            }
            "stroke-opacity" => {
                if let Ok(o) = value.trim().parse::<f64>() {
                    style.stroke_opacity = Some(o);
                }
            }
            _ => {}
        }
    }
}

/// The stylesheet rule every classic node shape gets, `.node rect, .node
/// circle, .node ellipse, .node polygon, .node path { fill: mainBkg;
/// stroke: nodeBorder; stroke-width: strokeWidth }`, as a scene style.
pub fn base_node_style(vars: &NodeStyleVars) -> Style {
    Style {
        fill: Some(Paint::Color(vars.main_bkg)),
        stroke: Some(Paint::Color(vars.node_border)),
        stroke_width: Some(vars.stroke_width),
        ..Style::default()
    }
}

/// The resolved style of a node's shape element: the stylesheet rule with
/// the user's `nodeStyles` applied on top.
pub fn node_style(ctx: &ShapeCtx, node: &Node) -> Style {
    let mut style = base_node_style(&ctx.vars);
    apply_declarations(&mut style, &styles2string(node).node_styles);
    style
}

/// A style with fill and stroke paints and the theme stroke width.
pub fn paint_style(vars: &NodeStyleVars, fill: Paint, stroke: Paint) -> Style {
    Style {
        fill: Some(fill),
        stroke: Some(stroke),
        stroke_width: Some(vars.stroke_width),
        ..Style::default()
    }
}

/// The label text fill: `color:` from the label styles, else the theme's
/// `nodeTextColor` (`.label text, span { fill: nodeTextColor }`).
pub fn label_fill(ctx: &ShapeCtx, node: &Node) -> Color {
    styles2string(node)
        .label_styles
        .get("color")
        .and_then(|c| parse_css_color(c))
        .and_then(|p| match p {
            Paint::Color(c) => Some(c),
            _ => None,
        })
        .unwrap_or(ctx.vars.node_text_color)
}

/// The font declarations of an inline label style (`font-size: 20px;
/// font-weight: bold`), which the browser applies to the label span over
/// every stylesheet rule. Values it cannot read are left unset.
pub fn font_from_declarations(decls: &Declarations) -> FontSpec {
    let mut font = FontSpec::default();
    for (key, value) in decls {
        let value = value.trim().trim_end_matches("!important").trim();
        match key.as_str() {
            "font-size" => font.size = parse_px(value).filter(|v| *v > 0.0).or(font.size),
            "font-family" if !value.is_empty() => font.family = Some(value.to_string()),
            "font-weight" => {
                font.weight = match value {
                    "normal" => Some(FontWeight::Normal),
                    // `bolder` over the normal weight of every label resolves
                    // to 700.
                    "bold" | "bolder" => Some(FontWeight::Bold),
                    other => other.parse::<u16>().ok().map(FontWeight::Number),
                }
                .or(font.weight);
            }
            "font-style" => {
                font.style = match value {
                    "italic" => Some(FontStyle::Italic),
                    "oblique" => Some(FontStyle::Oblique),
                    "normal" => Some(FontStyle::Normal),
                    _ => None,
                }
                .or(font.style);
            }
            _ => {}
        }
    }
    font
}

/// The font a node's labels are measured and drawn in: the inline label
/// styles over the diagram rule for this node (`Node::label_font`) over
/// the theme font, the cascade the browser resolves for the label span.
pub fn label_font(ctx: &ShapeCtx, node: &Node) -> FontSpec {
    font_from_declarations(&styles2string(node).label_styles)
        .merged_over(&node.label_font.merged_over(&ctx.font()))
}

// ---------------------------------------------------------------------------
// Labels

/// What `labelHelper` returns: the node group (`shapeSvg`), the measured
/// box, `halfPadding`, and the positioned label group to append last.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelHelperOut {
    pub shape_svg: Group,
    pub bbox: LabelBox,
    pub half_padding: f64,
    pub label: Group,
}

/// The wrapping width `createText` gets, as the label will measure it.
///
/// An HTML label whose label styles say `white-space: nowrap` (the class
/// note) never wraps: `addHtmlSpan` applies the style to the span, so when
/// the first measure hits `max-width` and the div turns into a fixed-width
/// `display: table` with `break-spaces`, the span still keeps its line,
/// and the table grows to that line's width. SVG text wraps by width alone.
fn html_wrap_width(node: &Node, html: bool, width: f64) -> Option<f64> {
    let nowrap = styles2string(node)
        .label_styles
        .get("white-space")
        .is_some_and(|v| v.trim_end_matches("!important").trim() == "nowrap");
    (!(html && nowrap)).then_some(width)
}

/// The `text-align` of a node's label styles. `createText` applies the
/// label style to the span, and the `<p>` that holds the label lines
/// inherits it, so a kanban card or a class note starts each line at the
/// left edge of the label box.
pub fn label_align(node: &Node) -> LabelAlign {
    let styles = styles2string(node).label_styles;
    match styles
        .get("text-align")
        .map(|v| v.trim_end_matches("!important").trim())
    {
        Some("left" | "start") => LabelAlign::Left,
        Some("right" | "end") => LabelAlign::Right,
        _ => LabelAlign::Center,
    }
}

/// Where a line of `width` starts in an HTML label box of `box_width`.
fn line_left(align: LabelAlign, box_width: f64, width: f64) -> f64 {
    match align {
        LabelAlign::Center => (box_width - width) / 2.0,
        LabelAlign::Left => 0.0,
        LabelAlign::Right => box_width - width,
    }
}

/// `labelHelper(parent, node, classes, { ignoreMinWidth })`.
pub fn label_helper(
    node: &Node,
    ctx: &ShapeCtx,
    classes: Option<&str>,
    ignore_min_width: bool,
) -> LabelHelperOut {
    let use_html_labels = ctx.use_html_labels(node);
    let css_classes = classes.unwrap_or("node default").to_string();
    let shape_svg = Group {
        common: Common {
            id: Some(node.dom_id().to_string()),
            class: Some(css_classes),
            ..Common::default()
        },
        children: Vec::new(),
    };

    let label = node.label.clone().unwrap_or_default();
    let is_markdown = node.label_type.as_deref() == Some("markdown");
    // An explicit node width wins over the diagram-level minimum; an empty
    // label has nothing to widen.
    let min_label_width = if !label.is_empty() && node.width.is_none() && !ignore_min_width {
        node.min_width.unwrap_or(0.0)
    } else {
        0.0
    };
    let opts = LabelOpts {
        font: label_font(ctx, node),
        html: use_html_labels,
        width: html_wrap_width(
            node,
            use_html_labels,
            node.width
                .or(node.wrapping_width)
                .unwrap_or(ctx.wrapping_width),
        ),
        min_width: min_label_width,
        markdown: is_markdown,
    };
    let mut bbox = ctx.measurer.measure(&label, &opts);
    bbox.align = label_align(node);
    // `withMinWidth`: SVG text keeps its centring as it grows.
    if bbox.width < min_label_width {
        if !use_html_labels {
            bbox.x -= (min_label_width - bbox.width) / 2.0;
        }
        bbox.width = min_label_width;
    }

    let half_padding = node.padding() / 2.0;
    let transform = if use_html_labels || node.center_label {
        translate(-bbox.width / 2.0, -bbox.height / 2.0)
    } else {
        translate(0.0, -bbox.height / 2.0)
    };
    let label_group = build_label_group(&bbox, ctx, label_fill(ctx, node), "label", transform);
    LabelHelperOut {
        shape_svg,
        bbox,
        half_padding,
        label: label_group,
    }
}

/// `insertLabel(parent, label, options)`: measures and builds one more
/// label group for shapes with several text blocks (kanban, usecase).
pub fn insert_label(
    text: &str,
    ctx: &ShapeCtx,
    node: &Node,
    width: Option<f64>,
    center_label: bool,
) -> (Group, LabelBox) {
    let use_html_labels = ctx.use_html_labels(node);
    let opts = LabelOpts {
        font: label_font(ctx, node),
        html: use_html_labels,
        width: html_wrap_width(node, use_html_labels, width.unwrap_or(ctx.wrapping_width)),
        min_width: 0.0,
        markdown: true,
    };
    let mut bbox = ctx.measurer.measure(text, &opts);
    bbox.align = label_align(node);
    let transform = if use_html_labels || center_label {
        translate(-bbox.width / 2.0, -bbox.height / 2.0)
    } else {
        translate(0.0, -bbox.height / 2.0)
    };
    let group = build_label_group(&bbox, ctx, label_fill(ctx, node), "label", transform);
    (group, bbox)
}

/// Measures one text block as a shape's own `createText` call would:
/// `width` is the wrapping width (`None` for `Number.POSITIVE_INFINITY`).
pub fn measure_text(
    ctx: &ShapeCtx,
    node: &Node,
    text: &str,
    width: Option<f64>,
    markdown: bool,
) -> LabelBox {
    let opts = LabelOpts {
        font: label_font(ctx, node),
        html: ctx.use_html_labels(node),
        width,
        min_width: 0.0,
        markdown,
    };
    ctx.measurer.measure(text, &opts)
}

/// `calculateTextWidth(text, config)` with `config = getConfig()`: the
/// root `fontFamily` and `fontSize`, not the label font. mermaid's default
/// `fontFamily` ends in `;`, which Chrome rejects, so the default config
/// measures in the page default font and comes out narrower than the
/// drawn label.
pub fn calculate_text_width(ctx: &ShapeCtx, text: &str) -> f64 {
    ctx.measurer.text_width(
        text,
        &ctx.vars.config_font_family,
        ctx.vars.config_font_size,
    )
}

/// Parses the absolute-command SVG path strings mermaid writes by hand
/// (`M`, `L`, `H`, `V`, `C`, `Q`, `A`, `Z`, upper case, any separators).
/// Returns `None` on a token it does not know.
pub fn parse_svg_path(s: &str) -> Option<PathData> {
    let mut d = PathData::new();
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    for c in s.chars() {
        if c.is_ascii_alphabetic() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            tokens.push(c.to_string());
        } else if c.is_whitespace() || c == ',' {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else if c == '-' && !current.is_empty() && !current.ends_with('e') {
            tokens.push(std::mem::take(&mut current));
            current.push(c);
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    let mut i = 0;
    let mut cur = Point::default();
    let num = |i: &mut usize| -> Option<f64> {
        let v = tokens.get(*i)?.parse::<f64>().ok()?;
        *i += 1;
        Some(v)
    };
    while i < tokens.len() {
        let cmd = tokens[i].clone();
        i += 1;
        match cmd.as_str() {
            "M" => {
                let x = num(&mut i)?;
                let y = num(&mut i)?;
                d.move_to(x, y);
                cur = Point::new(x, y);
            }
            "L" => {
                let x = num(&mut i)?;
                let y = num(&mut i)?;
                d.line_to(x, y);
                cur = Point::new(x, y);
            }
            "H" => {
                let x = num(&mut i)?;
                d.line_to(x, cur.y);
                cur.x = x;
            }
            "V" => {
                let y = num(&mut i)?;
                d.line_to(cur.x, y);
                cur.y = y;
            }
            "C" => {
                let x1 = num(&mut i)?;
                let y1 = num(&mut i)?;
                let x2 = num(&mut i)?;
                let y2 = num(&mut i)?;
                let x = num(&mut i)?;
                let y = num(&mut i)?;
                d.cubic_to(x1, y1, x2, y2, x, y);
                cur = Point::new(x, y);
            }
            "Q" => {
                let x1 = num(&mut i)?;
                let y1 = num(&mut i)?;
                let x = num(&mut i)?;
                let y = num(&mut i)?;
                d.quad_to(x1, y1, x, y);
                cur = Point::new(x, y);
            }
            "A" => {
                let rx = num(&mut i)?;
                let ry = num(&mut i)?;
                let rot = num(&mut i)?;
                let large = num(&mut i)? != 0.0;
                let sweep = num(&mut i)? != 0.0;
                let x = num(&mut i)?;
                let y = num(&mut i)?;
                d.arc_to(rx, ry, rot, large, sweep, x, y);
                cur = Point::new(x, y);
            }
            "Z" => {
                d.close();
            }
            _ => return None,
        }
    }
    Some(d)
}

/// Builds the `g.label` for a measured box. The group carries the
/// transform the caller computed; the text sits where the browser would
/// draw the HTML or SVG label inside the box.
pub fn build_label_group(
    bbox: &LabelBox,
    ctx: &ShapeCtx,
    fill: Color,
    class: &str,
    transform: Vec<TransformOp>,
) -> Group {
    let mut group = Group {
        common: Common {
            class: class_attr(class),
            transform,
            ..Common::default()
        },
        children: Vec::new(),
    };
    if ctx.html_labels
        && let Some(color) = bbox.underline
    {
        group.children.extend(underlines(bbox, ctx, color));
    }
    if let Some(text) = label_text(bbox, fill, ctx.html_labels) {
        group.children.push(Element::Text(text));
    }
    group
}

/// The `color` a decorating ancestor resolves `text-decoration-color:
/// currentcolor` against. No mermaid stylesheet sets `color` on the
/// `g.label` or `<a>` that carries a label's underline, so it is the page
/// default. The line takes the decorating box's color, not the label
/// text's: in the oracle's Chrome a red label under an underlined `g` gets
/// a black line.
pub const PAGE_TEXT_COLOR: Color = Color::BLACK;

/// The lines Blink paints for a `text-decoration: underline` that reaches
/// an HTML label from an ancestor, before the text as Blink paints them:
/// one under each line's advance, `max(1, ceil(thickness / 2))` below the
/// alphabetic baseline (`ComputeUnderlineOffsetAuto`), as thick as
/// `LabelMeasurer::underline_thickness`. Read off the oracle at 14, 16,
/// and 18 px Trebuchet MS: a 1 px line 1 px under the baseline. The gaps
/// `text-decoration-skip-ink` leaves around descenders are not drawn.
fn underlines(bbox: &LabelBox, ctx: &ShapeCtx, color: Color) -> Vec<Element> {
    let thickness = ctx.measurer.underline_thickness(&bbox.font);
    let gap = (thickness / 2.0).ceil().max(1.0);
    bbox.lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.width > 0.0)
        .map(|(i, line)| {
            let baseline = bbox.baseline + i as f64 * bbox.line_height;
            rect_element(
                line_left(bbox.align, bbox.width, line.width),
                baseline + gap,
                line.width,
                thickness,
                None,
                None,
                "",
                Style::fill(color),
            )
        })
        .collect()
}

/// The text element of a label, in the font it was measured in.
///
/// HTML mode: a `div` with `line-height: 1.5` and `text-align: center`,
/// so every line is centred in its line box of `line_height` pixels, or
/// starts at the edge the label's own `text-align` names.
/// SVG mode: `createFormattedText`, a `text` at `x = 0` whose `tspan`s
/// sit at `y = i * 1.1 - 0.1 em` with `dy = 1.1em`, anchored `middle` by
/// `.node .label text { text-anchor: middle }`.
pub fn label_text(bbox: &LabelBox, fill: Color, html: bool) -> Option<Text> {
    if bbox.is_empty() {
        return None;
    }
    let (anchor, line_x) = match (html, bbox.align) {
        (true, LabelAlign::Left) => (TextAnchor::Start, 0.0),
        (true, LabelAlign::Right) => (TextAnchor::End, bbox.width),
        (true, LabelAlign::Center) => (TextAnchor::Middle, bbox.width / 2.0),
        (false, _) => (TextAnchor::Middle, 0.0),
    };
    let mut runs = Vec::new();
    for (i, line) in bbox.lines.iter().enumerate() {
        for (j, run) in line.runs.iter().enumerate() {
            // HTML-mode runs keep their own spaces from `html_to_lines`;
            // SVG-mode runs are single words joined by one space.
            let text = if j == 0 || html {
                run.text.clone()
            } else {
                format!(" {}", run.text)
            };
            let font = FontSpec {
                weight: run.bold.then_some(FontWeight::Bold),
                style: run.italic.then_some(FontStyle::Italic),
                ..FontSpec::default()
            };
            let (x, dy) = if j == 0 {
                if html {
                    (
                        Some(line_x),
                        Some(Length::Px(if i == 0 {
                            bbox.baseline
                        } else {
                            bbox.line_height
                        })),
                    )
                } else {
                    (Some(0.0), Some(Length::Em(if i == 0 { 1.0 } else { 1.1 })))
                }
            } else {
                (None, None)
            };
            runs.push(TextRun {
                text,
                x,
                dy,
                font,
                fill: None,
            });
        }
    }
    Some(Text {
        common: Common {
            class: Some("nodeLabel".to_string()),
            style: Style {
                fill: Some(Paint::Color(fill)),
                ..Style::default()
            },
            ..Common::default()
        },
        x: line_x,
        y: 0.0,
        anchor: Some(anchor),
        // HTML lines sit on the baseline Chrome computes from the rounded
        // face metrics (`LabelBox::baseline`); `central` would resolve from
        // the unrounded ones and land up to a pixel off.
        baseline: Some(if html {
            Baseline::Alphabetic
        } else {
            Baseline::Auto
        }),
        font: bbox.font.clone(),
        runs,
    })
}

/// Puts the shape elements first and the label last, as `insert(...,
/// ':first-child')` followed by the earlier label insertion does.
pub fn assemble(mut shape_svg: Group, shape: Vec<Element>, label: Group) -> Group {
    shape_svg.children = shape;
    shape_svg.children.push(Element::Group(label));
    shape_svg
}

/// Assembles the node group and reports the bounds `updateNodeBounds`
/// wrote on `node`.
pub fn done(out: LabelHelperOut, shape: Vec<Element>, node: &Node) -> ShapeOut {
    let LabelHelperOut {
        shape_svg, label, ..
    } = out;
    ShapeOut {
        group: assemble(shape_svg, shape, label),
        width: node.width(),
        height: node.height(),
    }
}

/// `updateNodeBounds(node, element)` with the element's geometric box.
pub fn update_node_bounds(node: &mut Node, bbox: BBox) {
    node.width = Some(bbox.width);
    node.height = Some(bbox.height);
}

/// A bare node group (`parent.insert('g').attr('class', ...).attr('id',
/// ...)`) for the shapes that do not call `labelHelper`.
pub fn node_group(node: &Node, class: &str) -> Group {
    Group {
        common: Common {
            id: Some(node.dom_id().to_string()),
            class: class_attr(class),
            ..Common::default()
        },
        children: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Geometry

/// `insertPolygonShape(parent, w, h, points)`: a `polygon.label-container`
/// translated by `(-w / 2, h / 2)`.
pub fn insert_polygon_shape(w: f64, h: f64, points: &[Point], style: Style) -> Element {
    Element::Polygon(Polygon {
        common: Common {
            class: Some("label-container".to_string()),
            transform: translate(-w / 2.0, h / 2.0),
            style,
            ..Common::default()
        },
        points: points.to_vec(),
    })
}

/// `createPathFromPoints(points)`: `M` then `L`s then `Z`.
pub fn create_path_from_points(points: &[Point]) -> PathData {
    let mut d = PathData::new();
    for (i, p) in points.iter().enumerate() {
        if i == 0 {
            d.move_to(p.x, p.y);
        } else {
            d.line_to(p.x, p.y);
        }
    }
    d.close();
    d
}

/// The same without the closing `Z` (`path.replace('Z', '')` in the curly
/// brace shapes).
pub fn create_open_path_from_points(points: &[Point]) -> PathData {
    let mut d = PathData::new();
    for (i, p) in points.iter().enumerate() {
        if i == 0 {
            d.move_to(p.x, p.y);
        } else {
            d.line_to(p.x, p.y);
        }
    }
    d
}

/// `generateFullSineWavePoints(x1, y1, x2, y2, amplitude, numCycles)`.
pub fn generate_full_sine_wave_points(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    amplitude: f64,
    num_cycles: f64,
) -> Vec<Point> {
    let steps = 50;
    let delta_x = x2 - x1;
    let delta_y = y2 - y1;
    let cycle_length = delta_x / num_cycles;
    let frequency = (2.0 * std::f64::consts::PI) / cycle_length;
    let mid_y = y1 + delta_y / 2.0;
    (0..=steps)
        .map(|i| {
            let t = f64::from(i) / f64::from(steps);
            let x = x1 + t * delta_x;
            let y = mid_y + amplitude * (frequency * (x - x1)).sin();
            Point::new(x, y)
        })
        .collect()
}

/// `generateCirclePoints(centerX, centerY, radius, numPoints, startAngle,
/// endAngle)` of `util.ts`: note the negated output, which the shapes
/// rely on.
pub fn generate_circle_points(
    center_x: f64,
    center_y: f64,
    radius: f64,
    num_points: usize,
    start_angle: f64,
    end_angle: f64,
) -> Vec<Point> {
    let start = (start_angle * std::f64::consts::PI) / 180.0;
    let end = (end_angle * std::f64::consts::PI) / 180.0;
    let angle_step = (end - start) / (num_points as f64 - 1.0);
    (0..num_points)
        .map(|i| {
            let angle = start + i as f64 * angle_step;
            let x = center_x + radius * angle.cos();
            let y = center_y + radius * angle.sin();
            Point::new(-x, -y)
        })
        .collect()
}

/// `createRoundedRectPathD(x, y, totalWidth, totalHeight, radius)`.
pub fn rounded_rect_path(x: f64, y: f64, w: f64, h: f64, radius: f64) -> PathData {
    let mut d = PathData::new();
    d.move_to(x + radius, y)
        .line_to(x + w - radius, y)
        .arc_to(radius, radius, 0.0, false, true, x + w, y + radius)
        .line_to(x + w, y + h - radius)
        .arc_to(radius, radius, 0.0, false, true, x + w - radius, y + h)
        .line_to(x + radius, y + h)
        .arc_to(radius, radius, 0.0, false, true, x, y + h - radius)
        .line_to(x, y + radius)
        .arc_to(radius, radius, 0.0, false, true, x + radius, y)
        .close();
    d
}

/// An axis-aligned box as `getBBox()` reports it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl BBox {
    pub const EMPTY: BBox = BBox {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    fn from_extents(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> BBox {
        if !min_x.is_finite() {
            return BBox::EMPTY;
        }
        BBox {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        }
    }

    /// The union with another box.
    pub fn union(self, other: BBox) -> BBox {
        if self == BBox::EMPTY {
            return other;
        }
        if other == BBox::EMPTY {
            return self;
        }
        BBox::from_extents(
            self.x.min(other.x),
            self.y.min(other.y),
            (self.x + self.width).max(other.x + other.width),
            (self.y + self.height).max(other.y + other.height),
        )
    }

    pub fn translated(self, dx: f64, dy: f64) -> BBox {
        BBox {
            x: self.x + dx,
            y: self.y + dy,
            ..self
        }
    }
}

/// `getBBox()` of a polygon or polyline.
pub fn points_bbox(points: &[Point]) -> BBox {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for p in points {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    BBox::from_extents(min_x, min_y, max_x, max_y)
}

/// `getBBox()` of a path: line segments exactly, curves and arcs by
/// sampling 64 points, which is well inside the pixel grid.
pub fn path_bbox(d: &PathData) -> BBox {
    let mut pts: Vec<Point> = Vec::new();
    let mut current = Point::default();
    let mut start = Point::default();
    for cmd in d.commands() {
        match *cmd {
            Command::MoveTo(p) => {
                pts.push(p);
                current = p;
                start = p;
            }
            Command::LineTo(p) => {
                pts.push(p);
                current = p;
            }
            Command::QuadTo { control, to } => {
                for i in 0..=64 {
                    let t = f64::from(i) / 64.0;
                    let mt = 1.0 - t;
                    pts.push(Point::new(
                        mt * mt * current.x + 2.0 * mt * t * control.x + t * t * to.x,
                        mt * mt * current.y + 2.0 * mt * t * control.y + t * t * to.y,
                    ));
                }
                current = to;
            }
            Command::CubicTo { c1, c2, to } => {
                for i in 0..=64 {
                    let t = f64::from(i) / 64.0;
                    let mt = 1.0 - t;
                    let a = mt * mt * mt;
                    let b = 3.0 * mt * mt * t;
                    let c = 3.0 * mt * t * t;
                    let e = t * t * t;
                    pts.push(Point::new(
                        a * current.x + b * c1.x + c * c2.x + e * to.x,
                        a * current.y + b * c1.y + c * c2.y + e * to.y,
                    ));
                }
                current = to;
            }
            Command::ArcTo {
                rx,
                ry,
                x_axis_rotation,
                large_arc,
                sweep,
                to,
            } => {
                pts.extend(sample_arc(
                    current,
                    to,
                    rx,
                    ry,
                    x_axis_rotation,
                    large_arc,
                    sweep,
                ));
                current = to;
            }
            Command::Close => {
                current = start;
            }
        }
    }
    points_bbox(&pts)
}

/// Samples an SVG elliptical arc (endpoint parameterisation, SVG 1.1
/// appendix F.6.5) at 64 points. A degenerate radius yields the chord.
fn sample_arc(
    from: Point,
    to: Point,
    rx: f64,
    ry: f64,
    rotation_deg: f64,
    large_arc: bool,
    sweep: bool,
) -> Vec<Point> {
    if rx == 0.0 || ry == 0.0 || from == to {
        return vec![from, to];
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    let phi = rotation_deg.to_radians();
    let (sin_phi, cos_phi) = phi.sin_cos();
    let dx2 = (from.x - to.x) / 2.0;
    let dy2 = (from.y - to.y) / 2.0;
    let x1p = cos_phi * dx2 + sin_phi * dy2;
    let y1p = -sin_phi * dx2 + cos_phi * dy2;
    let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
    }
    let num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
    let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let coef = if den == 0.0 {
        0.0
    } else {
        (num / den).max(0.0).sqrt() * if large_arc == sweep { -1.0 } else { 1.0 }
    };
    let cxp = coef * (rx * y1p / ry);
    let cyp = coef * -(ry * x1p / rx);
    let cx = cos_phi * cxp - sin_phi * cyp + (from.x + to.x) / 2.0;
    let cy = sin_phi * cxp + cos_phi * cyp + (from.y + to.y) / 2.0;
    let angle = |ux: f64, uy: f64, vx: f64, vy: f64| -> f64 {
        let dot = ux * vx + uy * vy;
        let len = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
        let mut a = (dot / len).clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 {
            a = -a;
        }
        a
    };
    let theta1 = angle(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut dtheta = angle(
        (x1p - cxp) / rx,
        (y1p - cyp) / ry,
        (-x1p - cxp) / rx,
        (-y1p - cyp) / ry,
    );
    let tau = 2.0 * std::f64::consts::PI;
    if !sweep && dtheta > 0.0 {
        dtheta -= tau;
    } else if sweep && dtheta < 0.0 {
        dtheta += tau;
    }
    (0..=64)
        .map(|i| {
            let t = theta1 + dtheta * f64::from(i) / 64.0;
            let (st, ct) = t.sin_cos();
            Point::new(
                cos_phi * rx * ct - sin_phi * ry * st + cx,
                sin_phi * rx * ct + cos_phi * ry * st + cy,
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Element constructors

/// A class attribute; an empty string writes no attribute.
fn class_attr(class: &str) -> Option<String> {
    (!class.is_empty()).then(|| class.to_string())
}

/// A `path` element with a class, a style, and a transform.
pub fn path_element(
    d: PathData,
    class: &str,
    style: Style,
    transform: Vec<TransformOp>,
) -> Element {
    Element::Path(Path {
        common: Common {
            class: class_attr(class),
            transform,
            style,
            ..Common::default()
        },
        d,
    })
}

/// A `rect` element.
#[allow(clippy::too_many_arguments)]
pub fn rect_element(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    rx: Option<f64>,
    ry: Option<f64>,
    class: &str,
    style: Style,
) -> Element {
    Element::Rect(Rect {
        common: Common {
            class: class_attr(class),
            style,
            ..Common::default()
        },
        x,
        y,
        width,
        height,
        rx,
        ry,
    })
}

/// A `circle` element.
pub fn circle_element(cx: f64, cy: f64, r: f64, class: &str, style: Style) -> Element {
    Element::Circle(Circle {
        common: Common {
            class: class_attr(class),
            style,
            ..Common::default()
        },
        cx,
        cy,
        r,
    })
}

/// A `line` element.
pub fn line_element(x1: f64, y1: f64, x2: f64, y2: f64, class: &str, style: Style) -> Element {
    Element::Line(Line {
        common: Common {
            class: class_attr(class),
            style,
            ..Common::default()
        },
        x1,
        y1,
        x2,
        y2,
    })
}

/// A `g` with a class, a transform, and children.
pub fn group_element(class: &str, transform: Vec<TransformOp>, children: Vec<Element>) -> Element {
    Element::Group(Group {
        common: Common {
            class: class_attr(class),
            transform,
            ..Common::default()
        },
        children,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::shapes::FixedMeasurer;

    #[test]
    fn inline_label_fonts_win_over_the_node_rule_and_the_theme() {
        let m = FixedMeasurer::SNAPSHOT;
        let ctx = ShapeCtx::new(&m);
        let mut node = Node::flowchart("A", "A", "rect");
        node.label_font = FontSpec {
            family: Some("\"Open Sans\", sans-serif".to_string()),
            size: Some(14.0),
            ..FontSpec::default()
        };
        node.css_styles = vec!["font-size:20px".to_string(), "font-weight:bold".to_string()];
        let font = label_font(&ctx, &node);
        assert_eq!(font.size, Some(20.0));
        assert_eq!(font.weight, Some(FontWeight::Bold));
        assert_eq!(font.family.as_deref(), Some("\"Open Sans\", sans-serif"));
        node.css_styles.clear();
        assert_eq!(label_font(&ctx, &node).size, Some(14.0));
        node.label_font = FontSpec::default();
        assert_eq!(label_font(&ctx, &node), ctx.font());
    }

    #[test]
    fn font_declarations_parse_like_css() {
        let decls: Declarations = [
            ("font-size", "12px !important"),
            ("font-weight", "600"),
            ("font-style", "italic"),
            ("font-family", "Arial"),
            ("color", "red"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let font = font_from_declarations(&decls);
        assert_eq!(font.size, Some(12.0));
        assert_eq!(font.weight, Some(FontWeight::Number(600)));
        assert_eq!(font.style, Some(FontStyle::Italic));
        assert_eq!(font.family.as_deref(), Some("Arial"));
    }

    #[test]
    fn labels_draw_in_the_font_they_were_measured_in() {
        let m = FixedMeasurer::SNAPSHOT;
        let ctx = ShapeCtx::new(&m);
        let mut node = Node::flowchart("A", "Hello", "rect");
        node.label_font.size = Some(12.0);
        let out = label_helper(&node, &ctx, None, false);
        let Some(Element::Text(text)) = out.label.children.first() else {
            panic!("no label text");
        };
        assert_eq!(text.font.size, Some(12.0));
    }

    #[test]
    fn css_colors_parse() {
        assert_eq!(parse_css_color("none"), Some(Paint::None));
        assert_eq!(
            parse_css_color("#f9f"),
            Some(Paint::Color(Color::rgb(255, 153, 255)))
        );
        assert_eq!(
            parse_css_color("rgba(232,232,232, 0.8)"),
            Some(Paint::Color(Color::rgba(232, 232, 232, 0.8)))
        );
        assert_eq!(
            parse_css_color("red"),
            Some(Paint::Color(Color::rgb(255, 0, 0)))
        );
        assert_eq!(
            parse_css_color("url(#grad)"),
            Some(Paint::Reference("grad".into()))
        );
    }

    #[test]
    fn styles2string_splits_label_and_node_styles() {
        let mut node = Node::new("A", "A", "rect");
        node.css_styles = vec!["fill:#f9f".into(), "color:red".into()];
        node.css_compiled_styles = vec!["stroke:#333".into(), "fill:#000".into()];
        let s = styles2string(&node);
        assert_eq!(s.node_styles.get("fill").map(String::as_str), Some("#f9f"));
        assert_eq!(
            s.node_styles.get_index(0).map(|(k, _)| k.as_str()),
            Some("stroke")
        );
        assert_eq!(s.label_styles.get("color").map(String::as_str), Some("red"));
    }

    #[test]
    fn rounded_rect_bbox_is_the_rect() {
        let d = rounded_rect_path(-50.0, -20.0, 100.0, 40.0, 5.0);
        let b = path_bbox(&d);
        assert!((b.x + 50.0).abs() < 1e-9);
        assert!((b.width - 100.0).abs() < 1e-9);
        assert!((b.height - 40.0).abs() < 1e-9);
    }

    #[test]
    fn arc_bbox_covers_the_bulge() {
        // The cylinder cap: from (0, ry) to (w, ry) via the bottom.
        let mut d = PathData::new();
        d.move_to(0.0, 5.0)
            .arc_to(50.0, 5.0, 0.0, false, false, 100.0, 5.0);
        let b = path_bbox(&d);
        assert!((b.y - 5.0).abs() < 1e-6, "{b:?}");
        assert!((b.height - 5.0).abs() < 1e-3, "{b:?}");
    }

    #[test]
    fn circle_points_are_negated() {
        let pts = generate_circle_points(10.0, 0.0, 5.0, 2, 0.0, 180.0);
        assert!((pts[0].x + 15.0).abs() < 1e-9);
        assert!((pts[1].x + 5.0).abs() < 1e-9);
    }

    #[test]
    fn a_nowrap_label_style_keeps_html_labels_on_one_line() {
        let mut node = Node::flowchart("N", "a long note", "note");
        assert_eq!(html_wrap_width(&node, true, 120.0), Some(120.0));
        node.css_styles = vec!["white-space: nowrap".to_string()];
        assert_eq!(html_wrap_width(&node, true, 120.0), None);
        assert_eq!(html_wrap_width(&node, false, 120.0), Some(120.0));
    }

    #[test]
    fn an_inherited_underline_sits_one_pixel_under_each_baseline() {
        use crate::mermaid::shapes::LabelMeasurer;
        let m = FixedMeasurer::SNAPSHOT;
        let ctx = ShapeCtx::new(&m);
        let mut bbox = m.measure("ab<br>c", &LabelOpts::default());
        bbox.underline = Some(PAGE_TEXT_COLOR);
        let g = build_label_group(&bbox, &ctx, Color::BLACK, "label", Vec::new());
        let rects: Vec<&Rect> = g
            .children
            .iter()
            .filter_map(|e| match e {
                Element::Rect(r) => Some(r),
                _ => None,
            })
            .collect();
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].y, bbox.baseline + 1.0);
        assert_eq!(rects[1].y, bbox.baseline + bbox.line_height + 1.0);
        assert_eq!(rects[0].height, 1.0);
        assert!(matches!(g.children.last(), Some(Element::Text(_))));
    }

    #[test]
    fn a_text_align_label_style_moves_html_lines_to_that_edge() {
        let mut node = Node::flowchart("K", "card", "kanbanItem");
        assert_eq!(label_align(&node), LabelAlign::Center);
        node.css_styles = vec!["text-align: left".to_string()];
        assert_eq!(label_align(&node), LabelAlign::Left);
        let m = FixedMeasurer::SNAPSHOT;
        let ctx = ShapeCtx::new(&m);
        let out = label_helper(&node, &ctx, None, false);
        let text = label_text(&out.bbox, Color::BLACK, true).expect("text");
        assert_eq!(text.anchor, Some(TextAnchor::Start));
        assert_eq!(text.x, 0.0);
        // SVG text keeps the stylesheet's `text-anchor: middle`.
        let svg = label_text(&out.bbox, Color::BLACK, false).expect("text");
        assert_eq!(svg.anchor, Some(TextAnchor::Middle));
    }
}
