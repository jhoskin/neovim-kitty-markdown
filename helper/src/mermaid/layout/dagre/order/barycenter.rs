//! Port of `src/dagre/order/barycenter.js`.

use super::super::LayoutGraph;
use super::build_layer_graph::LayerGraph;

/// `{v, barycenter, weight}`; a node without in-edges has neither.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub v: String,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

/// `barycenter(g, movable)`: the weighted mean position of each node's
/// neighbours in the layer graph. `order` lives on the layout graph's
/// labels, which the JS layer graph shares.
pub fn barycenter(lg: &LayerGraph, g: &LayoutGraph, movable: &[String]) -> Vec<Entry> {
    movable
        .iter()
        .map(|v| {
            let in_v = lg.in_edges(v, None);
            if in_v.is_empty() {
                return Entry {
                    v: v.clone(),
                    barycenter: None,
                    weight: None,
                };
            }
            let mut sum = 0.0;
            let mut weight = 0.0;
            for e in in_v {
                let edge_weight = lg.edge(&e).weight;
                let order = g.try_node(&e.v).map_or(f64::NAN, |n| n.order as f64);
                sum += edge_weight * order;
                weight += edge_weight;
            }
            Entry {
                v: v.clone(),
                barycenter: Some(sum / weight),
                weight: Some(weight),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::NodeLabel;
    use crate::mermaid::layout::dagre::graphlib::{EdgeKey, Graph, GraphOptions};
    use crate::mermaid::layout::dagre::order::build_layer_graph::LayerEdge;
    use crate::mermaid::layout::dagre::util::new_layout_graph;

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
    fn assigns_an_undefined_barycenter_for_a_node_with_no_predecessors() {
        let g = new_layout_graph();
        let lg: LayerGraph = Graph::new(GraphOptions {
            compound: true,
            ..GraphOptions::default()
        });
        let entries = barycenter(&lg, &g, &["x".to_string()]);
        assert_eq!(
            entries,
            vec![Entry {
                v: "x".into(),
                barycenter: None,
                weight: None
            }]
        );
    }

    #[test]
    fn computes_the_weighted_mean_of_predecessor_orders() {
        let mut g = new_layout_graph();
        ordered(&mut g, "a", 1);
        ordered(&mut g, "b", 3);
        let mut lg: LayerGraph = Graph::new(GraphOptions {
            compound: true,
            ..GraphOptions::default()
        });
        lg.set_edge(&EdgeKey::new("a", "x"), LayerEdge { weight: 2.0 });
        lg.set_edge(&EdgeKey::new("b", "x"), LayerEdge { weight: 1.0 });
        let entries = barycenter(&lg, &g, &["x".to_string()]);
        assert_eq!(entries[0].barycenter, Some(5.0 / 3.0));
        assert_eq!(entries[0].weight, Some(3.0));
    }
}
