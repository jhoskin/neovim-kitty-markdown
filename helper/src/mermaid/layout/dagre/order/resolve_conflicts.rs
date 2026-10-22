//! Port of `src/dagre/order/resolve-conflicts.js` (Forster, "A Fast and
//! Simple Heuristic for Constrained Two-Level Crossing Reduction").

use std::collections::HashMap;

use super::super::graphlib::js_key_order;
use super::ConstraintGraph;
use super::barycenter::Entry;

/// `{vs, i, barycenter, weight}`: a group of nodes that must stay in order.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedEntry {
    pub vs: Vec<String>,
    pub i: usize,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

struct Mapped {
    indegree: usize,
    in_entries: Vec<usize>,
    out: Vec<usize>,
    vs: Vec<String>,
    i: usize,
    barycenter: Option<f64>,
    weight: Option<f64>,
    merged: bool,
}

/// `resolveConflicts(entries, cg)`.
pub fn resolve_conflicts(entries: &[Entry], cg: &ConstraintGraph) -> Vec<ResolvedEntry> {
    let mut mapped: Vec<Mapped> = Vec::with_capacity(entries.len());
    let mut index: HashMap<&str, usize> = HashMap::new();
    for (i, entry) in entries.iter().enumerate() {
        index.insert(entry.v.as_str(), i);
        let (barycenter, weight) = match entry.barycenter {
            Some(b) => (Some(b), entry.weight),
            None => (None, None),
        };
        mapped.push(Mapped {
            indegree: 0,
            in_entries: Vec::new(),
            out: Vec::new(),
            vs: vec![entry.v.clone()],
            i,
            barycenter,
            weight,
            merged: false,
        });
    }

    for e in cg.edges() {
        if let (Some(&iv), Some(&iw)) = (index.get(e.v.as_str()), index.get(e.w.as_str())) {
            mapped[iw].indegree += 1;
            mapped[iv].out.push(iw);
        }
    }

    // `_.filter(mappedEntries, ...)` walks a JS object keyed by node id.
    let source_set: Vec<usize> = js_key_order(entries.iter().map(|e| e.v.as_str()))
        .iter()
        .filter_map(|v| index.get(v.as_str()).copied())
        .filter(|&i| mapped[i].indegree == 0)
        .collect();

    do_resolve_conflicts(&mut mapped, source_set)
}

/// `doResolveConflicts(sourceSet)`.
fn do_resolve_conflicts(mapped: &mut [Mapped], mut source_set: Vec<usize>) -> Vec<ResolvedEntry> {
    let mut entries: Vec<usize> = Vec::new();

    while let Some(entry) = source_set.pop() {
        entries.push(entry);

        mapped[entry].in_entries.reverse();
        let ins = mapped[entry].in_entries.clone();
        for u in ins {
            if mapped[u].merged {
                continue;
            }
            let merge = match (mapped[u].barycenter, mapped[entry].barycenter) {
                (Some(ub), Some(vb)) => ub >= vb,
                _ => true,
            };
            if merge {
                merge_entries(mapped, entry, u);
            }
        }

        let outs = mapped[entry].out.clone();
        for w in outs {
            mapped[w].in_entries.push(entry);
            mapped[w].indegree -= 1;
            if mapped[w].indegree == 0 {
                source_set.push(w);
            }
        }
    }

    entries
        .into_iter()
        .filter(|&i| !mapped[i].merged)
        .map(|i| {
            let m = &mapped[i];
            ResolvedEntry {
                vs: m.vs.clone(),
                i: m.i,
                barycenter: m.barycenter,
                weight: m.weight,
            }
        })
        .collect()
}

/// `mergeEntries(target, source)`: `if (target.weight)` skips a missing or
/// zero weight, so two weightless entries end with a `NaN` barycenter.
fn merge_entries(mapped: &mut [Mapped], target: usize, source: usize) {
    let mut sum = 0.0;
    let mut weight = 0.0;

    if let Some(w) = truthy(mapped[target].weight) {
        sum += mapped[target].barycenter.unwrap_or(f64::NAN) * w;
        weight += w;
    }
    if let Some(w) = truthy(mapped[source].weight) {
        sum += mapped[source].barycenter.unwrap_or(f64::NAN) * w;
        weight += w;
    }

    let mut vs = mapped[source].vs.clone();
    vs.append(&mut mapped[target].vs);
    let source_i = mapped[source].i;
    let t = &mut mapped[target];
    t.vs = vs;
    t.barycenter = Some(sum / weight);
    t.weight = Some(weight);
    t.i = source_i.min(t.i);
    mapped[source].merged = true;
}

