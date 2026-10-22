//! Port of `src/dagre/rank/network-simplex.js` (Gansner et al., "A Technique
//! for Drawing Directed Graphs").

use indexmap::IndexSet;

use super::super::LayoutGraph;
use super::super::graphlib::EdgeKey;
use super::super::util::{lodash_min_by, simplify};
use super::feasible_tree::{TreeGraph, TreeNode, feasible_tree};
use super::util::{longest_path, rank_of, slack};

/// `networkSimplex(g)`: ranks on a simplified copy, then written back. JS
/// shares the node label objects between `g` and `simplify(g)`.
pub fn network_simplex(g: &mut LayoutGraph) {
    let mut s = simplify(g);
    longest_path(&mut s);
    let mut t = feasible_tree(&mut s);
    init_low_lim_values(&mut t, None);
    init_cut_values(&mut t, &s);
    while let Some(e) = leave_edge(&t) {
        // JS would throw on `undefined.v` without a candidate.
        let Some(f) = enter_edge(&t, &s, &e) else {
            break;
        };
        exchange_edges(&mut t, &mut s, &e, &f);
    }
    for v in s.nodes() {
        let rank = s.node(&v).rank;
        if let Some(node) = g.try_node_mut(&v) {
            node.rank = rank;
        }
    }
}

/// `initCutValues(t, g)`: cut values for every tree edge, children first.
pub fn init_cut_values(t: &mut TreeGraph, g: &LayoutGraph) {
    let mut vs = t.postorder(&t.nodes());
    vs.pop();
    for v in vs {
        assign_cut_value(t, g, &v);
    }
}

fn assign_cut_value(t: &mut TreeGraph, g: &LayoutGraph, child: &str) {
    let Some(parent) = t.node(child).parent.clone() else {
        return;
    };
    let cut = calc_cut_value(t, g, child);
    t.edge_mut(&EdgeKey::new(child, &parent)).cutvalue = Some(cut);
}

/// `calcCutValue(t, g, child)`: the cut value of the tree edge between
/// `child` and its parent.
pub fn calc_cut_value(t: &TreeGraph, g: &LayoutGraph, child: &str) -> f64 {
    let Some(parent) = t.node(child).parent.clone() else {
        return f64::NAN;
    };
    let mut child_is_tail = true;
    let graph_edge = match g.try_edge_vw(child, &parent) {
        Some(edge) => edge,
        None => {
            child_is_tail = false;
            g.edge(&EdgeKey::new(&parent, child))
        }
    };
    let mut cut_value = graph_edge.weight;
    for e in g.node_edges(child, None) {
        let is_out_edge = e.v == child;
        let other = if is_out_edge { &e.w } else { &e.v };
        if *other == parent {
            continue;
        }
        let points_to_head = is_out_edge == child_is_tail;
        let other_weight = g.edge(&e).weight;
        cut_value += if points_to_head {
            other_weight
        } else {
            -other_weight
        };
        if t.has_edge_vw(child, other) {
            let other_cut_value = t
                .edge(&EdgeKey::new(child, other))
                .cutvalue
                .unwrap_or(f64::NAN);
            cut_value += if points_to_head {
                -other_cut_value
            } else {
                other_cut_value
            };
        }
    }
    cut_value
}

/// `initLowLimValues(tree, root)`: DFS numbering from `root`, default
/// `tree.nodes()[0]`.
pub fn init_low_lim_values(tree: &mut TreeGraph, root: Option<&str>) {
    let root = match root {
        Some(r) => r.to_string(),
        None => match tree.nodes().into_iter().next() {
            Some(r) => r,
            None => return,
        },
    };
    let mut visited = IndexSet::new();
    dfs_assign_low_lim(tree, &mut visited, 1, &root, None);
}

fn dfs_assign_low_lim(
    tree: &mut TreeGraph,
    visited: &mut IndexSet<String>,
    mut next_lim: u64,
    v: &str,
    parent: Option<&str>,
) -> u64 {
    let low = next_lim;
    visited.insert(v.to_string());
    for w in tree.neighbors(v) {
        if !visited.contains(&w) {
            next_lim = dfs_assign_low_lim(tree, visited, next_lim, &w, Some(v));
        }
    }
    let label: &mut TreeNode = tree.node_mut(v);
    label.low = low;
    label.lim = next_lim;
    next_lim += 1;
    label.parent = parent.map(str::to_string);
    next_lim
}

/// `leaveEdge(tree)`: the first tree edge with a negative cut value.
pub fn leave_edge(tree: &TreeGraph) -> Option<EdgeKey> {
    tree.edges()
        .into_iter()
        .find(|e| tree.edge(e).cutvalue.is_some_and(|c| c < 0.0))
}

