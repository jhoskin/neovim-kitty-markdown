//! Port of mermaid 12.2.0 `rendering-util/rendering-elements/edges.js`,
//! `edgeMarker.ts`, `utils/lineWithOffset.ts`, and the edge-label math of
//! `utils.ts` (`calcLabelPosition`, `calcTerminalLabelPosition`,
//! `calcCardinalityPosition`).
//!
//! `insertEdge` turns the layout's point list into the path the oracle
//! draws: clip the ends to the node shapes, cut the path at cluster
//! boundaries, pre-round right-angle corners, offset the ends for the
//! markers, run the d3 curve, and name the markers. `insertEdgeLabel` and
//! `positionEdgeLabel` build and place the label groups.
//!
//! Stroke colors come from the stylesheet in mermaid (`.flowchart-link {
//! stroke: lineColor }`, `.edge-thickness-*`, `.edge-pattern-*`); the port
//! resolves those classes to attributes and applies the edge's own `style`
//! declarations on top.

use std::collections::HashMap;

use crate::mermaid::scene::curves::{self, CurveKind, ROUNDED_RADIUS};
use crate::mermaid::scene::markers::marker_id;
use crate::mermaid::scene::{
    Color, Common, Element, Group, Paint, Path, PathData, Point, Rect, Style,
};
use crate::mermaid::shapes::measure::{LabelBox, LabelOpts};
use crate::mermaid::shapes::util::{
    Declarations, apply_declarations, label_text, parse_css_color, translate,
};
use crate::mermaid::shapes::{Bounds, Intersector, Look, Node, ShapeCtx};

/// `edge.thickness` of `types.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Thickness {
    Normal,
    Thick,
    Invisible,
    Dotted,
}

impl Thickness {
    pub fn parse(s: &str) -> Option<Thickness> {
        match s {
            "normal" => Some(Thickness::Normal),
            "thick" => Some(Thickness::Thick),
            "invisible" => Some(Thickness::Invisible),
            "dotted" => Some(Thickness::Dotted),
            _ => None,
        }
    }
}

/// `edge.pattern` of `types.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    Solid,
    Dotted,
    Dashed,
}

impl Pattern {
    pub fn parse(s: &str) -> Option<Pattern> {
        match s {
            "solid" | "normal" => Some(Pattern::Solid),
            "dotted" => Some(Pattern::Dotted),
            "dashed" => Some(Pattern::Dashed),
            _ => None,
        }
    }
}

/// `Edge` of `rendering-util/types.ts`: the fields the renderer reads,
/// plus the layout results.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Edge {
    pub id: String,
    pub start: String,
    pub end: String,
    pub label: Option<String>,
    pub label_type: Option<String>,
    /// `classes`: `edge-thickness-normal edge-pattern-solid flowchart-link`
    /// for a flowchart edge.
    pub classes: Option<String>,
    /// `style`: `linkStyle` declarations, one per entry.
    pub style: Vec<String>,
    pub css_compiled_styles: Vec<String>,
    pub label_style: Vec<String>,
    pub arrow_type_start: Option<String>,
    pub arrow_type_end: Option<String>,
    pub thickness: Option<Thickness>,
    pub pattern: Option<Pattern>,
    /// `curve` as a name (`basis`, `linear`, `rounded`, ...).
    pub curve: Option<String>,
    pub labelpos: Option<String>,
    pub minlen: Option<f64>,
    pub start_label_left: Option<String>,
    pub start_label_right: Option<String>,
    pub end_label_left: Option<String>,
    pub end_label_right: Option<String>,
    pub look: Look,
    /// Layout output: the route and the label centre.
    pub points: Vec<Point>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    /// Label size from `insertEdgeLabel`.
    pub width: f64,
    pub height: f64,
    /// Set by `adjustClustersAndEdges` when an end was a cluster.
    pub from_cluster: Option<String>,
    pub to_cluster: Option<String>,
}

impl Edge {
    pub fn new(id: &str, start: &str, end: &str) -> Edge {
        Edge {
            id: id.to_string(),
            start: start.to_string(),
            end: end.to_string(),
            ..Edge::default()
        }
    }

    /// A flowchart edge as `flowDb.getData` builds it: `arrow_point` at
    /// the end, the flowchart classes, `labelpos` `c`.
    pub fn flowchart(id: &str, start: &str, end: &str) -> Edge {
        Edge {
            arrow_type_start: Some("none".to_string()),
            arrow_type_end: Some("arrow_point".to_string()),
            classes: Some("edge-thickness-normal edge-pattern-solid flowchart-link".to_string()),
            labelpos: Some("c".to_string()),
            thickness: Some(Thickness::Normal),
            pattern: Some(Pattern::Solid),
            label_type: Some("text".to_string()),
            ..Edge::new(id, start, end)
        }
    }

    /// `hasEdgeLabel(edge)`.
    pub fn has_label(&self) -> bool {
        let some = |s: &Option<String>| s.as_deref().is_some_and(|s| !s.is_empty());
        some(&self.label)
            || some(&self.start_label_left)
            || some(&self.start_label_right)
            || some(&self.end_label_left)
            || some(&self.end_label_right)
    }
}

/// `markerOffsets` of `lineWithOffset.ts`: how much shorter to draw the
/// line under a transparent marker.
pub fn marker_offset(arrow_type: &str) -> Option<f64> {
    match arrow_type {
        "aggregation" | "extension" | "composition" => Some(17.25),
        "dependency" => Some(6.0),
        "lollipop" => Some(13.5),
        "arrow_point" => Some(4.0),
        "arrow_barb" => Some(0.0),
        "arrow_barb_neo" => Some(5.5),
        _ => None,
    }
}

