//! Port of `src/dagre/acyclic.js`.

use indexmap::IndexSet;

use super::LayoutGraph;
use super::graphlib::EdgeKey;
use super::greedy_fas::greedy_fas;
use super::util::unique_id;

/// `acyclic.run(g)`: reverses a feedback arc set so the graph is a DAG.
pub fn run(g: &mut LayoutGraph) {
    let fas = if g.graph().acyclicer.as_deref() == Some("greedy") {
        greedy_fas(g, |g, e| g.edge(e).weight)
    } else {
        dfs_fas(g)
    };
    for e in fas {
        let Some(mut label) = g.remove_edge(&e) else {
            continue;
        };
        label.forward_name = e.name.clone();
        label.reversed = true;
        let name = unique_id("rev");
        g.set_edge(&EdgeKey::named(&e.w, &e.v, Some(&name)), label);
    }
}

/// `dfsFAS(g)`: every edge that closes a cycle in a DFS over `g.nodes()`.
fn dfs_fas(g: &LayoutGraph) -> Vec<EdgeKey> {
    let mut fas = Vec::new();
    let mut stack: IndexSet<String> = IndexSet::new();
    let mut visited: IndexSet<String> = IndexSet::new();
    for v in g.nodes() {
        dfs(g, &v, &mut stack, &mut visited, &mut fas);
    }
    fas
}

fn dfs(
    g: &LayoutGraph,
    v: &str,
    stack: &mut IndexSet<String>,
    visited: &mut IndexSet<String>,
    fas: &mut Vec<EdgeKey>,
) {
    if visited.contains(v) {
        return;
    }
    visited.insert(v.to_string());
    stack.insert(v.to_string());
    for e in g.out_edges(v, None) {
        if stack.contains(&e.w) {
            fas.push(e);
        } else {
            dfs(g, &e.w, stack, visited, fas);
        }
    }
    stack.shift_remove(v);
}

/// `acyclic.undo(g)`: restores every reversed edge under its original name.
pub fn undo(g: &mut LayoutGraph) {
    for e in g.edges() {
        if !g.edge(&e).reversed {
            continue;
        }
        let Some(mut label) = g.remove_edge(&e) else {
            continue;
        };
        let forward_name = label.forward_name.take();
        label.reversed = false;
        g.set_edge(&EdgeKey::named(&e.w, &e.v, forward_name.as_deref()), label);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::EdgeLabel;
    use crate::mermaid::layout::dagre::util::new_layout_graph;

    fn edge(g: &mut LayoutGraph, v: &str, w: &str) {
        g.set_edge(&EdgeKey::new(v, w), EdgeLabel::default());
    }

    #[test]
    fn run_reverses_the_back_edge_of_a_cycle() {
        let mut g = new_layout_graph();
        edge(&mut g, "a", "b");
        edge(&mut g, "b", "c");
        edge(&mut g, "c", "a");
        run(&mut g);
        let edges = g.edges();
        assert_eq!(edges.len(), 3);
        let reversed: Vec<&EdgeKey> = edges.iter().filter(|e| g.edge(e).reversed).collect();
        assert_eq!(reversed.len(), 1);
        assert_eq!((reversed[0].v.as_str(), reversed[0].w.as_str()), ("a", "c"));
        assert!(
            reversed[0]
                .name
                .as_deref()
                .is_some_and(|n| n.starts_with("rev"))
        );
    }

    #[test]
    fn undo_restores_the_original_edges() {
        let mut g = new_layout_graph();
        edge(&mut g, "a", "b");
        edge(&mut g, "b", "a");
        run(&mut g);
        undo(&mut g);
        let mut pairs: Vec<(String, String, Option<String>)> =
            g.edges().into_iter().map(|e| (e.v, e.w, e.name)).collect();
        pairs.sort();
        assert_eq!(
            pairs,
            vec![
                ("a".into(), "b".into(), None),
                ("b".into(), "a".into(), None)
            ]
        );
        assert!(g.edges().iter().all(|e| !g.edge(e).reversed));
    }

    #[test]
    fn greedy_acyclicer_also_breaks_cycles() {
        let mut g = new_layout_graph();
        g.graph_mut().acyclicer = Some("greedy".into());
        edge(&mut g, "a", "b");
        edge(&mut g, "b", "c");
        edge(&mut g, "c", "a");
        edge(&mut g, "c", "d");
        run(&mut g);
        let mut stack = IndexSet::new();
        let mut visited = IndexSet::new();
        let mut fas = Vec::new();
        for v in g.nodes() {
            dfs(&g, &v, &mut stack, &mut visited, &mut fas);
        }
        assert!(fas.is_empty(), "graph still has a cycle");
    }
}
