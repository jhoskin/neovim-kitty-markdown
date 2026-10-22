//! Port of `src/dagre/order/init-order.js`: a DFS from the lowest ranks
//! gives every leaf node a first position in its layer.

use indexmap::IndexSet;

use super::super::LayoutGraph;
use super::super::util::{lodash_max, lodash_sort_by};

/// `initOrder(g)`: a layering matrix in first-visit order.
pub fn init_order(g: &LayoutGraph) -> Vec<Vec<String>> {
    let mut visited: IndexSet<String> = IndexSet::new();
    let simple_nodes: Vec<String> = g
        .nodes()
        .into_iter()
        .filter(|v| g.children(Some(v)).is_empty())
        .collect();
    let max_rank = lodash_max(simple_nodes.iter().map(|v| g.node(v).rank));
    let mut layers: Vec<Vec<String>> = vec![Vec::new(); max_rank.map_or(0, |r| r as usize + 1)];

    let mut ordered = simple_nodes.clone();
    lodash_sort_by(&mut ordered, |v| g.node(v).rank);
    for v in ordered {
        dfs(g, &v, &mut visited, &mut layers);
    }
    layers
}

fn dfs(g: &LayoutGraph, v: &str, visited: &mut IndexSet<String>, layers: &mut Vec<Vec<String>>) {
    if visited.contains(v) {
        return;
    }
    visited.insert(v.to_string());
    if let Some(rank) = g.node(v).rank {
        let rank = rank as usize;
        if layers.len() <= rank {
            layers.resize(rank + 1, Vec::new());
        }
        layers[rank].push(v.to_string());
    }
    for w in g.successors(v) {
        dfs(g, &w, visited, layers);
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

    #[test]
    fn assigns_non_overlapping_orders_for_each_rank_in_a_tree() {
        let mut g = new_layout_graph();
        node(&mut g, "a", 0.0);
        node(&mut g, "b", 1.0);
        node(&mut g, "c", 2.0);
        node(&mut g, "d", 2.0);
        node(&mut g, "e", 1.0);
        for (v, w) in [("a", "b"), ("b", "c"), ("b", "d"), ("a", "e")] {
            g.set_edge(&EdgeKey::new(v, w), EdgeLabel::default());
        }
        let layering = init_order(&g);
        assert_eq!(layering, vec![vec!["a"], vec!["b", "e"], vec!["c", "d"]]);
    }

    #[test]
    fn visits_lower_ranks_first_and_skips_subgraph_nodes() {
        let mut g = new_layout_graph();
        node(&mut g, "b", 1.0);
        node(&mut g, "a", 0.0);
        g.set_node("sg", NodeLabel::default());
        g.set_parent("b", Some("sg"));
        let layering = init_order(&g);
        assert_eq!(layering, vec![vec!["a"], vec!["b"]]);
    }
}
