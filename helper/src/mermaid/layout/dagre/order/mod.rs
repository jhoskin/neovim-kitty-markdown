//! Port of `src/dagre/order/index.js`: crossing reduction with alternating
//! down and up sweeps, keeping the best layering seen.

pub mod add_subgraph_constraints;
pub mod barycenter;
pub mod build_layer_graph;
pub mod cross_count;
pub mod init_order;
pub mod resolve_conflicts;
pub mod sort;
pub mod sort_subgraph;

use super::LayoutGraph;
use super::graphlib::Graph;
use super::util::{build_layer_matrix, max_rank};
use add_subgraph_constraints::add_subgraph_constraints;
use build_layer_graph::{LayerGraph, Relationship, build_layer_graph};
use cross_count::cross_count;
use init_order::init_order;
use sort_subgraph::sort_subgraph;

/// The constraint graph `cg` shared by the layers of one sweep.
pub type ConstraintGraph = Graph<(), (), ()>;

/// `order(g)`: sets `order` on every ranked node.
pub fn order(g: &mut LayoutGraph) {
    // JS builds `_.range(1, NaN)` here for a graph without ranks and throws
    // on the next `Array(NaN)`; nothing to order, so return instead.
    let Some(max_rank) = max_rank(g) else { return };
    let max_rank = max_rank as i64;
    let mut down_layer_graphs: Vec<LayerGraph> = (1..=max_rank)
        .map(|rank| build_layer_graph(g, rank as f64, Relationship::InEdges))
        .collect();
    let mut up_layer_graphs: Vec<LayerGraph> = (0..max_rank)
        .rev()
        .map(|rank| build_layer_graph(g, rank as f64, Relationship::OutEdges))
        .collect();

    let layering = init_order(g);
    assign_order(g, &layering);

    let mut best_cc = f64::INFINITY;
    let mut best: Option<Vec<Vec<String>>> = None;

    let mut i = 0;
    let mut last_best = 0;
    while last_best < 4 {
        let graphs = if i % 2 == 1 {
            &mut down_layer_graphs
        } else {
            &mut up_layer_graphs
        };
        sweep_layer_graphs(g, graphs, i % 4 >= 2);

        let layering = build_layer_matrix(g);
        let cc = cross_count(g, &layering);
        if cc < best_cc {
            last_best = 0;
            best = Some(layering);
            best_cc = cc;
        }
        i += 1;
        last_best += 1;
    }

    if let Some(best) = best {
        assign_order(g, &best);
    }
}

/// `sweepLayerGraphs(layerGraphs, biasRight)`.
fn sweep_layer_graphs(g: &mut LayoutGraph, layer_graphs: &mut [LayerGraph], bias_right: bool) {
    let mut cg: ConstraintGraph = Graph::default();
    for lg in layer_graphs.iter_mut() {
        let root = lg.graph().root.clone();
        let sorted = sort_subgraph(lg, g, &root, &cg, bias_right);
        // JS writes `order` through the layer graph's node label, which is
        // the layout graph's label for every movable leaf node.
        for (i, v) in sorted.vs.iter().enumerate() {
            if let Some(node) = g.try_node_mut(v) {
                node.order = i;
            }
        }
        add_subgraph_constraints(lg, &mut cg, &sorted.vs);
    }
}

/// `assignOrder(g, layering)`.
fn assign_order(g: &mut LayoutGraph, layering: &[Vec<String>]) {
    for layer in layering {
        for (i, v) in layer.iter().enumerate() {
            g.node_mut(v).order = i;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::graphlib::EdgeKey;
    use crate::mermaid::layout::dagre::util::new_layout_graph;
    use crate::mermaid::layout::dagre::{EdgeLabel, NodeLabel};

    fn node(g: &mut LayoutGraph, v: &str, rank: f64) {
        g.set_node(
            v,
            NodeLabel {
                rank: Some(rank),
                ..NodeLabel::default()
            },
        );
    }

    fn edge(g: &mut LayoutGraph, v: &str, w: &str) {
        g.set_edge(&EdgeKey::new(v, w), EdgeLabel::default());
    }

    #[test]
    fn does_not_add_crossings_to_a_tree_structure() {
        let mut g = new_layout_graph();
        node(&mut g, "a", 1.0);
        for v in ["b", "e"] {
            node(&mut g, v, 2.0);
        }
        for v in ["c", "d", "f"] {
            node(&mut g, v, 3.0);
        }
        edge(&mut g, "a", "b");
        edge(&mut g, "b", "c");
        edge(&mut g, "b", "d");
        edge(&mut g, "a", "e");
        edge(&mut g, "e", "f");
        order(&mut g);
        let layering = build_layer_matrix(&g);
        assert_eq!(cross_count(&g, &layering), 0.0);
    }

    #[test]
    fn can_solve_a_simple_graph_with_one_crossing() {
        let mut g = new_layout_graph();
        for v in ["a", "d"] {
            node(&mut g, v, 1.0);
        }
        for v in ["b", "f", "e"] {
            node(&mut g, v, 2.0);
        }
        node(&mut g, "c", 3.0);
        edge(&mut g, "a", "b");
        edge(&mut g, "b", "c");
        edge(&mut g, "d", "e");
        edge(&mut g, "e", "c");
        edge(&mut g, "a", "f");
        order(&mut g);
        let layering = build_layer_matrix(&g);
        assert_eq!(cross_count(&g, &layering), 0.0);
    }

    #[test]
    fn can_minimize_crossings() {
        let mut g = new_layout_graph();
        node(&mut g, "a", 1.0);
        for v in ["b", "e", "g"] {
            node(&mut g, v, 2.0);
        }
        for v in ["c", "d", "h"] {
            node(&mut g, v, 3.0);
        }
        node(&mut g, "f", 4.0);
        edge(&mut g, "a", "b");
        edge(&mut g, "b", "c");
        edge(&mut g, "b", "d");
        edge(&mut g, "a", "e");
        edge(&mut g, "e", "c");
        edge(&mut g, "a", "g");
        edge(&mut g, "g", "h");
        edge(&mut g, "h", "f");
        order(&mut g);
        let layering = build_layer_matrix(&g);
        assert!(cross_count(&g, &layering) <= 1.0);
    }

    #[test]
    fn orders_are_a_permutation_per_rank() {
        let mut g = new_layout_graph();
        for (v, r) in [("a", 0.0), ("b", 0.0), ("c", 1.0), ("d", 1.0), ("e", 1.0)] {
            node(&mut g, v, r);
        }
        edge(&mut g, "a", "e");
        edge(&mut g, "b", "c");
        edge(&mut g, "b", "d");
        order(&mut g);
        let layering = build_layer_matrix(&g);
        assert_eq!(layering.len(), 2);
        assert_eq!(layering[0].len(), 2);
        assert_eq!(layering[1].len(), 3);
    }
}
