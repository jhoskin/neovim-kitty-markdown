//! d3-hierarchy 3.1.2: `hierarchy` with `sum`, `sort`, and the traversal
//! orders (`eachBefore`, `eachAfter`, `descendants`, `leaves`), and
//! `treemap` with its padding options, `round`, and the `treemapSquarify`
//! (`squarifyRatio`), `treemapDice`, and `treemapSlice` tiles.
//!
//! The tree lives in an arena: node 0 is the root and every node keeps the
//! indices of its children in data order (after `sort`, in sorted order).
//! The arithmetic keeps the JavaScript float behavior: `Math.max` returns
//! `NaN` when an argument is `NaN`, `Math.round` rounds half up, and a
//! falsy `parent.value` gives the dice and slice tiles a zero scale.

/// One `Node` of `hierarchy(data)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Node<T> {
    pub data: T,
    pub depth: usize,
    pub height: usize,
    pub parent: Option<usize>,
    /// `node.children`; `None` for a leaf, never an empty list.
    pub children: Option<Vec<usize>>,
    /// Set by [`Hierarchy::sum`]; 0 until then.
    pub value: f64,
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

/// The arena `hierarchy(data, children)` builds.
#[derive(Debug, Clone, PartialEq)]
pub struct Hierarchy<T> {
    pub nodes: Vec<Node<T>>,
}

/// JavaScript `Math.max` of two numbers.
fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

/// JavaScript `Math.round`: the nearest integer, ties toward +∞.
pub fn js_round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let floor = x.floor();
    // `x - floor` is exact for every finite double.
    if x - floor >= 0.5 { floor + 1.0 } else { floor }
}

/// JavaScript truthiness of a number.
fn truthy(x: f64) -> bool {
    x != 0.0 && !x.is_nan()
}

impl<T> Hierarchy<T> {
    /// `hierarchy(data, children)`: `children(d)` returns the child data,
    /// and an empty list makes a leaf. Heights come from `computeHeight`.
    pub fn new(root: T, mut children: impl FnMut(&T) -> Vec<T>) -> Hierarchy<T> {
        let mut h = Hierarchy {
            nodes: vec![Node::new(root, 0, None)],
        };
        // `while (node = nodes.pop())` with children pushed last to first,
        // so the arena fills in pre-order; the order is not observable.
        let mut stack = vec![0];
        while let Some(index) = stack.pop() {
            let childs = children(&h.nodes[index].data);
            if childs.is_empty() {
                continue;
            }
            let depth = h.nodes[index].depth + 1;
            let mut ids = Vec::with_capacity(childs.len());
            for data in childs {
                ids.push(h.nodes.len());
                h.nodes.push(Node::new(data, depth, Some(index)));
            }
            stack.extend(ids.iter().rev().copied());
            h.nodes[index].children = Some(ids);
        }
        for index in h.each_before() {
            h.compute_height(index);
        }
        h
    }

    /// `computeHeight(node)`.
    fn compute_height(&mut self, mut index: usize) {
        let mut height = 0;
        loop {
            self.nodes[index].height = height;
            let Some(parent) = self.nodes[index].parent else {
                break;
            };
            height += 1;
            if self.nodes[parent].height >= height {
                break;
            }
            index = parent;
        }
    }

    pub fn root(&self) -> &Node<T> {
        &self.nodes[0]
    }

    fn children_of(&self, index: usize) -> &[usize] {
        self.nodes[index].children.as_deref().unwrap_or(&[])
    }

    /// `node.eachBefore` from the root: pre-order.
    pub fn each_before(&self) -> Vec<usize> {
        let mut out = Vec::with_capacity(self.nodes.len());
        let mut stack = vec![0];
        while let Some(index) = stack.pop() {
            out.push(index);
            stack.extend(self.children_of(index).iter().rev().copied());
        }
        out
    }

    /// `node.eachAfter` from the root: every node after its children.
    pub fn each_after(&self) -> Vec<usize> {
        let mut next = Vec::with_capacity(self.nodes.len());
        let mut stack = vec![0];
        while let Some(index) = stack.pop() {
            next.push(index);
            stack.extend(self.children_of(index).iter().copied());
        }
        next.reverse();
        next
    }

