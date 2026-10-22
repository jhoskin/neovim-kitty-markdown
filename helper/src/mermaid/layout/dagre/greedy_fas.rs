//! Port of `src/dagre/greedy-fas.js` and the doubly linked list in
//! `src/dagre/data/list.js` (Eades, Lin, and Smyth's feedback arc set
//! heuristic with weights).

use std::collections::VecDeque;

use super::LayoutGraph;
use super::graphlib::{EdgeKey, Graph};
use super::util::js_max;

/// One `{v, in, out}` node label of the FAS graph, plus the bucket its list
/// links currently sit in.
struct Entry {
    v: String,
    in_weight: f64,
    out_weight: f64,
    bucket: Option<usize>,
}

/// `List` from `data/list.js`: `enqueue` unlinks the entry from any list it
/// is in and pushes it at the front, `dequeue` pops the back.
struct Buckets {
    lists: Vec<VecDeque<usize>>,
}

impl Buckets {
    fn enqueue(&mut self, entries: &mut [Entry], bucket: usize, idx: usize) {
        if let Some(old) = entries[idx].bucket.take() {
            self.lists[old].retain(|&i| i != idx);
        }
        self.lists[bucket].push_front(idx);
        entries[idx].bucket = Some(bucket);
    }

    fn dequeue(&mut self, entries: &mut [Entry], bucket: usize) -> Option<usize> {
        let idx = self.lists[bucket].pop_back()?;
        entries[idx].bucket = None;
        Some(idx)
    }
}

type FasGraph = Graph<usize, f64, ()>;

struct State {
    graph: FasGraph,
    entries: Vec<Entry>,
    buckets: Buckets,
    zero_idx: f64,
}

/// `greedyFAS(g, weightFn)`: the edges to reverse, expanded to every
/// parallel edge of the original graph.
pub fn greedy_fas<F>(g: &LayoutGraph, weight_fn: F) -> Vec<EdgeKey>
where
    F: Fn(&LayoutGraph, &EdgeKey) -> f64,
{
    if g.node_count() <= 1 {
        return Vec::new();
    }
    let mut state = build_state(g, weight_fn);
    let results = do_greedy_fas(&mut state);
    results
        .into_iter()
        .flat_map(|(v, w)| g.out_edges(&v, Some(&w)))
        .collect()
}

/// `doGreedyFAS(g, buckets, zeroIdx)`.
fn do_greedy_fas(state: &mut State) -> Vec<(String, String)> {
    let mut results = Vec::new();
    let sources = state.buckets.lists.len() - 1;
    let sinks = 0;
    while state.graph.node_count() > 0 {
        while let Some(entry) = state.buckets.dequeue(&mut state.entries, sinks) {
            remove_node(state, entry, false);
        }
        while let Some(entry) = state.buckets.dequeue(&mut state.entries, sources) {
            remove_node(state, entry, false);
        }
        if state.graph.node_count() > 0 {
            for i in (1..state.buckets.lists.len() - 1).rev() {
                if let Some(entry) = state.buckets.dequeue(&mut state.entries, i) {
                    results.extend(remove_node(state, entry, true));
                    break;
                }
            }
        }
    }
    results
}

/// `removeNode(g, buckets, zeroIdx, entry, collectPredecessors)`.
fn remove_node(
    state: &mut State,
    entry: usize,
    collect_predecessors: bool,
) -> Vec<(String, String)> {
    let mut results = Vec::new();
    let v = state.entries[entry].v.clone();
    for edge in state.graph.in_edges(&v, None) {
        let weight = *state.graph.edge(&edge);
        let u_entry = *state.graph.node(&edge.v);
        if collect_predecessors {
            results.push((edge.v.clone(), edge.w.clone()));
        }
        state.entries[u_entry].out_weight -= weight;
        assign_bucket(state, u_entry);
    }
    for edge in state.graph.out_edges(&v, None) {
        let weight = *state.graph.edge(&edge);
        let w_entry = *state.graph.node(&edge.w);
        state.entries[w_entry].in_weight -= weight;
        assign_bucket(state, w_entry);
    }
    state.graph.remove_node(&v);
    results
}

