//! Port of `src/dagre/coordinate-system.js`: dagre lays out top to bottom
//! and maps the result to the requested `rankdir` afterwards.

use super::LayoutGraph;

/// `coordinateSystem.adjust(g)`: swap node sizes for horizontal layouts.
pub fn adjust(g: &mut LayoutGraph) {
    let rank_dir = g.graph().rankdir.to_lowercase();
    if rank_dir == "lr" || rank_dir == "rl" {
        swap_width_height(g);
    }
}

/// `coordinateSystem.undo(g)`: flip and transpose coordinates back.
pub fn undo(g: &mut LayoutGraph) {
    let rank_dir = g.graph().rankdir.to_lowercase();
    if rank_dir == "bt" || rank_dir == "rl" {
        reverse_y(g);
    }
    if rank_dir == "lr" || rank_dir == "rl" {
        swap_xy(g);
        swap_width_height(g);
    }
}

fn swap_width_height(g: &mut LayoutGraph) {
    for v in g.nodes() {
        let node = g.node_mut(&v);
        std::mem::swap(&mut node.width, &mut node.height);
    }
    for e in g.edges() {
        let edge = g.edge_mut(&e);
        std::mem::swap(&mut edge.width, &mut edge.height);
    }
}

fn reverse_y(g: &mut LayoutGraph) {
    for v in g.nodes() {
        let node = g.node_mut(&v);
        node.y = -node.y;
    }
    for e in g.edges() {
        let edge = g.edge_mut(&e);
        for p in edge.points.iter_mut() {
            p.y = -p.y;
        }
        if let Some(y) = edge.y.as_mut() {
            *y = -*y;
        }
    }
}

fn swap_xy(g: &mut LayoutGraph) {
    for v in g.nodes() {
        let node = g.node_mut(&v);
        std::mem::swap(&mut node.x, &mut node.y);
    }
    for e in g.edges() {
        let edge = g.edge_mut(&e);
        for p in edge.points.iter_mut() {
            std::mem::swap(&mut p.x, &mut p.y);
        }
        if edge.x.is_some() {
            std::mem::swap(&mut edge.x, &mut edge.y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::graphlib::EdgeKey;
    use crate::mermaid::layout::dagre::util::new_layout_graph;
    use crate::mermaid::layout::dagre::{EdgeLabel, NodeLabel, Point};

    fn graph(rankdir: &str) -> LayoutGraph {
        let mut g = new_layout_graph();
        g.graph_mut().rankdir = rankdir.to_string();
        g.set_node(
            "a",
            NodeLabel {
                width: 10.0,
                height: 20.0,
                x: 1.0,
                y: 2.0,
                ..NodeLabel::default()
            },
        );
        g.set_edge(
            &EdgeKey::new("a", "a"),
            EdgeLabel {
                width: 3.0,
                height: 4.0,
                points: vec![Point { x: 5.0, y: 6.0 }],
                x: Some(7.0),
                y: Some(8.0),
                ..EdgeLabel::default()
            },
        );
        g
    }

    #[test]
    fn tb_leaves_everything_alone() {
        let mut g = graph("TB");
        adjust(&mut g);
        undo(&mut g);
        assert_eq!((g.node("a").x, g.node("a").y), (1.0, 2.0));
        assert_eq!((g.node("a").width, g.node("a").height), (10.0, 20.0));
    }

    #[test]
    fn bt_negates_y() {
        let mut g = graph("BT");
        undo(&mut g);
        assert_eq!(g.node("a").y, -2.0);
        let e = g.edge(&EdgeKey::new("a", "a"));
        assert_eq!(e.points[0].y, -6.0);
        assert_eq!(e.y, Some(-8.0));
    }

    #[test]
    fn lr_swaps_sizes_then_transposes() {
        let mut g = graph("LR");
        adjust(&mut g);
        assert_eq!((g.node("a").width, g.node("a").height), (20.0, 10.0));
        undo(&mut g);
        assert_eq!((g.node("a").width, g.node("a").height), (10.0, 20.0));
        assert_eq!((g.node("a").x, g.node("a").y), (2.0, 1.0));
        let e = g.edge(&EdgeKey::new("a", "a"));
        assert_eq!(e.points[0], Point { x: 6.0, y: 5.0 });
        assert_eq!((e.x, e.y), (Some(8.0), Some(7.0)));
    }

    #[test]
    fn rl_negates_then_transposes() {
        let mut g = graph("rl");
        adjust(&mut g);
        undo(&mut g);
        assert_eq!((g.node("a").x, g.node("a").y), (-2.0, 1.0));
    }
}
