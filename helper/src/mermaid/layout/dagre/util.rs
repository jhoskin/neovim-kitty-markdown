//! Port of `src/dagre/util.js` plus the lodash and `Math` semantics the rest
//! of the port leans on.

use std::cell::Cell;
use std::cmp::Ordering;

use super::graphlib::{Graph, GraphOptions};
use super::{Dummy, EdgeLabel, LayoutGraph, NodeLabel, Point};

thread_local! {
    static ID_COUNTER: Cell<u64> = const { Cell::new(0) };
}

/// lodash `_.uniqueId(prefix)`: one counter for every prefix, first id 1.
pub fn unique_id(prefix: &str) -> String {
    ID_COUNTER.with(|c| {
        let n = c.get() + 1;
        c.set(n);
        format!("{prefix}{n}")
    })
}

/// Starts the dummy id counter over, as a fresh JS page would.
pub fn reset_unique_ids() {
    ID_COUNTER.with(|c| c.set(0));
}

pub fn set_unique_id_counter(n: u64) {
    ID_COUNTER.with(|c| c.set(n));
}

pub fn unique_id_counter() -> u64 {
    ID_COUNTER.with(Cell::get)
}

/// `Math.max(a, b)`: `NaN` wins, unlike `f64::max`.
pub fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

/// `Math.min(a, b)`: `NaN` wins, unlike `f64::min`.
pub fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

/// lodash `_.max(array)`: skips `undefined` and `NaN`, `None` when nothing
/// is left, first maximum wins.
pub fn lodash_max<I: IntoIterator<Item = Option<f64>>>(values: I) -> Option<f64> {
    let mut best: Option<f64> = None;
    for v in values.into_iter().flatten() {
        if v.is_nan() {
            continue;
        }
        match best {
            Some(b) if v <= b => {}
            _ => best = Some(v),
        }
    }
    best
}

/// lodash `_.min(array)`: skips `undefined` and `NaN`, first minimum wins.
pub fn lodash_min<I: IntoIterator<Item = Option<f64>>>(values: I) -> Option<f64> {
    let mut best: Option<f64> = None;
    for v in values.into_iter().flatten() {
        if v.is_nan() {
            continue;
        }
        match best {
            Some(b) if v >= b => {}
            _ => best = Some(v),
        }
    }
    best
}

/// lodash `_.minBy(collection, fn)`: the first item whose key is minimal,
/// skipping items whose key is `undefined` or `NaN`.
pub fn lodash_min_by<T, F>(items: Vec<T>, mut key: F) -> Option<T>
where
    F: FnMut(&T) -> Option<f64>,
{
    let mut best: Option<(f64, T)> = None;
    for item in items {
        let Some(k) = key(&item) else { continue };
        if k.is_nan() {
            continue;
        }
        match &best {
            Some((b, _)) if k >= *b => {}
            _ => best = Some((k, item)),
        }
    }
    best.map(|(_, item)| item)
}

/// lodash `_.sortBy(array, fn)` for a numeric key: stable, `undefined`
/// keys last.
pub fn lodash_sort_by<T, F>(items: &mut [T], mut key: F)
where
    F: FnMut(&T) -> Option<f64>,
{
    stable_sort_by(items, |a, b| match (key(a), key(b)) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    });
}

/// A stable sort that never panics on an inconsistent comparator, the way
/// `Array.prototype.sort` does not. `order/sort.js` compares barycenters
/// that can be `NaN`, which breaks the total order `slice::sort_by` wants.
pub fn stable_sort_by<T, F>(items: &mut [T], mut cmp: F)
where
    F: FnMut(&T, &T) -> Ordering,
{
    for i in 1..items.len() {
        let mut j = i;
        while j > 0 && cmp(&items[j - 1], &items[j]) == Ordering::Greater {
            items.swap(j - 1, j);
            j -= 1;
        }
    }
}

/// `addDummyNode(g, type, attrs, name)`: a fresh `_.uniqueId(name)` not in
/// the graph, tagged with `dummy = type`.
pub fn add_dummy_node(
    g: &mut LayoutGraph,
    kind: Dummy,
    mut attrs: NodeLabel,
    name: &str,
) -> String {
    let mut v = unique_id(name);
    while g.has_node(&v) {
        v = unique_id(name);
    }
    attrs.dummy = Some(kind);
    g.set_node(&v, attrs);
    v
}