/// `enterEdge(t, g, edge)`: the non-tree edge with the least slack that
/// reconnects the two halves after `edge` leaves.
pub fn enter_edge(t: &TreeGraph, g: &LayoutGraph, edge: &EdgeKey) -> Option<EdgeKey> {
    let (mut v, mut w) = (edge.v.as_str(), edge.w.as_str());
    if !g.has_edge_vw(v, w) {
        v = edge.w.as_str();
        w = edge.v.as_str();
    }
    let v_label = t.node(v);
    let w_label = t.node(w);
    let mut tail_label = v_label;
    let mut flip = false;
    if v_label.lim > w_label.lim {
        tail_label = w_label;
        flip = true;
    }
    let candidates: Vec<EdgeKey> = g
        .edges()
        .into_iter()
        .filter(|e| {
            flip == is_descendant(t.node(&e.v), tail_label)
                && flip != is_descendant(t.node(&e.w), tail_label)
        })
        .collect();
    lodash_min_by(candidates, |e| Some(slack(g, e)))
}

/// `exchangeEdges(t, g, e, f)`: swaps `e` out of and `f` into the tree, then
/// renumbers, recomputes cut values, and updates ranks.
pub fn exchange_edges(t: &mut TreeGraph, g: &mut LayoutGraph, e: &EdgeKey, f: &EdgeKey) {
    t.remove_edge(&EdgeKey::new(&e.v, &e.w));
    t.set_edge(&EdgeKey::new(&f.v, &f.w), Default::default());
    init_low_lim_values(t, None);
    init_cut_values(t, g);
    update_ranks(t, g);
}

/// `updateRanks(t, g)`: ranks from the tree root down. The JS root test
/// `!g.node(v).parent` always picks the first node, because graph node labels
/// never carry `parent`.
fn update_ranks(t: &TreeGraph, g: &mut LayoutGraph) {
    let Some(root) = t.nodes().into_iter().next() else {
        return;
    };
    let vs = t.preorder(&[root]);
    for v in vs.into_iter().skip(1) {
        let Some(parent) = t.node(&v).parent.clone() else {
            continue;
        };
        let (minlen, flipped) = match g.try_edge_vw(&v, &parent) {
            Some(edge) => (edge.minlen, false),
            None => (g.edge(&EdgeKey::new(&parent, &v)).minlen, true),
        };
        let parent_rank = rank_of(g, &parent);
        g.node_mut(&v).rank = Some(parent_rank + if flipped { minlen } else { -minlen });
    }
}

