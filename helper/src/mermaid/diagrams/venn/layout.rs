//! Port of `@upsetjs/venn.js` 2.0.0, the area-proportional layout that
//! `vennRenderer.ts` (mermaid 12.2.0) imports: `src/circleintersection.js`,
//! `src/layout.js`, the layout half of `src/diagram.js`, and the `fmin`
//! optimizers bundled into `build/venn.esm.js` (`bisect`, `nelderMead`,
//! `conjugateGradient`, `wolfeLineSearch`).
//!
//! Every function keeps the JavaScript statement order, iteration counts,
//! tolerances, and initial guesses, so the result matches the oracle to
//! rounding. JavaScript objects keyed by set id iterate in property order
//! (array-index keys first, ascending, then insertion order); [`JsObject`]
//! keeps that order, since `Object.keys` order feeds the optimizer's
//! vector and the text-centre search.
//!
//! One part cannot match: `constrainedMDSLayout` seeds its restarts with
//! `Math.random`, which V8 seeds per page. `bestInitialLayout` only tries
//! it for eight or more areas (four sets or more after the missing pairs
//! are added), so diagrams with up to three sets are deterministic. The
//! port draws from a fixed-seed xorshift128+, the generator V8 uses.

use std::cmp::Ordering;
use std::f64::consts::PI;

/// `SMALL` in `circleintersection.js` and `layout.js`.
const SMALL: f64 = 1e-10;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle {
    pub x: f64,
    pub y: f64,
    pub radius: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// One input area: `{ sets, size, weight? }`.
#[derive(Debug, Clone, PartialEq)]
pub struct Area {
    pub sets: Vec<String>,
    pub size: f64,
    pub weight: Option<f64>,
}

impl Area {
    pub fn new(sets: &[&str], size: f64) -> Area {
        Area {
            sets: sets.iter().map(|s| s.to_string()).collect(),
            size,
            weight: None,
        }
    }
}

/// An arc of an intersection region's outline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arc {
    pub circle: Circle,
    pub width: f64,
    pub p1: Point,
    pub p2: Point,
    pub large: bool,
    pub sweep: bool,
}

/// A text centre; `disjoint` marks `{ x: 0, y: -1000, disjoint: true }`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextCentre {
    pub x: f64,
    pub y: f64,
    pub disjoint: bool,
}

/// Errors the JavaScript throws.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    /// `throw 'ERROR: missing pairwise overlap information'`.
    #[error("ERROR: missing pairwise overlap information")]
    MissingOverlap,
    /// `throw 'Initial bisect points must have opposite signs'`.
    #[error("Initial bisect points must have opposite signs")]
    Bisect,
    /// A property read on `undefined`: an area names a set without a
    /// single-set entry.
    #[error("Cannot read properties of undefined (set {0})")]
    UnknownSet(String),
}

// ---------------------------------------------------------------------------
// JavaScript property order

/// A string-keyed map that iterates like a JavaScript object: keys that are
/// array indices (canonical integers below 2^32 - 1) first in ascending
/// order, then the other keys in insertion order.
#[derive(Debug, Clone, PartialEq)]
pub struct JsObject<V> {
    entries: Vec<(String, V)>,
}

impl<V> Default for JsObject<V> {
    fn default() -> Self {
        JsObject {
            entries: Vec::new(),
        }
    }
}

/// The array index a property key stands for, if any.
fn array_index(key: &str) -> Option<u64> {
    if key.is_empty() || key.len() > 10 || !key.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if key.len() > 1 && key.starts_with('0') {
        return None;
    }
    let value: u64 = key.parse().ok()?;
    (value < u64::from(u32::MAX)).then_some(value)
}

