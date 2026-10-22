//! Port of `src/dagre/position/bk.js` (Brandes and Köpf, "Fast and Simple
//! Horizontal Coordinate Assignment") with dagre's block-graph compaction.

use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;

use super::super::graphlib::Graph;
use super::super::util::{
    build_layer_matrix, js_max, js_min, lodash_max, lodash_min, lodash_min_by, lodash_sort_by,
};
use super::super::{BorderType, Dummy, LayoutGraph};

/// `conflicts[v][w] = true` with `v < w` after a string comparison, so the
/// pair is a set entry.
pub type Conflicts = HashSet<(String, String)>;

/// `{root, align}` from `verticalAlignment`.
pub struct Alignment {
    pub root: HashMap<String, String>,
    pub align: HashMap<String, String>,
}

/// Which neighbours `verticalAlignment` aligns with.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Neighbors {
    Predecessors,
    Successors,
}

type BlockGraph = Graph<(), f64, ()>;

/// `findType1Conflicts(g, layering)`: a non-inner segment crossing an inner
/// segment (both ends dummies).
pub fn find_type1_conflicts(g: &LayoutGraph, layering: &[Vec<String>]) -> Conflicts {
    let mut conflicts = Conflicts::new();
    for i in 1..layering.len() {
        type1_visit_layer(g, &layering[i - 1], &layering[i], &mut conflicts);
    }
    conflicts
}

fn type1_visit_layer(
    g: &LayoutGraph,
    prev_layer: &[String],
    layer: &[String],
    conflicts: &mut Conflicts,
) {
    let mut k0 = 0.0;
    let mut scan_pos = 0;
    let prev_layer_length = prev_layer.len() as f64;
    let last_node = layer.last();

    for (i, v) in layer.iter().enumerate() {
        let w = find_other_inner_segment_node(g, v);
        let k1 = match &w {
            Some(w) => g.node(w).order as f64,
            None => prev_layer_length,
        };

        if w.is_some() || Some(v) == last_node {
            for scan_node in &layer[scan_pos..=i] {
                let scan_is_dummy = g.node(scan_node).dummy.is_some();
                for u in g.predecessors(scan_node) {
                    let u_label = g.node(&u);
                    let u_pos = u_label.order as f64;
                    if (u_pos < k0 || k1 < u_pos) && !(u_label.dummy.is_some() && scan_is_dummy) {
                        add_conflict(conflicts, &u, scan_node);
                    }
                }
            }
            scan_pos = i + 1;
            k0 = k1;
        }
    }
}

/// `findType2Conflicts(g, layering)`: dummy chains crossing subgraph
/// borders. The inner `scan` over the rest of the layer runs on every south
/// node, as in dagre.
pub fn find_type2_conflicts(g: &LayoutGraph, layering: &[Vec<String>]) -> Conflicts {
    let mut conflicts = Conflicts::new();
    for i in 1..layering.len() {
        type2_visit_layer(g, &layering[i - 1], &layering[i], &mut conflicts);
    }
    conflicts
}

fn type2_visit_layer(
    g: &LayoutGraph,
    north: &[String],
    south: &[String],
    conflicts: &mut Conflicts,
) {
    let mut prev_north_pos: f64 = -1.0;
    let mut next_north_pos: Option<f64> = None;
    let mut south_pos = 0;

    for (south_lookahead, v) in south.iter().enumerate() {
        if g.node(v).dummy == Some(Dummy::Border) {
            let predecessors = g.predecessors(v);
            if let Some(first) = predecessors.first() {
                let next = g.node(first).order as f64;
                next_north_pos = Some(next);
                type2_scan(
                    g,
                    south,
                    south_pos,
                    south_lookahead,
                    Some(prev_north_pos),
                    next,
                    conflicts,
                );
                south_pos = south_lookahead;
                prev_north_pos = next;
            }
        }
        type2_scan(
            g,
            south,
            south_pos,
            south.len(),
            next_north_pos,
            north.len() as f64,
            conflicts,
        );
    }
}

fn type2_scan(
    g: &LayoutGraph,
    south: &[String],
    south_pos: usize,
    south_end: usize,
    prev_north_border: Option<f64>,
    next_north_border: f64,
    conflicts: &mut Conflicts,
) {
    for v in south.iter().take(south_end).skip(south_pos) {
        if g.node(v).dummy.is_none() {
            continue;
        }
        for u in g.predecessors(v) {
            let u_node = g.node(&u);
            let u_order = u_node.order as f64;
            let before = prev_north_border.is_some_and(|p| u_order < p);
            if u_node.dummy.is_some() && (before || u_order > next_north_border) {
                add_conflict(conflicts, &u, v);
            }
        }
    }
}

