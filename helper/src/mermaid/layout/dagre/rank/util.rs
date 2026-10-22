//! Port of `src/dagre/rank/util.js`.

use indexmap::IndexSet;

use super::super::LayoutGraph;
use super::super::graphlib::EdgeKey;
use super::super::util::lodash_min;

/// `longestPath(g)`: every node gets the lowest rank its out-edges allow;
/// sinks get 0. Ranks are not normalized.
pub fn longest_path(g: &mut LayoutGraph) {
    let mut visited: IndexSet<String> = IndexSet::new();
    for v in g.sources() {
        dfs(g, &v, &mut visited);
    }
}

fn dfs(g: &mut LayoutGraph, v: &str, visited: &mut IndexSet<String>) -> f64 {
    if visited.contains(v) {
        // A visited node without a rank only exists inside a cycle, where JS
        // yields `undefined` and the arithmetic below turns into NaN.
        return g.node(v).rank.unwrap_or(f64::NAN);
    }
    visited.insert(v.to_string());
    let mut lengths = Vec::new();
    for e in g.out_edges(v, None) {
        let minlen = g.edge(&e).minlen;
        let rank = dfs(g, &e.w, visited) - minlen;
        lengths.push(Some(rank));
    }
    let rank = lodash_min(lengths).unwrap_or(0.0);
    g.node_mut(v).rank = Some(rank);
    rank
}

/// `slack(g, e)`: the edge length minus its `minlen`.
pub fn slack(g: &LayoutGraph, e: &EdgeKey) -> f64 {
    rank_of(g, &e.w) - rank_of(g, &e.v) - g.edge(e).minlen
}

/// The rank of a node that the ranker has already reached; `undefined`
/// would make the JS arithmetic NaN.
pub fn rank_of(g: &LayoutGraph, v: &str) -> f64 {
    g.node(v).rank.unwrap_or(f64::NAN)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::EdgeLabel;
    use crate::mermaid::layout::dagre::util::{new_layout_graph, normalize_ranks};

    fn edge(g: &mut LayoutGraph, v: &str, w: &str, minlen: f64) {
        g.set_edge(
            &EdgeKey::new(v, w),
            EdgeLabel {
                minlen,
                ..EdgeLabel::default()
            },
        );
    }

    #[test]
    fn a_single_node_gets_rank_zero() {
        let mut g = new_layout_graph();
        g.set_node("a", Default::default());
        longest_path(&mut g);
        assert_eq!(g.node("a").rank, Some(0.0));
    }

    #[test]
    fn ranks_respect_minlen_and_push_nodes_down() {
        let mut g = new_layout_graph();
        edge(&mut g, "a", "b", 1.0);
        edge(&mut g, "b", "c", 2.0);
        edge(&mut g, "a", "c", 1.0);
        longest_path(&mut g);
        normalize_ranks(&mut g);
        assert_eq!(g.node("a").rank, Some(0.0));
        assert_eq!(g.node("b").rank, Some(1.0));
        assert_eq!(g.node("c").rank, Some(3.0));
        assert_eq!(slack(&g, &EdgeKey::new("a", "b")), 0.0);
        assert_eq!(slack(&g, &EdgeKey::new("a", "c")), 2.0);
    }
}