/// `buildState(g, weightFn)`: the FAS graph with one aggregated edge per
/// node pair, and `maxOut + maxIn + 3` buckets.
fn build_state<F>(g: &LayoutGraph, weight_fn: F) -> State
where
    F: Fn(&LayoutGraph, &EdgeKey) -> f64,
{
    let mut fas_graph: FasGraph = Graph::default();
    let mut entries = Vec::new();
    let mut max_in: f64 = 0.0;
    let mut max_out: f64 = 0.0;
    for v in g.nodes() {
        fas_graph.set_node(&v, entries.len());
        entries.push(Entry {
            v,
            in_weight: 0.0,
            out_weight: 0.0,
            bucket: None,
        });
    }
    for e in g.edges() {
        let prev_weight = fas_graph.try_edge_vw(&e.v, &e.w).copied().unwrap_or(0.0);
        let weight = weight_fn(g, &e);
        fas_graph.set_edge_vw(&e.v, &e.w, prev_weight + weight);
        let v_entry = *fas_graph.node(&e.v);
        entries[v_entry].out_weight += weight;
        max_out = js_max(max_out, entries[v_entry].out_weight);
        let w_entry = *fas_graph.node(&e.w);
        entries[w_entry].in_weight += weight;
        max_in = js_max(max_in, entries[w_entry].in_weight);
    }
    // `_.range(n)` has `ceil(n)` elements; weights are integers in practice.
    let bucket_count = (max_out + max_in + 3.0).ceil().max(1.0) as usize;
    let mut state = State {
        graph: fas_graph,
        entries,
        buckets: Buckets {
            lists: vec![VecDeque::new(); bucket_count],
        },
        zero_idx: max_in + 1.0,
    };
    for v in state.graph.nodes() {
        let idx = *state.graph.node(&v);
        assign_bucket(&mut state, idx);
    }
    state
}

/// `assignBucket(buckets, zeroIdx, entry)`: sinks first, sources last,
/// everything else by `out - in`.
fn assign_bucket(state: &mut State, idx: usize) {
    let entry = &state.entries[idx];
    let last = state.buckets.lists.len() - 1;
    let bucket = if entry.out_weight == 0.0 || entry.out_weight.is_nan() {
        0
    } else if entry.in_weight == 0.0 || entry.in_weight.is_nan() {
        last
    } else {
        (entry.out_weight - entry.in_weight + state.zero_idx) as usize
    };
    state.buckets.enqueue(&mut state.entries, bucket, idx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::EdgeLabel;
    use crate::mermaid::layout::dagre::util::new_layout_graph;

    fn edge(g: &mut LayoutGraph, v: &str, w: &str, weight: f64) {
        g.set_edge(
            &EdgeKey::new(v, w),
            EdgeLabel {
                weight,
                ..EdgeLabel::default()
            },
        );
    }

    fn weights(g: &LayoutGraph, e: &EdgeKey) -> f64 {
        g.edge(e).weight
    }

    #[test]
    fn empty_and_acyclic_graphs_give_nothing() {
        let mut g = new_layout_graph();
        assert!(greedy_fas(&g, weights).is_empty());
        edge(&mut g, "a", "b", 1.0);
        edge(&mut g, "b", "c", 1.0);
        assert!(greedy_fas(&g, weights).is_empty());
    }

    #[test]
    fn a_two_cycle_loses_one_edge() {
        let mut g = new_layout_graph();
        edge(&mut g, "a", "b", 1.0);
        edge(&mut g, "b", "a", 1.0);
        let fas = greedy_fas(&g, weights);
        assert_eq!(fas.len(), 1);
    }

    #[test]
    fn the_lighter_edge_of_a_cycle_is_chosen() {
        let mut g = new_layout_graph();
        edge(&mut g, "a", "b", 2.0);
        edge(&mut g, "b", "c", 2.0);
        edge(&mut g, "c", "a", 1.0);
        edge(&mut g, "a", "d", 1.0);
        let fas = greedy_fas(&g, weights);
        assert_eq!(fas.len(), 1);
        assert_eq!((fas[0].v.as_str(), fas[0].w.as_str()), ("c", "a"));
    }

    #[test]
    fn parallel_edges_are_all_returned() {
        let mut g = new_layout_graph();
        edge(&mut g, "a", "b", 3.0);
        g.set_edge(&EdgeKey::named("b", "a", Some("x")), EdgeLabel::default());
        g.set_edge(&EdgeKey::named("b", "a", Some("y")), EdgeLabel::default());
        let fas = greedy_fas(&g, weights);
        assert_eq!(fas.len(), 2);
        assert!(fas.iter().all(|e| e.v == "b" && e.w == "a"));
    }
}