fn truthy(value: Option<f64>) -> Option<f64> {
    value.filter(|w| *w != 0.0 && !w.is_nan())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::graphlib::{EdgeKey, Graph};

    fn entry(v: &str, barycenter: Option<f64>, weight: Option<f64>) -> Entry {
        Entry {
            v: v.into(),
            barycenter,
            weight,
        }
    }

    fn by_v<'a>(entries: &'a [ResolvedEntry], v: &str) -> &'a ResolvedEntry {
        entries
            .iter()
            .find(|e| e.vs.contains(&v.to_string()))
            .unwrap_or_else(|| panic!("no entry holds {v}"))
    }

    #[test]
    fn returns_an_empty_list_for_an_empty_input() {
        let cg: ConstraintGraph = Graph::default();
        assert!(resolve_conflicts(&[], &cg).is_empty());
    }

    #[test]
    fn returns_a_singleton_for_a_single_node() {
        let cg: ConstraintGraph = Graph::default();
        let out = resolve_conflicts(&[entry("a", Some(2.0), Some(3.0))], &cg);
        assert_eq!(
            out,
            vec![ResolvedEntry {
                vs: vec!["a".into()],
                i: 0,
                barycenter: Some(2.0),
                weight: Some(3.0)
            }]
        );
    }

    #[test]
    fn does_not_merge_nodes_that_satisfy_the_constraint() {
        let mut cg: ConstraintGraph = Graph::default();
        cg.ensure_edge(&EdgeKey::new("a", "b"));
        let out = resolve_conflicts(
            &[
                entry("a", Some(1.0), Some(1.0)),
                entry("b", Some(2.0), Some(1.0)),
            ],
            &cg,
        );
        assert_eq!(out.len(), 2);
        assert_eq!(by_v(&out, "a").vs, vec!["a"]);
        assert_eq!(by_v(&out, "b").vs, vec!["b"]);
    }

    #[test]
    fn merges_nodes_that_violate_the_constraint() {
        let mut cg: ConstraintGraph = Graph::default();
        cg.ensure_edge(&EdgeKey::new("a", "b"));
        let out = resolve_conflicts(
            &[
                entry("a", Some(3.0), Some(1.0)),
                entry("b", Some(2.0), Some(1.0)),
            ],
            &cg,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].vs, vec!["a", "b"]);
        assert_eq!(out[0].i, 0);
        assert_eq!(out[0].barycenter, Some(2.5));
        assert_eq!(out[0].weight, Some(2.0));
    }

    #[test]
    fn merges_a_constrained_node_without_a_barycenter() {
        let mut cg: ConstraintGraph = Graph::default();
        cg.ensure_edge(&EdgeKey::new("a", "b"));
        let out = resolve_conflicts(
            &[entry("a", None, None), entry("b", Some(2.0), Some(1.0))],
            &cg,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].vs, vec!["a", "b"]);
        assert_eq!(out[0].barycenter, Some(2.0));
        assert_eq!(out[0].weight, Some(1.0));
    }

    #[test]
    fn handles_a_chain_of_violated_constraints() {
        let mut cg: ConstraintGraph = Graph::default();
        cg.ensure_edge(&EdgeKey::new("a", "b"));
        cg.ensure_edge(&EdgeKey::new("b", "c"));
        let out = resolve_conflicts(
            &[
                entry("a", Some(4.0), Some(1.0)),
                entry("b", Some(3.0), Some(1.0)),
                entry("c", Some(2.0), Some(1.0)),
            ],
            &cg,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].vs, vec!["a", "b", "c"]);
        assert_eq!(out[0].barycenter, Some(3.0));
        assert_eq!(out[0].weight, Some(3.0));
    }

    #[test]
    fn ignores_constraints_on_nodes_outside_the_entries() {
        let mut cg: ConstraintGraph = Graph::default();
        cg.ensure_edge(&EdgeKey::new("x", "a"));
        let out = resolve_conflicts(&[entry("a", Some(1.0), Some(1.0))], &cg);
        assert_eq!(out.len(), 1);
    }
}