    /// `node.descendants()`: the breadth-first iterator order.
    pub fn descendants(&self) -> Vec<usize> {
        let mut out = Vec::with_capacity(self.nodes.len());
        let mut level = vec![0];
        while !level.is_empty() {
            let mut next = Vec::new();
            for &index in &level {
                out.push(index);
                next.extend(self.children_of(index).iter().copied());
            }
            level = next;
        }
        out
    }

    /// `node.leaves()`: the nodes without children, in pre-order.
    pub fn leaves(&self) -> Vec<usize> {
        self.each_before()
            .into_iter()
            .filter(|&i| self.nodes[i].children.is_none())
            .collect()
    }

    /// `node.sum(value)`: `+value(d) || 0` plus the children, added from
    /// the last child to the first.
    pub fn sum(&mut self, value: impl Fn(&T) -> f64) -> &mut Self {
        for index in self.each_after() {
            let own = value(&self.nodes[index].data);
            let mut sum = if truthy(own) { own } else { 0.0 };
            if let Some(children) = &self.nodes[index].children {
                for &child in children.iter().rev() {
                    sum += self.nodes[child].value;
                }
            }
            self.nodes[index].value = sum;
        }
        self
    }

    /// `node.sort(compare)`: `Array.prototype.sort` on every children
    /// list, which is stable; a `NaN` comparison counts as equal.
    pub fn sort(&mut self, compare: impl Fn(&Node<T>, &Node<T>) -> f64) -> &mut Self {
        for index in self.each_before() {
            let Some(mut children) = self.nodes[index].children.take() else {
                continue;
            };
            children.sort_by(|&a, &b| {
                compare(&self.nodes[a], &self.nodes[b])
                    .partial_cmp(&0.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            self.nodes[index].children = Some(children);
        }
        self
    }
}

impl<T> Node<T> {
    fn new(data: T, depth: usize, parent: Option<usize>) -> Node<T> {
        Node {
            data,
            depth,
            height: 0,
            parent,
            children: None,
            value: 0.0,
            x0: 0.0,
            y0: 0.0,
            x1: 0.0,
            y1: 0.0,
        }
    }
}

/// `treemap.tile(...)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tile {
    /// `treemapSquarify.ratio(r)`; [`Tile::squarify`] is the default.
    Squarify(f64),
    Dice,
    Slice,
}

/// `phi`, the golden ratio `treemapSquarify` targets.
pub const PHI: f64 = 1.618_033_988_749_895;

impl Tile {
    /// `treemapSquarify`: the golden ratio.
    pub fn squarify() -> Tile {
        Tile::Squarify((1.0 + 5f64.sqrt()) / 2.0)
    }

    /// `treemapSquarify.ratio(x)`: values at or below 1 become 1.
    pub fn squarify_ratio(ratio: f64) -> Tile {
        Tile::Squarify(if ratio > 1.0 { ratio } else { 1.0 })
    }
}

/// A padding accessor: a constant or a function of the node.
pub type Padding<'a, T> = Box<dyn Fn(&Node<T>) -> f64 + 'a>;

/// `d3.treemap()` with its options.
pub struct Treemap<'a, T> {
    pub tile: Tile,
    pub round: bool,
    pub dx: f64,
    pub dy: f64,
    pub padding_inner: Padding<'a, T>,
    pub padding_top: Padding<'a, T>,
    pub padding_right: Padding<'a, T>,
    pub padding_bottom: Padding<'a, T>,
    pub padding_left: Padding<'a, T>,
}

fn constant<'a, T>(x: f64) -> Padding<'a, T> {
    Box::new(move |_| x)
}

impl<'a, T> Default for Treemap<'a, T> {
    fn default() -> Self {
        Treemap::new()
    }
}

impl<'a, T> Treemap<'a, T> {
    /// `treemap()`: squarify, no rounding, size `[1, 1]`, no padding.
    pub fn new() -> Treemap<'a, T> {
        Treemap {
            tile: Tile::squarify(),
            round: false,
            dx: 1.0,
            dy: 1.0,
            padding_inner: constant(0.0),
            padding_top: constant(0.0),
            padding_right: constant(0.0),
            padding_bottom: constant(0.0),
            padding_left: constant(0.0),
        }
    }

    pub fn tile(mut self, tile: Tile) -> Self {
        self.tile = tile;
        self
    }

    pub fn round(mut self, round: bool) -> Self {
        self.round = round;
        self
    }