/// `markerOffsets2`: only the neo look reads it, kept for completeness.
pub fn marker_offset2(arrow_type: &str) -> Option<f64> {
    match arrow_type {
        "arrow_point" => Some(4.0),
        "arrow_cross" | "arrow_circle" => Some(12.5),
        _ => None,
    }
}

/// `calculateDeltaAndAngle` of `lineWithOffset.ts`: `Math.atan(dy / dx)`,
/// which is `NaN` for a zero-length segment.
fn delta_and_angle_atan(p1: Point, p2: Point) -> (f64, f64, f64) {
    let dx = p2.x - p1.x;
    let dy = p2.y - p1.y;
    ((dy / dx).atan(), dx, dy)
}

/// `getLineFunctionsWithOffset(edge)` applied to every point: the `x` and
/// `y` accessors d3 `line()` calls, with the marker offsets and the
/// "too close to the end" adjustments.
pub fn line_points_with_offset(edge: &Edge, data: &[Point]) -> Vec<Point> {
    let n = data.len();
    if n == 0 {
        return Vec::new();
    }
    let first = data[0];
    let last = data[n - 1];
    let start_type = edge.arrow_type_start.as_deref().unwrap_or("");
    let end_type = edge.arrow_type_end.as_deref().unwrap_or("");
    let start_marker_height = marker_offset(start_type);
    let end_marker_height = marker_offset(end_type);
    let extra_room = 1.0;
    // `DIRECTION`: `'left'` when the path runs rightwards, else `'right'`;
    // same for `'down'`/`'up'`. A NaN compares false, as in JS.
    let dir_right = first.x >= last.x || first.x.is_nan() || last.x.is_nan();
    let dir_up = first.y >= last.y || first.y.is_nan() || last.y.is_nan();
    data.iter()
        .enumerate()
        .map(|(i, d)| {
            // x
            let mut ox = 0.0;
            if i == 0 && start_marker_height.is_some() {
                let (angle, dx, _) = delta_and_angle_atan(data[0], data[1.min(n - 1)]);
                ox = start_marker_height.unwrap_or(0.0)
                    * angle.cos()
                    * if dx >= 0.0 { 1.0 } else { -1.0 };
            } else if i == n - 1 && end_marker_height.is_some() {
                let (angle, dx, _) = delta_and_angle_atan(data[n - 1], data[n.saturating_sub(2)]);
                ox = end_marker_height.unwrap_or(0.0)
                    * angle.cos()
                    * if dx >= 0.0 { 1.0 } else { -1.0 };
            }
            let dte = (d.x - last.x).abs();
            let diy_end = (d.y - last.y).abs();
            let dts = (d.x - first.x).abs();
            let diy_start = (d.y - first.y).abs();
            if let Some(emh) = end_marker_height
                && dte < emh
                && dte > 0.0
                && diy_end < emh
            {
                let mut adjustment = emh + extra_room - dte;
                adjustment *= if dir_right { -1.0 } else { 1.0 };
                ox -= adjustment;
            }
            if let Some(smh) = start_marker_height
                && dts < smh
                && dts > 0.0
                && diy_start < smh
            {
                let mut adjustment = smh + extra_room - dts;
                adjustment *= if dir_right { -1.0 } else { 1.0 };
                ox += adjustment;
            }
            // y
            let mut oy = 0.0;
            if i == 0 && start_marker_height.is_some() {
                let (angle, _, dy) = delta_and_angle_atan(data[0], data[1.min(n - 1)]);
                oy = start_marker_height.unwrap_or(0.0)
                    * angle.sin().abs()
                    * if dy >= 0.0 { 1.0 } else { -1.0 };
            } else if i == n - 1 && end_marker_height.is_some() {
                let (angle, _, dy) = delta_and_angle_atan(data[n - 1], data[n.saturating_sub(2)]);
                oy = end_marker_height.unwrap_or(0.0)
                    * angle.sin().abs()
                    * if dy >= 0.0 { 1.0 } else { -1.0 };
            }
            let dte_y = (d.y - last.y).abs();
            let dix_end = (d.x - last.x).abs();
            let dts_y = (d.y - first.y).abs();
            let dix_start = (d.x - first.x).abs();
            if let Some(emh) = end_marker_height
                && dte_y < emh
                && dte_y > 0.0
                && dix_end < emh
            {
                let mut adjustment = emh + extra_room - dte_y;
                adjustment *= if dir_up { -1.0 } else { 1.0 };
                oy -= adjustment;
            }
            if let Some(smh) = start_marker_height
                && dts_y < smh
                && dts_y > 0.0
                && dix_start < smh
            {
                let mut adjustment = smh + extra_room - dts_y;
                adjustment *= if dir_up { -1.0 } else { 1.0 };
                oy += adjustment;
            }
            Point::new(d.x + ox, d.y + oy)
        })
        .collect()
}

/// `applyMarkerOffsetsToPoints(points, edge)` of `edges.js`, used by the
/// `rounded` curve: `Math.atan2` here, unlike `lineWithOffset.ts`.
pub fn apply_marker_offsets_to_points(points: &[Point], edge: &Edge) -> Vec<Point> {
    let mut new_points = points.to_vec();
    let n = points.len();
    if n >= 2
        && let Some(offset) = edge
            .arrow_type_start
            .as_deref()
            .and_then(marker_offset)
            .filter(|v| *v != 0.0)
    {
        let p1 = points[0];
        let p2 = points[1];
        let angle = (p2.y - p1.y).atan2(p2.x - p1.x);
        new_points[0].x = p1.x + offset * angle.cos();
        new_points[0].y = p1.y + offset * angle.sin();
    }
    if n >= 2
        && let Some(offset) = edge
            .arrow_type_end
            .as_deref()
            .and_then(marker_offset)
            .filter(|v| *v != 0.0)
    {
        let p1 = points[n - 1];
        let p2 = points[n - 2];
        let angle = (p1.y - p2.y).atan2(p1.x - p2.x);
        new_points[n - 1].x = p1.x - offset * angle.cos();
        new_points[n - 1].y = p1.y - offset * angle.sin();
    }
    new_points
}