/// `findOtherInnerSegmentNode(g, v)`: the dummy predecessor of a dummy.
fn find_other_inner_segment_node(g: &LayoutGraph, v: &str) -> Option<String> {
    g.node(v).dummy?;
    g.predecessors(v)
        .into_iter()
        .find(|u| g.node(u).dummy.is_some())
}

/// `addConflict(conflicts, v, w)`.
pub fn add_conflict(conflicts: &mut Conflicts, v: &str, w: &str) {
    let (a, b) = if v > w { (w, v) } else { (v, w) };
    conflicts.insert((a.to_string(), b.to_string()));
}

/// `hasConflict(conflicts, v, w)`.
pub fn has_conflict(conflicts: &Conflicts, v: &str, w: &str) -> bool {
    let (a, b) = if v > w { (w, v) } else { (v, w) };
    conflicts.contains(&(a.to_string(), b.to_string()))
}

/// `verticalAlignment(g, layering, conflicts, neighborFn)`: each node tries
/// to align with a median neighbour that is not in conflict.
pub fn vertical_alignment(
    g: &LayoutGraph,
    layering: &[Vec<String>],
    conflicts: &Conflicts,
    neighbors: Neighbors,
) -> Alignment {
    let mut root: HashMap<String, String> = HashMap::new();
    let mut align: HashMap<String, String> = HashMap::new();
    let mut pos: HashMap<String, usize> = HashMap::new();

    for layer in layering {
        for (order, v) in layer.iter().enumerate() {
            root.insert(v.clone(), v.clone());
            align.insert(v.clone(), v.clone());
            pos.insert(v.clone(), order);
        }
    }

    for layer in layering {
        let mut prev_idx: i64 = -1;
        for v in layer {
            let mut ws = match neighbors {
                Neighbors::Predecessors => g.predecessors(v),
                Neighbors::Successors => g.successors(v),
            };
            if ws.is_empty() {
                continue;
            }
            lodash_sort_by(&mut ws, |w| pos.get(w).map(|p| *p as f64));
            let mp = (ws.len() as f64 - 1.0) / 2.0;
            let lo = mp.floor() as usize;
            let hi = mp.ceil() as usize;
            for w in &ws[lo..=hi] {
                let Some(&w_pos) = pos.get(w) else { continue };
                let aligned = align.get(v).is_some_and(|a| a == v);
                if aligned && prev_idx < w_pos as i64 && !has_conflict(conflicts, v, w) {
                    let w_root = root.get(w).cloned().unwrap_or_else(|| w.clone());
                    align.insert(w.clone(), v.clone());
                    align.insert(v.clone(), w_root.clone());
                    root.insert(v.clone(), w_root);
                    prev_idx = w_pos as i64;
                }
            }
        }
    }

    Alignment { root, align }
}

/// `horizontalCompaction(g, layering, root, align, reverseSep)`: places
/// blocks at their smallest coordinate, then pulls them right as far as the
/// separation allows.
pub fn horizontal_compaction(
    g: &LayoutGraph,
    layering: &[Vec<String>],
    root: &HashMap<String, String>,
    align: &HashMap<String, String>,
    reverse_sep: bool,
) -> IndexMap<String, f64> {
    let mut xs: IndexMap<String, f64> = IndexMap::new();
    let block_g = build_block_graph(g, layering, root, reverse_sep);
    let border_type = if reverse_sep {
        BorderType::BorderLeft
    } else {
        BorderType::BorderRight
    };

    // First pass, assign smallest coordinates.
    iterate(
        &block_g,
        |elem, xs| {
            let x = block_g.in_edges(elem, None).iter().fold(0.0, |acc, e| {
                let prev = xs.get(&e.v).copied().unwrap_or(f64::NAN);
                js_max(acc, prev + block_g.edge(e))
            });
            xs.insert(elem.to_string(), x);
        },
        |bg, elem| bg.predecessors(elem),
        &mut xs,
    );

    // Second pass, assign greatest coordinates.
    iterate(
        &block_g,
        |elem, xs| {
            let min = block_g
                .out_edges(elem, None)
                .iter()
                .fold(f64::INFINITY, |acc, e| {
                    let next = xs.get(&e.w).copied().unwrap_or(f64::NAN);
                    js_min(acc, next - block_g.edge(e))
                });
            let node = g.node(elem);
            if min != f64::INFINITY && node.border_type != Some(border_type) {
                let current = xs.get(elem).copied().unwrap_or(f64::NAN);
                xs.insert(elem.to_string(), js_max(current, min));
            }
        },
        |bg, elem| bg.successors(elem),
        &mut xs,
    );

    // `_.forEach(align, v => xs[v] = xs[root[v]])` walks the values of
    // `align`, which cover every node.
    for v in align.values() {
        let r = root.get(v).cloned().unwrap_or_else(|| v.clone());
        let x = xs.get(&r).copied().unwrap_or(f64::NAN);
        xs.insert(v.clone(), x);
    }

    xs
}

