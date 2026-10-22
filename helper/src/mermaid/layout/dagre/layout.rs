//! Port of `src/dagre/layout.js`: the pipeline in `runLayout` and the helpers
//! that live in the same file.

use thiserror::Error;

use super::util::{
    add_dummy_node, as_non_compound_graph, build_layer_matrix, intersect_rect, js_max, js_min,
    new_layout_graph, normalize_ranks, remove_empty_ranks,
};
use super::{
    Dummy, EdgeLabel, GraphLabel, LayoutGraph, NodeLabel, Point, SelfEdge, acyclic,
    add_border_segments, coordinate_system, nesting_graph, normalize, order, parent_dummy_chains,
    position, rank,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LayoutError {
    /// `util.intersectRect` threw: an edge end sits on the centre of its
    /// node, so no boundary point exists.
    #[error("dagre: no intersection for edge {v} -> {w}: the edge starts at the node centre")]
    NoIntersection { v: String, w: String },
}

/// `layout(g)`: lays out `g` in place. Input attributes are read from the
/// labels, output goes to `x`, `y`, `points`, and the graph `width` and
/// `height`; a compound node also gets its `width` and `height`.
pub fn layout(g: &mut LayoutGraph) -> Result<(), LayoutError> {
    let mut layout_graph = build_layout_graph(g);
    run_layout(&mut layout_graph)?;
    update_input_graph(g, &layout_graph);
    Ok(())
}

/// `runLayout(g, time)`.
fn run_layout(g: &mut LayoutGraph) -> Result<(), LayoutError> {
    make_space_for_edge_labels(g);
    remove_self_edges(g);
    acyclic::run(g);
    nesting_graph::run(g);
    rank_non_compound(g);
    inject_edge_label_proxies(g);
    remove_empty_ranks(g);
    nesting_graph::cleanup(g);
    normalize_ranks(g);
    assign_rank_min_max(g);
    remove_edge_label_proxies(g);
    normalize::run(g);
    parent_dummy_chains::parent_dummy_chains(g);
    add_border_segments::add_border_segments(g);
    order::order(g);
    insert_self_edges(g);
    coordinate_system::adjust(g);
    position::position(g);
    position_self_edges(g);
    remove_border_nodes(g);
    normalize::undo(g);
    fixup_edge_label_coords(g);
    coordinate_system::undo(g);
    translate_graph(g);
    assign_node_intersects(g)?;
    reverse_points_for_reversed_edges(g);
    acyclic::undo(g);
    Ok(())
}

/// `rank(util.asNonCompoundGraph(g))`: JS shares the labels, the port
/// copies `rank` back.
fn rank_non_compound(g: &mut LayoutGraph) {
    let mut ng = as_non_compound_graph(g);
    rank::rank(&mut ng);
    for v in ng.nodes() {
        let r = ng.node(&v).rank;
        g.node_mut(&v).rank = r;
    }
}

/// `updateInputGraph(inputGraph, layoutGraph)`: copies the whitelisted
/// output attributes.
fn update_input_graph(input: &mut LayoutGraph, layout: &LayoutGraph) {
    for v in input.nodes() {
        let Some(layout_label) = layout.try_node(&v) else {
            continue;
        };
        let has_children = !layout.children(Some(&v)).is_empty();
        let (x, y, w, h) = (
            layout_label.x,
            layout_label.y,
            layout_label.width,
            layout_label.height,
        );
        let input_label = input.node_mut(&v);
        input_label.x = x;
        input_label.y = y;
        if has_children {
            input_label.width = w;
            input_label.height = h;
        }
    }

    for e in input.edges() {
        let Some(layout_label) = layout.try_edge(&e) else {
            continue;
        };
        let points = layout_label.points.clone();
        let (x, y) = (layout_label.x, layout_label.y);
        let input_label = input.edge_mut(&e);
        input_label.points = points;
        if x.is_some() {
            input_label.x = x;
            input_label.y = y;
        }
    }

    input.graph_mut().width = layout.graph().width;
    input.graph_mut().height = layout.graph().height;
}

/// `buildLayoutGraph(inputGraph)`: a fresh graph with only the attributes
/// that influence layout, defaults applied.
fn build_layout_graph(input: &LayoutGraph) -> LayoutGraph {
    let mut g = new_layout_graph();
    let il = input.graph();
    g.set_graph(GraphLabel {
        rankdir: il.rankdir.clone(),
        align: il.align.clone(),
        acyclicer: il.acyclicer.clone(),
        ranker: il.ranker.clone(),
        nodesep: il.nodesep,
        edgesep: il.edgesep,
        ranksep: il.ranksep,
        marginx: il.marginx,
        marginy: il.marginy,
        ..GraphLabel::default()
    });

    for v in input.nodes() {
        let node = input.node(&v);
        g.set_node(
            &v,
            NodeLabel {
                width: node.width,
                height: node.height,
                ..NodeLabel::default()
            },
        );
        g.set_parent(&v, input.parent(&v).as_deref());
    }

    for e in input.edges() {
        let edge = input.edge(&e);
        g.set_edge(
            &e,
            EdgeLabel {
                minlen: edge.minlen,
                weight: edge.weight,
                width: edge.width,
                height: edge.height,
                labeloffset: edge.labeloffset,
                labelpos: edge.labelpos.clone(),
                ..EdgeLabel::default()
            },
        );
    }

    g
}

/// `makeSpaceForEdgeLabels(g)`: halve `ranksep`, double `minlen`, and pad
/// labelled edges by `labeloffset`. The `rankdir` test is case-sensitive in
/// dagre, so a lower-case `tb` pads the height.
fn make_space_for_edge_labels(g: &mut LayoutGraph) {
    g.graph_mut().ranksep /= 2.0;
    let vertical = {
        let rankdir = &g.graph().rankdir;
        rankdir == "TB" || rankdir == "BT"
    };
    for e in g.edges() {
        let edge = g.edge_mut(&e);
        edge.minlen *= 2.0;
        if edge.labelpos.to_lowercase() != "c" {
            if vertical {
                edge.width += edge.labeloffset;
            } else {
                edge.height += edge.labeloffset;
            }
        }
    }
}

/// `injectEdgeLabelProxies(g)`: a dummy at the mid rank of every edge with a
/// sized label, so `removeEmptyRanks` keeps room for it.
fn inject_edge_label_proxies(g: &mut LayoutGraph) {
    for e in g.edges() {
        let edge = g.edge(&e);
        if !truthy(edge.width) || !truthy(edge.height) {
            continue;
        }
        let v = g.node(&e.v).rank.unwrap_or(f64::NAN);
        let w = g.node(&e.w).rank.unwrap_or(f64::NAN);
        let label = NodeLabel {
            rank: Some((w - v) / 2.0 + v),
            edge_obj: Some(e.clone()),
            ..NodeLabel::default()
        };
        add_dummy_node(g, Dummy::EdgeProxy, label, "_ep");
    }
}

/// `assignRankMinMax(g)`. The JS also sets `graph.maxRank` through
/// `_.max(number, number)`, which is `undefined`; nothing reads it.
fn assign_rank_min_max(g: &mut LayoutGraph) {
    for v in g.nodes() {
        let node = g.node(&v);
        let Some(top) = node.border_top.clone() else {
            continue;
        };
        let bottom = node.border_bottom.clone();
        let min_rank = g.node(&top).rank;
        let max_rank = bottom.and_then(|b| g.node(&b).rank);
        let node = g.node_mut(&v);
        node.min_rank = min_rank;
        node.max_rank = max_rank;
    }
}

/// `removeEdgeLabelProxies(g)`: the proxy rank becomes `labelRank`.
fn remove_edge_label_proxies(g: &mut LayoutGraph) {
    for v in g.nodes() {
        let node = g.node(&v);
        if node.dummy != Some(Dummy::EdgeProxy) {
            continue;
        }
        let rank = node.rank;
        if let Some(e) = node.edge_obj.clone()
            && let Some(edge) = g.try_edge_mut(&e)
        {
            edge.label_rank = rank;
        }
        g.remove_node(&v);
    }
}

/// `translateGraph(g)`: shifts everything to the top-left origin plus the
/// margins and records the graph size.
fn translate_graph(g: &mut LayoutGraph) {
    let mut min_x = f64::INFINITY;
    let mut max_x = 0.0;
    let mut min_y = f64::INFINITY;
    let mut max_y = 0.0;
    let margin_x = or_zero(g.graph().marginx);
    let margin_y = or_zero(g.graph().marginy);

    let mut extremes = |x: f64, y: f64, w: f64, h: f64| {
        min_x = js_min(min_x, x - w / 2.0);
        max_x = js_max(max_x, x + w / 2.0);
        min_y = js_min(min_y, y - h / 2.0);
        max_y = js_max(max_y, y + h / 2.0);
    };

    for v in g.nodes() {
        let n = g.node(&v);
        extremes(n.x, n.y, n.width, n.height);
    }
    for e in g.edges() {
        let edge = g.edge(&e);
        if let Some(x) = edge.x {
            extremes(x, edge.y.unwrap_or(f64::NAN), edge.width, edge.height);
        }
    }

    min_x -= margin_x;
    min_y -= margin_y;

    for v in g.nodes() {
        let node = g.node_mut(&v);
        node.x -= min_x;
        node.y -= min_y;
    }

    for e in g.edges() {
        let edge = g.edge_mut(&e);
        for p in edge.points.iter_mut() {
            p.x -= min_x;
            p.y -= min_y;
        }
        if let Some(x) = edge.x.as_mut() {
            *x -= min_x;
        }
        if let Some(y) = edge.y.as_mut() {
            *y -= min_y;
        }
    }

    let label = g.graph_mut();
    if min_x.is_finite() {
        label.width = max_x - min_x + margin_x;
        label.height = max_y - min_y + margin_y;
    } else {
        // No nodes at all: JS has already thrown in `order`; report an
        // empty drawing instead of `-Infinity`.
        label.width = 0.0;
        label.height = 0.0;
    }
}

/// `assignNodeIntersects(g)`: clips each edge to the boundary of its end
/// nodes.
fn assign_node_intersects(g: &mut LayoutGraph) -> Result<(), LayoutError> {
    for e in g.edges() {
        let node_v = g.node(&e.v).clone();
        let node_w = g.node(&e.w).clone();
        let edge = g.edge(&e);
        let (p1, p2) = if edge.points.is_empty() {
            (
                Point {
                    x: node_w.x,
                    y: node_w.y,
                },
                Point {
                    x: node_v.x,
                    y: node_v.y,
                },
            )
        } else {
            (edge.points[0], edge.points[edge.points.len() - 1])
        };
        let no_intersection = || LayoutError::NoIntersection {
            v: e.v.clone(),
            w: e.w.clone(),
        };
        let start = intersect_rect(&node_v, p1).ok_or_else(no_intersection)?;
        let end = intersect_rect(&node_w, p2).ok_or_else(no_intersection)?;
        let edge = g.edge_mut(&e);
        edge.points.insert(0, start);
        edge.points.push(end);
    }
    Ok(())
}

/// `fixupEdgeLabelCoords(g)`: moves a left or right label beside the edge.
fn fixup_edge_label_coords(g: &mut LayoutGraph) {
    for e in g.edges() {
        let edge = g.edge_mut(&e);
        let Some(x) = edge.x else { continue };
        if edge.labelpos == "l" || edge.labelpos == "r" {
            edge.width -= edge.labeloffset;
        }
        match edge.labelpos.as_str() {
            "l" => edge.x = Some(x - (edge.width / 2.0 + edge.labeloffset)),
            "r" => edge.x = Some(x + (edge.width / 2.0 + edge.labeloffset)),
            _ => {}
        }
    }
}

/// `reversePointsForReversedEdges(g)`.
fn reverse_points_for_reversed_edges(g: &mut LayoutGraph) {
    for e in g.edges() {
        let edge = g.edge_mut(&e);
        if edge.reversed {
            edge.points.reverse();
        }
    }
}

/// `removeBorderNodes(g)`: a compound node takes the box its border dummies
/// span, then the dummies go.
fn remove_border_nodes(g: &mut LayoutGraph) {
    for v in g.nodes() {
        if g.children(Some(&v)).is_empty() {
            continue;
        }
        let node = g.node(&v);
        let corners = (
            node.border_top.clone(),
            node.border_bottom.clone(),
            node.border_left.values().last().cloned(),
            node.border_right.values().last().cloned(),
        );
        let (Some(t), Some(b), Some(l), Some(r)) = corners else {
            continue;
        };
        let (t_y, b_y, l_x, r_x) = (g.node(&t).y, g.node(&b).y, g.node(&l).x, g.node(&r).x);
        let node = g.node_mut(&v);
        node.width = (r_x - l_x).abs();
        node.height = (b_y - t_y).abs();
        node.x = l_x + node.width / 2.0;
        node.y = t_y + node.height / 2.0;
    }

    for v in g.nodes() {
        if g.node(&v).dummy == Some(Dummy::Border) {
            g.remove_node(&v);
        }
    }
}

/// `removeSelfEdges(g)`: self loops leave the graph and wait on their node.
fn remove_self_edges(g: &mut LayoutGraph) {
    for e in g.edges() {
        if e.v != e.w {
            continue;
        }
        let Some(label) = g.remove_edge(&e) else {
            continue;
        };
        g.node_mut(&e.v).self_edges.push(SelfEdge { e, label });
    }
}

/// `insertSelfEdges(g)`: one `selfedge` dummy right of its node in the same
/// layer, with the orders shifted to make room.
fn insert_self_edges(g: &mut LayoutGraph) {
    let layers = build_layer_matrix(g);
    for layer in layers {
        let mut order_shift = 0;
        for (i, v) in layer.iter().enumerate() {
            let node = g.node_mut(v);
            node.order = i + order_shift;
            let rank = node.rank;
            let self_edges = std::mem::take(&mut node.self_edges);
            for self_edge in self_edges {
                order_shift += 1;
                let label = NodeLabel {
                    width: self_edge.label.width,
                    height: self_edge.label.height,
                    rank,
                    order: i + order_shift,
                    edge_obj: Some(self_edge.e),
                    edge_label: Some(self_edge.label),
                    ..NodeLabel::default()
                };
                add_dummy_node(g, Dummy::SelfEdge, label, "_se");
            }
        }
    }
}

/// `positionSelfEdges(g)`: a five-point loop on the right of the node.
fn position_self_edges(g: &mut LayoutGraph) {
    for v in g.nodes() {
        let node = g.node(&v);
        if node.dummy != Some(Dummy::SelfEdge) {
            continue;
        }
        let Some(e) = node.edge_obj.clone() else {
            continue;
        };
        let (node_x, node_y) = (node.x, node.y);
        let self_node = g.node(&e.v);
        let x = self_node.x + self_node.width / 2.0;
        let y = self_node.y;
        let dx = node_x - x;
        let dy = self_node.height / 2.0;
        let Some(label) = g.node_mut(&v).edge_label.take() else {
            continue;
        };
        g.set_edge(&e, label);
        g.remove_node(&v);
        let label = g.edge_mut(&e);
        label.points = vec![
            Point {
                x: x + (2.0 * dx) / 3.0,
                y: y - dy,
            },
            Point {
                x: x + (5.0 * dx) / 6.0,
                y: y - dy,
            },
            Point { x: x + dx, y },
            Point {
                x: x + (5.0 * dx) / 6.0,
                y: y + dy,
            },
            Point {
                x: x + (2.0 * dx) / 3.0,
                y: y + dy,
            },
        ];
        label.x = Some(node_x);
        label.y = Some(node_y);
    }
}

/// JS truthiness of a number: not 0 and not NaN.
fn truthy(x: f64) -> bool {
    x != 0.0 && !x.is_nan()
}

/// `value || 0` for a number.
fn or_zero(x: f64) -> f64 {
    if truthy(x) { x } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::graphlib::EdgeKey;
    use crate::mermaid::layout::dagre::util::reset_unique_ids;

    fn node(g: &mut LayoutGraph, v: &str, width: f64, height: f64) {
        g.set_node(
            v,
            NodeLabel {
                width,
                height,
                ..NodeLabel::default()
            },
        );
    }

    fn edge(g: &mut LayoutGraph, v: &str, w: &str) {
        g.set_edge(&EdgeKey::new(v, w), EdgeLabel::default());
    }

    #[test]
    fn can_layout_a_single_node() {
        reset_unique_ids();
        let mut g = new_layout_graph();
        node(&mut g, "a", 50.0, 100.0);
        assert_eq!(layout(&mut g), Ok(()));
        assert_eq!((g.node("a").x, g.node("a").y), (25.0, 50.0));
        assert_eq!((g.graph().width, g.graph().height), (50.0, 100.0));
    }

    #[test]
    fn can_layout_two_nodes_on_the_same_rank() {
        reset_unique_ids();
        let mut g = new_layout_graph();
        g.graph_mut().nodesep = 200.0;
        node(&mut g, "a", 50.0, 100.0);
        node(&mut g, "b", 75.0, 200.0);
        assert_eq!(layout(&mut g), Ok(()));
        assert_eq!((g.node("a").x, g.node("a").y), (25.0, 100.0));
        assert_eq!(
            (g.node("b").x, g.node("b").y),
            (50.0 + 200.0 + 75.0 / 2.0, 100.0)
        );
    }

    #[test]
    fn can_layout_two_nodes_connected_by_an_edge() {
        reset_unique_ids();
        let mut g = new_layout_graph();
        g.graph_mut().ranksep = 300.0;
        node(&mut g, "a", 50.0, 100.0);
        node(&mut g, "b", 75.0, 200.0);
        edge(&mut g, "a", "b");
        assert_eq!(layout(&mut g), Ok(()));
        assert_eq!((g.node("a").x, g.node("a").y), (75.0 / 2.0, 50.0));
        assert_eq!(
            (g.node("b").x, g.node("b").y),
            (75.0 / 2.0, 100.0 + 300.0 + 100.0)
        );
        let e = g.edge(&EdgeKey::new("a", "b"));
        // The doubled minlen leaves a dummy on the middle rank.
        assert_eq!(
            e.points,
            vec![
                Point { x: 37.5, y: 100.0 },
                Point { x: 37.5, y: 250.0 },
                Point { x: 37.5, y: 400.0 }
            ]
        );
        assert_eq!(e.x, None);
    }

    #[test]
    fn can_layout_an_edge_with_a_label() {
        reset_unique_ids();
        let mut g = new_layout_graph();
        g.graph_mut().ranksep = 300.0;
        node(&mut g, "a", 50.0, 100.0);
        node(&mut g, "b", 75.0, 200.0);
        g.set_edge(
            &EdgeKey::new("a", "b"),
            EdgeLabel {
                width: 60.0,
                height: 70.0,
                labelpos: "c".to_string(),
                ..EdgeLabel::default()
            },
        );
        assert_eq!(layout(&mut g), Ok(()));
        assert_eq!((g.node("a").x, g.node("a").y), (75.0 / 2.0, 50.0));
        assert_eq!(
            (g.node("b").x, g.node("b").y),
            (75.0 / 2.0, 100.0 + 150.0 + 70.0 + 150.0 + 100.0)
        );
        let e = g.edge(&EdgeKey::new("a", "b"));
        assert_eq!(e.x, Some(75.0 / 2.0));
        assert_eq!(e.y, Some(100.0 + 150.0 + 70.0 / 2.0));
    }

    #[test]
    fn lays_out_a_self_loop_and_a_cycle_without_error() {
        reset_unique_ids();
        let mut g = new_layout_graph();
        node(&mut g, "a", 50.0, 50.0);
        node(&mut g, "b", 50.0, 50.0);
        edge(&mut g, "a", "a");
        edge(&mut g, "a", "b");
        edge(&mut g, "b", "a");
        assert_eq!(layout(&mut g), Ok(()));
        let loop_edge = g.edge(&EdgeKey::new("a", "a"));
        assert_eq!(loop_edge.points.len(), 7);
        assert!(loop_edge.x.is_some());
        assert_eq!(g.edge(&EdgeKey::new("b", "a")).points.len(), 3);
        assert!(g.edges().iter().all(|e| !g.edge(e).reversed));
    }

    #[test]
    fn sizes_a_compound_node_around_its_children() {
        reset_unique_ids();
        let mut g = new_layout_graph();
        node(&mut g, "a", 50.0, 50.0);
        node(&mut g, "b", 50.0, 50.0);
        node(&mut g, "sg", 0.0, 0.0);
        g.set_parent("a", Some("sg"));
        g.set_parent("b", Some("sg"));
        edge(&mut g, "a", "b");
        assert_eq!(layout(&mut g), Ok(()));
        let sg = g.node("sg");
        assert!(sg.width >= 50.0);
        assert!(sg.height >= 150.0);
        assert!(g.nodes().iter().all(|v| g.node(v).dummy.is_none()));
    }

    #[test]
    fn an_empty_graph_has_zero_size() {
        let mut g = new_layout_graph();
        assert_eq!(layout(&mut g), Ok(()));
    }
}