/// `outsideNode(node, point)`.
fn outside_node(node: &Bounds, point: Point) -> bool {
    let dx = (point.x - node.x).abs();
    let dy = (point.y - node.y).abs();
    dx >= node.width / 2.0 || dy >= node.height / 2.0
}

/// `intersection(node, outsidePoint, insidePoint)` of `edges.js`: where the
/// segment from an outside point to an inside point crosses the box.
pub fn intersection(node: &Bounds, outside: Point, inside: Point) -> Point {
    let x = node.x;
    let y = node.y;
    let w = node.width / 2.0;
    let h = node.height / 2.0;
    let q_total = (outside.y - inside.y).abs();
    let r_total = (outside.x - inside.x).abs();

    if (y - outside.y).abs() * w > (x - outside.x).abs() * h {
        // Intersection is top or bottom of rect.
        let q = if inside.y < outside.y {
            outside.y - h - y
        } else {
            y - h - outside.y
        };
        let r = (r_total * q) / q_total;
        let mut res = Point::new(
            if inside.x < outside.x {
                inside.x + r
            } else {
                inside.x - r_total + r
            },
            if inside.y < outside.y {
                inside.y + q_total - q
            } else {
                inside.y - q_total + q
            },
        );
        if r == 0.0 {
            res.x = outside.x;
            res.y = outside.y;
        }
        if r_total == 0.0 {
            res.x = outside.x;
        }
        if q_total == 0.0 {
            res.y = outside.y;
        }
        res
    } else {
        // Intersection on sides of rect.
        let r = if inside.x < outside.x {
            outside.x - w - x
        } else {
            x - w - outside.x
        };
        let q = (q_total * r) / r_total;
        let mut rx = if inside.x < outside.x {
            inside.x + r_total - r
        } else {
            inside.x - r_total + r
        };
        let mut ry = if inside.y < outside.y {
            inside.y + q
        } else {
            inside.y - q
        };
        if r == 0.0 {
            rx = outside.x;
            ry = outside.y;
        }
        if r_total == 0.0 {
            rx = outside.x;
        }
        if q_total == 0.0 {
            ry = outside.y;
        }
        Point::new(rx, ry)
    }
}

/// `cutPathAtIntersect(_points, boundaryNode)`: keeps the points outside
/// the cluster and replaces the first inside point by the boundary
/// crossing.
pub fn cut_path_at_intersect(points_in: &[Point], boundary: &Bounds) -> Vec<Point> {
    let mut points: Vec<Point> = Vec::new();
    let mut last_point_outside = points_in.first().copied().unwrap_or_default();
    let mut is_inside = false;
    for &point in points_in {
        if !outside_node(boundary, point) && !is_inside {
            let inter = intersection(boundary, last_point_outside, point);
            if !points.iter().any(|e| e.x == inter.x && e.y == inter.y) {
                points.push(inter);
            }
            is_inside = true;
        } else {
            last_point_outside = point;
            if !is_inside {
                points.push(point);
            }
        }
    }
    points
}

/// `extractCornerPoints(points)`: indices of right-angle corners whose
/// legs are both longer than 5.
fn extract_corner_point_positions(points: &[Point]) -> Vec<usize> {
    let mut positions = Vec::new();
    if points.len() < 3 {
        return positions;
    }
    for i in 1..points.len() - 1 {
        let prev = points[i - 1];
        let curr = points[i];
        let next = points[i + 1];
        if (prev.x == curr.x
            && curr.y == next.y
            && (curr.x - next.x).abs() > 5.0
            && (curr.y - prev.y).abs() > 5.0)
            || (prev.y == curr.y
                && curr.x == next.x
                && (curr.x - prev.x).abs() > 5.0
                && (curr.y - next.y).abs() > 5.0)
        {
            positions.push(i);
        }
    }
    positions
}

/// `findAdjacentPoint(pointA, pointB, distance)`: the point `distance`
/// before `pointB` on the segment from `pointA`.
fn find_adjacent_point(a: Point, b: Point, distance: f64) -> Point {
    let x_diff = b.x - a.x;
    let y_diff = b.y - a.y;
    let length = (x_diff * x_diff + y_diff * y_diff).sqrt();
    let ratio = distance / length;
    Point::new(b.x - ratio * x_diff, b.y - ratio * y_diff)
}

/// `fixCorners(lineData)`: replaces each right-angle corner by two points
/// 5 away and a nudged corner, so the basis curve rounds it tightly.
pub fn fix_corners(line_data: &[Point]) -> Vec<Point> {
    let positions = extract_corner_point_positions(line_data);
    let mut out = Vec::new();
    for (i, &p) in line_data.iter().enumerate() {
        if positions.contains(&i) {
            let prev_point = line_data[i - 1];
            let next_point = line_data[i + 1];
            let corner_point = p;
            let new_prev = find_adjacent_point(prev_point, corner_point, 5.0);
            let new_next = find_adjacent_point(next_point, corner_point, 5.0);
            let x_diff = new_next.x - new_prev.x;
            let y_diff = new_next.y - new_prev.y;
            out.push(new_prev);
            let a = 2.0_f64.sqrt() * 2.0;
            let mut new_corner = corner_point;
            if (next_point.x - prev_point.x).abs() > 10.0
                && (next_point.y - prev_point.y).abs() >= 10.0
            {
                let r = 5.0;
                if corner_point.x == new_prev.x {
                    new_corner = Point::new(
                        if x_diff < 0.0 {
                            new_prev.x - r + a
                        } else {
                            new_prev.x + r - a
                        },
                        if y_diff < 0.0 {
                            new_prev.y - a
                        } else {
                            new_prev.y + a
                        },
                    );
                } else {
                    new_corner = Point::new(
                        if x_diff < 0.0 {
                            new_prev.x - a
                        } else {
                            new_prev.x + a
                        },
                        if y_diff < 0.0 {
                            new_prev.y - r + a
                        } else {
                            new_prev.y + r - a
                        },
                    );
                }
            }
            out.push(new_corner);
            out.push(new_next);
        } else {
            out.push(p);
        }
    }
    out
}