/// `simplify(g)`: a non-multigraph with weights summed and minlens maxed
/// over parallel edges. Node labels are copies; the caller writes `rank`
/// back where JS shares the objects.
pub fn simplify(g: &LayoutGraph) -> LayoutGraph {
    let mut simplified: LayoutGraph = Graph::new(GraphOptions::default());
    simplified.set_graph(g.graph().clone());
    for v in g.nodes() {
        simplified.set_node(&v, g.node(&v).clone());
    }
    for e in g.edges() {
        let (weight, minlen) = match simplified.try_edge_vw(&e.v, &e.w) {
            Some(simple) => (simple.weight, simple.minlen),
            None => (0.0, 1.0),
        };
        let label = g.edge(&e);
        simplified.set_edge_vw(
            &e.v,
            &e.w,
            EdgeLabel::weight_minlen(weight + label.weight, js_max(minlen, label.minlen)),
        );
    }
    simplified
}

/// `asNonCompoundGraph(g)`: the leaf nodes and every edge, no hierarchy.
/// Labels are copies; callers write results back.
pub fn as_non_compound_graph(g: &LayoutGraph) -> LayoutGraph {
    let mut simplified: LayoutGraph = Graph::new(GraphOptions {
        multigraph: g.is_multigraph(),
        ..GraphOptions::default()
    });
    simplified.set_graph(g.graph().clone());
    for v in g.nodes() {
        if g.children(Some(&v)).is_empty() {
            simplified.set_node(&v, g.node(&v).clone());
        }
    }
    for e in g.edges() {
        simplified.set_edge(&e, g.edge(&e).clone());
    }
    simplified
}

/// `intersectRect(rect, point)`: where a line from the rectangle centre to
/// `point` leaves the rectangle. `None` when the point is the centre, where
/// JS throws.
pub fn intersect_rect(rect: &NodeLabel, point: Point) -> Option<Point> {
    let x = rect.x;
    let y = rect.y;
    let dx = point.x - x;
    let dy = point.y - y;
    let mut w = rect.width / 2.0;
    let mut h = rect.height / 2.0;
    if dx == 0.0 && dy == 0.0 {
        return None;
    }
    let (sx, sy);
    if dy.abs() * w > dx.abs() * h {
        if dy < 0.0 {
            h = -h;
        }
        sx = (h * dx) / dy;
        sy = h;
    } else {
        if dx < 0.0 {
            w = -w;
        }
        sx = w;
        sy = (w * dy) / dx;
    }
    Some(Point {
        x: x + sx,
        y: y + sy,
    })
}

/// `buildLayerMatrix(g)`: `layering[rank][order] = v` for every ranked node.
/// A later node with the same `(rank, order)` replaces an earlier one, as
/// the JS assignment does; holes are dropped.
pub fn build_layer_matrix(g: &LayoutGraph) -> Vec<Vec<String>> {
    let count = max_rank(g).map_or(0, |r| r as usize + 1);
    let mut layering: Vec<Vec<Option<String>>> = vec![Vec::new(); count];
    for v in g.nodes() {
        let node = g.node(&v);
        if let Some(rank) = node.rank {
            let layer = &mut layering[rank as usize];
            if layer.len() <= node.order {
                layer.resize(node.order + 1, None);
            }
            layer[node.order] = Some(v);
        }
    }
    layering
        .into_iter()
        .map(|layer| layer.into_iter().flatten().collect())
        .collect()
}

/// `normalizeRanks(g)`: shifts ranks so the smallest is 0.
pub fn normalize_ranks(g: &mut LayoutGraph) {
    let nodes = g.nodes();
    let min = lodash_min(nodes.iter().map(|v| g.node(v).rank));
    let Some(min) = min else { return };
    for v in nodes {
        let node = g.node_mut(&v);
        if let Some(rank) = node.rank.as_mut() {
            *rank -= min;
        }
    }
}

/// `removeEmptyRanks(g)`: closes empty layers whose index is not a multiple
/// of `nodeRankFactor`. JS stores fractional ranks as non-index properties
/// that `_.forEach` never visits, so such nodes are neither counted nor
/// shifted here.
pub fn remove_empty_ranks(g: &mut LayoutGraph) {
    let nodes = g.nodes();
    let Some(offset) = lodash_min(nodes.iter().map(|v| g.node(v).rank)) else {
        return;
    };
    let mut layers: Vec<Option<Vec<String>>> = Vec::new();
    for v in &nodes {
        let Some(rank) = g.node(v).rank else { continue };
        let rank = rank - offset;
        if rank < 0.0 || rank.fract() != 0.0 {
            continue;
        }
        let index = rank as usize;
        if layers.len() <= index {
            layers.resize(index + 1, None);
        }
        layers[index].get_or_insert_with(Vec::new).push(v.clone());
    }
    let mut delta: f64 = 0.0;
    // `i % undefined` is NaN, which `!== 0`; a factor of 1 makes every empty
    // layer removable the same way, and the factor is always set before this
    // runs.
    let factor = g.graph().node_rank_factor.unwrap_or(1.0);
    for (i, vs) in layers.iter().enumerate() {
        let empty_removable = vs.is_none() && (i as f64) % factor != 0.0;
        if empty_removable {
            delta -= 1.0;
        } else if delta != 0.0 {
            for v in vs.iter().flatten() {
                if let Some(rank) = g.node_mut(v).rank.as_mut() {
                    *rank += delta;
                }
            }
        }
    }
}

