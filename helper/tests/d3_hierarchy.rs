//! `layout::d3::hierarchy` against d3-hierarchy 3.1.2. The expected
//! coordinates are what node printed for the same trees and options with
//! the real library (`d3.hierarchy(data).sum(d => d.value ?? 0)`, an
//! optional `sort((a, b) => b.value - a.value)`, then the treemap), every
//! node in `eachBefore` order.

use kitty_markdown_helper::mermaid::layout::d3::hierarchy::{Hierarchy, Tile, Treemap};

#[derive(Debug, Clone)]
struct Datum {
    name: &'static str,
    value: Option<f64>,
    children: Vec<Datum>,
}

fn leaf(name: &'static str, value: f64) -> Datum {
    Datum {
        name,
        value: Some(value),
        children: Vec::new(),
    }
}

fn section(name: &'static str, children: Vec<Datum>) -> Datum {
    Datum {
        name,
        value: None,
        children,
    }
}

type Expected = [(&'static str, f64, f64, f64, f64)];

fn build(data: Datum, sorted: bool) -> Hierarchy<Datum> {
    let mut h = Hierarchy::new(data, |d| d.children.clone());
    h.sum(|d| d.value.unwrap_or(0.0));
    if sorted {
        h.sort(|a, b| b.value - a.value);
    }
    h
}

fn check(h: &Hierarchy<Datum>, expected: &Expected) {
    let order = h.each_before();
    assert_eq!(order.len(), expected.len());
    for (&index, &(name, x0, y0, x1, y1)) in order.iter().zip(expected) {
        let n = &h.nodes[index];
        assert_eq!(n.data.name, name);
        for (got, want, field) in [
            (n.x0, x0, "x0"),
            (n.y0, y0, "y0"),
            (n.x1, x1, "x1"),
            (n.y1, y1, "y1"),
        ] {
            assert!(
                (got - want).abs() <= 1e-9,
                "{name}.{field}: got {got}, want {want}"
            );
        }
    }
}

/// The docs example with the options `renderer.ts` sets.
#[test]
fn squarify_with_the_mermaid_options() {
    let data = section(
        "",
        vec![
            section(
                "Section 1",
                vec![
                    leaf("Leaf 1.1", 12.0),
                    section("Section 1.2", vec![leaf("Leaf 1.2.1", 12.0)]),
                ],
            ),
            section(
                "Section 2",
                vec![
                    leaf("Leaf 2.1", 20.0),
                    leaf("Leaf 2.2", 25.0),
                    leaf("Leaf 2.3", 12.0),
                ],
            ),
        ],
    );
    let mut h = build(data, true);
    let branch = |d: &kitty_markdown_helper::mermaid::layout::d3::hierarchy::Node<Datum>| {
        if d.children.is_some() { 10.0 } else { 0.0 }
    };
    Treemap::new()
        .size([1000.0, 400.0])
        .padding_top(|d| if d.children.is_some() { 35.0 } else { 0.0 })
        .padding_inner(|_| 10.0)
        .padding_left(branch)
        .padding_right(branch)
        .padding_bottom(branch)
        .round(true)
        .layout(&mut h);
    check(
        &h,
        &[
            ("", 0.0, 0.0, 1000.0, 400.0),
            ("Section 2", 10.0, 35.0, 697.0, 390.0),
            ("Leaf 2.2", 20.0, 70.0, 307.0, 380.0),
            ("Leaf 2.1", 317.0, 70.0, 687.0, 260.0),
            ("Leaf 2.3", 317.0, 270.0, 687.0, 380.0),
            ("Section 1", 707.0, 35.0, 990.0, 390.0),
            ("Leaf 1.1", 717.0, 70.0, 843.0, 380.0),
            ("Section 1.2", 853.0, 70.0, 980.0, 380.0),
            ("Leaf 1.2.1", 863.0, 105.0, 970.0, 370.0),
        ],
    );
    assert_eq!(h.root().height, 3);
    assert_eq!(h.root().value, 81.0);
}

/// `treemapSquarify.ratio(1)`, unsorted, unrounded, with inner and outer
/// padding.
#[test]
fn squarify_ratio_one_unrounded() {
    let data = section(
        "r",
        vec![
            leaf("a", 6.0),
            leaf("b", 6.0),
            leaf("c", 4.0),
            leaf("d", 3.0),
            leaf("e", 2.0),
            leaf("f", 2.0),
            leaf("g", 1.0),
            section("h", vec![leaf("h1", 5.0), leaf("h2", 0.5)]),
        ],
    );
    let mut h = build(data, false);
    Treemap::new()
        .tile(Tile::squarify_ratio(1.0))
        .size([300.0, 200.0])
        .padding_inner(|_| 3.0)
        .padding_outer(2.0)
        .layout(&mut h);
    check(
        &h,
        &[
            ("r", 0.0, 0.0, 300.0, 200.0),
            ("a", 2.0, 2.0, 120.62711864406779, 98.5),
            ("b", 2.0, 101.5, 120.62711864406779, 198.0),
            ("c", 123.62711864406779, 2.0, 221.9830508474576, 78.6),
            ("d", 224.9830508474576, 2.0, 298.0, 78.6),
            ("e", 123.62711864406779, 81.6, 188.19774011299435, 138.3),
            ("f", 123.62711864406779, 141.3, 188.19774011299435, 198.0),
            ("g", 191.19774011299435, 81.6, 298.0, 96.96923076923076),
            ("h", 191.19774011299435, 99.96923076923076, 298.0, 198.0),
            (
                "h1",
                193.19774011299435,
                101.96923076923076,
                286.38161273754497,
                196.0,
            ),
            ("h2", 289.38161273754497, 101.96923076923076, 296.0, 196.0),
        ],
    );
}

/// `treemapDice` with `padding(1.5)`, sorted, and a zero leaf.
#[test]
fn dice_with_padding() {
    let data = section(
        "r",
        vec![
            leaf("a", 1.0),
            leaf("b", 2.0),
            section("c", vec![leaf("c1", 3.0), leaf("c2", 4.0)]),
            leaf("d", 0.0),
        ],
    );
    let mut h = build(data, true);
    Treemap::new()
        .tile(Tile::Dice)
        .size([120.0, 50.0])
        .padding(1.5)
        .layout(&mut h);
    check(
        &h,
        &[
            ("r", 0.0, 0.0, 120.0, 50.0),
            ("c", 1.5, 1.5, 82.95, 48.5),
            ("c2", 3.0, 3.0, 47.18571428571429, 47.0),
            ("c1", 48.68571428571429, 3.0, 81.45000000000002, 47.0),
            ("b", 84.45, 1.5, 106.65, 48.5),
            ("a", 108.15, 1.5, 118.5, 48.5),
            ("d", 119.25, 1.5, 119.25, 48.5),
        ],
    );
}

/// `treemapSlice`, unsorted, rounded, with zero-valued nodes and a top
/// padding on every node.
#[test]
fn slice_rounded_with_zeros() {
    let data = section(
        "r",
        vec![
            leaf("a", 0.0),
            leaf("b", 7.25),
            section("c", vec![leaf("c1", 1.0), leaf("c2", 0.0), leaf("c3", 2.0)]),
            leaf("d", 3.5),
        ],
    );
    let mut h = build(data, false);
    Treemap::new()
        .tile(Tile::Slice)
        .size([33.3, 100.0])
        .padding_top(|_| 4.0)
        .round(true)
        .layout(&mut h);
    check(
        &h,
        &[
            ("r", 0.0, 0.0, 33.0, 100.0),
            ("a", 0.0, 4.0, 33.0, 4.0),
            ("b", 0.0, 4.0, 33.0, 55.0),
            ("c", 0.0, 55.0, 33.0, 76.0),
            ("c1", 0.0, 59.0, 33.0, 64.0),
            ("c2", 0.0, 64.0, 33.0, 64.0),
            ("c3", 0.0, 64.0, 33.0, 76.0),
            ("d", 0.0, 76.0, 33.0, 100.0),
        ],
    );
}

/// `descendants()` is breadth first and `leaves()` pre-order.
#[test]
fn traversal_orders_match_d3() {
    let data = section(
        "r",
        vec![
            section("a", vec![leaf("a1", 1.0), leaf("a2", 1.0)]),
            leaf("b", 1.0),
            section("c", vec![leaf("c1", 1.0)]),
        ],
    );
    let h = build(data, false);
    let names = |ids: Vec<usize>| {
        ids.into_iter()
            .map(|i| h.nodes[i].data.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        names(h.descendants()),
        ["r", "a", "b", "c", "a1", "a2", "c1"]
    );
    assert_eq!(names(h.leaves()), ["a1", "a2", "b", "c1"]);
}
