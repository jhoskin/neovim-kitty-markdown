//! Port of `src/dagre/order/add-subgraph-constraints.js`: after a layer is
//! sorted, sibling subgraphs get an ordering edge in the constraint graph so
//! later layers keep them in the same order.

use std::collections::HashMap;

use super::super::graphlib::EdgeKey;
use super::ConstraintGraph;
use super::build_layer_graph::LayerGraph;

/// `addSubgraphConstraints(g, cg, vs)`.
pub fn add_subgraph_constraints(lg: &LayerGraph, cg: &mut ConstraintGraph, vs: &[String]) {
    let mut prev: HashMap<String, String> = HashMap::new();
    let mut root_prev: Option<String> = None;

    for v in vs {
        let mut child = lg.parent(v);
        while let Some(c) = child {
            let parent = lg.parent(&c);
            let prev_child = match &parent {
                Some(p) => prev.insert(p.clone(), c.clone()),
                None => root_prev.replace(c.clone()),
            };
            if let Some(pc) = prev_child
                && pc != c
            {
                cg.ensure_edge(&EdgeKey::new(&pc, &c));
                break;
            }
            child = parent;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::graphlib::{Graph, GraphOptions};

    fn layer_graph() -> LayerGraph {
        Graph::new(GraphOptions {
            compound: true,
            ..GraphOptions::default()
        })
    }

    fn vs(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn does_not_change_cg_for_flat_sets_of_nodes() {
        let mut lg = layer_graph();
        for v in ["a", "b", "c"] {
            lg.ensure_node(v);
        }
        let mut cg: ConstraintGraph = Graph::default();
        add_subgraph_constraints(&lg, &mut cg, &vs(&["a", "b", "c"]));
        assert_eq!(cg.node_count(), 0);
    }

    #[test]
    fn adds_a_constraint_between_sibling_subgraphs() {
        let mut lg = layer_graph();
        lg.set_parent("a", Some("sg1"));
        lg.set_parent("b", Some("sg2"));
        let mut cg: ConstraintGraph = Graph::default();
        add_subgraph_constraints(&lg, &mut cg, &vs(&["a", "b"]));
        assert_eq!(cg.edges(), vec![EdgeKey::new("sg1", "sg2")]);
    }

    #[test]
    fn constrains_nested_subgraphs_at_the_level_where_they_differ() {
        let mut lg = layer_graph();
        lg.set_parent("a", Some("sg1"));
        lg.set_parent("sg1", Some("sg3"));
        lg.set_parent("b", Some("sg2"));
        lg.set_parent("sg2", Some("sg3"));
        lg.set_parent("c", Some("sg4"));
        let mut cg: ConstraintGraph = Graph::default();
        add_subgraph_constraints(&lg, &mut cg, &vs(&["a", "b", "c"]));
        let mut edges: Vec<(String, String)> = cg.edges().into_iter().map(|e| (e.v, e.w)).collect();
        edges.sort();
        assert_eq!(
            edges,
            vec![
                ("sg1".to_string(), "sg2".to_string()),
                ("sg3".to_string(), "sg4".to_string())
            ]
        );
    }

    #[test]
    fn does_not_constrain_a_subgraph_against_itself() {
        let mut lg = layer_graph();
        lg.set_parent("a", Some("sg1"));
        lg.set_parent("b", Some("sg1"));
        let mut cg: ConstraintGraph = Graph::default();
        add_subgraph_constraints(&lg, &mut cg, &vs(&["a", "b"]));
        assert_eq!(cg.edge_count(), 0);
    }
}