/// `addBorderNode(g, prefix)`: the two-argument form `nesting-graph.js`
/// uses, a zero-size `border` dummy without rank or order.
pub fn add_border_node(g: &mut LayoutGraph, prefix: &str) -> String {
    add_dummy_node(g, Dummy::Border, NodeLabel::default(), prefix)
}

/// `maxRank(g)`: the largest defined rank.
pub fn max_rank(g: &LayoutGraph) -> Option<f64> {
    lodash_max(g.nodes().iter().map(|v| g.node(v).rank))
}

/// `partition(collection, fn)`.
pub fn partition<T, F: FnMut(&T) -> bool>(items: Vec<T>, mut pred: F) -> (Vec<T>, Vec<T>) {
    let mut lhs = Vec::new();
    let mut rhs = Vec::new();
    for item in items {
        if pred(&item) {
            lhs.push(item);
        } else {
            rhs.push(item);
        }
    }
    (lhs, rhs)
}

/// A layout graph with the options mermaid and the tests use.
pub fn new_layout_graph() -> LayoutGraph {
    Graph::new(GraphOptions {
        directed: true,
        multigraph: true,
        compound: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lodash_extrema_skip_missing_and_nan() {
        assert_eq!(lodash_max([Some(f64::NAN), Some(1.0), None]), Some(1.0));
        assert_eq!(
            lodash_min([None, Some(2.0), Some(1.0), Some(1.0)]),
            Some(1.0)
        );
        assert_eq!(lodash_min::<[Option<f64>; 0]>([]), None);
    }

    #[test]
    fn min_by_keeps_the_first_minimum() {
        let items = vec![
            ("a", None),
            ("b", Some(3.0)),
            ("c", Some(1.0)),
            ("d", Some(1.0)),
        ];
        assert_eq!(lodash_min_by(items, |(_, k)| *k).map(|i| i.0), Some("c"));
    }

    #[test]
    fn js_max_propagates_nan() {
        assert!(js_max(1.0, f64::NAN).is_nan());
        assert_eq!(js_max(1.0, 2.0), 2.0);
        assert!(js_min(f64::NAN, 1.0).is_nan());
    }

    #[test]
    fn sort_by_is_stable_and_puts_undefined_last() {
        let mut items = vec![(2.0, "a"), (f64::NAN, "u"), (1.0, "b"), (2.0, "c")];
        lodash_sort_by(
            &mut items,
            |(k, _)| if k.is_nan() { None } else { Some(*k) },
        );
        let names: Vec<&str> = items.iter().map(|i| i.1).collect();
        assert_eq!(names, vec!["b", "a", "c", "u"]);
    }

    #[test]
    fn intersect_rect_hits_the_right_side() {
        let rect = NodeLabel {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 4.0,
            ..NodeLabel::default()
        };
        assert_eq!(
            intersect_rect(&rect, Point { x: 20.0, y: 0.0 }),
            Some(Point { x: 5.0, y: 0.0 })
        );
        assert_eq!(
            intersect_rect(&rect, Point { x: 0.0, y: -20.0 }),
            Some(Point { x: 0.0, y: -2.0 })
        );
        assert_eq!(intersect_rect(&rect, Point { x: 0.0, y: 0.0 }), None);
    }

    #[test]
    fn unique_ids_share_one_counter() {
        reset_unique_ids();
        assert_eq!(unique_id("_root"), "_root1");
        assert_eq!(unique_id("_d"), "_d2");
        assert_eq!(unique_id_counter(), 2);
    }

    #[test]
    fn remove_empty_ranks_closes_gaps_but_keeps_factor_multiples() {
        let mut g = new_layout_graph();
        for (v, rank) in [("a", 0.0), ("b", 1.0), ("c", 4.0)] {
            g.set_node(
                v,
                NodeLabel {
                    rank: Some(rank),
                    ..NodeLabel::default()
                },
            );
        }
        // `i % 1` is always 0, so a factor of 1 never removes a layer.
        g.graph_mut().node_rank_factor = Some(1.0);
        remove_empty_ranks(&mut g);
        assert_eq!(g.node("c").rank, Some(4.0));

        let mut g = new_layout_graph();
        for (v, rank) in [("a", 0.0), ("b", 1.0), ("c", 4.0)] {
            g.set_node(
                v,
                NodeLabel {
                    rank: Some(rank),
                    ..NodeLabel::default()
                },
            );
        }
        g.graph_mut().node_rank_factor = Some(2.0);
        remove_empty_ranks(&mut g);
        assert_eq!(g.node("c").rank, Some(3.0));
    }
}