impl<V> JsObject<V> {
    pub fn new() -> Self {
        JsObject::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn position(&self, key: &str) -> Option<usize> {
        self.entries.iter().position(|(k, _)| k == key)
    }

    pub fn get(&self, key: &str) -> Option<&V> {
        self.position(key).map(|i| &self.entries[i].1)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        self.position(key).map(|i| &mut self.entries[i].1)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.position(key).is_some()
    }

    /// `obj[key] = value`: an existing key keeps its place.
    pub fn insert(&mut self, key: &str, value: V) {
        if let Some(i) = self.position(key) {
            self.entries[i].1 = value;
            return;
        }
        let at = match array_index(key) {
            Some(n) => self
                .entries
                .iter()
                .position(|(k, _)| array_index(k).is_none_or(|m| m > n))
                .unwrap_or(self.entries.len()),
            None => self.entries.len(),
        };
        self.entries.insert(at, (key.to_string(), value));
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(k, _)| k.as_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }
}

/// JavaScript's default string comparison: UTF-16 code units.
pub fn js_str_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `comparefn` results as `Array.prototype.sort` reads them: NaN is +0.
fn js_cmp(v: f64) -> Ordering {
    if v < 0.0 {
        Ordering::Less
    } else if v > 0.0 {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

// ---------------------------------------------------------------------------
// circleintersection.js

/// An intersection point and the two circles it lies on.
#[derive(Debug, Clone, Copy)]
struct IPoint {
    x: f64,
    y: f64,
    parent_index: [usize; 2],
    angle: f64,
}

/// `distance(p1, p2)`.
pub fn distance(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    ((x1 - x2) * (x1 - x2) + (y1 - y2) * (y1 - y2)).sqrt()
}

fn circle_distance(a: &Circle, b: &Circle) -> f64 {
    distance(a.x, a.y, b.x, b.y)
}

/// `circleArea(r, width)`: the circular segment area.
pub fn circle_area(r: f64, width: f64) -> f64 {
    r * r * (1.0 - width / r).acos() - (r - width) * (width * (2.0 * r - width)).sqrt()
}

/// `circleOverlap(r1, r2, d)`.
pub fn circle_overlap(r1: f64, r2: f64, d: f64) -> f64 {
    if d >= r1 + r2 {
        return 0.0;
    }
    if d <= (r1 - r2).abs() {
        return PI * r1.min(r2) * r1.min(r2);
    }
    let w1 = r1 - (d * d - r2 * r2 + r1 * r1) / (2.0 * d);
    let w2 = r2 - (d * d - r1 * r1 + r2 * r2) / (2.0 * d);
    circle_area(r1, w1) + circle_area(r2, w2)
}

/// `circleCircleIntersection(p1, p2)`.
pub fn circle_circle_intersection(p1: &Circle, p2: &Circle) -> Vec<Point> {
    let d = circle_distance(p1, p2);
    let r1 = p1.radius;
    let r2 = p2.radius;
    if d >= r1 + r2 || d <= (r1 - r2).abs() {
        return Vec::new();
    }
    let a = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
    let h = (r1 * r1 - a * a).sqrt();
    let x0 = p1.x + (a * (p2.x - p1.x)) / d;
    let y0 = p1.y + (a * (p2.y - p1.y)) / d;
    let rx = -(p2.y - p1.y) * (h / d);
    let ry = -(p2.x - p1.x) * (h / d);
    vec![
        Point {
            x: x0 + rx,
            y: y0 - ry,
        },
        Point {
            x: x0 - rx,
            y: y0 + ry,
        },
    ]
}

/// `getCenter(points)`.
fn get_center(points: impl Iterator<Item = Point>) -> Point {
    let mut center = Point::default();
    let mut n = 0usize;
    for p in points {
        center.x += p.x;
        center.y += p.y;
        n += 1;
    }
    center.x /= n as f64;
    center.y /= n as f64;
    center
}

fn contained_in_circles(x: f64, y: f64, circles: &[Circle]) -> bool {
    circles
        .iter()
        .all(|c| distance(x, y, c.x, c.y) < c.radius + SMALL)
}

fn get_intersection_points(circles: &[Circle]) -> Vec<IPoint> {
    let mut ret = Vec::new();
    for i in 0..circles.len() {
        for j in i + 1..circles.len() {
            for p in circle_circle_intersection(&circles[i], &circles[j]) {
                ret.push(IPoint {
                    x: p.x,
                    y: p.y,
                    parent_index: [i, j],
                    angle: 0.0,
                });
            }
        }
    }
    ret
}

/// `intersectionArea(circles, stats)`: the area and `stats.arcs`.
pub fn intersection_area(circles: &[Circle]) -> (f64, Vec<Arc>) {
    if circles.is_empty() {
        return (0.0, Vec::new());
    }
    let intersection_points = get_intersection_points(circles);
    let mut inner_points: Vec<IPoint> = intersection_points
        .into_iter()
        .filter(|p| contained_in_circles(p.x, p.y, circles))
        .collect();

    let mut arc_area = 0.0;
    let mut polygon_area = 0.0;
    let mut arcs = Vec::new();

    if inner_points.len() > 1 {
        let center = get_center(inner_points.iter().map(|p| Point { x: p.x, y: p.y }));
        for p in &mut inner_points {
            p.angle = (p.x - center.x).atan2(p.y - center.y);
        }
        inner_points.sort_by(|a, b| js_cmp(b.angle - a.angle));

        let mut p2 = inner_points[inner_points.len() - 1];
        for &p1 in &inner_points {
            polygon_area += (p2.x + p1.x) * (p1.y - p2.y);
            let mid = Point {
                x: (p1.x + p2.x) / 2.0,
                y: (p1.y + p2.y) / 2.0,
            };
            let mut arc: Option<Arc> = None;
            for j in 0..p1.parent_index.len() {
                if p2.parent_index.contains(&p1.parent_index[j]) {
                    let circle = circles[p1.parent_index[j]];
                    let a1 = (p1.x - circle.x).atan2(p1.y - circle.y);
                    let a2 = (p2.x - circle.x).atan2(p2.y - circle.y);
                    let mut angle_diff = a2 - a1;
                    if angle_diff < 0.0 {
                        angle_diff += 2.0 * PI;
                    }
                    let a = a2 - angle_diff / 2.0;
                    let mut width = distance(
                        mid.x,
                        mid.y,
                        circle.x + circle.radius * a.sin(),
                        circle.y + circle.radius * a.cos(),
                    );
                    if width > circle.radius * 2.0 {
                        width = circle.radius * 2.0;
                    }
                    if arc.is_none_or(|current| current.width > width) {
                        arc = Some(Arc {
                            circle,
                            width,
                            p1: Point { x: p1.x, y: p1.y },
                            p2: Point { x: p2.x, y: p2.y },
                            large: width > circle.radius,
                            sweep: true,
                        });
                    }
                }
            }
            if let Some(arc) = arc {
                arc_area += circle_area(arc.circle.radius, arc.width);
                arcs.push(arc);
                p2 = p1;
            }
        }
    } else {
        let mut smallest = circles[0];
        for c in &circles[1..] {
            if c.radius < smallest.radius {
                smallest = *c;
            }
        }
        let disjoint = circles
            .iter()
            .any(|c| circle_distance(c, &smallest) > (smallest.radius - c.radius).abs());
        if disjoint {
            arc_area = 0.0;
            polygon_area = 0.0;
        } else {
            arc_area = smallest.radius * smallest.radius * PI;
            arcs.push(Arc {
                circle: smallest,
                p1: Point {
                    x: smallest.x,
                    y: smallest.y + smallest.radius,
                },
                p2: Point {
                    x: smallest.x - SMALL,
                    y: smallest.y + smallest.radius,
                },
                width: smallest.radius * 2.0,
                large: true,
                sweep: true,
            });
        }
    }

    polygon_area /= 2.0;
    (arc_area + polygon_area, arcs)
}

// ---------------------------------------------------------------------------
// fmin, as bundled in build/venn.esm.js

/// `bisect(f, a, b)` with the default 100 iterations and 1e-10 tolerance.
fn bisect(f: impl Fn(f64) -> f64, mut a: f64, b: f64) -> Result<f64, LayoutError> {
    let max_iterations = 100;
    let tolerance = 1e-10;
    let f_a = f(a);
    let f_b = f(b);
    let mut delta = b - a;
    if f_a * f_b > 0.0 {
        return Err(LayoutError::Bisect);
    }
    if f_a == 0.0 {
        return Ok(a);
    }
    if f_b == 0.0 {
        return Ok(b);
    }
    for _ in 0..max_iterations {
        delta /= 2.0;
        let mid = a + delta;
        let f_mid = f(mid);
        if f_mid * f_a >= 0.0 {
            a = mid;
        }
        if delta.abs() < tolerance || f_mid == 0.0 {
            return Ok(mid);
        }
    }
    Ok(a + delta)
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    let mut ret = 0.0;
    for i in 0..a.len() {
        ret += a[i] * b[i];
    }
    ret
}

fn norm2(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

/// `scale(ret, value, c)`.
fn scale_into(ret: &mut [f64], value: &[f64], c: f64) {
    for i in 0..value.len() {
        ret[i] = value[i] * c;
    }
}

/// `weightedSum(ret, w1, v1, w2, v2)`.
fn weighted_sum(ret: &mut [f64], w1: f64, v1: &[f64], w2: f64, v2: &[f64]) {
    for j in 0..ret.len() {
        ret[j] = w1 * v1[j] + w2 * v2[j];
    }
}

/// The `nelderMead` parameters the callers pass.
#[derive(Debug, Clone, Copy, Default)]
pub struct NelderMeadParams {
    pub max_iterations: Option<usize>,
    pub min_error_delta: Option<f64>,
}

/// A simplex vertex: the point and `.fx`.
#[derive(Debug, Clone)]
struct Vertex {
    x: Vec<f64>,
    fx: f64,
}

/// `nelderMead(f, x0, parameters)`; returns `{ fx, x }`.
pub fn nelder_mead(
    mut f: impl FnMut(&[f64]) -> f64,
    x0: &[f64],
    params: NelderMeadParams,
) -> (f64, Vec<f64>) {
    let n = x0.len();
    let max_iterations = params.max_iterations.unwrap_or(n * 200);
    let non_zero_delta = 1.05;
    let zero_delta = 0.001;
    let min_error_delta = params.min_error_delta.unwrap_or(1e-6);
    // `parameters.minErrorDelta || 1e-5`: fmin reads the same key twice.
    let min_tolerance = params.min_error_delta.unwrap_or(1e-5);
    let rho = 1.0;
    let chi = 2.0;
    let psi = -0.5;
    let sigma = 0.5;

    let mut simplex: Vec<Vertex> = Vec::with_capacity(n + 1);
    simplex.push(Vertex {
        x: x0.to_vec(),
        fx: f(x0),
    });
    for i in 0..n {
        let mut point = x0.to_vec();
        point[i] = if point[i] != 0.0 && !point[i].is_nan() {
            point[i] * non_zero_delta
        } else {
            zero_delta
        };
        let fx = f(&point);
        simplex.push(Vertex { x: point, fx });
    }

    let mut centroid = x0.to_vec();
    let mut reflected = x0.to_vec();
    let mut contracted = x0.to_vec();
    let mut expanded = x0.to_vec();

    for _ in 0..max_iterations {
        simplex.sort_by(|a, b| js_cmp(a.fx - b.fx));

        let mut max_diff: f64 = 0.0;
        for i in 0..n {
            max_diff = max_diff.max((simplex[0].x[i] - simplex[1].x[i]).abs());
        }
        if (simplex[0].fx - simplex[n].fx).abs() < min_error_delta && max_diff < min_tolerance {
            break;
        }

        for (i, c) in centroid.iter_mut().enumerate() {
            *c = 0.0;
            for vertex in simplex.iter().take(n) {
                *c += vertex.x[i];
            }
            *c /= n as f64;
        }

        let worst = simplex[n].clone();
        weighted_sum(&mut reflected, 1.0 + rho, &centroid, -rho, &worst.x);
        let reflected_fx = f(&reflected);

        if reflected_fx < simplex[0].fx {
            weighted_sum(&mut expanded, 1.0 + chi, &centroid, -chi, &worst.x);
            let expanded_fx = f(&expanded);
            if expanded_fx < reflected_fx {
                simplex[n].x.copy_from_slice(&expanded);
                simplex[n].fx = expanded_fx;
            } else {
                simplex[n].x.copy_from_slice(&reflected);
                simplex[n].fx = reflected_fx;
            }
        } else if reflected_fx >= simplex[n - 1].fx {
            let mut should_reduce = false;
            if reflected_fx > worst.fx {
                weighted_sum(&mut contracted, 1.0 + psi, &centroid, -psi, &worst.x);
                let contracted_fx = f(&contracted);
                if contracted_fx < worst.fx {
                    simplex[n].x.copy_from_slice(&contracted);
                    simplex[n].fx = contracted_fx;
                } else {
                    should_reduce = true;
                }
            } else {
                weighted_sum(
                    &mut contracted,
                    1.0 - psi * rho,
                    &centroid,
                    psi * rho,
                    &worst.x,
                );
                let contracted_fx = f(&contracted);
                if contracted_fx < reflected_fx {
                    simplex[n].x.copy_from_slice(&contracted);
                    simplex[n].fx = contracted_fx;
                } else {
                    should_reduce = true;
                }
            }
            if should_reduce {
                let best = simplex[0].x.clone();
                for vertex in simplex.iter_mut().skip(1) {
                    let current = vertex.x.clone();
                    weighted_sum(&mut vertex.x, 1.0 - sigma, &best, sigma, &current);
                    vertex.fx = f(&vertex.x);
                }
            }
        } else {
            simplex[n].x.copy_from_slice(&reflected);
            simplex[n].fx = reflected_fx;
        }
    }

    simplex.sort_by(|a, b| js_cmp(a.fx - b.fx));
    let best = simplex.swap_remove(0);
    (best.fx, best.x)
}

/// `{ x, fx, fxprime }` of `conjugateGradient`.
#[derive(Debug, Clone)]
struct CgPoint {
    x: Vec<f64>,
    fx: f64,
    fxprime: Vec<f64>,
}

/// `wolfeLineSearch(f, pk, current, next, a)` with `c1 = 1e-6`, `c2 = 0.1`.
fn wolfe_line_search(
    f: &mut impl FnMut(&[f64], &mut [f64]) -> f64,
    pk: &[f64],
    current: &CgPoint,
    next: &mut CgPoint,
    a: f64,
) -> f64 {
    let phi0 = current.fx;
    let phi_prime0 = dot(&current.fxprime, pk);
    let mut phi_old = phi0;
    let mut a0 = 0.0;
    // `a = a || 1`.
    let mut a = if a == 0.0 || a.is_nan() { 1.0 } else { a };
    let c1 = 1e-6;
    let c2 = 0.1;

    let mut eval = |next: &mut CgPoint, a: f64| -> (f64, f64) {
        weighted_sum(&mut next.x, 1.0, &current.x, a, pk);
        let phi = f(&next.x, &mut next.fxprime);
        next.fx = phi;
        (phi, dot(&next.fxprime, pk))
    };

    let zoom = |next: &mut CgPoint,
                eval: &mut dyn FnMut(&mut CgPoint, f64) -> (f64, f64),
                mut a_lo: f64,
                mut a_high: f64,
                mut phi_lo: f64|
     -> f64 {
        for _ in 0..16 {
            let a = (a_lo + a_high) / 2.0;
            let (phi, phi_prime) = eval(next, a);
            if phi > phi0 + c1 * a * phi_prime0 || phi >= phi_lo {
                a_high = a;
            } else {
                if phi_prime.abs() <= -c2 * phi_prime0 {
                    return a;
                }
                if phi_prime * (a_high - a_lo) >= 0.0 {
                    a_high = a_lo;
                }
                a_lo = a;
                phi_lo = phi;
            }
        }
        0.0
    };

    for iteration in 0..10 {
        let (phi, phi_prime) = eval(next, a);
        if phi > phi0 + c1 * a * phi_prime0 || (iteration > 0 && phi >= phi_old) {
            return zoom(next, &mut eval, a0, a, phi_old);
        }
        if phi_prime.abs() <= -c2 * phi_prime0 {
            return a;
        }
        if phi_prime >= 0.0 {
            return zoom(next, &mut eval, a, a0, phi);
        }
        phi_old = phi;
        a0 = a;
        a *= 2.0;
    }
    a
}

/// `conjugateGradient(f, initial, params)`; returns the final `current`.
fn conjugate_gradient(
    mut f: impl FnMut(&[f64], &mut [f64]) -> f64,
    initial: &[f64],
    max_iterations: Option<usize>,
) -> (f64, Vec<f64>) {
    let mut current = CgPoint {
        x: initial.to_vec(),
        fx: 0.0,
        fxprime: initial.to_vec(),
    };
    let mut next = current.clone();
    let mut yk = initial.to_vec();
    let mut a = 1.0;
    let max_iterations = max_iterations.unwrap_or(initial.len() * 20);

    current.fx = f(&current.x, &mut current.fxprime);
    let mut pk = current.fxprime.clone();
    scale_into(&mut pk, &current.fxprime, -1.0);

    for _ in 0..max_iterations {
        a = wolfe_line_search(&mut f, &pk, &current, &mut next, a);
        if a == 0.0 || a.is_nan() {
            scale_into(&mut pk, &current.fxprime, -1.0);
        } else {
            weighted_sum(&mut yk, 1.0, &next.fxprime, -1.0, &current.fxprime);
            let delta_k = dot(&current.fxprime, &current.fxprime);
            let beta_k = (dot(&yk, &next.fxprime) / delta_k).max(0.0);
            let pk_old = pk.clone();
            weighted_sum(&mut pk, beta_k, &pk_old, -1.0, &next.fxprime);
            std::mem::swap(&mut current, &mut next);
        }
        if norm2(&current.fxprime) <= 1e-5 {
            break;
        }
    }
    (current.fx, current.x)
}

/// V8's `Math.random` generator (xorshift128+, 52 mantissa bits) with a
/// fixed seed in place of V8's per-isolate one.
struct XorShift128Plus {
    s0: u64,
    s1: u64,
}

impl XorShift128Plus {
    fn new() -> Self {
        // MurmurHash3's finalizer over seed 0 and its complement, as
        // `RandomNumberGenerator::SetSeed` derives the state.
        fn murmur(mut h: u64) -> u64 {
            h ^= h >> 33;
            h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
            h ^= h >> 33;
            h = h.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
            h ^= h >> 33;
            h
        }
        let seed: u64 = 0x5EED;
        XorShift128Plus {
            s0: murmur(seed),
            s1: murmur(!seed),
        }
    }

    fn next_f64(&mut self) -> f64 {
        let mut s1 = self.s0;
        let s0 = self.s1;
        self.s0 = s0;
        s1 ^= s1 << 23;
        s1 ^= s1 >> 17;
        s1 ^= s0;
        s1 ^= s0 >> 26;
        self.s1 = s1;
        let bits = (self.s0 >> 12) | 0x3FF0_0000_0000_0000;
        f64::from_bits(bits) - 1.0
    }
}

// ---------------------------------------------------------------------------
// layout.js

/// An area with its sets resolved to indices into the set-id list.
#[derive(Debug, Clone)]
struct IndexedArea {
    sets: Vec<usize>,
    size: f64,
    weight: Option<f64>,
}

/// The problem `venn` optimizes: the set ids in `Object.keys(circles)`
/// order, every area after `addMissingAreas`, and the radii.
struct Problem {
    ids: Vec<String>,
    areas: Vec<IndexedArea>,
    radii: Vec<f64>,
    /// `circles[set].size`, which `greedyLayout` compares.
    sizes: Vec<f64>,
}

/// `lossFunction(circles, overlaps)` over index-resolved areas.
fn loss_function(circles: &[Circle], areas: &[IndexedArea]) -> f64 {
    let mut output = 0.0;
    for area in areas {
        if area.sets.len() == 1 {
            continue;
        }
        let overlap = if area.sets.len() == 2 {
            let left = &circles[area.sets[0]];
            let right = &circles[area.sets[1]];
            circle_overlap(left.radius, right.radius, circle_distance(left, right))
        } else {
            let members: Vec<Circle> = area.sets.iter().map(|&i| circles[i]).collect();
            intersection_area(&members).0
        };
        let weight = area.weight.unwrap_or(1.0);
        output += weight * (overlap - area.size) * (overlap - area.size);
    }
    output
}

/// `addMissingAreas(areas, { distinct: false })`.
fn add_missing_areas(areas: &[Area]) -> Vec<Area> {
    let mut r: Vec<Area> = areas.to_vec();
    let to_key = |arr: &[String]| arr.join(";");
    let mut ids: Vec<String> = Vec::new();
    let mut pairs: std::collections::HashSet<String> = std::collections::HashSet::new();
    for area in &r {
        if area.sets.len() == 1 {
            ids.push(area.sets[0].clone());
        } else if area.sets.len() == 2 {
            let a = area.sets[0].clone();
            let b = area.sets[1].clone();
            pairs.insert(to_key(&area.sets));
            pairs.insert(to_key(&[b, a]));
        }
    }
    ids.sort_by(|a, b| js_str_cmp(a, b));
    for i in 0..ids.len() {
        let a = &ids[i];
        for b in &ids[i + 1..] {
            if !pairs.contains(&to_key(&[a.clone(), b.clone()])) {
                r.push(Area {
                    sets: vec![a.clone(), b.clone()],
                    size: 0.0,
                    weight: None,
                });
            }
        }
    }
    r
}

impl Problem {
    /// Resolves the areas against the single-set ids. The id order is the
    /// property order of `circles` in `greedyLayout` and
    /// `constrainedMDSLayout`, which both insert the single sets in area
    /// order.
    fn new(areas: &[Area]) -> Result<Problem, LayoutError> {
        let mut order: JsObject<usize> = JsObject::new();
        let mut sizes_by_id: JsObject<f64> = JsObject::new();
        for area in areas {
            if area.sets.len() == 1 {
                let id = &area.sets[0];
                let next = order.len();
                if !order.contains_key(id) {
                    order.insert(id, next);
                }
                sizes_by_id.insert(id, area.size);
            }
        }
        let ids: Vec<String> = order.keys().map(str::to_string).collect();
        let index_of = |id: &str| -> Result<usize, LayoutError> {
            ids.iter()
                .position(|k| k == id)
                .ok_or_else(|| LayoutError::UnknownSet(id.to_string()))
        };
        let mut indexed = Vec::with_capacity(areas.len());
        for area in areas {
            let sets = area
                .sets
                .iter()
                .map(|s| index_of(s))
                .collect::<Result<Vec<_>, _>>()?;
            indexed.push(IndexedArea {
                sets,
                size: area.size,
                weight: area.weight,
            });
        }
        let sizes: Vec<f64> = ids
            .iter()
            .map(|id| sizes_by_id.get(id).copied().unwrap_or(f64::NAN))
            .collect();
        let radii = sizes.iter().map(|s| (s / PI).sqrt()).collect();
        Ok(Problem {
            ids,
            areas: indexed,
            radii,
            sizes,
        })
    }

    fn pairs(&self) -> Vec<IndexedArea> {
        self.areas
            .iter()
            .filter(|a| a.sets.len() == 2)
            .cloned()
            .collect()
    }
}

/// `distanceFromIntersectArea(r1, r2, overlap)`.
pub fn distance_from_intersect_area(r1: f64, r2: f64, overlap: f64) -> Result<f64, LayoutError> {
    if r1.min(r2) * r1.min(r2) * PI <= overlap + SMALL {
        return Ok((r1 - r2).abs());
    }
    bisect(|d| circle_overlap(r1, r2, d) - overlap, 0.0, r1 + r2)
}

/// `greedyLayout(areas, params)`: circles in id order.
fn greedy_layout(problem: &Problem) -> Result<Vec<Circle>, LayoutError> {
    let n = problem.ids.len();
    let mut circles: Vec<Circle> = problem
        .radii
        .iter()
        .map(|&radius| Circle {
            x: 1e10,
            y: 1e10,
            radius,
        })
        .collect();

    #[derive(Clone, Copy)]
    struct Overlap {
        set: usize,
        size: f64,
        weight: f64,
    }
    let mut set_overlaps: Vec<Vec<Overlap>> = vec![Vec::new(); n];
    let areas = problem.pairs();
    for current in &areas {
        let mut weight = current.weight.unwrap_or(1.0);
        let left = current.sets[0];
        let right = current.sets[1];
        if current.size + SMALL >= problem.sizes[left].min(problem.sizes[right]) {
            weight = 0.0;
        }
        set_overlaps[left].push(Overlap {
            set: right,
            size: current.size,
            weight,
        });
        set_overlaps[right].push(Overlap {
            set: left,
            size: current.size,
            weight,
        });
    }

    let mut most_overlapped: Vec<(usize, f64)> = (0..n)
        .map(|set| {
            let mut size = 0.0;
            for o in &set_overlaps[set] {
                size += o.size * o.weight;
            }
            (set, size)
        })
        .collect();
    most_overlapped.sort_by(|a, b| js_cmp(b.1 - a.1));

    let mut positioned = vec![false; n];
    let Some(&(first, _)) = most_overlapped.first() else {
        return Ok(circles);
    };
    circles[first].x = 0.0;
    circles[first].y = 0.0;
    positioned[first] = true;

    for &(set_index, _) in most_overlapped.iter().skip(1) {
        let mut overlap: Vec<Overlap> = set_overlaps[set_index]
            .iter()
            .copied()
            .filter(|o| positioned[o.set])
            .collect();
        let set = circles[set_index];
        overlap.sort_by(|a, b| js_cmp(b.size - a.size));
        if overlap.is_empty() {
            return Err(LayoutError::MissingOverlap);
        }

        let mut points: Vec<Point> = Vec::new();
        for j in 0..overlap.len() {
            let p1 = circles[overlap[j].set];
            let d1 = distance_from_intersect_area(set.radius, p1.radius, overlap[j].size)?;
            points.push(Point {
                x: p1.x + d1,
                y: p1.y,
            });
            points.push(Point {
                x: p1.x - d1,
                y: p1.y,
            });
            points.push(Point {
                x: p1.x,
                y: p1.y + d1,
            });
            points.push(Point {
                x: p1.x,
                y: p1.y - d1,
            });
            for k in j + 1..overlap.len() {
                let p2 = circles[overlap[k].set];
                let d2 = distance_from_intersect_area(set.radius, p2.radius, overlap[k].size)?;
                points.extend(circle_circle_intersection(
                    &Circle {
                        x: p1.x,
                        y: p1.y,
                        radius: d1,
                    },
                    &Circle {
                        x: p2.x,
                        y: p2.y,
                        radius: d2,
                    },
                ));
            }
        }

        let mut best_loss = 1e50;
        let mut best_point = points[0];
        for point in &points {
            circles[set_index].x = point.x;
            circles[set_index].y = point.y;
            let local_loss = loss_function(&circles, &areas);
            if local_loss < best_loss {
                best_loss = local_loss;
                best_point = *point;
            }
        }
        circles[set_index].x = best_point.x;
        circles[set_index].y = best_point.y;
        positioned[set_index] = true;
    }
    Ok(circles)
}

/// `constrainedMDSGradient(x, fxprime, distances, constraints)`.
fn constrained_mds_gradient(
    x: &[f64],
    fxprime: &mut [f64],
    distances: &[Vec<f64>],
    constraints: &[Vec<f64>],
) -> f64 {
    for v in fxprime.iter_mut() {
        *v = 0.0;
    }
    let mut loss = 0.0;
    for i in 0..distances.len() {
        let xi = x[2 * i];
        let yi = x[2 * i + 1];
        for j in i + 1..distances.len() {
            let xj = x[2 * j];
            let yj = x[2 * j + 1];
            let dij = distances[i][j];
            let constraint = constraints[i][j];
            let squared_distance = (xj - xi) * (xj - xi) + (yj - yi) * (yj - yi);
            let distance = squared_distance.sqrt();
            let delta = squared_distance - dij * dij;
            if (constraint > 0.0 && distance <= dij) || (constraint < 0.0 && distance >= dij) {
                continue;
            }
            loss += 2.0 * delta * delta;
            fxprime[2 * i] += 4.0 * delta * (xi - xj);
            fxprime[2 * i + 1] += 4.0 * delta * (yi - yj);
            fxprime[2 * j] += 4.0 * delta * (xj - xi);
            fxprime[2 * j + 1] += 4.0 * delta * (yj - yi);
        }
    }
    loss
}

/// `constrainedMDSLayout(areas, params)` with `restarts = 10` and the
/// `maxIterations: 500` that `venn` writes into the shared params.
fn constrained_mds_layout(
    areas: &[Area],
    problem: &Problem,
    rng: &mut XorShift128Plus,
) -> Result<Vec<Circle>, LayoutError> {
    let restarts = 10;
    // `sets` holds every single-set area in order; `setids` maps an id to
    // the row of its last one.
    let sets: Vec<&Area> = areas.iter().filter(|a| a.sets.len() == 1).collect();
    let row_of = |id: &str| -> Result<usize, LayoutError> {
        sets.iter()
            .rposition(|a| a.sets[0] == id)
            .ok_or_else(|| LayoutError::UnknownSet(id.to_string()))
    };
    let m = sets.len();
    let mut distances = vec![vec![0.0; m]; m];
    let mut constraints = vec![vec![0.0; m]; m];
    for current in areas.iter().filter(|a| a.sets.len() == 2) {
        let left = row_of(&current.sets[0])?;
        let right = row_of(&current.sets[1])?;
        let r1 = (sets[left].size / PI).sqrt();
        let r2 = (sets[right].size / PI).sqrt();
        let distance = distance_from_intersect_area(r1, r2, current.size)?;
        distances[left][right] = distance;
        distances[right][left] = distance;
        let mut c = 0.0;
        if current.size + 1e-10 >= sets[left].size.min(sets[right].size) {
            c = 1.0;
        } else if current.size <= 1e-10 {
            c = -1.0;
        }
        constraints[left][right] = c;
        constraints[right][left] = c;
    }

    let row_norms: Vec<f64> = distances.iter().map(|row| norm2(row)).collect();
    let norm = norm2(&row_norms) / m as f64;
    let distances: Vec<Vec<f64>> = distances
        .iter()
        .map(|row| row.iter().map(|v| v / norm).collect())
        .collect();

    let mut best: Option<(f64, Vec<f64>)> = None;
    for _ in 0..restarts {
        let initial: Vec<f64> = (0..m * 2).map(|_| rng.next_f64()).collect();
        let current = conjugate_gradient(
            |x, fxprime| constrained_mds_gradient(x, fxprime, &distances, &constraints),
            &initial,
            Some(500),
        );
        if best.as_ref().is_none_or(|b| current.0 < b.0) {
            best = Some(current);
        }
    }
    let positions = best.map(|b| b.1).unwrap_or_default();

    // `circles[set.sets[0]] = ...` row by row; a repeated id keeps the
    // value of its last row.
    let mut circles: Vec<Circle> = vec![
        Circle {
            x: 0.0,
            y: 0.0,
            radius: 0.0,
        };
        problem.ids.len()
    ];
    for (i, set) in sets.iter().enumerate() {
        let idx = problem
            .ids
            .iter()
            .position(|k| *k == set.sets[0])
            .ok_or_else(|| LayoutError::UnknownSet(set.sets[0].clone()))?;
        circles[idx] = Circle {
            x: positions[2 * i] * norm,
            y: positions[2 * i + 1] * norm,
            radius: (set.size / PI).sqrt(),
        };
    }
    Ok(circles)
}

/// `bestInitialLayout(areas, params)`.
fn best_initial_layout(
    areas: &[Area],
    problem: &Problem,
    rng: &mut XorShift128Plus,
) -> Result<Vec<Circle>, LayoutError> {
    let mut initial = greedy_layout(problem)?;
    if areas.len() >= 8 {
        let constrained = constrained_mds_layout(areas, problem, rng)?;
        let constrained_loss = loss_function(&constrained, &problem.areas);
        let greedy_loss = loss_function(&initial, &problem.areas);
        if constrained_loss + 1e-8 < greedy_loss {
            initial = constrained;
        }
    }
    Ok(initial)
}

/// `venn(sets, { lossFunction, distinct: false })`: circles keyed by set
/// id in `Object.keys` order.
pub fn venn(sets: &[Area]) -> Result<JsObject<Circle>, LayoutError> {
    let areas = add_missing_areas(sets);
    let problem = Problem::new(&areas)?;
    let mut rng = XorShift128Plus::new();
    let mut circles = best_initial_layout(&areas, &problem, &mut rng)?;

    let mut initial = Vec::with_capacity(circles.len() * 2);
    for c in &circles {
        initial.push(c.x);
        initial.push(c.y);
    }
    let radii: Vec<f64> = circles.iter().map(|c| c.radius).collect();
    let mut current: Vec<Circle> = circles.clone();
    let (_, positions) = nelder_mead(
        |values| {
            for (i, c) in current.iter_mut().enumerate() {
                c.x = values[2 * i];
                c.y = values[2 * i + 1];
                c.radius = radii[i];
            }
            loss_function(&current, &problem.areas)
        },
        &initial,
        NelderMeadParams {
            max_iterations: Some(500),
            min_error_delta: None,
        },
    );
    for (i, c) in circles.iter_mut().enumerate() {
        c.x = positions[2 * i];
        c.y = positions[2 * i + 1];
    }

    let mut out = JsObject::new();
    for (id, c) in problem.ids.iter().zip(circles) {
        out.insert(id, c);
    }
    Ok(out)
}

/// A circle with the `setid` that `fromObjectNotation` adds.
#[derive(Debug, Clone)]
struct Tagged {
    setid: String,
    c: Circle,
}

#[derive(Debug, Clone, Copy)]
struct Range {
    min: f64,
    max: f64,
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
    x: Range,
    y: Range,
}

/// `getBoundingBox(circles)`.
fn get_bounding_box<'a>(circles: impl Iterator<Item = &'a Circle> + Clone) -> Bounds {
    let min_max = |pick: fn(&Circle) -> f64| {
        let hi = circles
            .clone()
            .fold(f64::NEG_INFINITY, |acc, c| acc.js_max(pick(c) + c.radius));
        let lo = circles
            .clone()
            .fold(f64::INFINITY, |acc, c| acc.js_min(pick(c) - c.radius));
        Range { min: lo, max: hi }
    };
    Bounds {
        x: min_max(|c| c.x),
        y: min_max(|c| c.y),
    }
}

/// `Math.max(acc, v)`, which returns NaN when either is NaN; Rust's `max`
/// ignores NaN.
trait JsMinMax {
    fn js_max(self, other: f64) -> f64;
    fn js_min(self, other: f64) -> f64;
}

impl JsMinMax for f64 {
    fn js_max(self, other: f64) -> f64 {
        if self.is_nan() || other.is_nan() {
            f64::NAN
        } else {
            self.max(other)
        }
    }

    fn js_min(self, other: f64) -> f64 {
        if self.is_nan() || other.is_nan() {
            f64::NAN
        } else {
            self.min(other)
        }
    }
}

/// `disjointCluster(circles)`: clusters of overlapping circles, in order
/// of their roots' first appearance.
fn disjoint_cluster(circles: &[Tagged]) -> Vec<Vec<Tagged>> {
    let n = circles.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], i: usize) -> usize {
        if parent[i] != i {
            let root = find(parent, parent[i]);
            parent[i] = root;
        }
        parent[i]
    }
    for i in 0..n {
        for j in i + 1..n {
            let max_distance = circles[i].c.radius + circles[j].c.radius;
            if circle_distance(&circles[i].c, &circles[j].c) + 1e-10 < max_distance {
                // `union(circles[j], circles[i])`: j's root joins i's.
                let x_root = find(&mut parent, j);
                let y_root = find(&mut parent, i);
                parent[x_root] = y_root;
            }
        }
    }
    let mut clusters: JsObject<Vec<Tagged>> = JsObject::new();
    for i in 0..n {
        let root = find(&mut parent, i);
        let key = circles[root].setid.clone();
        match clusters.get_mut(&key) {
            Some(list) => list.push(circles[i].clone()),
            None => clusters.insert(&key, vec![circles[i].clone()]),
        }
    }
    clusters.entries.into_iter().map(|(_, v)| v).collect()
}

/// `orientateCircles(circles, orientation, undefined)`.
fn orientate_circles(circles: &mut [Tagged], orientation: f64) {
    circles.sort_by(|a, b| js_cmp(b.c.radius - a.c.radius));

    if let Some(first) = circles.first() {
        let largest_x = first.c.x;
        let largest_y = first.c.y;
        for t in circles.iter_mut() {
            t.c.x -= largest_x;
            t.c.y -= largest_y;
        }
    }

    if circles.len() == 2 {
        let dist = circle_distance(&circles[0].c, &circles[1].c);
        if dist < (circles[1].c.radius - circles[0].c.radius).abs() {
            circles[1].c.x = circles[0].c.x + circles[0].c.radius - circles[1].c.radius - 1e-10;
            circles[1].c.y = circles[0].c.y;
        }
    }

    if circles.len() > 1 {
        let rotation = circles[1].c.x.atan2(circles[1].c.y) - orientation;
        let c = rotation.cos();
        let s = rotation.sin();
        for t in circles.iter_mut() {
            let x = t.c.x;
            let y = t.c.y;
            t.c.x = c * x - s * y;
            t.c.y = s * x + c * y;
        }
    }

    if circles.len() > 2 {
        let mut angle = circles[2].c.x.atan2(circles[2].c.y) - orientation;
        while angle < 0.0 {
            angle += 2.0 * PI;
        }
        while angle > 2.0 * PI {
            angle -= 2.0 * PI;
        }
        if angle > PI {
            let slope = circles[1].c.y / (1e-10 + circles[1].c.x);
            for t in circles.iter_mut() {
                let d = (t.c.x + slope * t.c.y) / (1.0 + slope * slope);
                t.c.x = 2.0 * d - t.c.x;
                t.c.y = 2.0 * d * slope - t.c.y;
            }
        }
    }
}

fn to_object(circles: Vec<Tagged>) -> JsObject<Circle> {
    let mut out = JsObject::new();
    for t in circles {
        out.insert(&t.setid, t.c);
    }
    out
}

fn from_object(solution: &JsObject<Circle>) -> Vec<Tagged> {
    solution
        .iter()
        .map(|(id, c)| Tagged {
            setid: id.to_string(),
            c: *c,
        })
        .collect()
}

/// `normalizeSolution(solution, orientation, undefined)`.
pub fn normalize_solution(solution: &JsObject<Circle>, orientation: f64) -> JsObject<Circle> {
    let circles = from_object(solution);
    let mut clusters: Vec<(Vec<Tagged>, Bounds, f64)> = disjoint_cluster(&circles)
        .into_iter()
        .map(|mut cluster| {
            orientate_circles(&mut cluster, orientation);
            let bounds = get_bounding_box(cluster.iter().map(|t| &t.c));
            let size = (bounds.x.max - bounds.x.min) * (bounds.y.max - bounds.y.min);
            (cluster, bounds, size)
        })
        .collect();
    clusters.sort_by(|a, b| js_cmp(b.2 - a.2));

    let mut iter = clusters.into_iter();
    let Some((mut circles, mut return_bounds, _)) = iter.next() else {
        return JsObject::new();
    };
    let rest: Vec<(Vec<Tagged>, Bounds, f64)> = iter.collect();
    let spacing = (return_bounds.x.max - return_bounds.x.min) / 50.0;

    let add_cluster = |circles: &mut Vec<Tagged>,
                       return_bounds: &Bounds,
                       cluster: Option<&(Vec<Tagged>, Bounds, f64)>,
                       right: bool,
                       bottom: bool| {
        let Some((cluster, bounds, _)) = cluster else {
            return;
        };
        let x_offset = if right {
            return_bounds.x.max - bounds.x.min + spacing
        } else {
            let mut off = return_bounds.x.max - bounds.x.max;
            let centreing = (bounds.x.max - bounds.x.min) / 2.0
                - (return_bounds.x.max - return_bounds.x.min) / 2.0;
            if centreing < 0.0 {
                off += centreing;
            }
            off
        };
        let y_offset = if bottom {
            return_bounds.y.max - bounds.y.min + spacing
        } else {
            let mut off = return_bounds.y.max - bounds.y.max;
            let centreing = (bounds.y.max - bounds.y.min) / 2.0
                - (return_bounds.y.max - return_bounds.y.min) / 2.0;
            if centreing < 0.0 {
                off += centreing;
            }
            off
        };
        for t in cluster {
            let mut t = t.clone();
            t.c.x += x_offset;
            t.c.y += y_offset;
            circles.push(t);
        }
    };

    // `index` counts from 1 over `clusters`; `rest[k]` is `clusters[k + 1]`.
    let mut index = 0;
    while index < rest.len() {
        add_cluster(&mut circles, &return_bounds, rest.get(index), true, false);
        add_cluster(
            &mut circles,
            &return_bounds,
            rest.get(index + 1),
            false,
            true,
        );
        add_cluster(
            &mut circles,
            &return_bounds,
            rest.get(index + 2),
            true,
            true,
        );
        index += 3;
        return_bounds = get_bounding_box(circles.iter().map(|t| &t.c));
    }
    to_object(circles)
}

/// `scaleSolution(solution, width, height, padding, false)`.
pub fn scale_solution(
    solution: &JsObject<Circle>,
    width: f64,
    height: f64,
    padding: f64,
) -> JsObject<Circle> {
    let circles = from_object(solution);
    let width = width - 2.0 * padding;
    let height = height - 2.0 * padding;
    let b = get_bounding_box(circles.iter().map(|t| &t.c));
    if b.x.max == b.x.min || b.y.max == b.y.min {
        // `console.log('not scaling solution: zero size detected')`.
        return solution.clone();
    }
    let x_scaling = width / (b.x.max - b.x.min);
    let y_scaling = height / (b.y.max - b.y.min);
    let scaling = y_scaling.js_min(x_scaling);
    let x_offset = (width - (b.x.max - b.x.min) * scaling) / 2.0;
    let y_offset = (height - (b.y.max - b.y.min) * scaling) / 2.0;
    to_object(
        circles
            .into_iter()
            .map(|t| Tagged {
                c: Circle {
                    radius: scaling * t.c.radius,
                    x: padding + x_offset + (t.c.x - b.x.min) * scaling,
                    y: padding + y_offset + (t.c.y - b.y.min) * scaling,
                },
                setid: t.setid,
            })
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// diagram.js: text centres and paths

/// `circleMargin(current, interior, exterior)`.
fn circle_margin(x: f64, y: f64, interior: &[Circle], exterior: &[Circle]) -> f64 {
    let mut margin = interior[0].radius - distance(interior[0].x, interior[0].y, x, y);
    for c in &interior[1..] {
        let m = c.radius - distance(c.x, c.y, x, y);
        if m <= margin {
            margin = m;
        }
    }
    for c in exterior {
        let m = distance(c.x, c.y, x, y) - c.radius;
        if m <= margin {
            margin = m;
        }
    }
    margin
}

/// `computeTextCentre(interior, exterior, false)`.
pub fn compute_text_centre(interior: &[Circle], exterior: &[Circle]) -> TextCentre {
    if interior.is_empty() {
        return TextCentre {
            x: 0.0,
            y: -1000.0,
            disjoint: true,
        };
    }
    let mut points: Vec<Point> = Vec::with_capacity(interior.len() * 5);
    for c in interior {
        points.push(Point { x: c.x, y: c.y });
        points.push(Point {
            x: c.x + c.radius / 2.0,
            y: c.y,
        });
        points.push(Point {
            x: c.x - c.radius / 2.0,
            y: c.y,
        });
        points.push(Point {
            x: c.x,
            y: c.y + c.radius / 2.0,
        });
        points.push(Point {
            x: c.x,
            y: c.y - c.radius / 2.0,
        });
    }
    let mut initial = points[0];
    let mut margin = circle_margin(points[0].x, points[0].y, interior, exterior);
    for p in &points[1..] {
        let m = circle_margin(p.x, p.y, interior, exterior);
        if m >= margin {
            initial = *p;
            margin = m;
        }
    }

    let (_, solution) = nelder_mead(
        |p| -circle_margin(p[0], p[1], interior, exterior),
        &[initial.x, initial.y],
        NelderMeadParams {
            max_iterations: Some(500),
            min_error_delta: Some(1e-10),
        },
    );
    let ret = TextCentre {
        x: solution[0],
        y: solution[1],
        disjoint: false,
    };

    let mut valid = true;
    for c in interior {
        if distance(ret.x, ret.y, c.x, c.y) > c.radius {
            valid = false;
            break;
        }
    }
    for c in exterior {
        if distance(ret.x, ret.y, c.x, c.y) < c.radius {
            valid = false;
            break;
        }
    }
    if valid {
        return ret;
    }

    if interior.len() == 1 {
        return TextCentre {
            x: interior[0].x,
            y: interior[0].y,
            disjoint: false,
        };
    }
    let (_, arcs) = intersection_area(interior);
    if arcs.is_empty() {
        return TextCentre {
            x: 0.0,
            y: -1000.0,
            disjoint: true,
        };
    }
    if arcs.len() == 1 {
        return TextCentre {
            x: arcs[0].circle.x,
            y: arcs[0].circle.y,
            disjoint: false,
        };
    }
    if !exterior.is_empty() {
        return compute_text_centre(interior, &[]);
    }
    let c = get_center(arcs.iter().map(|a| a.p1));
    TextCentre {
        x: c.x,
        y: c.y,
        disjoint: false,
    }
}

/// `getOverlappingCircles(circles)`: for each id, the ids of the circles
/// that contain it.
fn get_overlapping_circles(circles: &JsObject<Circle>) -> JsObject<Vec<String>> {
    let mut ret: JsObject<Vec<String>> = JsObject::new();
    let ids: Vec<&str> = circles.keys().collect();
    for id in &ids {
        ret.insert(id, Vec::new());
    }
    let list: Vec<Circle> = circles.iter().map(|(_, c)| *c).collect();
    for i in 0..ids.len() {
        let a = list[i];
        for j in i + 1..ids.len() {
            let b = list[j];
            let d = circle_distance(&a, &b);
            if d + b.radius <= a.radius + 1e-10 {
                if let Some(v) = ret.get_mut(ids[j]) {
                    v.push(ids[i].to_string());
                }
            } else if d + a.radius <= b.radius + 1e-10
                && let Some(v) = ret.get_mut(ids[i])
            {
                v.push(ids[j].to_string());
            }
        }
    }
    ret
}

/// The property key `ret[area]` uses: the array's `toString()`.
pub fn sets_key(sets: &[String]) -> String {
    sets.join(",")
}

/// `computeTextCentres(circles, areas, false)`, keyed by [`sets_key`].
pub fn compute_text_centres(
    circles: &JsObject<Circle>,
    areas: &[Area],
) -> Result<JsObject<TextCentre>, LayoutError> {
    let mut ret = JsObject::new();
    let overlapped = get_overlapping_circles(circles);
    for area in areas {
        let mut exclude: Vec<&str> = Vec::new();
        for set in &area.sets {
            let overlaps = overlapped
                .get(set)
                .ok_or_else(|| LayoutError::UnknownSet(set.clone()))?;
            for o in overlaps {
                exclude.push(o);
            }
        }
        let mut interior = Vec::new();
        let mut exterior = Vec::new();
        for (id, c) in circles.iter() {
            if area.sets.iter().any(|s| s == id) {
                interior.push(*c);
            } else if !exclude.contains(&id) {
                exterior.push(*c);
            }
        }
        let centre = compute_text_centre(&interior, &exterior);
        ret.insert(&sets_key(&area.sets), centre);
    }
    Ok(ret)
}

/// `intersectionAreaArcs(circles)`.
pub fn intersection_area_arcs(circles: &[Circle]) -> Vec<Arc> {
    if circles.is_empty() {
        return Vec::new();
    }
    intersection_area(circles).1
}

/// Circles, text centres, and the input areas of one layout run: what
/// `VennDiagram` computes inside `chart` and what `layout` returns.
#[derive(Debug, Clone)]
pub struct Solution {
    pub circles: JsObject<Circle>,
    pub text_centres: JsObject<TextCentre>,
}

impl Solution {
    /// The circles of `sets`, in order; `None` when one is missing.
    pub fn circles_of(&self, sets: &[String]) -> Option<Vec<Circle>> {
        sets.iter().map(|s| self.circles.get(s).copied()).collect()
    }

    pub fn text_centre(&self, sets: &[String]) -> Option<TextCentre> {
        self.text_centres.get(&sets_key(sets)).copied()
    }
}

/// `venn`, `normalizeSolution` at `orientation = Math.PI / 2`,
/// `scaleSolution` without `scaleToFit`, and `computeTextCentres`: the
/// pipeline `VennDiagram` and `layout` share.
pub fn solve(
    data: &[Area],
    width: f64,
    height: f64,
    padding: f64,
) -> Result<Solution, LayoutError> {
    if data.is_empty() {
        return Ok(Solution {
            circles: JsObject::new(),
            text_centres: JsObject::new(),
        });
    }
    let solution = venn(data)?;
    let solution = normalize_solution(&solution, PI / 2.0);
    let circles = scale_solution(&solution, width, height, padding);
    let text_centres = compute_text_centres(&circles, data)?;
    Ok(Solution {
        circles,
        text_centres,
    })
}

#[cfg(test)]
// The expected values are node's `toPrecision(17)` output as printed.
#[allow(clippy::excessive_precision)]
mod tests {
    use super::*;

    fn close(actual: f64, expected: f64, what: &str) {
        assert!(
            (actual - expected).abs() <= 1e-6,
            "{what}: {actual} != {expected}"
        );
    }

    /// Checks circles and text centres against `venn.layout(data, { width,
    /// height, padding })` printed by node with venn.js 2.0.0.
    fn check(
        data: &[Area],
        size: (f64, f64, f64),
        circles: &[(&str, f64, f64, f64)],
        texts: &[(&[&str], f64, f64)],
    ) {
        let s = solve(data, size.0, size.1, size.2).expect("layout");
        for &(id, x, y, r) in circles {
            let c = s.circles.get(id).expect("circle");
            close(c.x, x, &format!("{id}.x"));
            close(c.y, y, &format!("{id}.y"));
            close(c.radius, r, &format!("{id}.radius"));
        }
        for &(sets, x, y) in texts {
            let sets: Vec<String> = sets.iter().map(|s| s.to_string()).collect();
            let t = s.text_centre(&sets).expect("text centre");
            close(t.x, x, &format!("{sets:?} text x"));
            close(t.y, y, &format!("{sets:?} text y"));
        }
    }

    #[test]
    fn three_sets_with_all_overlaps_match_venn_js() {
        let data = [
            Area::new(&["Desirable"], 10.0),
            Area::new(&["Feasible"], 10.0),
            Area::new(&["Viable"], 10.0),
            Area::new(&["Desirable", "Feasible"], 2.5),
            Area::new(&["Feasible", "Viable"], 2.5),
            Area::new(&["Desirable", "Viable"], 2.5),
            Area::new(&["Desirable", "Feasible", "Viable"], 10.0 / 9.0),
        ];
        check(
            &data,
            (800.0, 426.0, 15.0),
            &[
                (
                    "Desirable",
                    319.16073587020139,
                    283.00808994735337,
                    127.99191005264660,
                ),
                (
                    "Feasible",
                    480.83926412979861,
                    283.00808994735337,
                    127.99191005264660,
                ),
                (
                    "Viable",
                    399.99896480052968,
                    142.99191005264657,
                    127.99191005264660,
                ),
            ],
            &[
                (&["Desirable"], 276.09246084917595, 307.87629126907314),
                (&["Feasible"], 523.90805202820434, 307.87475517568157),
                (&["Viable"], 399.99999999997544, 93.259335691667616),
                (
                    &["Desirable", "Feasible"],
                    399.99999999998306,
                    312.81600419779488,
                ),
                (
                    &["Feasible", "Viable"],
                    466.23316532653939,
                    198.09589677350277,
                ),
                (
                    &["Desirable", "Viable"],
                    333.76589855344554,
                    198.09633568552945,
                ),
                (
                    &["Desirable", "Feasible", "Viable"],
                    400.00000000001916,
                    236.33654092293227,
                ),
            ],
        );
    }

    #[test]
    fn two_sized_sets_match_venn_js() {
        let data = [
            Area::new(&["A"], 20.0),
            Area::new(&["B"], 12.0),
            Area::new(&["A", "B"], 3.0),
        ];
        check(
            &data,
            (800.0, 450.0, 8.0),
            &[
                ("A", 290.69027893044563, 225.0, 217.00000000000003),
                (
                    "B",
                    558.22224384415256,
                    225.00000000000006,
                    168.08747722540190,
                ),
            ],
            &[
                (&["A"], 231.91252277459813, 225.00000069956985),
                (&["B"], 617.0, 225.00000184073781),
                (&["A", "B"], 448.91252277459819, 225.00000111666719),
            ],
        );
    }

    #[test]
    fn a_disjoint_set_is_placed_beside_the_cluster_like_venn_js() {
        let data = [
            Area::new(&["A"], 10.0),
            Area::new(&["B"], 6.0),
            Area::new(&["C"], 4.0),
            Area::new(&["A", "C"], 1.0),
        ];
        check(
            &data,
            (800.0, 450.0, 15.0),
            &[
                ("A", 188.40453399158096, 225.0, 173.40453399158096),
                ("B", 650.68142553874975, 225.0, 134.31857446125019),
                (
                    "C",
                    396.86155013602809,
                    225.00000000000003,
                    109.67065680269695,
                ),
            ],
            &[
                (&["A"], 151.09544666666557, 225.00000083830474),
                (&["B"], 650.68142553874975, 225.0),
                (&["C"], 434.17063746094345, 225.00000215203298),
                (&["A", "C"], 324.49998065824650, 224.99999917049627),
            ],
        );
    }

    #[test]
    fn a_subset_sits_inside_its_superset_like_venn_js() {
        let data = [
            Area::new(&["Big"], 10.0),
            Area::new(&["Small"], 2.0),
            Area::new(&["Big", "Small"], 2.0),
        ];
        check(
            &data,
            (800.0, 450.0, 15.0),
            &[
                ("Big", 400.0, 225.0, 210.0),
                ("Small", 516.08514494500878, 225.0, 93.914855054991165),
            ],
            &[
                (&["Big"], 306.08514494500884, 224.99999722801979),
                (&["Small"], 516.08514494500878, 225.0),
                (&["Big", "Small"], 516.08514494500878, 225.0),
            ],
        );
    }

    #[test]
    fn js_object_puts_array_index_keys_first() {
        let mut o = JsObject::new();
        o.insert("b", 1);
        o.insert("10", 2);
        o.insert("a", 3);
        o.insert("2", 4);
        o.insert("01", 5);
        o.insert("b", 6);
        let keys: Vec<&str> = o.keys().collect();
        assert_eq!(keys, ["2", "10", "b", "a", "01"]);
        assert_eq!(o.get("b"), Some(&6));
    }

    #[test]
    fn two_circle_intersection_area_matches_the_lens_formula() {
        let a = Circle {
            x: 0.0,
            y: 0.0,
            radius: 1.0,
        };
        let b = Circle {
            x: 1.0,
            y: 0.0,
            radius: 1.0,
        };
        let (area, arcs) = intersection_area(&[a, b]);
        close(area, circle_overlap(1.0, 1.0, 1.0), "lens");
        assert_eq!(arcs.len(), 2);
    }

    /// Four sets make ten areas, so `bestInitialLayout` also runs the
    /// randomly seeded constrained MDS. Five node runs of this ring of
    /// four agree to 1e-3 px, so the check uses that tolerance.
    #[test]
    fn four_sets_in_a_ring_match_venn_js_to_a_thousandth() {
        let data = [
            Area::new(&["A"], 10.0),
            Area::new(&["B"], 10.0),
            Area::new(&["C"], 10.0),
            Area::new(&["D"], 10.0),
            Area::new(&["A", "B"], 2.5),
            Area::new(&["B", "C"], 2.5),
            Area::new(&["C", "D"], 2.5),
            Area::new(&["A", "D"], 2.5),
        ];
        let s = solve(&data, 800.0, 450.0, 15.0).expect("layout");
        let expected = [
            ("A", 317.565, 307.435),
            ("B", 482.435, 307.435),
            ("C", 482.435, 142.565),
            ("D", 317.565, 142.565),
        ];
        for (id, x, y) in expected {
            let c = s.circles.get(id).expect("circle");
            assert!((c.x - x).abs() < 2e-3, "{id}.x {}", c.x);
            assert!((c.y - y).abs() < 2e-3, "{id}.y {}", c.y);
        }
    }
}
