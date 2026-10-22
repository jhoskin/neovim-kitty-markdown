//! Port of `rendering-elements/clusters.js`: the `rect`, `roundedWithTitle`,
//! `noteGroup`, and `divider` cluster shapes. The agentflow, kanban,
//! usecase-boundary, and swimlane shapes draw as `rect` for now.

use crate::mermaid::scene::{Color, Common, Element, Group, Paint, Style};
use crate::mermaid::shapes::measure::{LabelBox, LabelOpts};
use crate::mermaid::shapes::util::{
    apply_declarations, build_label_group, rect_element, styles2string, translate,
};
use crate::mermaid::shapes::{Intersector, Node, ShapeCtx};

/// What `insertCluster` returns besides the group: the label box the
/// layout glue reads.
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterOut {
    pub group: Group,
    pub label_bbox: LabelBox,
}

fn measure_cluster_label(node: &Node, ctx: &ShapeCtx) -> LabelBox {
    let label = node.label.clone().unwrap_or_default();
    let markdown = node.label_type.as_deref() == Some("markdown");
    let opts = LabelOpts {
        font: ctx.font(),
        html: ctx.html_labels,
        // `createText` with `width: node.width` for markdown, `createLabel`
        // (infinite width) otherwise.
        width: if markdown { node.width } else { None },
        min_width: 0.0,
        markdown,
    };
    ctx.measurer.measure(&label, &opts)
}

fn cluster_label_group(bbox: &LabelBox, ctx: &ShapeCtx, fill: Color, x: f64, y: f64) -> Group {
    let mut g = build_label_group(bbox, ctx, fill, "cluster-label", translate(x, y));
    if !bbox.is_empty() && !ctx.html_labels {
        // SVG text is centred on x = 0 by `text-anchor: middle`; the HTML
        // box starts at its left edge.
        g.common.transform = translate(x, y);
    }
    g
}

/// `rect(parent, node)`: the flowchart subgraph. Writes `node.diff`,
/// `node.width`, `node.height` like the JS.
pub fn rect(node: &mut Node, ctx: &ShapeCtx, sub_graph_title_top_margin: f64) -> (ClusterOut, f64) {
    let bbox = measure_cluster_label(node, ctx);
    let padding = node.padding();
    let (width, diff) = if node.width() <= bbox.width + padding {
        let w = bbox.width + padding;
        (w, (w - node.width()) / 2.0 - padding)
    } else {
        (node.width(), -padding)
    };
    let height = node.height();
    let x = node.x - width / 2.0;
    let y = node.y - height / 2.0;
    let mut style = Style {
        fill: Some(Paint::Color(ctx.vars.cluster_bkg)),
        stroke: Some(Paint::Color(ctx.vars.cluster_border)),
        stroke_width: Some(1.0),
        ..Style::default()
    };
    apply_declarations(&mut style, &styles2string(node).node_styles);
    let rect = rect_element(x, y, width, height, node.rx, node.ry, "", style);
    let label = cluster_label_group(
        &bbox,
        ctx,
        ctx.vars.title_color,
        node.x - bbox.width / 2.0,
        node.y - node.height() / 2.0 + sub_graph_title_top_margin,
    );
    let group = Group {
        common: Common {
            id: Some(node.dom_id().to_string()),
            class: Some(format!("cluster {}", node.css_classes)),
            ..Common::default()
        },
        children: vec![rect, Element::Group(label)],
    };
    node.width = Some(width);
    node.height = Some(height);
    node.intersector = Intersector::Rect;
    (
        ClusterOut {
            group,
            label_bbox: bbox,
        },
        diff,
    )
}

/// `noteGroup(parent, node)`: an invisible frame around a note.
pub fn note_group(node: &mut Node, _ctx: &ShapeCtx) -> (ClusterOut, f64) {
    let rect = rect_element(
        node.x - node.width() / 2.0,
        node.y - node.height() / 2.0,
        node.width(),
        node.height(),
        node.rx,
        node.ry,
        "",
        Style {
            fill: Some(Paint::None),
            ..Style::default()
        },
    );
    let group = Group {
        common: Common {
            id: Some(node.dom_id().to_string()),
            class: Some("note-cluster".to_string()),
            ..Common::default()
        },
        children: vec![rect],
    };
    node.intersector = Intersector::Rect;
    (
        ClusterOut {
            group,
            label_bbox: LabelBox::default(),
        },
        0.0,
    )
}