/// `iterate(setXsFunc, nextNodesFunc)`: the explicit-stack DFS of
/// `horizontalCompaction`.
fn iterate<S, N>(block_g: &BlockGraph, mut set_xs: S, next_nodes: N, xs: &mut IndexMap<String, f64>)
where
    S: FnMut(&str, &mut IndexMap<String, f64>),
    N: Fn(&BlockGraph, &str) -> Vec<String>,
{
    let mut stack = block_g.nodes();
    let mut visited: HashSet<String> = HashSet::new();
    while let Some(elem) = stack.pop() {
        if visited.contains(&elem) {
            set_xs(&elem, xs);
        } else {
            visited.insert(elem.clone());
            stack.push(elem.clone());
            stack.extend(next_nodes(block_g, &elem));
        }
    }
}

/// `buildBlockGraph(g, layering, root, reverseSep)`: one node per block,
/// one edge per adjacent block pair with the required separation.
fn build_block_graph(
    g: &LayoutGraph,
    layering: &[Vec<String>],
    root: &HashMap<String, String>,
    reverse_sep: bool,
) -> BlockGraph {
    let mut block_graph: BlockGraph = Graph::default();
    let node_sep = g.graph().nodesep;
    let edge_sep = g.graph().edgesep;

    for layer in layering {
        let mut u: Option<&String> = None;
        for v in layer {
            let v_root = root.get(v).cloned().unwrap_or_else(|| v.clone());
            block_graph.ensure_node(&v_root);
            if let Some(u) = u {
                let u_root = root.get(u).cloned().unwrap_or_else(|| u.clone());
                let prev_max = block_graph
                    .try_edge_vw(&u_root, &v_root)
                    .copied()
                    .filter(|m| *m != 0.0 && !m.is_nan())
                    .unwrap_or(0.0);
                let separation = sep(g, v, u, node_sep, edge_sep, reverse_sep);
                block_graph.set_edge_vw(&u_root, &v_root, js_max(separation, prev_max));
            }
            u = Some(v);
        }
    }

    block_graph
}

/// `findSmallestWidthAlignment(g, xss)`: the alignment name with the
/// narrowest extent; the first one wins a tie.
pub fn find_smallest_width_alignment(
    g: &LayoutGraph,
    xss: &IndexMap<String, IndexMap<String, f64>>,
) -> Option<String> {
    lodash_min_by(xss.keys().cloned().collect(), |name| {
        let xs = xss.get(name)?;
        let mut max = f64::NEG_INFINITY;
        let mut min = f64::INFINITY;
        for (v, x) in xs {
            let half_width = width(g, v) / 2.0;
            max = js_max(x + half_width, max);
            min = js_min(x - half_width, min);
        }
        Some(max - min)
    })
}

