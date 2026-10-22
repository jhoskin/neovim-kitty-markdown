//! Port of `src/dagre/order/sort-subgraph.js`: sorts the children of one
//! layer-graph node, recursing into subgraphs and pinning border nodes.

use std::collections::HashMap;

use super::super::LayoutGraph;
use super::ConstraintGraph;
use super::barycenter::{Entry, barycenter};
use super::build_layer_graph::LayerGraph;
use super::resolve_conflicts::{ResolvedEntry, resolve_conflicts};
use super::sort::{SortResult, sort};

/// `sortSubgraph(g, v, cg, biasRight)`. `g` is the layout graph the JS
/// layer graph shares its node labels with.
pub fn sort_subgraph(
    lg: &LayerGraph,
    g: &LayoutGraph,
    v: &str,
    cg: &ConstraintGraph,
    bias_right: bool,
) -> SortResult {
    let mut movable = lg.children(Some(v));
    let node = lg.try_node(v);
    let bl = node.and_then(|n| n.border_left.clone());
    let br = node.and_then(|n| n.border_right.clone());
    let mut subgraphs: HashMap<String, SortResult> = HashMap::new();

    if let Some(bl) = &bl {
        movable.retain(|w| w != bl && Some(w) != br.as_ref());
    }

    let mut barycenters = barycenter(lg, g, &movable);
    for entry in barycenters.iter_mut() {
        if !lg.children(Some(&entry.v)).is_empty() {
            let subgraph_result = sort_subgraph(lg, g, &entry.v, cg, bias_right);
            if subgraph_result.barycenter.is_some() {
                merge_barycenters(entry, &subgraph_result);
            }
            subgraphs.insert(entry.v.clone(), subgraph_result);
        }
    }

    let mut entries = resolve_conflicts(&barycenters, cg);
    expand_subgraphs(&mut entries, &subgraphs);

    let mut result = sort(entries, bias_right);

    if let Some(bl) = bl {
        let br = br.unwrap_or_default();
        let mut vs = vec![bl.clone()];
        vs.append(&mut result.vs);
        vs.push(br.clone());
        result.vs = vs;

        let bl_preds = lg.predecessors(&bl);
        if !bl_preds.is_empty() {
            let order_of = |u: Option<&String>| {
                u.and_then(|u| g.try_node(u))
                    .map_or(f64::NAN, |n| n.order as f64)
            };
            let bl_pred_order = order_of(bl_preds.first());
            let br_pred_order = order_of(lg.predecessors(&br).first());
            if result.barycenter.is_none() {
                result.barycenter = Some(0.0);
                result.weight = Some(0.0);
            }
            let b = result.barycenter.unwrap_or(f64::NAN);
            let w = result.weight.unwrap_or(f64::NAN);
            result.barycenter = Some((b * w + bl_pred_order + br_pred_order) / (w + 2.0));
            result.weight = Some(w + 2.0);
        }
    }

    result
}

/// `expandSubgraphs(entries, subgraphs)`: a subgraph id in `vs` becomes the
/// subgraph's sorted nodes.
fn expand_subgraphs(entries: &mut [ResolvedEntry], subgraphs: &HashMap<String, SortResult>) {
    for entry in entries.iter_mut() {
        let mut vs = Vec::new();
        for v in entry.vs.drain(..) {
            match subgraphs.get(&v) {
                Some(sub) => vs.extend(sub.vs.iter().cloned()),
                None => vs.push(v),
            }
        }
        entry.vs = vs;
    }
}