/// `isDescendant(tree, vLabel, rootLabel)`.
fn is_descendant(v_label: &TreeNode, root_label: &TreeNode) -> bool {
    root_label.low <= v_label.lim && v_label.lim <= root_label.lim
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::EdgeLabel;
    use crate::mermaid::layout::dagre::graphlib::{Graph, GraphOptions};
    use crate::mermaid::layout::dagre::rank::feasible_tree::TreeEdge;
    use crate::mermaid::layout::dagre::util::{new_layout_graph, normalize_ranks};

    fn edge(g: &mut LayoutGraph, v: &str, w: &str, minlen: f64, weight: f64) {
        g.set_edge(
            &EdgeKey::new(v, w),
            EdgeLabel {
                minlen,
                weight,
                ..EdgeLabel::default()
            },
        );
    }

    fn ranks(g: &LayoutGraph) -> Vec<(String, f64)> {
        g.nodes()
            .into_iter()
            .map(|v| {
                let r = g.node(&v).rank.unwrap_or(f64::NAN);
                (v, r)
            })
            .collect()
    }

    fn undirected() -> TreeGraph {
        Graph::new(GraphOptions {
            directed: false,
            ..GraphOptions::default()
        })
    }

    #[test]
    fn can_assign_a_rank_to_a_single_node() {
        let mut g = new_layout_graph();
        g.set_node("a", Default::default());
        network_simplex(&mut g);
        normalize_ranks(&mut g);
        assert_eq!(g.node("a").rank, Some(0.0));
    }

    #[test]
    fn can_assign_ranks_to_a_two_node_connected_graph() {
        let mut g = new_layout_graph();
        edge(&mut g, "a", "b", 1.0, 1.0);
        network_simplex(&mut g);
        normalize_ranks(&mut g);
        assert_eq!(g.node("a").rank, Some(0.0));
        assert_eq!(g.node("b").rank, Some(1.0));
    }

    #[test]
    fn can_assign_ranks_for_a_diamond() {
        let mut g = new_layout_graph();
        edge(&mut g, "a", "b", 1.0, 1.0);
        edge(&mut g, "b", "d", 1.0, 1.0);
        edge(&mut g, "a", "c", 1.0, 1.0);
        edge(&mut g, "c", "d", 1.0, 1.0);
        network_simplex(&mut g);
        normalize_ranks(&mut g);
        assert_eq!(
            ranks(&g),
            vec![
                ("a".into(), 0.0),
                ("b".into(), 1.0),
                ("d".into(), 2.0),
                ("c".into(), 1.0)
            ]
        );
    }

    #[test]
    fn uses_the_minlen_attribute_on_the_edge() {
        let mut g = new_layout_graph();
        edge(&mut g, "a", "b", 1.0, 1.0);
        edge(&mut g, "b", "c", 2.0, 1.0);
        edge(&mut g, "a", "c", 1.0, 1.0);
        network_simplex(&mut g);
        normalize_ranks(&mut g);
        assert_eq!(g.node("a").rank, Some(0.0));
        assert_eq!(g.node("b").rank, Some(1.0));
        assert_eq!(g.node("c").rank, Some(3.0));
    }

    #[test]
    fn can_rank_the_gansner_graph() {
        let mut g = new_layout_graph();
        for (v, w) in [
            ("a", "b"),
            ("b", "c"),
            ("c", "d"),
            ("d", "h"),
            ("a", "e"),
            ("e", "g"),
            ("g", "h"),
            ("a", "f"),
            ("f", "g"),
        ] {
            edge(&mut g, v, w, 1.0, 1.0);
        }
        network_simplex(&mut g);
        normalize_ranks(&mut g);
        let rank = |v: &str| g.node(v).rank.unwrap_or(f64::NAN);
        assert_eq!(rank("a"), 0.0);
        assert_eq!(rank("b"), 1.0);
        assert_eq!(rank("c"), 2.0);
        assert_eq!(rank("d"), 3.0);
        assert_eq!(rank("h"), 4.0);
        assert_eq!(rank("e"), 1.0);
        assert_eq!(rank("f"), 1.0);
        assert_eq!(rank("g"), 2.0);
    }

    #[test]
    fn heavier_edges_are_kept_short() {
        let mut g = new_layout_graph();
        edge(&mut g, "a", "b", 1.0, 1.0);
        edge(&mut g, "b", "c", 1.0, 1.0);
        edge(&mut g, "c", "d", 1.0, 1.0);
        edge(&mut g, "a", "e", 1.0, 1.0);
        edge(&mut g, "e", "d", 1.0, 10.0);
        network_simplex(&mut g);
        normalize_ranks(&mut g);
        let rank = |v: &str| g.node(v).rank.unwrap_or(f64::NAN);
        assert_eq!(rank("d"), 3.0);
        assert_eq!(rank("e"), 2.0);
    }

    #[test]
    fn low_lim_values_number_a_tree_in_postorder() {
        let mut t = undirected();
        t.set_edge_vw("a", "b", TreeEdge::default());
        t.set_edge_vw("b", "c", TreeEdge::default());
        t.set_edge_vw("b", "d", TreeEdge::default());
        init_low_lim_values(&mut t, Some("a"));
        assert_eq!((t.node("a").low, t.node("a").lim), (1, 4));
        assert_eq!((t.node("b").low, t.node("b").lim), (1, 3));
        assert_eq!(t.node("b").parent.as_deref(), Some("a"));
        assert_eq!(t.node("a").parent, None);
        assert_eq!(t.node("c").lim, 1);
        assert_eq!(t.node("d").lim, 2);
    }

    #[test]
    fn cut_value_of_a_single_tree_edge_is_its_weight() {
        let mut g = new_layout_graph();
        edge(&mut g, "a", "b", 1.0, 3.0);
        let mut t = undirected();
        t.set_edge_vw("a", "b", TreeEdge::default());
        init_low_lim_values(&mut t, Some("a"));
        init_cut_values(&mut t, &g);
        assert_eq!(t.edge(&EdgeKey::new("a", "b")).cutvalue, Some(3.0));
    }

    #[test]
    fn cut_value_counts_non_tree_edges_against_the_tree_edge() {
        let mut g = new_layout_graph();
        edge(&mut g, "a", "b", 1.0, 1.0);
        edge(&mut g, "b", "c", 1.0, 1.0);
        edge(&mut g, "a", "c", 1.0, 2.0);
        let mut t = undirected();
        t.set_edge_vw("a", "b", TreeEdge::default());
        t.set_edge_vw("b", "c", TreeEdge::default());
        init_low_lim_values(&mut t, Some("a"));
        init_cut_values(&mut t, &g);
        assert_eq!(t.edge(&EdgeKey::new("a", "b")).cutvalue, Some(3.0));
        assert_eq!(t.edge(&EdgeKey::new("b", "c")).cutvalue, Some(3.0));
        assert!(leave_edge(&t).is_none());
    }

    #[test]
    fn leave_edge_finds_the_first_negative_cut_value() {
        let mut t = undirected();
        t.set_edge_vw(
            "a",
            "b",
            TreeEdge {
                cutvalue: Some(1.0),
            },
        );
        t.set_edge_vw(
            "b",
            "c",
            TreeEdge {
                cutvalue: Some(-2.0),
            },
        );
        t.set_edge_vw(
            "c",
            "d",
            TreeEdge {
                cutvalue: Some(-1.0),
            },
        );
        assert_eq!(leave_edge(&t), Some(EdgeKey::new("b", "c")));
    }
}
