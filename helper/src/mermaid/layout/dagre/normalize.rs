//! Port of `src/dagre/normalize.js`: long edges become chains of dummy nodes
//! one rank apart, and `undo` folds them back into edge points.

use super::graphlib::EdgeKey;
use super::util::add_dummy_node;
use super::{Dummy, EdgeLabel, LayoutGraph, NodeLabel, Point};

/// `normalize.run(g)`.
pub fn run(g: &mut LayoutGraph) {
    g.graph_mut().dummy_chains = Vec::new();
    for e in g.edges() {
        normalize_edge(g, &e);
    }
}

/// `normalizeEdge(g, e)`: an edge that spans more than one rank is replaced
/// by `_d` dummies; the dummy on `labelRank` carries the label size.
fn normalize_edge(g: &mut LayoutGraph, e: &EdgeKey) {
    let mut v = e.v.clone();
    let mut v_rank = g.node(&v).rank.unwrap_or(f64::NAN);
    let w = e.w.clone();
    let w_rank = g.node(&w).rank.unwrap_or(f64::NAN);
    let name = e.name.as_deref();

    if w_rank == v_rank + 1.0 {
        return;
    }

    let Some(mut edge_label) = g.remove_edge(e) else {
        return;
    };
    let label_rank = edge_label.label_rank;
    let weight = edge_label.weight;
    let width = edge_label.width;
    let height = edge_label.height;
    let labelpos = edge_label.labelpos.clone();
    // The JS sets `points = []` on every loop pass; the label object is shared
    // along the chain, so the port keeps it on the chain head.
    edge_label.points = Vec::new();
    let mut head_label = Some(edge_label);

    let mut i = 0;
    v_rank += 1.0;
    while v_rank < w_rank {
        let mut attrs = NodeLabel {
            rank: Some(v_rank),
            edge_obj: Some(e.clone()),
            ..NodeLabel::default()
        };
        let mut kind = Dummy::Edge;
        if Some(v_rank) == label_rank {
            attrs.width = width;
            attrs.height = height;
            kind = Dummy::EdgeLabel;
            attrs.labelpos = Some(labelpos.clone());
        }
        if i == 0 {
            attrs.edge_label = head_label.take();
        }
        let dummy = add_dummy_node(g, kind, attrs, "_d");
        g.set_edge(
            &EdgeKey::named(&v, &dummy, name),
            EdgeLabel::with_weight(weight),
        );
        if i == 0 {
            g.graph_mut().dummy_chains.push(dummy.clone());
        }
        v = dummy;
        i += 1;
        v_rank += 1.0;
    }

    g.set_edge(
        &EdgeKey::named(&v, &w, name),
        EdgeLabel::with_weight(weight),
    );
}

/// `normalize.undo(g)`: walks each chain, removing dummies and collecting
/// their coordinates as the original edge's points.
pub fn undo(g: &mut LayoutGraph) {
    for head in g.graph().dummy_chains.clone() {
        let mut v = head;
        let (edge_obj, orig_label) = {
            let node = g.node_mut(&v);
            (node.edge_obj.clone(), node.edge_label.take())
        };
        let (Some(edge_obj), Some(orig_label)) = (edge_obj, orig_label) else {
            continue;
        };
        g.set_edge(&edge_obj, orig_label);
        loop {
            let node = g.node(&v);
            let Some(dummy) = node.dummy else { break };
            let (x, y, width, height) = (node.x, node.y, node.width, node.height);
            let next = g.successors(&v).into_iter().next();
            g.remove_node(&v);
            let label = g.edge_mut(&edge_obj);
            label.points.push(Point { x, y });
            if dummy == Dummy::EdgeLabel {
                label.x = Some(x);
                label.y = Some(y);
                label.width = width;
                label.height = height;
            }
            // JS reads `g.node(undefined).dummy` and throws here; a chain
            // always ends at the real target node.
            let Some(next) = next else { break };
            v = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::util::new_layout_graph;

    fn ranked(g: &mut LayoutGraph, v: &str, rank: f64) {
        g.set_node(
            v,
            NodeLabel {
                rank: Some(rank),
                ..NodeLabel::default()
            },
        );
    }

    #[test]
    fn does_not_change_a_short_edge() {
        let mut g = new_layout_graph();
        ranked(&mut g, "a", 0.0);
        ranked(&mut g, "b", 1.0);
        g.set_edge(&EdgeKey::new("a", "b"), EdgeLabel::default());
        run(&mut g);
        assert_eq!(g.edges(), vec![EdgeKey::new("a", "b")]);
        assert!(g.graph().dummy_chains.is_empty());
    }

    #[test]
    fn splits_a_two_layer_edge_into_two_segments() {
        let mut g = new_layout_graph();
        ranked(&mut g, "a", 0.0);
        ranked(&mut g, "b", 2.0);
        g.set_edge(
            &EdgeKey::new("a", "b"),
            EdgeLabel {
                weight: 2.0,
                ..EdgeLabel::default()
            },
        );
        run(&mut g);
        let dummies: Vec<String> = g
            .nodes()
            .into_iter()
            .filter(|v| g.node(v).dummy.is_some())
            .collect();
        assert_eq!(dummies.len(), 1);
        let d = &dummies[0];
        assert_eq!(g.node(d).rank, Some(1.0));
        assert_eq!(g.node(d).dummy, Some(Dummy::Edge));
        assert_eq!(g.successors("a"), vec![d.clone()]);
        assert_eq!(g.successors(d), vec!["b"]);
        assert_eq!(g.edge(&EdgeKey::new("a", d)).weight, 2.0);
        assert_eq!(g.graph().dummy_chains, vec![d.clone()]);
    }

    #[test]
    fn assigns_the_label_to_the_dummy_on_label_rank() {
        let mut g = new_layout_graph();
        ranked(&mut g, "a", 0.0);
        ranked(&mut g, "b", 4.0);
        g.set_edge(
            &EdgeKey::new("a", "b"),
            EdgeLabel {
                width: 20.0,
                height: 10.0,
                label_rank: Some(2.0),
                labelpos: "c".to_string(),
                ..EdgeLabel::default()
            },
        );
        run(&mut g);
        let label_dummy = g
            .nodes()
            .into_iter()
            .find(|v| g.node(v).dummy == Some(Dummy::EdgeLabel));
        let d = label_dummy.as_deref().unwrap_or("");
        assert_eq!(g.node(d).rank, Some(2.0));
        assert_eq!(g.node(d).width, 20.0);
        assert_eq!(g.node(d).labelpos.as_deref(), Some("c"));
    }

    #[test]
    fn undo_restores_the_edge_with_dummy_points() {
        let mut g = new_layout_graph();
        ranked(&mut g, "a", 0.0);
        ranked(&mut g, "b", 3.0);
        g.set_edge(&EdgeKey::new("a", "b"), EdgeLabel::default());
        run(&mut g);
        for v in g.nodes() {
            let node = g.node_mut(&v);
            if node.dummy.is_some() {
                node.x = 5.0;
                node.y = node.rank.unwrap_or(0.0) * 10.0;
            }
        }
        undo(&mut g);
        assert_eq!(g.node_count(), 2);
        assert_eq!(g.edges(), vec![EdgeKey::new("a", "b")]);
        assert_eq!(
            g.edge(&EdgeKey::new("a", "b")).points,
            vec![Point { x: 5.0, y: 10.0 }, Point { x: 5.0, y: 20.0 }]
        );
    }
}