/// `roundedWithTitle(parent, node)`: the state diagram composite with a
/// title strip.
pub fn rounded_with_title(node: &mut Node, ctx: &ShapeCtx) -> (ClusterOut, f64) {
    let bbox = measure_cluster_label(node, ctx);
    let padding = node.padding();
    let (width, diff) = if node.width() <= bbox.width + padding {
        let w = bbox.width + padding;
        (w, (w - node.width()) / 2.0 - padding)
    } else {
        (node.width(), -padding)
    };
    let height = node.height();
    let inner_height = node.height() - bbox.height - 6.0;
    let x = node.x - width / 2.0;
    let y = node.y - height / 2.0;
    node.width = Some(width);
    let inner_y = node.y - node.height() / 2.0 + bbox.height + 2.0;
    let is_alt = node.css_classes.contains("statediagram-cluster-alt");
    let outer = rect_element(
        x,
        y,
        width,
        height,
        None,
        None,
        "outer",
        Style {
            fill: Some(Paint::Color(ctx.vars.composite_title_background)),
            stroke: Some(Paint::Color(ctx.vars.node_border)),
            stroke_width: Some(ctx.vars.stroke_width),
            ..Style::default()
        },
    );
    let inner = rect_element(
        x,
        inner_y,
        width,
        inner_height,
        None,
        None,
        "inner",
        Style {
            fill: Some(Paint::Color(if is_alt {
                ctx.vars.alt_background
            } else {
                ctx.vars.composite_background
            })),
            stroke: Some(Paint::Color(ctx.vars.node_border)),
            stroke_width: Some(ctx.vars.stroke_width),
            ..Style::default()
        },
    );
    let label = cluster_label_group(
        &bbox,
        ctx,
        ctx.vars.title_color,
        node.x - bbox.width / 2.0,
        y + 1.0 - if ctx.html_labels { 0.0 } else { 3.0 },
    );
    let group = Group {
        common: Common {
            id: Some(node.dom_id().to_string()),
            class: Some(node.css_classes.clone()),
            ..Common::default()
        },
        children: vec![
            Element::Group(Group {
                common: Common::default(),
                children: vec![outer],
            }),
            Element::Group(label),
            inner,
        ],
    };
    node.height = Some(height);
    node.label_bbox = Some(bbox.dims());
    node.intersector = Intersector::Rect;
    (
        ClusterOut {
            group,
            label_bbox: bbox,
        },
        diff,
    )
}

/// `divider(parent, node)`: a concurrency region of a composite state.
pub fn divider(node: &mut Node, ctx: &ShapeCtx) -> (ClusterOut, f64) {
    let width = node.width();
    let diff = -node.padding();
    let height = node.height();
    let x = node.x - width / 2.0;
    let y = node.y - height / 2.0;
    let rect = rect_element(
        x,
        y,
        width,
        height,
        None,
        None,
        "divider",
        Style {
            fill: Some(Paint::Color(ctx.vars.alt_background)),
            stroke: Some(Paint::Color(ctx.vars.node_border)),
            stroke_width: Some(ctx.vars.stroke_width),
            ..Style::default()
        },
    );
    let group = Group {
        common: Common {
            id: Some(node.dom_id().to_string()),
            class: Some(node.css_classes.clone()),
            ..Common::default()
        },
        children: vec![Element::Group(Group {
            common: Common::default(),
            children: vec![rect],
        })],
    };
    node.width = Some(width);
    node.height = Some(height);
    node.intersector = Intersector::Rect;
    (
        ClusterOut {
            group,
            label_bbox: LabelBox::default(),
        },
        diff,
    )
}

/// `insertCluster(elem, node)`: picks the cluster shape by `node.shape`
/// (default `rect`). Returns the group and `node.diff`.
pub fn insert_cluster(
    node: &mut Node,
    ctx: &ShapeCtx,
    sub_graph_title_top_margin: f64,
) -> (ClusterOut, f64) {
    match node.shape.as_str() {
        "roundedWithTitle" => rounded_with_title(node, ctx),
        "noteGroup" => note_group(node, ctx),
        "divider" => divider(node, ctx),
        _ => rect(node, ctx, sub_graph_title_top_margin),
    }
}