/// `alignCoordinates(xss, alignTo)`: left alignments share the minimum,
/// right alignments the maximum, of the chosen alignment.
pub fn align_coordinates(
    xss: &mut IndexMap<String, IndexMap<String, f64>>,
    align_to: Option<&str>,
) {
    let (align_to_min, align_to_max) = match align_to.and_then(|a| xss.get(a)) {
        Some(xs) => (
            lodash_min(xs.values().map(|x| Some(*x))).unwrap_or(f64::NAN),
            lodash_max(xs.values().map(|x| Some(*x))).unwrap_or(f64::NAN),
        ),
        None => (f64::NAN, f64::NAN),
    };

    for vert in ["u", "d"] {
        for horiz in ["l", "r"] {
            let alignment = format!("{vert}{horiz}");
            if Some(alignment.as_str()) == align_to {
                continue;
            }
            let Some(xs) = xss.get(&alignment) else {
                continue;
            };
            let delta = if horiz == "l" {
                align_to_min - lodash_min(xs.values().map(|x| Some(*x))).unwrap_or(f64::NAN)
            } else {
                align_to_max - lodash_max(xs.values().map(|x| Some(*x))).unwrap_or(f64::NAN)
            };
            if delta != 0.0 && !delta.is_nan() {
                let shifted: IndexMap<String, f64> =
                    xs.iter().map(|(v, x)| (v.clone(), x + delta)).collect();
                xss.insert(alignment, shifted);
            }
        }
    }
}

/// `balance(xss, align)`: the median pair of the four alignments, or the
/// one alignment the graph asked for.
pub fn balance(
    xss: &IndexMap<String, IndexMap<String, f64>>,
    align: Option<&str>,
) -> IndexMap<String, f64> {
    let Some(ul) = xss.get("ul") else {
        return IndexMap::new();
    };
    ul.keys()
        .map(|v| {
            let x = match align {
                Some(a) => xss
                    .get(&a.to_lowercase())
                    .and_then(|xs| xs.get(v))
                    .copied()
                    .unwrap_or(f64::NAN),
                None => {
                    let mut values: Vec<f64> = xss
                        .values()
                        .map(|xs| xs.get(v).copied().unwrap_or(f64::NAN))
                        .collect();
                    lodash_sort_by(&mut values, |x| if x.is_nan() { None } else { Some(*x) });
                    (values[1] + values[2]) / 2.0
                }
            };
            (v.clone(), x)
        })
        .collect()
}

/// `positionX(g)`: x for every node of a non-compound, ranked, ordered
/// graph.
pub fn position_x(g: &LayoutGraph) -> IndexMap<String, f64> {
    let layering = build_layer_matrix(g);
    let mut conflicts = find_type1_conflicts(g, &layering);
    conflicts.extend(find_type2_conflicts(g, &layering));

    let mut xss: IndexMap<String, IndexMap<String, f64>> = IndexMap::new();

    for vert in ["u", "d"] {
        let mut adjusted_layering: Vec<Vec<String>> = if vert == "u" {
            layering.clone()
        } else {
            layering.iter().rev().cloned().collect()
        };
        for horiz in ["l", "r"] {
            if horiz == "r" {
                adjusted_layering = adjusted_layering
                    .iter()
                    .map(|inner| inner.iter().rev().cloned().collect())
                    .collect();
            }

            let neighbors = if vert == "u" {
                Neighbors::Predecessors
            } else {
                Neighbors::Successors
            };
            let alignment = vertical_alignment(g, &adjusted_layering, &conflicts, neighbors);
            let mut xs = horizontal_compaction(
                g,
                &adjusted_layering,
                &alignment.root,
                &alignment.align,
                horiz == "r",
            );
            if horiz == "r" {
                for x in xs.values_mut() {
                    *x = -*x;
                }
            }
            xss.insert(format!("{vert}{horiz}"), xs);
        }
    }

    let smallest_width = find_smallest_width_alignment(g, &xss);
    align_coordinates(&mut xss, smallest_width.as_deref());
    balance(&xss, g.graph().align.as_deref())
}

/// `sep(nodeSep, edgeSep, reverseSep)(g, v, w)`: the distance between the
/// centres of two neighbours in a layer, with the label offset of an
/// `edge-label` dummy folded in.
fn sep(g: &LayoutGraph, v: &str, w: &str, node_sep: f64, edge_sep: f64, reverse_sep: bool) -> f64 {
    let v_label = g.node(v);
    let w_label = g.node(w);
    let mut sum = 0.0;

    sum += v_label.width / 2.0;
    let mut delta: Option<f64> = None;
    if let Some(labelpos) = &v_label.labelpos {
        match labelpos.to_lowercase().as_str() {
            "l" => delta = Some(-v_label.width / 2.0),
            "r" => delta = Some(v_label.width / 2.0),
            _ => {}
        }
    }
    if let Some(d) = delta.filter(|d| *d != 0.0 && !d.is_nan()) {
        sum += if reverse_sep { d } else { -d };
    }
    delta = None;

    sum += (if v_label.dummy.is_some() {
        edge_sep
    } else {
        node_sep
    }) / 2.0;
    sum += (if w_label.dummy.is_some() {
        edge_sep
    } else {
        node_sep
    }) / 2.0;

    sum += w_label.width / 2.0;
    if let Some(labelpos) = &w_label.labelpos {
        match labelpos.to_lowercase().as_str() {
            "l" => delta = Some(w_label.width / 2.0),
            "r" => delta = Some(-w_label.width / 2.0),
            _ => {}
        }
    }
    if let Some(d) = delta.filter(|d| *d != 0.0 && !d.is_nan()) {
        sum += if reverse_sep { d } else { -d };
    }

    sum
}

