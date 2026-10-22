//! Port of `src/dagre/nesting-graph.js` (Sander, "Layout of Compound Directed
//! Graphs"): a root dummy, border dummies above and below each subgraph, and
//! nesting edges that keep a subgraph's nodes between its borders.

use indexmap::IndexMap;

use super::util::{add_border_node, add_dummy_node, lodash_max};
use super::{Dummy, EdgeLabel, LayoutGraph, NodeLabel};

/// `nestingGraph.run(g)`.
pub fn run(g: &mut LayoutGraph) {
    let root = add_dummy_node(g, Dummy::Root, NodeLabel::default(), "_root");
    let depths = tree_depths(g);
    let height = lodash_max(depths.values().map(|d| Some(*d))).unwrap_or(f64::NAN) - 1.0;
    let node_sep = 2.0 * height + 1.0;

    g.graph_mut().nesting_root = Some(root.clone());

    for e in g.edges() {
        g.edge_mut(&e).minlen *= node_sep;
    }

    let weight = sum_weights(g) + 1.0;

    for child in g.children(None) {
        dfs(g, &root, node_sep, weight, height, &depths, &child);
    }

    g.graph_mut().node_rank_factor = Some(node_sep);
}

#[allow(clippy::too_many_arguments)]
fn dfs(
    g: &mut LayoutGraph,
    root: &str,
    node_sep: f64,
    weight: f64,
    height: f64,
    depths: &IndexMap<String, f64>,
    v: &str,
) {
    let children = g.children(Some(v));
    if children.is_empty() {
        if v != root {
            g.set_edge_vw(root, v, EdgeLabel::weight_minlen(0.0, node_sep));
        }
        return;
    }

    let top = add_border_node(g, "_bt");
    let bottom = add_border_node(g, "_bb");

    g.set_parent(&top, Some(v));
    g.node_mut(v).border_top = Some(top.clone());
    g.set_parent(&bottom, Some(v));
    g.node_mut(v).border_bottom = Some(bottom.clone());

    let depth_v = depths.get(v).copied().unwrap_or(f64::NAN);
    for child in children {
        dfs(g, root, node_sep, weight, height, depths, &child);

        let child_node = g.node(&child);
        let child_top = child_node
            .border_top
            .clone()
            .unwrap_or_else(|| child.clone());
        let child_bottom = child_node
            .border_bottom
            .clone()
            .unwrap_or_else(|| child.clone());
        let this_weight = if child_node.border_top.is_some() {
            weight
        } else {
            2.0 * weight
        };
        let minlen = if child_top != child_bottom {
            1.0
        } else {
            height - depth_v + 1.0
        };

        g.set_edge_vw(
            &top,
            &child_top,
            EdgeLabel {
                weight: this_weight,
                minlen,
                nesting_edge: true,
                ..EdgeLabel::default()
            },
        );
        g.set_edge_vw(
            &child_bottom,
            &bottom,
            EdgeLabel {
                weight: this_weight,
                minlen,
                nesting_edge: true,
                ..EdgeLabel::default()
            },
        );
    }

    if g.parent(v).is_none() {
        g.set_edge_vw(root, &top, EdgeLabel::weight_minlen(0.0, height + depth_v));
    }
}

/// `treeDepths(g)`: depth 1 for root-level nodes, in `g.children()` order.
fn tree_depths(g: &LayoutGraph) -> IndexMap<String, f64> {
    fn walk(g: &LayoutGraph, v: &str, depth: f64, depths: &mut IndexMap<String, f64>) {
        for child in g.children(Some(v)) {
            walk(g, &child, depth + 1.0, depths);
        }
        depths.insert(v.to_string(), depth);
    }
    let mut depths = IndexMap::new();
    for v in g.children(None) {
        walk(g, &v, 1.0, &mut depths);
    }
    depths
}

fn sum_weights(g: &LayoutGraph) -> f64 {
    g.edges().iter().fold(0.0, |acc, e| acc + g.edge(e).weight)
}

/// `nestingGraph.cleanup(g)`: removes the root and every nesting edge.
pub fn cleanup(g: &mut LayoutGraph) {
    if let Some(root) = g.graph_mut().nesting_root.take() {
        g.remove_node(&root);
    }
    for e in g.edges() {
        if g.edge(&e).nesting_edge {
            g.remove_edge(&e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::graphlib::EdgeKey;
    use crate::mermaid::layout::dagre::util::{new_layout_graph, reset_unique_ids};

    #[test]
    fn connects_a_disconnected_graph_through_the_root() {
        reset_unique_ids();
        let mut g = new_layout_graph();
        g.set_node("a", NodeLabel::default());
        g.set_node("b", NodeLabel::default());
        run(&mut g);
        let root = g.graph().nesting_root.clone().unwrap_or_default();
        assert_eq!(root, "_root1");
        assert_eq!(g.successors(&root), vec!["a", "b"]);
        assert_eq!(g.edge(&EdgeKey::new(&root, "a")).minlen, 1.0);
        assert_eq!(g.graph().node_rank_factor, Some(1.0));
        cleanup(&mut g);
        assert_eq!(g.nodes(), vec!["a", "b"]);
        assert!(g.edges().is_empty());
    }

    #[test]
    fn adds_border_nodes_and_nesting_edges_for_a_subgraph() {
        reset_unique_ids();
        let mut g = new_layout_graph();
        g.set_node("a", NodeLabel::default());
        g.set_node("sg", NodeLabel::default());
        g.set_parent("a", Some("sg"));
        g.set_edge(&EdgeKey::new("a", "b"), EdgeLabel::default());
        run(&mut g);
        let sg = g.node("sg");
        let top = sg.border_top.clone().unwrap_or_default();
        let bottom = sg.border_bottom.clone().unwrap_or_default();
        assert!(top.starts_with("_bt"));
        assert!(bottom.starts_with("_bb"));
        assert_eq!(g.parent(&top), Some("sg".to_string()));
        // height = max depth - 1 = 1, so nodeSep = 3 and the edge minlen triples.
        assert_eq!(g.edge(&EdgeKey::new("a", "b")).minlen, 3.0);
        let top_a = g.edge(&EdgeKey::new(&top, "a"));
        assert!(top_a.nesting_edge);
        assert_eq!(top_a.minlen, 1.0);
        assert_eq!(top_a.weight, 2.0 * 2.0);
        let root = g.graph().nesting_root.clone().unwrap_or_default();
        assert_eq!(g.edge(&EdgeKey::new(&root, &top)).minlen, 2.0);
        cleanup(&mut g);
        assert!(!g.has_node(&root));
        assert_eq!(g.edges(), vec![EdgeKey::new("a", "b")]);
    }
}