    pub fn size(mut self, size: [f64; 2]) -> Self {
        self.dx = size[0];
        self.dy = size[1];
        self
    }

    pub fn padding_inner(mut self, f: impl Fn(&Node<T>) -> f64 + 'a) -> Self {
        self.padding_inner = Box::new(f);
        self
    }

    pub fn padding_top(mut self, f: impl Fn(&Node<T>) -> f64 + 'a) -> Self {
        self.padding_top = Box::new(f);
        self
    }

    pub fn padding_right(mut self, f: impl Fn(&Node<T>) -> f64 + 'a) -> Self {
        self.padding_right = Box::new(f);
        self
    }

    pub fn padding_bottom(mut self, f: impl Fn(&Node<T>) -> f64 + 'a) -> Self {
        self.padding_bottom = Box::new(f);
        self
    }

    pub fn padding_left(mut self, f: impl Fn(&Node<T>) -> f64 + 'a) -> Self {
        self.padding_left = Box::new(f);
        self
    }

    /// `treemap.paddingOuter(x)`: the same constant on all four sides.
    pub fn padding_outer(self, x: f64) -> Self {
        self.padding_top(move |_| x)
            .padding_right(move |_| x)
            .padding_bottom(move |_| x)
            .padding_left(move |_| x)
    }

    /// `treemap.padding(x)`: `paddingInner(x).paddingOuter(x)`.
    pub fn padding(self, x: f64) -> Self {
        self.padding_inner(move |_| x).padding_outer(x)
    }

    /// `treemap(root)`: positions every node, then rounds when asked.
    pub fn layout(&self, h: &mut Hierarchy<T>) {
        {
            let root = &mut h.nodes[0];
            root.x0 = 0.0;
            root.y0 = 0.0;
            root.x1 = self.dx;
            root.y1 = self.dy;
        }
        let order = h.each_before();
        let mut padding_stack = vec![0.0];
        for &index in &order {
            self.position_node(h, index, &mut padding_stack);
        }
        if self.round {
            for &index in &order {
                let n = &mut h.nodes[index];
                n.x0 = js_round(n.x0);
                n.y0 = js_round(n.y0);
                n.x1 = js_round(n.x1);
                n.y1 = js_round(n.y1);
            }
        }
    }

    /// `positionNode(node)`.
    fn position_node(&self, h: &mut Hierarchy<T>, index: usize, padding_stack: &mut Vec<f64>) {
        let depth = h.nodes[index].depth;
        let mut p = padding_stack.get(depth).copied().unwrap_or(f64::NAN);
        let node = &h.nodes[index];
        let (mut x0, mut y0, mut x1, mut y1) = (node.x0 + p, node.y0 + p, node.x1 - p, node.y1 - p);
        if x1 < x0 {
            x0 = (x0 + x1) / 2.0;
            x1 = x0;
        }
        if y1 < y0 {
            y0 = (y0 + y1) / 2.0;
            y1 = y0;
        }
        {
            let node = &mut h.nodes[index];
            node.x0 = x0;
            node.y0 = y0;
            node.x1 = x1;
            node.y1 = y1;
        }
        let node = &h.nodes[index];
        if node.children.is_none() {
            return;
        }
        p = (self.padding_inner)(node) / 2.0;
        if padding_stack.len() <= depth + 1 {
            padding_stack.resize(depth + 2, f64::NAN);
        }
        padding_stack[depth + 1] = p;
        x0 += (self.padding_left)(node) - p;
        y0 += (self.padding_top)(node) - p;
        x1 -= (self.padding_right)(node) - p;
        y1 -= (self.padding_bottom)(node) - p;
        if x1 < x0 {
            x0 = (x0 + x1) / 2.0;
            x1 = x0;
        }
        if y1 < y0 {
            y0 = (y0 + y1) / 2.0;
            y1 = y0;
        }
        match self.tile {
            Tile::Squarify(ratio) => squarify_ratio(ratio, h, index, x0, y0, x1, y1),
            Tile::Dice => {
                let children = h.children_of(index).to_vec();
                let value = h.nodes[index].value;
                dice(h, &children, value, x0, y0, x1, y1);
            }
            Tile::Slice => {
                let children = h.children_of(index).to_vec();
                let value = h.nodes[index].value;
                slice(h, &children, value, x0, y0, x1, y1);
            }
        }
    }
}

