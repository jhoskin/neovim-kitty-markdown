//! Port of `src/dagre/rank/feasible-tree.js`.

use super::super::LayoutGraph;
use super::super::graphlib::{EdgeKey, Graph, GraphOptions};
use super::super::util::lodash_min_by;
use super::util::slack;

/// Node label of the tight tree: `low`, `lim`, and `parent` from
/// `network-simplex.js dfsAssignLowLim`.
#[derive(Clone, Debug, Default)]
pub struct TreeNode {
    pub low: u64,
    pub lim: u64,
    pub parent: Option<String>,
}

/// Edge label of the tight tree: `cutvalue` from `network-simplex.js`.
#[derive(Clone, Debug, Default)]
pub struct TreeEdge {
    pub cutvalue: Option<f64>,
}

/// The undirected tight tree.
pub type TreeGraph = Graph<TreeNode, TreeEdge, ()>;

/// `feasibleTree(g)`: grows a spanning tree of tight edges from
/// `g.nodes()[0]`, shifting ranks to make the next edge tight each round.
/// A disconnected graph stops early where JS throws on `undefined.v`.
pub fn feasible_tree(g: &mut LayoutGraph) -> TreeGraph {
    let mut t: TreeGraph = Graph::new(GraphOptions {
        directed: false,
        ..GraphOptions::default()
    });
    let Some(start) = g.nodes().into_iter().next() else {
        return t;
    };
    let size = g.node_count();
    t.set_node(&start, TreeNode::default());
    while tight_tree(&mut t, g) < size {
        let Some(edge) = find_min_slack_edge(&t, g) else {
            break;
        };
        let delta = if t.has_node(&edge.v) {
            slack(g, &edge)
        } else {
            -slack(g, &edge)
        };
        shift_ranks(&t, g, delta);
    }
    t
}

/// `tightTree(t, g)`: adds every node reachable over tight edges and returns
/// the tree size.
fn tight_tree(t: &mut TreeGraph, g: &LayoutGraph) -> usize {
    for v in t.nodes() {
        dfs(t, g, &v);
    }
    t.node_count()
}

fn dfs(t: &mut TreeGraph, g: &LayoutGraph, v: &str) {
    for e in g.node_edges(v, None) {
        let w = if v == e.v { &e.w } else { &e.v };
        // `!slack(g, e)` is true for 0 and for NaN.
        let s = slack(g, &e);
        if !t.has_node(w) && (s == 0.0 || s.is_nan()) {
            t.set_node(w, TreeNode::default());
            t.set_edge(&EdgeKey::new(v, w), TreeEdge::default());
            dfs(t, g, w);
        }
    }
}

/// `findMinSlackEdge(t, g)`: the first edge with one endpoint in the tree
/// and the smallest slack.
fn find_min_slack_edge(t: &TreeGraph, g: &LayoutGraph) -> Option<EdgeKey> {
    lodash_min_by(g.edges(), |e| {
        if t.has_node(&e.v) != t.has_node(&e.w) {
            Some(slack(g, e))
        } else {
            None
        }
    })
}

fn shift_ranks(t: &TreeGraph, g: &mut LayoutGraph, delta: f64) {
    for v in t.nodes() {
        if let Some(rank) = g.node_mut(&v).rank.as_mut() {
            *rank += delta;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::EdgeLabel;
    use crate::mermaid::layout::dagre::util::new_layout_graph;

    fn ranked(g: &mut LayoutGraph, v: &str, rank: f64) {
        g.node_mut(v).rank = Some(rank);
    }

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
    fn creates_a_tree_for_a_trivial_input_graph() {
        let mut g = new_layout_graph();
        edge(&mut g, "a", "b", 1.0);
        ranked(&mut g, "a", 0.0);
        ranked(&mut g, "b", 1.0);
        let t = feasible_tree(&mut g);
        assert_eq!(g.node("b").rank, Some(1.0));
        assert_eq!(t.node_count(), 2);
        assert!(t.has_edge_vw("a", "b"));
    }

    #[test]
    fn tightens_the_graph_when_the_tree_is_not_spanning() {
        let mut g = new_layout_graph();
        edge(&mut g, "a", "b", 1.0);
        edge(&mut g, "a", "c", 1.0);
        edge(&mut g, "b", "d", 1.0);
        edge(&mut g, "c", "d", 1.0);
        ranked(&mut g, "a", 0.0);
        ranked(&mut g, "b", 1.0);
        ranked(&mut g, "c", 2.0);
        ranked(&mut g, "d", 3.0);
        let t = feasible_tree(&mut g);
        assert_eq!(t.node_count(), 4);
        for e in g.edges() {
            assert_eq!(slack(&g, &e), 0.0, "edge {e:?} is not tight");
        }
    }

    #[test]
    fn shifts_the_tree_side_that_holds_the_min_slack_edge_tail() {
        let mut g = new_layout_graph();
        edge(&mut g, "a", "b", 1.0);
        ranked(&mut g, "a", 0.0);
        ranked(&mut g, "b", 4.0);
        let t = feasible_tree(&mut g);
        assert_eq!(t.node_count(), 2);
        assert_eq!(g.node("b").rank, Some(4.0));
        assert_eq!(g.node("a").rank, Some(3.0));
    }
}
