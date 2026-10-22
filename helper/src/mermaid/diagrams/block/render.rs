//! Port of `diagrams/block/blockRenderer.ts` and `renderHelpers.ts`
//! (mermaid 12.2.0), with the block-specific branches of
//! `rendering-elements/shapes/composite.ts` and `blockArrow.ts`.
//!
//! `draw` measures every block through the shape library
//! (`calculateBlockSizes`), runs the column layout, draws the blocks at
//! their positions (`insertBlocks`), draws the edges between block centres
//! (`insertEdges`), and sizes the SVG from the drawn extent as
//! `nodes.getBBox()` does.

use std::collections::HashMap;

use serde_json::Value;

use super::db::{Block, BlockDb, BlockType, Size};
use super::layout;
use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::edges::{self, Edge, EdgeCtx, EdgePaths, Pattern, Thickness};
use crate::mermaid::scene::markers::{MarkerType, insert_markers};
use crate::mermaid::scene::{
    Baseline, Common, Element, FontSpec, Group, Length, Marker, Paint, Point, Scene, Style,
    TransformOp, ViewBox,
};
use crate::mermaid::shapes::util::{
    BBox, Declarations, apply_declarations, base_node_style, done, get_node_classes,
    insert_polygon_shape, label_helper, path_bbox, points_bbox, rect_element, translate,
    update_node_bounds,
};
use crate::mermaid::shapes::{
    self, Intersector, LabelBox, LabelLine, LabelMeasurer, LabelOpts, LabelRun, Look, Node,
    ShapeCtx, ShapeOut,
};
use crate::mermaid::styles::block::BlockStyles;
use crate::text::markdown::{self, Line, WordType};
use crate::text::{FontSpec as MeasureSpec, TextMetrics};

/// mermaid-cli's SVG id; `db.setDiagramId(id)` prefixes every node id.
const SVG_ID: &str = "my-svg";
/// `block.padding` of the config schema.
const DEFAULT_PADDING: f64 = 8.0;
/// `line-height: 1.5` of the HTML label div.
const HTML_LINE_HEIGHT: f64 = 1.5;

// ---------------------------------------------------------------------------
// Label measurement

/// `createText` over the text package: HTML labels through `markdownToHTML`
/// or `nonMarkdownToHTML` and the label div's `getBoundingClientRect`, SVG
/// labels through `createFormattedText` and `getBBox`.
///
/// Temporary: `TextMetrics` implements `LabelMeasurer` on main, and this
/// adapter goes away at integration.
struct Measurer<'a> {
    metrics: &'a TextMetrics<'a>,
    /// The `.label { font-family }` rule, the theme's `fontFamily`.
    family: String,
}

fn runs_of(line: &Line) -> Vec<LabelRun> {
    line.iter()
        .map(|word| LabelRun {
            text: word.content.clone(),
            bold: word.kind == WordType::Strong,
            italic: word.kind == WordType::Em,
        })
        .collect()
}

