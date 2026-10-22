//! Port of `src/dagre/order/cross-count.js` (Barth et al., "Bilayer Cross
//! Counting"): weighted crossings with an accumulator tree.

use std::collections::HashMap;

use super::super::LayoutGraph;
use super::super::util::lodash_sort_by;

/// `crossCount(g, layering)`.
pub fn cross_count(g: &LayoutGraph, layering: &[Vec<String>]) -> f64 {
    let mut cc = 0.0;
    for i in 1..layering.len() {
        cc += two_layer_cross_count(g, &layering[i - 1], &layering[i]);
    }
    cc
}

struct SouthEntry {
    pos: Option<usize>,
    weight: f64,
}

/// `twoLayerCrossCount(g, northLayer, southLayer)`.
fn two_layer_cross_count(g: &LayoutGraph, north_layer: &[String], south_layer: &[String]) -> f64 {
    let south_pos: HashMap<&str, usize> = south_layer
        .iter()
        .enumerate()
        .map(|(i, v)| (v.as_str(), i))
        .collect();
    let mut south_entries: Vec<SouthEntry> = Vec::new();
    for v in north_layer {
        let mut entries: Vec<SouthEntry> = g
            .out_edges(v, None)
            .into_iter()
            .map(|e| SouthEntry {
                pos: south_pos.get(e.w.as_str()).copied(),
                weight: g.edge(&e).weight,
            })
            .collect();
        lodash_sort_by(&mut entries, |e| e.pos.map(|p| p as f64));
        south_entries.extend(entries);
    }

    let mut first_index = 1usize;
    while first_index < south_layer.len() {
        first_index <<= 1;
    }
    let tree_size = 2 * first_index - 1;
    first_index -= 1;
    let mut tree = vec![0.0f64; tree_size];

    let mut cc = 0.0;
    for entry in south_entries {
        // An edge into a node outside the south layer cannot occur after
        // normalization; JS would index the tree with NaN.
        let Some(pos) = entry.pos else { continue };
        let mut index = pos + first_index;
        tree[index] += entry.weight;
        let mut weight_sum = 0.0;
        while index > 0 {
            if index % 2 == 1 {
                weight_sum += tree[index + 1];
            }
            index = (index - 1) >> 1;
            tree[index] += entry.weight;
        }
        cc += entry.weight * weight_sum;
    }

    cc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::EdgeLabel;
    use crate::mermaid::layout::dagre::graphlib::EdgeKey;
    use crate::mermaid::layout::dagre::util::new_layout_graph;

    fn layers(spec: &[&[&str]]) -> Vec<Vec<String>> {
        spec.iter()
            .map(|l| l.iter().map(|s| s.to_string()).collect())
            .collect()
    }

    fn edge(g: &mut LayoutGraph, v: &str, w: &str, weight: f64) {
        g.set_edge(
            &EdgeKey::new(v, w),
            EdgeLabel {
                weight,
                ..EdgeLabel::default()
            },
        );
    }

    #[test]
    fn returns_zero_for_an_empty_layering_and_for_no_crossings() {
        let mut g = new_layout_graph();
        assert_eq!(cross_count(&g, &[]), 0.0);
        edge(&mut g, "a1", "b1", 1.0);
        edge(&mut g, "a2", "b2", 1.0);
        assert_eq!(
            cross_count(&g, &layers(&[&["a1", "a2"], &["b1", "b2"]])),
            0.0
        );
    }

    #[test]
    fn counts_a_single_crossing() {
        let mut g = new_layout_graph();
        edge(&mut g, "a1", "b1", 1.0);
        edge(&mut g, "a2", "b2", 1.0);
        assert_eq!(
            cross_count(&g, &layers(&[&["a1", "a2"], &["b2", "b1"]])),
            1.0
        );
    }

    #[test]
    fn weights_crossings_by_the_product_of_edge_weights() {
        let mut g = new_layout_graph();
        edge(&mut g, "a1", "b1", 2.0);
        edge(&mut g, "a2", "b2", 3.0);
        assert_eq!(
            cross_count(&g, &layers(&[&["a1", "a2"], &["b2", "b1"]])),
            6.0
        );
    }

    #[test]
    fn counts_crossings_across_several_layers() {
        let mut g = new_layout_graph();
        edge(&mut g, "a1", "b1", 1.0);
        edge(&mut g, "a2", "b2", 1.0);
        edge(&mut g, "b1", "c1", 1.0);
        edge(&mut g, "b2", "c2", 1.0);
        let l = layers(&[&["a1", "a2"], &["b2", "b1"], &["c1", "c2"]]);
        assert_eq!(cross_count(&g, &l), 2.0);
    }

    #[test]
    fn counts_the_classic_six_edge_example() {
        let mut g = new_layout_graph();
        for (v, w) in [
            ("a1", "b1"),
            ("a1", "b3"),
            ("a2", "b2"),
            ("a2", "b4"),
            ("a3", "b1"),
            ("a3", "b3"),
        ] {
            edge(&mut g, v, w, 1.0);
        }
        let l = layers(&[&["a1", "a2", "a3"], &["b1", "b2", "b3", "b4"]]);
        assert_eq!(cross_count(&g, &l), 5.0);
    }
}
