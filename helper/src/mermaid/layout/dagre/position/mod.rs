//! Port of `src/dagre/position/index.js`.

pub mod bk;

use super::LayoutGraph;
use super::util::{as_non_compound_graph, build_layer_matrix, lodash_max};

/// `position(g)`: y from the layering, x from Brandes-Köpf. JS runs on a
/// label-sharing non-compound view; the port copies `x` and `y` back.
pub fn position(g: &mut LayoutGraph) {
    let mut ng = as_non_compound_graph(g);
    position_y(&mut ng);
    for (v, x) in bk::position_x(&ng) {
        ng.node_mut(&v).x = x;
    }
    for v in ng.nodes() {
        let (x, y) = {
            let n = ng.node(&v);
            (n.x, n.y)
        };
        let target = g.node_mut(&v);
        target.x = x;
        target.y = y;
    }
}

/// `positionY(g)`: each layer is centred on the tallest node in it.
fn position_y(g: &mut LayoutGraph) {
    let layering = build_layer_matrix(g);
    let rank_sep = g.graph().ranksep;
    let mut prev_y = 0.0;
    for layer in layering {
        let max_height =
            lodash_max(layer.iter().map(|v| Some(g.node(v).height))).unwrap_or(f64::NAN);
        for v in &layer {
            g.node_mut(v).y = prev_y + max_height / 2.0;
        }
        prev_y += max_height + rank_sep;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::NodeLabel;
    use crate::mermaid::layout::dagre::util::new_layout_graph;

    #[test]
    fn position_y_centres_layers_on_their_tallest_node() {
        let mut g = new_layout_graph();
        g.graph_mut().ranksep = 10.0;
        for (v, rank, order, height) in [
            ("a", 0.0, 0, 20.0),
            ("b", 0.0, 1, 40.0),
            ("c", 1.0, 0, 10.0),
        ] {
            g.set_node(
                v,
                NodeLabel {
                    rank: Some(rank),
                    order,
                    height,
                    width: 10.0,
                    ..NodeLabel::default()
                },
            );
        }
        position_y(&mut g);
        assert_eq!(g.node("a").y, 20.0);
        assert_eq!(g.node("b").y, 20.0);
        assert_eq!(g.node("c").y, 40.0 + 10.0 + 5.0);
    }

    #[test]
    fn position_skips_compound_nodes() {
        let mut g = new_layout_graph();
        g.set_node(
            "a",
            NodeLabel {
                rank: Some(0.0),
                width: 10.0,
                height: 10.0,
                ..NodeLabel::default()
            },
        );
        g.set_node("sg", NodeLabel::default());
        g.set_parent("a", Some("sg"));
        position(&mut g);
        assert_eq!(g.node("a").y, 5.0);
        assert_eq!(g.node("sg").y, 0.0);
    }
}