impl LabelMeasurer for Measurer<'_> {
    fn measure(&self, text: &str, opts: &LabelOpts) -> LabelBox {
        let size = opts.font.size.unwrap_or(16.0);
        let family = opts
            .font
            .family
            .clone()
            .unwrap_or_else(|| self.family.clone());
        let spec = MeasureSpec::new(family, size as f32);
        let max_width = opts.width.filter(|w| w.is_finite()).map(|w| w as f32);
        if opts.html {
            let html = if opts.markdown {
                markdown::markdown_to_html(text)
            } else {
                markdown::non_markdown_to_html(text)
            };
            let lines = markdown::html_to_lines(&html);
            let label = self.metrics.html_label(&lines, &spec, max_width);
            let lines: Vec<LabelLine> = label
                .lines
                .iter()
                .map(|line| LabelLine {
                    runs: runs_of(line),
                    width: f64::from(self.metrics.words_width(line, &spec)),
                })
                .collect();
            LabelBox {
                x: 0.0,
                y: 0.0,
                width: f64::from(label.width).max(opts.min_width),
                height: f64::from(label.height),
                line_height: HTML_LINE_HEIGHT * size,
                lines,
            }
        } else {
            let normalized = markdown::normalize_breaks(text);
            let lines = if opts.markdown {
                markdown::markdown_to_lines(&normalized)
            } else {
                markdown::non_markdown_to_lines(&normalized)
            };
            let label = self
                .metrics
                .svg_label(&lines, &spec, max_width.unwrap_or(f32::INFINITY));
            let lines: Vec<LabelLine> = label
                .lines
                .iter()
                .map(|line| LabelLine {
                    runs: runs_of(line),
                    width: f64::from(self.metrics.compute_width_of_text(line, &spec)),
                })
                .collect();
            let width = f64::from(label.width).max(opts.min_width);
            LabelBox {
                x: -width / 2.0,
                y: 0.0,
                width,
                height: f64::from(label.height),
                line_height: 1.1 * size,
                lines,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Nodes

/// The node `getNodeFromBlock` builds: the shared [`Node`] plus the block
/// diagram fields `directions`, `positioned`, `widthInColumns`, and
/// `style` that `composite.ts` and `blockArrow.ts` read.
struct BlockNode {
    node: Node,
    directions: Vec<String>,
    positioned: bool,
    width_in_columns: i64,
    /// `getStylesFromArray(vertex.styles).style`.
    style: String,
}

/// `getNodeFromBlock(block, db, positioned)`.
fn node_from_block(
    block: &Block,
    db: &BlockDb,
    positioned: bool,
    conf_padding: f64,
    look: Look,
) -> BlockNode {
    let mut class_str = if block.classes.is_empty() {
        "default".to_string()
    } else {
        block.classes.join(" ")
    };
    class_str.push_str(" flowchart-label");
    let css_compiled_styles: Vec<String> = block
        .classes
        .iter()
        .flat_map(|name| {
            db.classes()
                .get(name)
                .map(|class| class.styles.clone())
                .unwrap_or_default()
        })
        .collect();

    let mut radius = 0.0;
    let mut padding = None;
    let shape = match block.kind {
        BlockType::Round => {
            radius = 5.0;
            "rect"
        }
        BlockType::Composite => {
            padding = Some(0.0);
            "composite"
        }
        BlockType::Square => "rect",
        BlockType::Diamond => "question",
        BlockType::Hexagon => "hexagon",
        BlockType::BlockArrow => "block_arrow",
        BlockType::Odd => "rect_left_inv_arrow",
        BlockType::LeanRight => "lean_right",
        BlockType::LeanLeft => "lean_left",
        BlockType::Trapezoid => "trapezoid",
        BlockType::InvTrapezoid => "inv_trapezoid",
        BlockType::RectLeftInvArrow => "rect_left_inv_arrow",
        BlockType::Circle => "circle",
        // Ellipses are broken in mermaid (issue 5976): `shapes.ellipse` is
        // undefined, so `insertNode` throws `No such shape`.
        BlockType::Ellipse => "ellipse",
        BlockType::Stadium => "stadium",
        BlockType::Subroutine => "subroutine",
        BlockType::Cylinder => "cylinder",
        BlockType::Group => "rect",
        BlockType::DoubleCircle => "doublecircle",
        _ => "rect",
    };
    let (style, label_style) = edges::get_styles_from_array(&block.styles);
    let bounds = block.size.unwrap_or_default();
    let width = (bounds.width != 0.0).then_some(bounds.width);
    let height = (bounds.height != 0.0).then_some(bounds.height);
    let node = Node {
        id: block.id.clone(),
        dom_id: Some(format!("{SVG_ID}-{}", block.id)),
        label: Some(block.label.clone()),
        label_style: Some(label_style),
        css_styles: block.styles.clone(),
        css_compiled_styles,
        css_classes: class_str,
        shape: shape.to_string(),
        padding: Some(padding.unwrap_or(conf_padding)),
        width,
        height,
        wrapping_width: Some(width.unwrap_or(f64::INFINITY)),
        rx: Some(radius),
        ry: Some(radius),
        look,
        x: bounds.x,
        y: bounds.y,
        ..Node::default()
    };
    BlockNode {
        node,
        directions: block.directions.clone(),
        positioned,
        width_in_columns: block.width_in_columns(),
        style,
    }
}

/// `key: value; key: value` to declarations.
fn declarations_of(style: &str) -> Declarations {
    let mut map = Declarations::new();
    for part in style.split(';') {
        if let Some((k, v)) = part.split_once(':') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    map
}

/// `composite(parent, node)` of `shapes/composite.ts`: the label from
/// `labelHelper` and a `rect.cluster.composite` that is the block's size
/// when positioned and the label's size plus padding before.
fn draw_composite(bn: &mut BlockNode, ctx: &ShapeCtx, styles: &BlockStyles) -> ShapeOut {
    let node = &mut bn.node;
    // `['node', cssClasses, class].filter(Boolean).join(' ')`; `class` and
    // `cssClasses` carry the same string.
    let classes = format!("node {} {}", node.css_classes, node.css_classes);
    let out = label_helper(node, ctx, Some(&classes), false);
    let node_padding = node.padding();
    let (total_width, total_height) = if bn.positioned {
        (node.width(), node.height())
    } else {
        (
            out.bbox.width + node_padding,
            out.bbox.height + node_padding,
        )
    };
    let (x, y) = if bn.positioned {
        (-total_width / 2.0, -total_height / 2.0)
    } else {
        (
            -out.bbox.width / 2.0 - out.half_padding,
            -out.bbox.height / 2.0 - out.half_padding,
        )
    };
    let mut style = styles.composite_style();
    apply_declarations(&mut style, &declarations_of(&bn.style));
    let rect = rect_element(
        x,
        y,
        total_width,
        total_height,
        node.rx,
        node.ry,
        "basic cluster composite label-container",
        style,
    );
    update_node_bounds(
        node,
        BBox {
            x,
            y,
            width: total_width,
            height: total_height,
        },
    );
    node.intersector = Intersector::Rect;
    done(out, vec![rect], node)
}

/// `expandAndDeduplicateDirections` and `getDirectionKey` of
/// `blockArrow.ts`: the `|`-joined directions in `right, left, up, down`
/// order, or `point`.
fn direction_key(directions: &[String]) -> String {
    let mut has = [false; 4];
    for direction in directions {
        match direction.as_str() {
            "x" => {
                has[0] = true;
                has[1] = true;
            }
            "y" => {
                has[2] = true;
                has[3] = true;
            }
            "right" => has[0] = true,
            "left" => has[1] = true,
            "up" => has[2] = true,
            "down" => has[3] = true,
            _ => {}
        }
    }
    let key: Vec<&str> = ["right", "left", "up", "down"]
        .iter()
        .zip(has)
        .filter_map(|(name, on)| on.then_some(*name))
        .collect();
    if key.is_empty() {
        "point".to_string()
    } else {
        key.join("|")
    }
}

/// `getArrowPoints(directions, bbox, node, totalWidth)`: the outline of
/// a block arrow in the polygon's own coordinates.
pub fn arrow_points(
    directions: &[String],
    bbox: (f64, f64),
    node_padding: f64,
    total_width: Option<f64>,
) -> Vec<Point> {
    let padding = node_padding / 2.0;
    let height = bbox.1 + 4.0 * padding;
    let midpoint = height / 2.0;
    let width = total_width.unwrap_or(bbox.0 + 2.0 * midpoint + 2.0 * padding);
    let p = |x: f64, y: f64| Point::new(x, y);
    let (w, h, m, pad) = (width, height, midpoint, padding);
    match direction_key(directions).as_str() {
        "right|left|up|down" => vec![
            p(0.0, 0.0),
            p(m, 0.0),
            p(w / 2.0, 2.0 * pad),
            p(w - m, 0.0),
            p(w, 0.0),
            p(w, -h / 3.0),
            p(w + 2.0 * pad, -h / 2.0),
            p(w, (-2.0 * h) / 3.0),
            p(w, -h),
            p(w - m, -h),
            p(w / 2.0, -h - 2.0 * pad),
            p(m, -h),
            p(0.0, -h),
            p(0.0, (-2.0 * h) / 3.0),
            p(-2.0 * pad, -h / 2.0),
            p(0.0, -h / 3.0),
        ],
        "right|left|up" => vec![
            p(m, 0.0),
            p(w - m, 0.0),
            p(w, -h / 2.0),
            p(w - m, -h),
            p(m, -h),
            p(0.0, -h / 2.0),
        ],
        "right|left|down" => vec![p(0.0, 0.0), p(m, -h), p(w - m, -h), p(w, 0.0)],
        "right|up|down" => vec![p(0.0, 0.0), p(w, -m), p(w, -h + m), p(0.0, -h)],
        "left|up|down" => vec![p(w, 0.0), p(0.0, -m), p(0.0, -h + m), p(w, -h)],
        "right|left" => vec![
            p(m, 0.0),
            p(m, -pad),
            p(w - m, -pad),
            p(w - m, 0.0),
            p(w, -h / 2.0),
            p(w - m, -h),
            p(w - m, -h + pad),
            p(m, -h + pad),
            p(m, -h),
            p(0.0, -h / 2.0),
        ],
        "up|down" => vec![
            p(w / 2.0, 0.0),
            p(0.0, -pad),
            p(m, -pad),
            p(m, -h + pad),
            p(0.0, -h + pad),
            p(w / 2.0, -h),
            p(w, -h + pad),
            p(w - m, -h + pad),
            p(w - m, -pad),
            p(w, -pad),
        ],
        "right|up" => vec![p(0.0, 0.0), p(w, -m), p(0.0, -h)],
        "right|down" => vec![p(0.0, 0.0), p(w, 0.0), p(0.0, -h)],
        "left|up" => vec![p(w, 0.0), p(0.0, -m), p(w, -h)],
        "left|down" => vec![p(w, 0.0), p(0.0, 0.0), p(w, -h)],
        "right" => vec![
            p(m, -pad),
            p(m, -pad),
            p(w - m, -pad),
            p(w - m, 0.0),
            p(w, -h / 2.0),
            p(w - m, -h),
            p(w - m, -h + pad),
            p(m, -h + pad),
            p(m, -h + pad),
        ],
        "left" => vec![
            p(m, 0.0),
            p(m, -pad),
            p(w - m, -pad),
            p(w - m, -h + pad),
            p(m, -h + pad),
            p(m, -h),
            p(0.0, -h / 2.0),
        ],
        "up" => vec![
            p(m, -pad),
            p(m, -h + pad),
            p(0.0, -h + pad),
            p(w / 2.0, -h),
            p(w, -h + pad),
            p(w - m, -h + pad),
            p(w - m, -pad),
        ],
        "down" => vec![
            p(w / 2.0, 0.0),
            p(0.0, -pad),
            p(m, -pad),
            p(m, -h + pad),
            p(w - m, -h + pad),
            p(w - m, -pad),
            p(w, -pad),
        ],
        _ => vec![p(0.0, 0.0)],
    }
}

/// `block_arrow(parent, node)` of `shapes/blockArrow.ts`.
fn draw_block_arrow(bn: &mut BlockNode, ctx: &ShapeCtx) -> ShapeOut {
    let node = &mut bn.node;
    let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
    let node_padding = node.padding();
    let height = out.bbox.height + 2.0 * node_padding;
    let midpoint = height / 2.0;
    let natural_width = out.bbox.width + 2.0 * midpoint + node_padding;
    let node_width = node.width();
    let is_spanning = bn.positioned && bn.width_in_columns > 1 && node_width > natural_width;
    let width = if is_spanning {
        node_width
    } else {
        natural_width
    };
    let points = arrow_points(
        &bn.directions,
        (out.bbox.width, out.bbox.height),
        node_padding,
        Some(width),
    );
    // `.node polygon { fill: mainBkg; stroke: nodeBorder }` and the block's
    // own `style` attribute on top.
    let mut style = base_node_style(&ctx.vars);
    apply_declarations(&mut style, &declarations_of(&bn.style));
    let polygon = insert_polygon_shape(width, height, &points, style);
    update_node_bounds(node, points_bbox(&points));
    node.intersector = Intersector::Polygon(points);
    done(out, vec![polygon], node)
}

/// `insertNode(elem, node, { config })` for a block: the two block shapes
/// here, everything else through the shape library.
fn draw_node(
    bn: &mut BlockNode,
    ctx: &ShapeCtx,
    styles: &BlockStyles,
) -> Result<ShapeOut, HelperError> {
    match bn.node.shape.as_str() {
        "composite" => Ok(draw_composite(bn, ctx, styles)),
        "block_arrow" => Ok(draw_block_arrow(bn, ctx)),
        _ => shapes::insert_node(&mut bn.node, ctx),
    }
}

// ---------------------------------------------------------------------------
// getBBox

fn translation(ops: &[TransformOp]) -> (f64, f64) {
    ops.iter().fold((0.0, 0.0), |(dx, dy), op| match op {
        TransformOp::Translate(x, y) => (dx + x, dy + y),
        _ => (dx, dy),
    })
}

/// `getBBox()` of an element with `offset` already applied by its
/// ancestors: geometry only, as the browser reports it. Text is left out;
/// every label here sits inside its shape or inside a background rect.
fn element_bbox(element: &Element, offset: (f64, f64)) -> BBox {
    let own = |common: &Common| {
        let (dx, dy) = translation(&common.transform);
        (offset.0 + dx, offset.1 + dy)
    };
    match element {
        Element::Group(g) => {
            let at = own(&g.common);
            g.children
                .iter()
                .fold(BBox::EMPTY, |acc, child| acc.union(element_bbox(child, at)))
        }
        Element::Rect(r) => {
            let (dx, dy) = own(&r.common);
            BBox {
                x: r.x + dx,
                y: r.y + dy,
                width: r.width,
                height: r.height,
            }
        }
        Element::Circle(c) => {
            let (dx, dy) = own(&c.common);
            BBox {
                x: c.cx - c.r + dx,
                y: c.cy - c.r + dy,
                width: 2.0 * c.r,
                height: 2.0 * c.r,
            }
        }
        Element::Ellipse(e) => {
            let (dx, dy) = own(&e.common);
            BBox {
                x: e.cx - e.rx + dx,
                y: e.cy - e.ry + dy,
                width: 2.0 * e.rx,
                height: 2.0 * e.ry,
            }
        }
        Element::Line(l) => {
            let (dx, dy) = own(&l.common);
            points_bbox(&[Point::new(l.x1, l.y1), Point::new(l.x2, l.y2)]).translated(dx, dy)
        }
        Element::Polygon(p) => {
            let (dx, dy) = own(&p.common);
            points_bbox(&p.points).translated(dx, dy)
        }
        Element::Polyline(p) => {
            let (dx, dy) = own(&p.common);
            points_bbox(&p.points).translated(dx, dy)
        }
        Element::Path(p) => {
            let (dx, dy) = own(&p.common);
            path_bbox(&p.d).translated(dx, dy)
        }
        Element::Text(_) | Element::Image(_) | Element::Use(_) | Element::Marker(_) => BBox::EMPTY,
    }
}

// ---------------------------------------------------------------------------
// draw

/// Places the lines of every HTML label on the baseline Chrome gives them.
///
/// `label_text` centres each line in its `line-height: 1.5` box with
/// `dominant-baseline: central`, which resvg resolves from the unrounded
/// font metrics. Chrome's inline layout puts the baseline at
/// `floor((lineHeight - (ascent + descent)) / 2) + ascent` with the rounded
/// metrics (`FontHeight::AddLeading`), about one pixel higher for the
/// default font. Temporary: the fix belongs in `shapes::util::label_text`;
/// this pass goes away when that lands.
fn html_baselines(element: &mut Element, metrics: &TextMetrics<'_>, family: &str) {
    match element {
        Element::Group(g) => {
            for child in &mut g.children {
                html_baselines(child, metrics, family);
            }
        }
        Element::Text(text) if text.baseline == Some(Baseline::Central) => {
            let size = text.font.size.unwrap_or(16.0);
            let spec = MeasureSpec::new(text.font.family.as_deref().unwrap_or(family), size as f32);
            let Some(face) = metrics.fonts().resolve(
                &spec.family,
                crate::text::fonts::Weight(spec.weight),
                spec.style.into(),
            ) else {
                return;
            };
            let (ascent, descent) = metrics.face_ascent_descent(face, size as f32);
            let (ascent, descent) = (f64::from(ascent), f64::from(descent));
            let line_height = HTML_LINE_HEIGHT * size;
            let first = ((line_height - (ascent + descent)) / 2.0).floor() + ascent;
            text.baseline = Some(Baseline::Alphabetic);
            if let Some(run) = text.runs.first_mut()
                && run.dy.is_some()
            {
                run.dy = Some(Length::Px(first));
            }
        }
        _ => {}
    }
}

/// `performOperations`: every block, then its children, depth first.
fn tree_order(db: &BlockDb) -> Vec<usize> {
    fn walk(db: &BlockDb, blocks: &[usize], out: &mut Vec<usize>) {
        for &index in blocks {
            out.push(index);
            walk(db, &db.block(index).children, out);
        }
    }
    let mut out = Vec::new();
    walk(db, db.blocks(), &mut out);
    out
}

/// `addEdgeMarker`'s colored clone: the marker under a new id with the
/// edge's stroke color.
fn colored_marker(defs: &[Marker], reference: &edges::MarkerRef) -> Option<Marker> {
    let color = reference.color?;
    let mut marker = defs.iter().find(|m| m.id == reference.original_id)?.clone();
    marker.id = reference.id.clone();
    let mut paint = Style {
        stroke: Some(Paint::Color(color)),
        ..Style::default()
    };
    if reference.fill {
        paint.fill = Some(Paint::Color(color));
    }
    marker.style.stroke = paint.stroke.clone();
    if reference.fill {
        marker.style.fill = paint.fill.clone();
    }
    Some(marker)
}

/// `blockRenderer.draw(text, id, version, diagObj)`.
pub fn draw(
    db: &mut BlockDb,
    resolved: &Resolved,
    ctx: &RenderCtx<'_>,
) -> Result<Scene, HelperError> {
    let block_conf = &resolved.raw["block"];
    let padding = block_conf
        .get("padding")
        .and_then(Value::as_f64)
        .unwrap_or(DEFAULT_PADDING);
    let use_max_width = block_conf
        .get("useMaxWidth")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let look = Look::parse(&resolved.appearance.look).unwrap_or_default();
    let styles = BlockStyles::resolve(&resolved.theme_variables);
    let measurer = Measurer {
        metrics: &ctx.metrics,
        family: styles.font_family.clone(),
    };
    let mut shape_ctx = ShapeCtx::new(&measurer);
    shape_ctx.vars = styles.vars.clone();
    // `evaluate(getConfig().htmlLabels)`: anything but `false` is on.
    shape_ctx.html_labels = resolved.config.html_labels != Some(false);
    shape_ctx.wrapping_width = *resolved.config.flowchart.wrapping_width;

    // insertMarkers(svg, ['point', 'circle', 'cross'], 'block', id)
    let mut defs = insert_markers(
        &[MarkerType::Point, MarkerType::Circle, MarkerType::Cross],
        "block",
        SVG_ID,
    );
    for marker in &mut defs {
        marker.apply_paint(&styles.marker_style());
    }

    // calculateBlockSizes
    let order = tree_order(db);
    for &index in &order {
        let block = db.block(index).clone();
        let mut bn = node_from_block(&block, db, false, padding, look);
        let out = draw_node(&mut bn, &shape_ctx, &styles)?;
        let bbox = element_bbox(&Element::Group(out.group), (0.0, 0.0));
        db.block_mut(index).size = Some(Size {
            width: bbox.width,
            height: bbox.height,
            x: 0.0,
            y: 0.0,
        });
    }

    let bounds = layout::layout(db, padding);

    // insertBlocks
    let mut children: Vec<Element> = Vec::new();
    let mut positioned: HashMap<String, Node> = HashMap::new();
    let mut drawn = BBox::EMPTY;
    for &index in &order {
        let block = db.block(index).clone();
        if block.kind == BlockType::Space {
            continue;
        }
        let mut bn = node_from_block(&block, db, true, padding, look);
        let out = draw_node(&mut bn, &shape_ctx, &styles)?;
        let mut group = out.group;
        // positionNode(node)
        group.common.transform = translate(bn.node.x, bn.node.y);
        let element = Element::Group(group);
        drawn = drawn.union(element_bbox(&element, (0.0, 0.0)));
        children.push(element);
        positioned.insert(block.id.clone(), bn.node);
    }

    // insertEdges
    let edge_ctx = EdgeCtx {
        shapes: &shape_ctx,
        curve: Some(resolved.config.flowchart.curve.clone()),
        sub_graph_title_total_margin: 0.0,
    };
    let cluster_db = HashMap::new();
    for edge_block in db.edges().to_vec() {
        if edge_block.start.is_empty() || edge_block.end.is_empty() {
            continue;
        }
        let size_of = |id: &str| db.index_of(id).and_then(|i| db.block(i).size);
        let (Some(start), Some(end)) = (size_of(&edge_block.start), size_of(&edge_block.end))
        else {
            continue;
        };
        let points = vec![
            Point::new(start.x, start.y),
            Point::new(
                start.x + (end.x - start.x) / 2.0,
                start.y + (end.y - start.y) / 2.0,
            ),
            Point::new(end.x, end.y),
        ];
        let thickness_class = if edge_block.thickness == "thick" {
            "edge-thickness-thick"
        } else {
            "edge-thickness-normal"
        };
        let pattern_class = if edge_block.pattern == "dotted" {
            "edge-pattern-dotted"
        } else {
            "edge-pattern-solid"
        };
        let mut edge = Edge {
            id: format!("{SVG_ID}-{}", edge_block.id),
            start: edge_block.start.clone(),
            end: edge_block.end.clone(),
            label: Some(edge_block.label.clone()),
            classes: Some(format!(
                "{thickness_class} {pattern_class} flowchart-link LS-a1 LE-b1"
            )),
            arrow_type_start: Some(edge_block.arrow_type_start.clone()),
            arrow_type_end: Some(edge_block.arrow_type_end.clone()),
            thickness: Thickness::parse(&edge_block.thickness),
            pattern: Pattern::parse(&edge_block.pattern),
            look,
            points: points.clone(),
            ..Edge::default()
        };
        // `g.node(id)` carries the block's size and its shape's intersect
        // closure; a block that was never drawn has no closure, and mermaid
        // then skips the clipping, which the default node reproduces.
        let fallback = Node::default();
        let start_node = positioned.get(&edge_block.start).unwrap_or(&fallback);
        let end_node = positioned.get(&edge_block.end).unwrap_or(&fallback);
        let out = edges::insert_edge(
            &edge,
            &cluster_db,
            "block",
            start_node,
            end_node,
            SVG_ID,
            &edge_ctx,
        );
        drawn = drawn.union(element_bbox(&out.path, (0.0, 0.0)));
        children.push(out.path);
        for reference in &out.markers {
            if let Some(marker) = colored_marker(&defs, reference) {
                defs.push(marker);
            }
        }
        if !edge_block.label.is_empty() {
            edge.label_style = vec!["stroke: #333; stroke-width: 1.5px;fill:none;".to_string()];
            let mut labels = edges::insert_edge_label(&mut edge, &shape_ctx);
            edge.x = Some(points[1].x);
            edge.y = Some(points[1].y);
            let paths = EdgePaths {
                original_path: points.clone(),
                updated_path: None,
            };
            edges::position_edge_label(&mut labels, &edge, &paths, 0.0);
            if let Some(label) = labels.label {
                let element = Element::Group(label);
                drawn = drawn.union(element_bbox(&element, (0.0, 0.0)));
                children.push(element);
            }
        }
    }

    // `nodes.node().getBBox()` when finite, else the layout bounds.
    let finite = |v: f64| v.is_finite();
    let (bx, by, bw, bh) = if drawn != BBox::EMPTY && finite(drawn.width) && finite(drawn.height) {
        (drawn.x, drawn.y, drawn.width, drawn.height)
    } else {
        (bounds.x, bounds.y, bounds.width, bounds.height)
    };
    let magic_factor = if bh > 0.0 {
        (0.125 * (bw / bh)).round().max(1.0)
    } else {
        1.0
    };
    let width = bw + 10.0;
    let height = if use_max_width {
        bh + 10.0
    } else {
        bh + magic_factor + 10.0
    };

    if shape_ctx.html_labels {
        for child in &mut children {
            html_baselines(child, &ctx.metrics, &styles.font_family);
        }
    }
    let nodes = Group {
        common: Common {
            class: Some("block".to_string()),
            ..Common::default()
        },
        children,
    };
    let mut scene = Scene::new(width, height);
    scene.view_box = ViewBox::new(bx - 5.0, by - 5.0, bw + 10.0, bh + 10.0);
    scene.font_family = styles.font_family.clone();
    scene.defs = defs.into_iter().map(Element::Marker).collect();
    // The root stylesheet: `font-family`, `font-size`.
    scene.root = Group {
        common: Common::with_style(Style {
            font: FontSpec {
                size: Some(styles.font_size),
                ..FontSpec::default()
            },
            ..Style::default()
        }),
        children: vec![Element::Group(nodes)],
    };
    Ok(scene)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_keys_follow_mermaid_order() {
        let dirs = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(direction_key(&dirs(&["down", "right"])), "right|down");
        assert_eq!(direction_key(&dirs(&["x"])), "right|left");
        assert_eq!(direction_key(&dirs(&["y", "x"])), "right|left|up|down");
        assert_eq!(direction_key(&dirs(&["x", "down"])), "right|left|down");
        assert_eq!(direction_key(&dirs(&[])), "point");
        assert_eq!(direction_key(&dirs(&["right "])), "point");
    }

    #[test]
    fn right_arrow_outline_spans_the_width() {
        let points = arrow_points(&["right".to_string()], (40.0, 24.0), 8.0, None);
        // height 24 + 16 = 40, midpoint 20, width 40 + 40 + 8 = 88.
        assert_eq!(points.len(), 9);
        assert_eq!(points[4], Point::new(88.0, -20.0));
        let bbox = points_bbox(&points);
        assert_eq!(bbox.x, 20.0);
        assert_eq!(bbox.width, 68.0);
        assert_eq!(bbox.height, 40.0);
    }

    #[test]
    fn element_bbox_follows_translations() {
        let rect = rect_element(-10.0, -5.0, 20.0, 10.0, None, None, "", Style::default());
        let group = Group {
            common: Common {
                transform: translate(100.0, 50.0),
                ..Common::default()
            },
            children: vec![rect],
        };
        let bbox = element_bbox(&Element::Group(group), (0.0, 0.0));
        assert_eq!(
            bbox,
            BBox {
                x: 90.0,
                y: 45.0,
                width: 20.0,
                height: 10.0
            }
        );
    }
}
