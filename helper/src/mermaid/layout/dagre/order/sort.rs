//! Port of `src/dagre/order/sort.js`: entries with a barycenter are sorted,
//! entries without one keep their original index.

use std::cmp::Ordering;

use super::super::util::{lodash_sort_by, partition, stable_sort_by};
use super::resolve_conflicts::ResolvedEntry;

/// `{vs, barycenter, weight}` returned by `sort` and `sortSubgraph`.
#[derive(Clone, Debug, PartialEq)]
pub struct SortResult {
    pub vs: Vec<String>,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

/// `sort(entries, biasRight)`.
pub fn sort(entries: Vec<ResolvedEntry>, bias_right: bool) -> SortResult {
    let (mut sortable, mut unsortable) = partition(entries, |e| e.barycenter.is_some());
    lodash_sort_by(&mut unsortable, |e| Some(-(e.i as f64)));
    let mut vs: Vec<Vec<String>> = Vec::new();
    let mut sum = 0.0;
    let mut weight = 0.0;

    stable_sort_by(&mut sortable, compare_with_bias(bias_right));

    let mut vs_index = consume_unsortable(&mut vs, &mut unsortable, 0);

    for entry in sortable {
        vs_index += entry.vs.len();
        sum += entry.barycenter.unwrap_or(f64::NAN) * entry.weight.unwrap_or(f64::NAN);
        weight += entry.weight.unwrap_or(f64::NAN);
        vs.push(entry.vs);
        vs_index = consume_unsortable(&mut vs, &mut unsortable, vs_index);
    }

    let mut result = SortResult {
        vs: vs.into_iter().flatten().collect(),
        barycenter: None,
        weight: None,
    };
    if weight != 0.0 && !weight.is_nan() {
        result.barycenter = Some(sum / weight);
        result.weight = Some(weight);
    }
    result
}

/// `consumeUnsortable(vs, unsortable, index)`: pops every unsortable entry
/// whose original index has been reached.
fn consume_unsortable(
    vs: &mut Vec<Vec<String>>,
    unsortable: &mut Vec<ResolvedEntry>,
    mut index: usize,
) -> usize {
    while let Some(last) = unsortable.last() {
        if last.i > index {
            break;
        }
        let Some(last) = unsortable.pop() else { break };
        vs.push(last.vs);
        index += 1;
    }
    index
}

/// `compareWithBias(bias)`: by barycenter, then by index (reversed when
/// biased right). A `NaN` barycenter compares equal to everything.
fn compare_with_bias(bias: bool) -> impl FnMut(&ResolvedEntry, &ResolvedEntry) -> Ordering {
    move |a, b| {
        let (ab, bb) = (
            a.barycenter.unwrap_or(f64::NAN),
            b.barycenter.unwrap_or(f64::NAN),
        );
        if ab < bb {
            return Ordering::Less;
        } else if ab > bb {
            return Ordering::Greater;
        }
        if bias { b.i.cmp(&a.i) } else { a.i.cmp(&b.i) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(vs: &[&str], i: usize, barycenter: Option<f64>, weight: Option<f64>) -> ResolvedEntry {
        ResolvedEntry {
            vs: vs.iter().map(|s| s.to_string()).collect(),
            i,
            barycenter,
            weight,
        }
    }

    #[test]
    fn sorts_nodes_by_barycenter() {
        let out = sort(
            vec![
                entry(&["a"], 0, Some(2.0), Some(3.0)),
                entry(&["b"], 1, Some(1.0), Some(2.0)),
            ],
            false,
        );
        assert_eq!(out.vs, vec!["b", "a"]);
        assert_eq!(out.barycenter, Some((2.0 * 3.0 + 1.0 * 2.0) / 5.0));
        assert_eq!(out.weight, Some(5.0));
    }

    #[test]
    fn can_sort_super_nodes() {
        let out = sort(
            vec![
                entry(&["a", "c", "d"], 0, Some(2.0), Some(3.0)),
                entry(&["b"], 1, Some(1.0), Some(2.0)),
            ],
            false,
        );
        assert_eq!(out.vs, vec!["b", "a", "c", "d"]);
    }

    #[test]
    fn biases_to_the_left_by_default_and_right_when_asked() {
        let entries = vec![
            entry(&["a"], 0, Some(1.0), Some(1.0)),
            entry(&["b"], 1, Some(1.0), Some(1.0)),
        ];
        assert_eq!(sort(entries.clone(), false).vs, vec!["a", "b"]);
        assert_eq!(sort(entries, true).vs, vec!["b", "a"]);
    }

    #[test]
    fn keeps_unsortable_entries_at_their_index() {
        let out = sort(
            vec![
                entry(&["a"], 0, Some(2.0), Some(1.0)),
                entry(&["x"], 1, None, None),
                entry(&["b"], 2, Some(1.0), Some(1.0)),
                entry(&["y"], 3, None, None),
            ],
            false,
        );
        assert_eq!(out.vs, vec!["b", "x", "a", "y"]);
    }

    #[test]
    fn leading_unsortable_entries_come_first() {
        let out = sort(
            vec![
                entry(&["x"], 0, None, None),
                entry(&["a"], 1, Some(1.0), Some(1.0)),
            ],
            false,
        );
        assert_eq!(out.vs, vec!["x", "a"]);
        assert_eq!(out.weight, Some(1.0));
    }

    #[test]
    fn no_sortable_entries_means_no_barycenter() {
        let out = sort(vec![entry(&["x"], 0, None, None)], false);
        assert_eq!(out.vs, vec!["x"]);
        assert_eq!(out.barycenter, None);
        assert_eq!(out.weight, None);
    }
}
