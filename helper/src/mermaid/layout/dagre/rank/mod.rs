//! Port of `src/dagre/rank/index.js`: the `ranker` switch.

pub mod feasible_tree;
pub mod network_simplex;
pub mod util;

use super::LayoutGraph;

/// `rank(g)`: `network-simplex` (default), `tight-tree`, or `longest-path`.
pub fn rank(g: &mut LayoutGraph) {
    match g.graph().ranker.as_deref() {
        Some("tight-tree") => {
            util::longest_path(g);
            feasible_tree::feasible_tree(g);
        }
        Some("longest-path") => util::longest_path(g),
        _ => network_simplex::network_simplex(g),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::EdgeLabel;
    use crate::mermaid::layout::dagre::graphlib::EdgeKey;
    use crate::mermaid::layout::dagre::util::{new_layout_graph, normalize_ranks};

    fn graph() -> LayoutGraph {
        let mut g = new_layout_graph();
        for (v, w) in [("a", "b"), ("b", "c"), ("c", "d"), ("a", "e"), ("e", "d")] {
            g.set_edge(&EdgeKey::new(v, w), EdgeLabel::default());
        }
        g
    }

    #[test]
    fn every_ranker_respects_minlen() {
        for ranker in ["network-simplex", "tight-tree", "longest-path"] {
            let mut g = graph();
            g.graph_mut().ranker = Some(ranker.to_string());
            rank(&mut g);
            normalize_ranks(&mut g);
            for e in g.edges() {
                let v = g.node(&e.v).rank.unwrap_or(f64::NAN);
                let w = g.node(&e.w).rank.unwrap_or(f64::NAN);
                assert!(w - v >= 1.0, "{ranker}: edge {e:?} too short");
            }
        }
    }

    #[test]
    fn longest_path_pushes_e_down_and_network_simplex_pulls_it_up() {
        let mut g = graph();
        g.graph_mut().ranker = Some("longest-path".to_string());
        rank(&mut g);
        normalize_ranks(&mut g);
        assert_eq!(g.node("e").rank, Some(2.0));

        let mut g = graph();
        rank(&mut g);
        normalize_ranks(&mut g);
        assert_eq!(g.node("d").rank, Some(3.0));
        assert!(matches!(g.node("e").rank, Some(r) if (1.0..=2.0).contains(&r)));
    }
}