/// `resolveEdgeCurveType(edge.curve)` with the config fallback: the edge's
/// own curve name wins, else `flowchart.curve`.
pub fn resolve_edge_curve_type<'a>(
    edge: &'a Edge,
    config_curve: Option<&'a str>,
) -> Option<&'a str> {
    edge.curve.as_deref().or(config_curve)
}

/// The `switch (edgeCurveType)` of `insertEdge`: unknown and missing names
/// fall back to `curveBasis`; `rounded` draws with `curveLinear` input but
/// goes through `generateRoundedPath`.
pub fn curve_for(name: Option<&str>) -> CurveKind {
    match name {
        Some("linear") | Some("rounded") => CurveKind::Linear,
        Some("basis") => CurveKind::Basis,
        Some("cardinal") => CurveKind::Cardinal,
        Some("bumpX") => CurveKind::BumpX,
        Some("bumpY") => CurveKind::BumpY,
        Some("catmullRom") => CurveKind::CatmullRom,
        Some("monotoneX") => CurveKind::MonotoneX,
        Some("monotoneY") => CurveKind::MonotoneY,
        Some("natural") => CurveKind::Natural,
        Some("step") => CurveKind::Step,
        Some("stepAfter") => CurveKind::StepAfter,
        Some("stepBefore") => CurveKind::StepBefore,
        _ => CurveKind::Basis,
    }
}

/// `interpolateToCurve(interpolate, defaultCurve)` of `utils.ts`.
pub fn interpolate_to_curve(interpolate: Option<&str>, default: CurveKind) -> CurveKind {
    match interpolate {
        None | Some("") => default,
        Some(name) => CurveKind::from_mermaid(name).unwrap_or(default),
    }
}

/// `isLabelCoordinateInPath(point, dAttr)`: whether the rounded midpoint
/// coordinate appears in the path text, with every decimal in the path
/// rounded first.
pub fn is_label_coordinate_in_path(point: Point, d: &str) -> bool {
    let round = |v: f64| (v + 0.5).floor();
    let rounded_x = format!("{}", round(point.x));
    let rounded_y = format!("{}", round(point.y));
    let mut sanitized = String::new();
    let bytes: Vec<char> = d.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i + 1 < bytes.len() && bytes[i] == '.' && bytes[i + 1].is_ascii_digit() {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                let text: String = bytes[start..i].iter().collect();
                let v: f64 = text.parse().unwrap_or(0.0);
                sanitized.push_str(&format!("{}", round(v)));
            } else {
                sanitized.extend(&bytes[start..i]);
            }
        } else {
            sanitized.push(bytes[i]);
            i += 1;
        }
    }
    sanitized.contains(&rounded_x) || sanitized.contains(&rounded_y)
}

// ---------------------------------------------------------------------------
// utils.ts label positions

fn distance(p1: Point, p2: Point) -> f64 {
    ((p2.x - p1.x).powi(2) + (p2.y - p1.y).powi(2)).sqrt()
}

/// `roundNumber(num, precision)`.
fn round_number(num: f64, precision: i32) -> f64 {
    let factor = 10f64.powi(precision);
    (num * factor + 0.5).floor() / factor
}

/// `calculatePoint(points, distanceToTraverse)`: the point that far along
/// the polyline. `None` is the JS throw when the path is too short.
pub fn calculate_point(points: &[Point], distance_to_traverse: f64) -> Option<Point> {
    let mut prev: Option<Point> = None;
    let mut remaining = distance_to_traverse;
    for &point in points {
        if let Some(p) = prev {
            let vector_distance = distance(point, p);
            if vector_distance == 0.0 {
                return Some(p);
            }
            if vector_distance < remaining {
                remaining -= vector_distance;
            } else {
                let ratio = remaining / vector_distance;
                if ratio <= 0.0 {
                    return Some(p);
                }
                if ratio >= 1.0 {
                    return Some(point);
                }
                return Some(Point::new(
                    round_number((1.0 - ratio) * p.x + ratio * point.x, 5),
                    round_number((1.0 - ratio) * p.y + ratio * point.y, 5),
                ));
            }
        }
        prev = Some(point);
    }
    None
}

/// `traverseEdge(points)`: the point halfway along the polyline.
fn traverse_edge(points: &[Point]) -> Option<Point> {
    let mut prev: Option<Point> = None;
    let mut total = 0.0;
    for &p in points {
        if let Some(q) = prev {
            total += distance(p, q);
        }
        prev = Some(p);
    }
    calculate_point(points, total / 2.0)
}

/// `calcLabelPosition(points)`.
pub fn calc_label_position(points: &[Point]) -> Option<Point> {
    if points.len() == 1 {
        return Some(points[0]);
    }
    traverse_edge(points)
}