/// `treemapDice(parent, x0, y0, x1, y1)` over `nodes` with the parent's
/// `value`: left to right, widths proportional to the values.
pub fn dice<T>(
    h: &mut Hierarchy<T>,
    nodes: &[usize],
    value: f64,
    mut x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) {
    // `parent.value && (x1 - x0) / parent.value`.
    let k = if truthy(value) {
        (x1 - x0) / value
    } else {
        value
    };
    for &index in nodes {
        let node = &mut h.nodes[index];
        node.y0 = y0;
        node.y1 = y1;
        node.x0 = x0;
        x0 += node.value * k;
        node.x1 = x0;
    }
}

/// `treemapSlice(parent, x0, y0, x1, y1)`: top to bottom.
pub fn slice<T>(
    h: &mut Hierarchy<T>,
    nodes: &[usize],
    value: f64,
    x0: f64,
    mut y0: f64,
    x1: f64,
    y1: f64,
) {
    let k = if truthy(value) {
        (y1 - y0) / value
    } else {
        value
    };
    for &index in nodes {
        let node = &mut h.nodes[index];
        node.x0 = x0;
        node.x1 = x1;
        node.y0 = y0;
        y0 += node.value * k;
        node.y1 = y0;
    }
}

/// `squarifyRatio(ratio, parent, x0, y0, x1, y1)`: rows of children whose
/// worst aspect ratio stops improving, each diced or sliced across the
/// shorter side of the remaining rectangle.
pub fn squarify_ratio<T>(
    ratio: f64,
    h: &mut Hierarchy<T>,
    parent: usize,
    mut x0: f64,
    mut y0: f64,
    x1: f64,
    y1: f64,
) {
    let nodes = h.children_of(parent).to_vec();
    let n = nodes.len();
    let mut value = h.nodes[parent].value;
    let (mut i0, mut i1) = (0, 0);
    while i0 < n {
        let dx = x1 - x0;
        let dy = y1 - y0;

        // Find the next non-empty node.
        let mut sum_value;
        loop {
            sum_value = h.nodes[nodes[i1]].value;
            i1 += 1;
            if truthy(sum_value) || i1 >= n {
                break;
            }
        }
        let mut min_value = sum_value;
        let mut max_value = sum_value;
        let alpha = js_max(dy / dx, dx / dy) / (value * ratio);
        let mut beta = sum_value * sum_value * alpha;
        let mut min_ratio = js_max(max_value / beta, beta / min_value);

        // Keep adding nodes while the aspect ratio maintains or improves.
        while i1 < n {
            let node_value = h.nodes[nodes[i1]].value;
            sum_value += node_value;
            if node_value < min_value {
                min_value = node_value;
            }
            if node_value > max_value {
                max_value = node_value;
            }
            beta = sum_value * sum_value * alpha;
            let new_ratio = js_max(max_value / beta, beta / min_value);
            if new_ratio > min_ratio {
                sum_value -= node_value;
                break;
            }
            min_ratio = new_ratio;
            i1 += 1;
        }

        // Position the row: `dice` when the rectangle is taller than wide.
        // The tile gets the old edge, then the edge moves past the row.
        let row = &nodes[i0..i1];
        if dx < dy {
            let start = y0;
            let end = if truthy(value) {
                y0 += dy * sum_value / value;
                y0
            } else {
                y1
            };
            dice(h, row, sum_value, x0, start, x1, end);
        } else {
            let start = x0;
            let end = if truthy(value) {
                x0 += dx * sum_value / value;
                x0
            } else {
                x1
            };
            slice(h, row, sum_value, start, y0, end, y1);
        }
        value -= sum_value;
        i0 = i1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_round_ties_up() {
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(0.499_999_999_999_999_94), 0.0);
        assert_eq!(js_round(-0.4), 0.0);
    }

    #[test]
    fn js_max_propagates_nan() {
        assert!(js_max(f64::NAN, 1.0).is_nan());
        assert!(js_max(1.0, f64::NAN).is_nan());
        assert_eq!(js_max(1.0, 2.0), 2.0);
    }

    #[test]
    fn phi_matches_the_formula() {
        assert_eq!(Tile::squarify(), Tile::Squarify(PHI));
        assert_eq!(Tile::squarify_ratio(0.5), Tile::Squarify(1.0));
    }
}
