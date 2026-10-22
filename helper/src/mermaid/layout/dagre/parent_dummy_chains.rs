//! Port of `src/dagre/parent-dummy-chains.js`: each dummy of a long edge is
//! parented to the subgraph that spans its rank along the path between the
//! edge's endpoints through their lowest common ancestor.

use indexmap::IndexMap;

use super::LayoutGraph;

/// `parentDummyChains(g)`.
pub fn parent_dummy_chains(g: &mut LayoutGraph) {
    let postorder_nums = postorder(g);

    for head in g.graph().dummy_chains.clone() {
        let mut v = head;
        let Some(edge_obj) = g.node(&v).edge_obj.clone() else {
            continue;
        };
        let (path, lca) = find_path(g, &postorder_nums, &edge_obj.v, &edge_obj.w);
        let mut path_idx = 0;
        let mut path_v: Option<String> = path.first().cloned().flatten();
        let mut ascending = true;

        while v != edge_obj.w {
            let node_rank = g.node(&v).rank.unwrap_or(f64::NAN);

            if ascending {
                loop {
                    path_v = path.get(path_idx).cloned().flatten();
                    if path_v == lca {
                        break;
                    }
                    let below = match &path_v {
                        Some(p) => g.node(p).max_rank.is_some_and(|m| m < node_rank),
                        None => false,
                    };
                    if !below {
                        break;
                    }
                    path_idx += 1;
                }
                if path_v == lca {
                    ascending = false;
                }
            }

            if !ascending {
                while path_idx + 1 < path.len() {
                    path_v = path[path_idx + 1].clone();
                    let covers = match &path_v {
                        Some(p) => g.node(p).min_rank.is_some_and(|m| m <= node_rank),
                        None => false,
                    };
                    if !covers {
                        break;
                    }
                    path_idx += 1;
                }
                path_v = path.get(path_idx).cloned().flatten();
            }

            g.set_parent(&v, path_v.as_deref());
            // JS indexes `g.successors(v)[0]`; a chain dummy has one successor.
            let Some(next) = g.successors(&v).into_iter().next() else {
                break;
            };
            v = next;
        }
    }
}

/// `findPath(g, postorderNums, v, w)`: the ancestors of `v` up to the LCA,
/// then the ancestors of `w` down from the LCA. `None` stands for the graph
/// root, which `g.parent` reports as `undefined`.
fn find_path(
    g: &LayoutGraph,
    nums: &IndexMap<String, (usize, usize)>,
    v: &str,
    w: &str,
) -> (Vec<Option<String>>, Option<String>) {
    let (v_low, v_lim) = nums.get(v).copied().unwrap_or((0, 0));
    let (w_low, w_lim) = nums.get(w).copied().unwrap_or((0, 0));
    let low = v_low.min(w_low);
    let lim = v_lim.max(w_lim);

    let mut v_path = Vec::new();
    let mut parent = Some(v.to_string());
    loop {
        parent = parent.as_deref().and_then(|p| g.parent(p));
        v_path.push(parent.clone());
        let keep_going = match &parent {
            Some(p) => {
                let (p_low, p_lim) = nums.get(p).copied().unwrap_or((0, 0));
                p_low > low || lim > p_lim
            }
            None => false,
        };
        if !keep_going {
            break;
        }
    }
    let lca = parent;

    let mut w_path = Vec::new();
    let mut parent = Some(w.to_string());
    loop {
        parent = parent.as_deref().and_then(|p| g.parent(p));
        if parent == lca {
            break;
        }
        w_path.push(parent.clone());
    }

    w_path.reverse();
    v_path.extend(w_path);
    (v_path, lca)
}

/// `postorder(g)`: `{low, lim}` per node from a DFS over the hierarchy.
fn postorder(g: &LayoutGraph) -> IndexMap<String, (usize, usize)> {
    fn walk(
        g: &LayoutGraph,
        v: &str,
        lim: &mut usize,
        result: &mut IndexMap<String, (usize, usize)>,
    ) {
        let low = *lim;
        for child in g.children(Some(v)) {
            walk(g, &child, lim, result);
        }
        result.insert(v.to_string(), (low, *lim));
        *lim += 1;
    }
    let mut result = IndexMap::new();
    let mut lim = 0;
    for v in g.children(None) {
        walk(g, &v, &mut lim, &mut result);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::graphlib::EdgeKey;
    use crate::mermaid::layout::dagre::util::new_layout_graph;
    use crate::mermaid::layout::dagre::{Dummy, EdgeLabel, NodeLabel};

    fn node(g: &mut LayoutGraph, v: &str, rank: f64) {
        g.set_node(
            v,
            NodeLabel {
                rank: Some(rank),
                ..NodeLabel::default()
            },
        );
    }

    fn subgraph(g: &mut LayoutGraph, v: &str, min: f64, max: f64) {
        g.set_node(
            v,
            NodeLabel {
                min_rank: Some(min),
                max_rank: Some(max),
                ..NodeLabel::default()
            },
        );
    }

    fn dummy(g: &mut LayoutGraph, v: &str, rank: f64, e: &EdgeKey) {
        g.set_node(
            v,
            NodeLabel {
                rank: Some(rank),
                dummy: Some(Dummy::Edge),
                edge_obj: Some(e.clone()),
                ..NodeLabel::default()
            },
        );
    }

    #[test]
    fn dummies_of_an_edge_into_a_subgraph_join_that_subgraph() {
        let mut g = new_layout_graph();
        node(&mut g, "a", 0.0);
        node(&mut g, "b", 3.0);
        subgraph(&mut g, "sg", 2.0, 3.0);
        g.set_parent("b", Some("sg"));
        let e = EdgeKey::new("a", "b");
        dummy(&mut g, "d1", 1.0, &e);
        dummy(&mut g, "d2", 2.0, &e);
        g.set_edge(&EdgeKey::new("a", "d1"), EdgeLabel::default());
        g.set_edge(&EdgeKey::new("d1", "d2"), EdgeLabel::default());
        g.set_edge(&EdgeKey::new("d2", "b"), EdgeLabel::default());
        g.graph_mut().dummy_chains = vec!["d1".to_string()];
        parent_dummy_chains(&mut g);
        assert_eq!(g.parent("d1"), None);
        assert_eq!(g.parent("d2"), Some("sg".to_string()));
    }

    #[test]
    fn dummies_leaving_a_subgraph_stay_while_it_spans_their_rank() {
        let mut g = new_layout_graph();
        node(&mut g, "a", 0.0);
        node(&mut g, "b", 3.0);
        subgraph(&mut g, "sg", 0.0, 1.0);
        g.set_parent("a", Some("sg"));
        let e = EdgeKey::new("a", "b");
        dummy(&mut g, "d1", 1.0, &e);
        dummy(&mut g, "d2", 2.0, &e);
        g.set_edge(&EdgeKey::new("a", "d1"), EdgeLabel::default());
        g.set_edge(&EdgeKey::new("d1", "d2"), EdgeLabel::default());
        g.set_edge(&EdgeKey::new("d2", "b"), EdgeLabel::default());
        g.graph_mut().dummy_chains = vec!["d1".to_string()];
        parent_dummy_chains(&mut g);
        assert_eq!(g.parent("d1"), Some("sg".to_string()));
        assert_eq!(g.parent("d2"), None);
    }
}