/// `calcCardinalityPosition(isRelationTypePresent, points, initialPosition)`.
pub fn calc_cardinality_position(
    is_relation_type_present: bool,
    points: &[Point],
    initial_position: Point,
) -> Option<Point> {
    let mut pts = points.to_vec();
    if pts.first().copied() != Some(initial_position) {
        pts.reverse();
    }
    let center = calculate_point(&pts, 25.0)?;
    let d = if is_relation_type_present { 10.0 } else { 5.0 };
    let angle = (pts[0].y - center.y).atan2(pts[0].x - center.x);
    Some(Point::new(
        angle.sin() * d + (pts[0].x + center.x) / 2.0,
        -angle.cos() * d + (pts[0].y + center.y) / 2.0,
    ))
}

/// The `position` argument of `calcTerminalLabelPosition`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalSide {
    StartLeft,
    StartRight,
    EndLeft,
    EndRight,
}

/// `calcTerminalLabelPosition(terminalMarkerSize, position, points)`.
pub fn calc_terminal_label_position(
    terminal_marker_size: f64,
    position: TerminalSide,
    points_in: &[Point],
) -> Option<Point> {
    let mut points = points_in.to_vec();
    if !matches!(position, TerminalSide::StartLeft | TerminalSide::StartRight) {
        points.reverse();
    }
    let center = calculate_point(&points, 25.0 + terminal_marker_size)?;
    let d = 10.0 + terminal_marker_size * 0.5;
    let angle = (points[0].y - center.y).atan2(points[0].x - center.x);
    let pi = std::f64::consts::PI;
    let (sx, cy) = match position {
        TerminalSide::StartLeft => ((angle + pi).sin(), -(angle + pi).cos()),
        TerminalSide::EndRight => ((angle - pi).sin(), -(angle - pi).cos()),
        TerminalSide::EndLeft | TerminalSide::StartRight => (angle.sin(), -angle.cos()),
    };
    Some(Point::new(
        sx * d + (points[0].x + center.x) / 2.0,
        cy * d + (points[0].y + center.y) / 2.0,
    ))
}

/// `getStylesFromArray(arr)`: splits declarations into the shape style and
/// the label style (`color:` and `text-align:`).
pub fn get_styles_from_array(arr: &[String]) -> (String, String) {
    let mut style = String::new();
    let mut label_style = String::new();
    for element in arr {
        if element.starts_with("color:") || element.starts_with("text-align:") {
            label_style.push_str(element);
            label_style.push(';');
        } else {
            style.push_str(element);
            style.push(';');
        }
    }
    (style, label_style)
}

// ---------------------------------------------------------------------------
// insertEdge

/// Everything `insertEdge` reads from `getConfig()` besides the shapes'
/// context.
pub struct EdgeCtx<'a> {
    pub shapes: &'a ShapeCtx<'a>,
    /// `flowchart.curve`, the fallback when the edge names no curve.
    pub curve: Option<String>,
    /// `getSubGraphTitleMargins(config).subGraphTitleTotalMargin`.
    pub sub_graph_title_total_margin: f64,
}

/// `arrowTypesMap` of `edgeMarker.ts`: the marker name and whether a
/// colored variant fills it.
pub fn arrow_type_info(arrow_type: &str) -> Option<(&'static str, bool)> {
    Some(match arrow_type {
        "arrow_cross" => ("cross", false),
        "arrow_point" => ("point", true),
        "arrow_barb" => ("barb", true),
        "arrow_barb_neo" => ("barb", true),
        "arrow_circle" => ("circle", false),
        "arrow_hierarchy" => ("hierarchy", false),
        "aggregation" => ("aggregation", false),
        "extension" => ("extension", false),
        "composition" => ("composition", true),
        "dependency" => ("dependency", true),
        "lollipop" => ("lollipop", false),
        "only_one" => ("onlyOne", false),
        "zero_or_one" => ("zeroOrOne", false),
        "one_or_more" => ("oneOrMore", false),
        "zero_or_more" => ("zeroOrMore", false),
        "requirement_arrow" => ("requirement_arrow", false),
        "requirement_contains" => ("requirement_contains", false),
        _ => return None,
    })
}

/// A marker the edge references. When `color` is set, the caller clones the
/// marker `original_id` under `id` with that stroke (and fill when
/// `fill`), as `addEdgeMarker` does in the DOM.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkerRef {
    pub id: String,
    pub original_id: String,
    pub marker_name: &'static str,
    pub color: Option<Color>,
    pub fill: bool,
}

/// `paths` returned by `insertEdge`: the route the label math uses.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EdgePaths {
    pub original_path: Vec<Point>,
    pub updated_path: Option<Vec<Point>>,
}

/// What `insertEdge` leaves behind: the path element, the paths, and the
/// markers it named.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeOut {
    pub path: Element,
    pub paths: EdgePaths,
    pub markers: Vec<MarkerRef>,
}

fn thickness_classes(edge: &Edge) -> (&'static str, f64, Option<Paint>) {
    match edge.thickness {
        Some(Thickness::Thick) => ("edge-thickness-thick", 3.5, None),
        Some(Thickness::Invisible) => ("edge-thickness-invisible", 0.0, Some(Paint::None)),
        _ => ("edge-thickness-normal", f64::NAN, None),
    }
}

fn pattern_classes(edge: &Edge) -> (&'static str, Option<Vec<f64>>) {
    match edge.pattern {
        Some(Pattern::Dotted) => ("edge-pattern-dotted", Some(vec![2.0])),
        Some(Pattern::Dashed) => ("edge-pattern-dashed", Some(vec![3.0])),
        _ => ("edge-pattern-solid", None),
    }
}