/// `mergeBarycenters(target, other)`.
fn merge_barycenters(target: &mut Entry, other: &SortResult) {
    let ob = other.barycenter.unwrap_or(f64::NAN);
    let ow = other.weight.unwrap_or(f64::NAN);
    match (target.barycenter, target.weight) {
        (Some(tb), tw) => {
            let tw = tw.unwrap_or(f64::NAN);
            target.barycenter = Some((tb * tw + ob * ow) / (tw + ow));
            target.weight = Some(tw + ow);
        }
        (None, _) => {
            target.barycenter = other.barycenter;
            target.weight = other.weight;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::NodeLabel;
    use crate::mermaid::layout::dagre::graphlib::{EdgeKey, Graph, GraphOptions};
    use crate::mermaid::layout::dagre::order::build_layer_graph::{LayerEdge, LayerNode};
    use crate::mermaid::layout::dagre::util::new_layout_graph;

    fn layer_graph() -> LayerGraph {
        Graph::new(GraphOptions {
            compound: true,
            ..GraphOptions::default()
        })
    }

    fn ordered(g: &mut LayoutGraph, v: &str, order: usize) {
        g.set_node(
            v,
            NodeLabel {
                order,
                ..NodeLabel::default()
            },
        );
    }

    #[test]
    fn sorts_a_flat_subgraph_based_on_barycenter() {
        let mut g = new_layout_graph();
        ordered(&mut g, "x", 0);
        ordered(&mut g, "y", 1);
        let mut lg = layer_graph();
        lg.set_edge(&EdgeKey::new("y", "a"), LayerEdge { weight: 1.0 });
        lg.set_edge(&EdgeKey::new("x", "b"), LayerEdge { weight: 1.0 });
        lg.set_parent("a", Some("movable"));
        lg.set_parent("b", Some("movable"));
        let cg: ConstraintGraph = Graph::default();
        let result = sort_subgraph(&lg, &g, "movable", &cg, false);
        assert_eq!(result.vs, vec!["b", "a"]);
        assert_eq!(result.barycenter, Some(0.5));
        assert_eq!(result.weight, Some(2.0));
    }

    #[test]
    fn preserves_the_pos_of_a_node_without_neighbors_in_a_flat_subgraph() {
        let mut g = new_layout_graph();
        ordered(&mut g, "x", 0);
        ordered(&mut g, "y", 1);
        let mut lg = layer_graph();
        lg.set_parent("a", Some("movable"));
        lg.set_parent("b", Some("movable"));
        lg.set_parent("c", Some("movable"));
        lg.set_edge(&EdgeKey::new("y", "a"), LayerEdge { weight: 1.0 });
        lg.set_edge(&EdgeKey::new("x", "c"), LayerEdge { weight: 1.0 });
        let cg: ConstraintGraph = Graph::default();
        let result = sort_subgraph(&lg, &g, "movable", &cg, false);
        assert_eq!(result.vs, vec!["c", "b", "a"]);
    }

    #[test]
    fn sorts_a_nested_subgraph_and_merges_its_barycenter() {
        let mut g = new_layout_graph();
        for (v, o) in [("x", 0), ("y", 1), ("z", 2)] {
            ordered(&mut g, v, o);
        }
        let mut lg = layer_graph();
        lg.set_parent("a", Some("movable"));
        lg.set_parent("sg", Some("movable"));
        lg.set_parent("b", Some("sg"));
        lg.set_parent("c", Some("sg"));
        lg.set_edge(&EdgeKey::new("z", "a"), LayerEdge { weight: 1.0 });
        lg.set_edge(&EdgeKey::new("y", "b"), LayerEdge { weight: 1.0 });
        lg.set_edge(&EdgeKey::new("x", "c"), LayerEdge { weight: 1.0 });
        let cg: ConstraintGraph = Graph::default();
        let result = sort_subgraph(&lg, &g, "movable", &cg, false);
        assert_eq!(result.vs, vec!["c", "b", "a"]);
        assert_eq!(result.barycenter, Some(1.0));
        assert_eq!(result.weight, Some(3.0));
    }

    #[test]
    fn pins_border_nodes_and_folds_their_predecessors_into_the_barycenter() {
        let mut g = new_layout_graph();
        for (v, o) in [("x", 1), ("bl_pred", 0), ("br_pred", 4)] {
            ordered(&mut g, v, o);
        }
        let mut lg = layer_graph();
        lg.set_node(
            "sg",
            LayerNode {
                border_left: Some("bl".into()),
                border_right: Some("br".into()),
            },
        );
        for v in ["bl", "a", "br"] {
            lg.set_parent(v, Some("sg"));
        }
        lg.set_edge(&EdgeKey::new("x", "a"), LayerEdge { weight: 1.0 });
        lg.set_edge(&EdgeKey::new("bl_pred", "bl"), LayerEdge { weight: 1.0 });
        lg.set_edge(&EdgeKey::new("br_pred", "br"), LayerEdge { weight: 1.0 });
        let cg: ConstraintGraph = Graph::default();
        let result = sort_subgraph(&lg, &g, "sg", &cg, false);
        assert_eq!(result.vs, vec!["bl", "a", "br"]);
        assert_eq!(result.barycenter, Some((1.0 + 0.0 + 4.0) / 3.0));
        assert_eq!(result.weight, Some(3.0));
    }
}
