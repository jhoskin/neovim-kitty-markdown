//! Port of `src/dagre/add-border-segments.js`: one left and one right border
//! dummy per rank a subgraph spans, chained with weight-1 edges.

use super::util::add_dummy_node;
use super::{BorderType, Dummy, EdgeLabel, LayoutGraph, NodeLabel};

/// `addBorderSegments(g)`.
pub fn add_border_segments(g: &mut LayoutGraph) {
    for v in g.children(None) {
        dfs(g, &v);
    }
}

fn dfs(g: &mut LayoutGraph, v: &str) {
    let children = g.children(Some(v));
    for child in children {
        dfs(g, &child);
    }

    let node = g.node(v);
    let Some(min_rank) = node.min_rank else {
        return;
    };
    let max_rank = node.max_rank.unwrap_or(f64::NAN) + 1.0;
    {
        let node = g.node_mut(v);
        node.border_left.clear();
        node.border_right.clear();
    }
    let mut rank = min_rank;
    while rank < max_rank {
        add_border_node(g, BorderType::BorderLeft, "_bl", v, rank);
        add_border_node(g, BorderType::BorderRight, "_br", v, rank);
        rank += 1.0;
    }
}

/// `addBorderNode(g, prop, prefix, sg, sgNode, rank)`.
fn add_border_node(g: &mut LayoutGraph, prop: BorderType, prefix: &str, sg: &str, rank: f64) {
    let label = NodeLabel {
        rank: Some(rank),
        border_type: Some(prop),
        ..NodeLabel::default()
    };
    let prev = chain(g.node(sg), prop).get(&((rank - 1.0) as i64)).cloned();
    let curr = add_dummy_node(g, Dummy::Border, label, prefix);
    chain_mut(g.node_mut(sg), prop).insert(rank as i64, curr.clone());
    g.set_parent(&curr, Some(sg));
    if let Some(prev) = prev {
        g.set_edge_vw(&prev, &curr, EdgeLabel::with_weight(1.0));
    }
}

fn chain(node: &NodeLabel, prop: BorderType) -> &std::collections::BTreeMap<i64, String> {
    match prop {
        BorderType::BorderLeft => &node.border_left,
        BorderType::BorderRight => &node.border_right,
    }
}

fn chain_mut(
    node: &mut NodeLabel,
    prop: BorderType,
) -> &mut std::collections::BTreeMap<i64, String> {
    match prop {
        BorderType::BorderLeft => &mut node.border_left,
        BorderType::BorderRight => &mut node.border_right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::graphlib::EdgeKey;
    use crate::mermaid::layout::dagre::util::new_layout_graph;

    #[test]
    fn adds_a_border_pair_per_rank_and_chains_them() {
        let mut g = new_layout_graph();
        g.set_node(
            "sg",
            NodeLabel {
                min_rank: Some(1.0),
                max_rank: Some(2.0),
                ..NodeLabel::default()
            },
        );
        g.set_node("a", NodeLabel::default());
        g.set_parent("a", Some("sg"));
        add_border_segments(&mut g);
        let sg = g.node("sg");
        assert_eq!(sg.border_left.len(), 2);
        assert_eq!(sg.border_right.len(), 2);
        let bl1 = sg.border_left[&1].clone();
        let bl2 = sg.border_left[&2].clone();
        assert_eq!(g.node(&bl1).rank, Some(1.0));
        assert_eq!(g.node(&bl1).dummy, Some(Dummy::Border));
        assert_eq!(g.node(&bl1).border_type, Some(BorderType::BorderLeft));
        assert_eq!(g.parent(&bl1), Some("sg".to_string()));
        assert_eq!(g.edge(&EdgeKey::new(&bl1, &bl2)).weight, 1.0);
        assert_eq!(g.children(Some("sg")).len(), 5);
    }

    #[test]
    fn skips_nodes_without_a_rank_span() {
        let mut g = new_layout_graph();
        g.set_node("a", NodeLabel::default());
        add_border_segments(&mut g);
        assert_eq!(g.node_count(), 1);
    }
}