/// Splits `key:value` declarations.
fn declarations(decls: impl IntoIterator<Item = String>) -> Declarations {
    let mut map = Declarations::new();
    for decl in decls {
        for part in decl.split(';') {
            if let Some((k, v)) = part.split_once(':') {
                map.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    map
}

/// `insertEdge(elem, edge, clusterDb, diagramType, startNode, endNode,
/// diagramId)`. `cluster_db` maps a cluster id to the bounds of its drawn
/// rectangle (`clusterDb.get(id).node`).
pub fn insert_edge(
    edge: &Edge,
    cluster_db: &HashMap<String, Bounds>,
    diagram_type: &str,
    start_node: &Node,
    end_node: &Node,
    diagram_id: &str,
    ctx: &EdgeCtx,
) -> EdgeOut {
    let mut points = edge.points.clone();
    let mut points_has_changed = false;
    let edge_class_styles: Vec<String> = edge
        .css_compiled_styles
        .iter()
        .filter(|decl| {
            !decl
                .split_once(':')
                .is_some_and(|(k, _)| crate::mermaid::shapes::util::is_label_style(k.trim()))
        })
        .cloned()
        .collect();

    // Original clipping: drop dagre's own end points and clip the next
    // ones to the node shapes.
    // `if (head.intersect && tail.intersect)`: a cluster node drawn by a
    // sub-render has no intersect, and the path keeps dagre's own ends.
    if points.len() >= 2
        && start_node.intersector != Intersector::None
        && end_node.intersector != Intersector::None
    {
        let inner: Vec<Point> = points[1..points.len() - 1].to_vec();
        let first_inner = inner
            .first()
            .copied()
            .unwrap_or(Point::new(end_node.x, end_node.y));
        let last_inner = inner
            .last()
            .copied()
            .unwrap_or(Point::new(start_node.x, start_node.y));
        let mut clipped = vec![start_node.intersect(first_inner)];
        clipped.extend(inner);
        clipped.push(end_node.intersect(last_inner));
        points = clipped;
    }
    if let Some(to_cluster) = edge.to_cluster.as_deref()
        && let Some(bounds) = cluster_db.get(to_cluster)
    {
        points = cut_path_at_intersect(&edge.points, bounds);
        points_has_changed = true;
    }
    if let Some(from_cluster) = edge.from_cluster.as_deref()
        && let Some(bounds) = cluster_db.get(from_cluster)
    {
        let mut reversed = points.clone();
        reversed.reverse();
        let mut cut = cut_path_at_intersect(&reversed, bounds);
        cut.reverse();
        points = cut;
        points_has_changed = true;
    }

    let mut line_data: Vec<Point> = points.iter().copied().filter(|p| !p.y.is_nan()).collect();
    let edge_curve_type = resolve_edge_curve_type(edge, ctx.curve.as_deref());
    if edge_curve_type != Some("rounded") {
        line_data = fix_corners(&line_data);
    }
    let curve = curve_for(edge_curve_type);

    let (thickness_class, thickness_width, thickness_fill) = thickness_classes(edge);
    let (pattern_class, dasharray) = pattern_classes(edge);
    let mut stroke_classes = format!("{thickness_class} {pattern_class}");

    let line_path = if edge_curve_type == Some("rounded") {
        curves::rounded(
            &apply_marker_offsets_to_points(&line_data, edge),
            ROUNDED_RADIUS,
        )
    } else {
        curves::line(&line_points_with_offset(edge, &line_data), curve)
    };

    // The stylesheet: `.flowchart-link { stroke: lineColor; fill: none }`,
    // `.edge-thickness-*`, `.edge-pattern-*`; then the inline style.
    let mut style = Style {
        fill: Some(thickness_fill.unwrap_or(Paint::None)),
        stroke: Some(Paint::Color(ctx.shapes.vars.line_color)),
        stroke_width: Some(if thickness_width.is_nan() {
            ctx.shapes.vars.stroke_width
        } else {
            thickness_width
        }),
        stroke_dasharray: dasharray,
        ..Style::default()
    };
    let mut inline = declarations(edge_class_styles.iter().cloned());
    inline.extend(declarations(edge.style.iter().cloned()));
    apply_declarations(&mut style, &inline);
    let stroke_color = inline
        .get("stroke")
        .and_then(|s| parse_css_color(s))
        .and_then(|p| match p {
            Paint::Color(c) => Some(c),
            _ => None,
        });

    if let Some(classes) = edge.classes.as_deref().filter(|c| !c.is_empty()) {
        stroke_classes.push(' ');
        stroke_classes.push_str(classes);
    }

    let mut markers = Vec::new();
    for (position, arrow_type) in [
        ("Start", edge.arrow_type_start.as_deref()),
        ("End", edge.arrow_type_end.as_deref()),
    ] {
        let Some(arrow_type) = arrow_type.filter(|t| !t.is_empty() && *t != "none") else {
            continue;
        };
        let Some((marker_name, fill)) = arrow_type_info(arrow_type) else {
            continue;
        };
        let original_id = marker_id(
            diagram_id,
            diagram_type,
            &format!("{marker_name}{position}"),
        );
        let (id, color) = match stroke_color {
            Some(color) => {
                let raw = inline.get("stroke").cloned().unwrap_or_default();
                let color_id: String = raw
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                    .collect();
                (format!("{original_id}_{color_id}"), Some(color))
            }
            None => (original_id.clone(), None),
        };
        if position == "Start" {
            style.marker_start = Some(id.clone());
        } else {
            style.marker_end = Some(id.clone());
        }
        markers.push(MarkerRef {
            id,
            original_id,
            marker_name,
            color,
            fill,
        });
    }

    let d_text = line_path.to_svg();
    let path = Element::Path(Path {
        common: Common {
            id: Some(format!("{diagram_id}-{}", edge.id)),
            class: Some(format!(" {stroke_classes}")),
            style,
            ..Common::default()
        },
        d: line_path,
    });

    let mid_index = points.len() / 2;
    if let Some(point) = points.get(mid_index)
        && !is_label_coordinate_in_path(*point, &d_text)
    {
        points_has_changed = true;
    }
    let paths = EdgePaths {
        original_path: edge.points.clone(),
        updated_path: points_has_changed.then_some(points),
    };
    EdgeOut {
        path,
        paths,
        markers,
    }
}

// ---------------------------------------------------------------------------
// Edge labels

/// The groups `insertEdgeLabel` creates, each positioned later by
/// `positionEdgeLabel`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EdgeLabels {
    /// `g.edgeLabel` with the measured size, when the edge has a label.
    pub label: Option<Group>,
    pub start_left: Option<Group>,
    pub start_right: Option<Group>,
    pub end_left: Option<Group>,
    pub end_right: Option<Group>,
}

/// `getLabelStyles(styleArray)` reduced to the `color:` the text reads.
fn label_color(edge: &Edge, ctx: &ShapeCtx) -> Color {
    declarations(edge.label_style.iter().cloned())
        .get("color")
        .and_then(|c| parse_css_color(c))
        .and_then(|p| match p {
            Paint::Color(c) => Some(c),
            _ => None,
        })
        .unwrap_or(ctx.vars.node_text_color)
}

/// One edge label block: the inner `g.label` holding the background and
/// the text, centred by `computeLabelTransform`, plus its measured box.
fn label_block(
    text: &str,
    ctx: &ShapeCtx,
    color: Color,
    markdown: bool,
    width: Option<f64>,
) -> (Group, LabelBox) {
    let opts = LabelOpts {
        font: ctx.font(),
        html: ctx.html_labels,
        width,
        min_width: 0.0,
        markdown,
    };
    let bbox = ctx.measurer.measure(text, &opts);
    let mut children = Vec::new();
    if !bbox.is_empty() {
        // HTML labels: the `.edgeLabel` span and its `p` paint
        // `edgeLabelBackground` behind the text; SVG labels: a `rect.background`
        // 2 px around the text at opacity 0.5.
        let (bx, by, bw, bh, opacity) = if ctx.html_labels {
            (0.0, 0.0, bbox.width, bbox.height, None)
        } else {
            (
                bbox.x - 2.0,
                bbox.y - 2.0,
                bbox.width + 4.0,
                bbox.height + 4.0,
                Some(0.5),
            )
        };
        children.push(Element::Rect(Rect {
            common: Common {
                class: Some("background".to_string()),
                style: Style {
                    fill: Some(Paint::Color(ctx.vars.edge_label_background)),
                    stroke: Some(Paint::None),
                    opacity,
                    ..Style::default()
                },
                ..Common::default()
            },
            x: bx,
            y: by,
            width: bw,
            height: bh,
            rx: None,
            ry: None,
        }));
        if let Some(mut text) = label_text(&bbox, ctx, color, ctx.html_labels) {
            text.common.class = Some("edgeLabel".to_string());
            children.push(Element::Text(text));
        }
    }
    // computeLabelTransform
    let transform = if ctx.html_labels {
        translate(-bbox.width / 2.0, -bbox.height / 2.0)
    } else {
        translate(-(bbox.x + bbox.width / 2.0), -(bbox.y + bbox.height / 2.0))
    };
    let group = Group {
        common: Common {
            class: Some("label".to_string()),
            transform,
            ..Common::default()
        },
        children,
    };
    (group, bbox)
}

/// `insertEdgeLabel(elem, edge)`: builds the label groups and writes the
/// label size to `edge.width`/`edge.height`.
pub fn insert_edge_label(edge: &mut Edge, ctx: &ShapeCtx) -> EdgeLabels {
    let color = label_color(edge, ctx);
    let mut out = EdgeLabels::default();
    let label = edge.label.clone().unwrap_or_default();
    let is_markdown = edge.label_type.as_deref() == Some("markdown");
    // `createText` defaults `width` to 200 when neither branch sets it.
    let (inner, bbox) = label_block(&label, ctx, color, is_markdown, Some(200.0));
    let (width, height) = if ctx.html_labels || bbox.is_empty() {
        (bbox.width, bbox.height)
    } else {
        // `labelElement.getBBox()` includes the background padding.
        (bbox.width + 4.0, bbox.height + 4.0)
    };
    out.label = Some(Group {
        common: Common {
            class: Some("edgeLabel".to_string()),
            ..Common::default()
        },
        children: vec![Element::Group(inner)],
    });
    edge.width = width;
    edge.height = height;

    let terminal = |text: &Option<String>| -> Option<Group> {
        let text = text.as_deref().filter(|t| !t.is_empty())?;
        let (mut inner, _) = label_block(text, ctx, color, false, None);
        inner.common.class = Some("inner".to_string());
        Some(Group {
            common: Common {
                class: Some("edgeTerminals".to_string()),
                ..Common::default()
            },
            children: vec![Element::Group(inner)],
        })
    };
    out.start_left = terminal(&edge.start_label_left);
    out.start_right = terminal(&edge.start_label_right);
    out.end_left = terminal(&edge.end_label_left);
    out.end_right = terminal(&edge.end_label_right);
    out
}

/// `resolveEdgeLabelPosition(edge, paths)`.
pub fn resolve_edge_label_position(edge: &Edge, paths: &EdgePaths) -> (f64, f64) {
    let mut x = edge.x.unwrap_or(0.0);
    let mut y = edge.y.unwrap_or(0.0);
    if let Some(updated) = &paths.updated_path
        && let Some(updated_mid) = calc_label_position(updated)
    {
        if !paths.original_path.is_empty()
            && let Some(original_mid) = calc_label_position(&paths.original_path)
        {
            x = edge.x.unwrap_or(0.0) + (updated_mid.x - original_mid.x);
            y = edge.y.unwrap_or(0.0) + (updated_mid.y - original_mid.y);
        } else {
            x = updated_mid.x;
            y = updated_mid.y;
        }
    }
    (x, y)
}

/// `positionEdgeLabel(edge, paths)`: translates the label groups to their
/// final places.
pub fn position_edge_label(
    labels: &mut EdgeLabels,
    edge: &Edge,
    paths: &EdgePaths,
    sub_graph_title_total_margin: f64,
) {
    if edge.label.as_deref().is_some_and(|l| !l.is_empty())
        && let Some(el) = labels.label.as_mut()
    {
        let (x, y) = resolve_edge_label_position(edge, paths);
        el.common.transform = translate(x, y + sub_graph_title_total_margin / 2.0);
    }
    let path: &[Point] = paths
        .updated_path
        .as_deref()
        .unwrap_or(&paths.original_path);
    let start_size = if edge
        .arrow_type_start
        .as_deref()
        .is_some_and(|t| !t.is_empty())
    {
        10.0
    } else {
        0.0
    };
    let end_size = if edge
        .arrow_type_end
        .as_deref()
        .is_some_and(|t| !t.is_empty())
    {
        10.0
    } else {
        0.0
    };
    let fallback = Point::new(edge.x.unwrap_or(0.0), edge.y.unwrap_or(0.0));
    for (group, side, size) in [
        (&mut labels.start_left, TerminalSide::StartLeft, start_size),
        (
            &mut labels.start_right,
            TerminalSide::StartRight,
            start_size,
        ),
        (&mut labels.end_left, TerminalSide::EndLeft, end_size),
        (&mut labels.end_right, TerminalSide::EndRight, end_size),
    ] {
        if let Some(el) = group.as_mut() {
            let pos = if path.is_empty() {
                fallback
            } else {
                calc_terminal_label_position(size, side, path).unwrap_or(fallback)
            };
            el.common.transform = translate(pos.x, pos.y);
        }
    }
}

/// `generateRoundedPath(points, radius)` lives in the scene package as
/// [`curves::rounded`]; re-exported here so edge callers find it.
pub fn generate_rounded_path(points: &[Point], radius: f64) -> PathData {
    curves::rounded(points, radius)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersection_keeps_the_js_numbers() {
        let node = Bounds::new(0.0, 0.0, 100.0, 50.0);
        let p = intersection(&node, Point::new(200.0, 0.0), Point::new(0.0, 0.0));
        assert_eq!(p, Point::new(50.0, 0.0));
        // A vertical approach has `R === 0`, which the JS answers with the
        // outside point itself.
        let p = intersection(&node, Point::new(0.0, -200.0), Point::new(0.0, 0.0));
        assert_eq!(p, Point::new(0.0, -200.0));
        // The diagonal case: the JS mirrors `x` (75 where the geometry says
        // 25); the port keeps that.
        let p = intersection(&node, Point::new(100.0, 100.0), Point::new(0.0, 0.0));
        assert_eq!(p, Point::new(75.0, 25.0));
    }

    #[test]
    fn cut_path_keeps_outside_points_and_one_crossing() {
        let cluster = Bounds::new(0.0, 100.0, 100.0, 100.0);
        let pts = vec![
            Point::new(-100.0, -50.0),
            Point::new(-20.0, 30.0),
            Point::new(0.0, 100.0),
        ];
        let cut = cut_path_at_intersect(&pts, &cluster);
        assert_eq!(cut.len(), 3);
        assert_eq!(cut[0], pts[0]);
        assert_eq!(cut[1], pts[1]);
        assert_eq!(cut[2], intersection(&cluster, pts[1], pts[2]));
    }

    #[test]
    fn fix_corners_rounds_a_right_angle() {
        let pts = vec![
            Point::new(0.0, 0.0),
            Point::new(0.0, 100.0),
            Point::new(100.0, 100.0),
        ];
        let fixed = fix_corners(&pts);
        assert_eq!(fixed.len(), 5);
        assert_eq!(fixed[1], Point::new(0.0, 95.0));
        assert_eq!(fixed[3], Point::new(5.0, 100.0));
        let a = 2.0_f64.sqrt() * 2.0;
        assert!((fixed[2].x - (0.0 + 5.0 - a)).abs() < 1e-12);
        assert!((fixed[2].y - (95.0 + a)).abs() < 1e-12);
    }

    #[test]
    fn calculate_point_walks_the_polyline() {
        let pts = vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
        ];
        assert_eq!(calculate_point(&pts, 15.0), Some(Point::new(10.0, 5.0)));
        assert_eq!(calc_label_position(&pts), Some(Point::new(10.0, 0.0)));
    }

    #[test]
    fn terminal_label_position_offsets_perpendicular() {
        let pts = vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)];
        let p = calc_terminal_label_position(0.0, TerminalSide::StartLeft, &pts).unwrap();
        assert!((p.x - 12.5).abs() < 1e-9);
        assert!((p.y + 10.0).abs() < 1e-9);
        let p = calc_terminal_label_position(0.0, TerminalSide::StartRight, &pts).unwrap();
        assert!((p.y - 10.0).abs() < 1e-9);
    }

    #[test]
    fn label_coordinate_check_rounds_decimals() {
        assert!(is_label_coordinate_in_path(
            Point::new(13.2, 99.0),
            "M12.5,3L7,8"
        ));
        assert!(!is_label_coordinate_in_path(
            Point::new(50.0, 60.0),
            "M12.5,3L7,8"
        ));
    }

    #[test]
    fn marker_offsets_shorten_the_line_ends() {
        let edge = Edge::flowchart("e", "A", "B");
        let pts = vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)];
        let out = line_points_with_offset(&edge, &pts);
        assert_eq!(out[0], Point::new(0.0, 0.0));
        assert!((out[1].x - 96.0).abs() < 1e-9);
    }
}
