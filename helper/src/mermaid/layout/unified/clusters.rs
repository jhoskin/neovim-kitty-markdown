//! Port of `rendering-elements/clusters.js`: the `rect`, `roundedWithTitle`,
//! `noteGroup`, `divider`, and `usecaseSystemBoundary` cluster shapes. The
//! agentflow, kanban, and swimlane shapes draw as `rect` for now.

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

/// `getUsecaseSystemBoundaryGeometry(node, labelBBox)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundaryGeometry {
    pub package: bool,
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
    pub body_y: f64,
    pub body_height: f64,
    pub tab_height: f64,
    pub tab_width: f64,
}

pub fn usecase_boundary_geometry(node: &Node, label: &LabelBox, package: bool) -> BoundaryGeometry {
    let padding = node.padding();
    let horizontal_label_padding = if package { 20.0 } else { padding };
    let tab_height = if package { label.height + 10.0 } else { 0.0 };
    let width = node.width().max(label.width + horizontal_label_padding);
    let height = if package {
        node.height().max(tab_height + padding * 2.0)
    } else {
        node.height()
    };
    let x = node.x - width / 2.0;
    let y = node.y - height / 2.0;
    BoundaryGeometry {
        package,
        width,
        height,
        x,
        y,
        body_y: y + tab_height,
        body_height: height - tab_height,
        tab_height,
        tab_width: if package {
            width.min(80f64.max(label.width + 20.0))
        } else {
            0.0
        },
    }
}

/// `usecaseSystemBoundary(parent, node)`: the use case system boundary, a
/// body rect and, for `type: package`, a title tab. The boundary type
/// travels in the `system-boundary-<type>` class `getData` sets, since
/// the node carries no `boundaryType` field.
pub fn usecase_system_boundary(
    node: &mut Node,
    ctx: &ShapeCtx,
    sub_graph_title_top_margin: f64,
) -> (ClusterOut, f64) {
    let package = node
        .css_classes
        .split_whitespace()
        .any(|c| c == "system-boundary-package");
    let boundary_type = if package { "package" } else { "rect" };
    // `createText(labelEl, node.label, { style, useHtmlLabels, isNode })`:
    // Markdown on, the default width of 200.
    let opts = LabelOpts {
        font: ctx.font(),
        html: ctx.html_labels,
        width: Some(200.0),
        min_width: 0.0,
        markdown: true,
    };
    let bbox = ctx
        .measurer
        .measure(node.label.as_deref().unwrap_or_default(), &opts);
    let g = usecase_boundary_geometry(node, &bbox, package);
    let padding = node.padding();
    let diff = if node.width() <= bbox.width + padding {
        (g.width - node.width()) / 2.0 - padding
    } else {
        -padding
    };
    let styles = styles2string(node);
    let mut style = Style {
        fill: Some(Paint::Color(ctx.vars.cluster_bkg)),
        stroke: Some(Paint::Color(ctx.vars.cluster_border)),
        stroke_width: Some(1.0),
        ..Style::default()
    };
    apply_declarations(&mut style, &styles.node_styles);
    let mut children = Vec::new();
    if package {
        children.push(rect_element(
            g.x,
            g.y,
            g.tab_width,
            g.tab_height,
            None,
            None,
            "boundary-tab system-boundary-package-tab label-container",
            style.clone(),
        ));
    }
    children.push(rect_element(
        g.x,
        g.body_y,
        g.width,
        g.body_height,
        None,
        None,
        "boundary-body label-container",
        style,
    ));
    let title_color = styles
        .label_styles
        .get("color")
        .and_then(|c| crate::mermaid::shapes::util::parse_css_color(c))
        .and_then(|p| match p {
            Paint::Color(c) => Some(c),
            _ => None,
        })
        .unwrap_or(ctx.vars.title_color);
    let (lx, ly) = if package {
        (
            g.x + (g.tab_width - bbox.width) / 2.0,
            g.y + (g.tab_height - bbox.height) / 2.0,
        )
    } else {
        (node.x - bbox.width / 2.0, g.y + sub_graph_title_top_margin)
    };
    let mut label = cluster_label_group(&bbox, ctx, title_color, lx, ly);
    label.common.class = Some("cluster-label system-boundary-title".to_string());
    children.push(Element::Group(label));
    let group = Group {
        common: Common {
            id: Some(node.dom_id().to_string()),
            class: Some(format!(
                "cluster usecase-system-boundary usecase-system-boundary-{boundary_type} {}",
                node.css_classes
            )),
            ..Common::default()
        },
        children,
    };
    node.width = Some(g.width);
    node.height = Some(g.height);
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
        "usecaseSystemBoundary" => usecase_system_boundary(node, ctx, sub_graph_title_top_margin),
        _ => rect(node, ctx, sub_graph_title_top_margin),
    }
}