fn width(g: &LayoutGraph, v: &str) -> f64 {
    g.node(v).width
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::EdgeLabel;
    use crate::mermaid::layout::dagre::NodeLabel;
    use crate::mermaid::layout::dagre::graphlib::EdgeKey;
    use crate::mermaid::layout::dagre::util::new_layout_graph;

    fn node(g: &mut LayoutGraph, v: &str, rank: f64, order: usize) {
        g.set_node(
            v,
            NodeLabel {
                rank: Some(rank),
                order,
                width: 10.0,
                height: 10.0,
                ..NodeLabel::default()
            },
        );
    }

    fn dummy(g: &mut LayoutGraph, v: &str, rank: f64, order: usize) {
        node(g, v, rank, order);
        g.node_mut(v).dummy = Some(Dummy::Edge);
    }

    fn edge(g: &mut LayoutGraph, v: &str, w: &str) {
        g.set_edge(&EdgeKey::new(v, w), EdgeLabel::default());
    }

    fn layers(spec: &[&[&str]]) -> Vec<Vec<String>> {
        spec.iter()
            .map(|l| l.iter().map(|s| s.to_string()).collect())
            .collect()
    }

    #[test]
    fn type1_conflicts_are_found_for_a_non_inner_segment_crossing_an_inner_one() {
        let mut g = new_layout_graph();
        node(&mut g, "a", 0.0, 0);
        dummy(&mut g, "d0", 0.0, 1);
        dummy(&mut g, "d1", 1.0, 0);
        node(&mut g, "b", 1.0, 1);
        edge(&mut g, "d0", "d1");
        edge(&mut g, "a", "b");
        let conflicts = find_type1_conflicts(&g, &layers(&[&["a", "d0"], &["d1", "b"]]));
        assert!(has_conflict(&conflicts, "a", "b"));
        assert!(!has_conflict(&conflicts, "d0", "d1"));
    }

    #[test]
    fn type1_conflicts_ignore_two_non_inner_segments_crossing() {
        let mut g = new_layout_graph();
        node(&mut g, "a", 0.0, 0);
        node(&mut g, "b", 0.0, 1);
        node(&mut g, "c", 1.0, 0);
        node(&mut g, "d", 1.0, 1);
        edge(&mut g, "a", "d");
        edge(&mut g, "b", "c");
        let conflicts = find_type1_conflicts(&g, &layers(&[&["a", "b"], &["c", "d"]]));
        assert!(conflicts.is_empty());
    }

    #[test]
    fn type2_conflicts_are_found_for_an_inner_segment_crossing_a_border() {
        let mut g = new_layout_graph();
        dummy(&mut g, "d0", 0.0, 0);
        node(&mut g, "bl0", 0.0, 1);
        g.node_mut("bl0").dummy = Some(Dummy::Border);
        node(&mut g, "bl1", 1.0, 0);
        g.node_mut("bl1").dummy = Some(Dummy::Border);
        dummy(&mut g, "d1", 1.0, 1);
        edge(&mut g, "d0", "d1");
        edge(&mut g, "bl0", "bl1");
        let conflicts = find_type2_conflicts(&g, &layers(&[&["d0", "bl0"], &["bl1", "d1"]]));
        assert!(has_conflict(&conflicts, "d0", "d1"));
    }

    #[test]
    fn vertical_alignment_aligns_with_a_single_predecessor() {
        let mut g = new_layout_graph();
        node(&mut g, "a", 0.0, 0);
        node(&mut g, "b", 1.0, 0);
        edge(&mut g, "a", "b");
        let layering = layers(&[&["a"], &["b"]]);
        let a = vertical_alignment(&g, &layering, &Conflicts::new(), Neighbors::Predecessors);
        assert_eq!(a.root["b"], "a");
        assert_eq!(a.align["a"], "b");
        assert_eq!(a.align["b"], "a");
    }

    #[test]
    fn vertical_alignment_takes_the_median_and_skips_conflicts() {
        let mut g = new_layout_graph();
        for (i, v) in ["a", "b", "c"].iter().enumerate() {
            node(&mut g, v, 0.0, i);
        }
        node(&mut g, "d", 1.0, 0);
        for v in ["a", "b", "c"] {
            edge(&mut g, v, "d");
        }
        let layering = layers(&[&["a", "b", "c"], &["d"]]);
        let a = vertical_alignment(&g, &layering, &Conflicts::new(), Neighbors::Predecessors);
        assert_eq!(a.root["d"], "b");

        let mut conflicts = Conflicts::new();
        add_conflict(&mut conflicts, "b", "d");
        let a = vertical_alignment(&g, &layering, &conflicts, Neighbors::Predecessors);
        assert_eq!(a.root["d"], "d");
    }

    #[test]
    fn horizontal_compaction_separates_nodes_in_a_layer() {
        let mut g = new_layout_graph();
        node(&mut g, "a", 0.0, 0);
        node(&mut g, "b", 0.0, 1);
        let layering = layers(&[&["a", "b"]]);
        let a = vertical_alignment(&g, &layering, &Conflicts::new(), Neighbors::Predecessors);
        let xs = horizontal_compaction(&g, &layering, &a.root, &a.align, false);
        assert_eq!(xs["a"], 0.0);
        assert_eq!(xs["b"], 10.0 / 2.0 + 50.0 + 10.0 / 2.0);
    }

    #[test]
    fn horizontal_compaction_keeps_a_block_at_one_x() {
        let mut g = new_layout_graph();
        node(&mut g, "a", 0.0, 0);
        node(&mut g, "b", 0.0, 1);
        node(&mut g, "c", 1.0, 0);
        edge(&mut g, "b", "c");
        let layering = layers(&[&["a", "b"], &["c"]]);
        let a = vertical_alignment(&g, &layering, &Conflicts::new(), Neighbors::Predecessors);
        let xs = horizontal_compaction(&g, &layering, &a.root, &a.align, false);
        assert_eq!(xs["b"], xs["c"]);
        assert_eq!(xs["b"], 60.0);
    }

    #[test]
    fn position_x_centres_a_single_child_under_two_parents() {
        let mut g = new_layout_graph();
        node(&mut g, "a", 0.0, 0);
        node(&mut g, "b", 0.0, 1);
        node(&mut g, "c", 1.0, 0);
        edge(&mut g, "a", "c");
        edge(&mut g, "b", "c");
        let xs = position_x(&g);
        assert_eq!(xs["a"], 0.0);
        assert_eq!(xs["b"], 60.0);
        assert_eq!(xs["c"], 30.0);
    }

    #[test]
    fn balance_uses_the_requested_alignment_when_given() {
        let mut xss: IndexMap<String, IndexMap<String, f64>> = IndexMap::new();
        for (name, x) in [("ul", 0.0), ("ur", 10.0), ("dl", 20.0), ("dr", 30.0)] {
            xss.insert(name.to_string(), IndexMap::from([("a".to_string(), x)]));
        }
        assert_eq!(balance(&xss, None)["a"], 15.0);
        assert_eq!(balance(&xss, Some("DR"))["a"], 30.0);
    }

    #[test]
    fn sep_folds_the_label_offset_of_an_edge_label_dummy() {
        let mut g = new_layout_graph();
        node(&mut g, "a", 0.0, 0);
        node(&mut g, "l", 0.0, 1);
        g.node_mut("l").dummy = Some(Dummy::EdgeLabel);
        g.node_mut("l").labelpos = Some("r".to_string());
        g.node_mut("l").width = 20.0;
        // v = l (labelpos r): width/2 - width/2 = 0; sep halves 10 + 25; w = a: 5.
        assert_eq!(
            sep(&g, "l", "a", 50.0, 20.0, false),
            10.0 - 10.0 + 10.0 + 25.0 + 5.0
        );
        assert_eq!(
            sep(&g, "l", "a", 50.0, 20.0, true),
            10.0 + 10.0 + 10.0 + 25.0 + 5.0
        );
    }
}
