//! The marker table of mermaid 12.2.0
//! `rendering-util/rendering-elements/markers.js`, as scene elements.
//!
//! Each entry reproduces one `markers[name]` function: the same marker ids
//! (`<id>_<type>-<name>` with `Start`, `End`, and `-margin` variants), the
//! same `refX`/`refY`, sizes, units, and geometry, and the inline
//! `.style(...)`/`.attr(...)` values the function sets itself. Colors come
//! from mermaid's stylesheet (`.marker { fill; stroke }`), which the styles
//! package resolves; the caller applies them with
//! [`Marker::apply_paint`], which fills only what a marker leaves unset.
//!
//! The `*_neo` and `barbNeo` entries belong to the `neo` look, which the
//! helper reports as `unsupported_feature`, so they are not ported;
//! [`NEO_ONLY`] lists them for the completeness test.

use super::{
    Circle, Color, Common, Element, Group, Line, Marker, MarkerUnits, Orient, Paint, Path,
    PathData, Point, Polygon, Style, ViewBox,
};

/// A key of the `markers` object in `markers.js` that the classic look uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkerType {
    Extension,
    Composition,
    Aggregation,
    Dependency,
    Lollipop,
    Point,
    Circle,
    Cross,
    Barb,
    Hierarchy,
    OnlyOne,
    ZeroOrOne,
    OneOrMore,
    ZeroOrMore,
    RequirementArrow,
    RequirementContains,
}

impl MarkerType {
    pub const ALL: [MarkerType; 16] = [
        MarkerType::Extension,
        MarkerType::Composition,
        MarkerType::Aggregation,
        MarkerType::Dependency,
        MarkerType::Lollipop,
        MarkerType::Point,
        MarkerType::Circle,
        MarkerType::Cross,
        MarkerType::Barb,
        MarkerType::Hierarchy,
        MarkerType::OnlyOne,
        MarkerType::ZeroOrOne,
        MarkerType::OneOrMore,
        MarkerType::ZeroOrMore,
        MarkerType::RequirementArrow,
        MarkerType::RequirementContains,
    ];

    /// The key in `markers.js`.
    pub fn name(self) -> &'static str {
        match self {
            MarkerType::Extension => "extension",
            MarkerType::Composition => "composition",
            MarkerType::Aggregation => "aggregation",
            MarkerType::Dependency => "dependency",
            MarkerType::Lollipop => "lollipop",
            MarkerType::Point => "point",
            MarkerType::Circle => "circle",
            MarkerType::Cross => "cross",
            MarkerType::Barb => "barb",
            MarkerType::Hierarchy => "hierarchy",
            MarkerType::OnlyOne => "only_one",
            MarkerType::ZeroOrOne => "zero_or_one",
            MarkerType::OneOrMore => "one_or_more",
            MarkerType::ZeroOrMore => "zero_or_more",
            MarkerType::RequirementArrow => "requirement_arrow",
            MarkerType::RequirementContains => "requirement_contains",
        }
    }

    pub fn from_name(name: &str) -> Option<MarkerType> {
        MarkerType::ALL.iter().copied().find(|m| m.name() == name)
    }
}

/// Keys of `markers.js` that only the `neo` look selects.
pub const NEO_ONLY: [&str; 5] = [
    "barbNeo",
    "only_one_neo",
    "zero_or_one_neo",
    "one_or_more_neo",
    "zero_or_more_neo",
];

/// `id + '_' + type + '-' + name`, the id every marker and every
/// `marker-start`/`marker-end` reference uses.
pub fn marker_id(id: &str, diagram_type: &str, name: &str) -> String {
    format!("{id}_{diagram_type}-{name}")
}

/// `insertMarkers(elem, markerArray, type, id)`: every marker definition the
/// listed types create, in `markers.js` order within each type.
pub fn insert_markers(types: &[MarkerType], diagram_type: &str, id: &str) -> Vec<Marker> {
    let mut out = Vec::new();
    for ty in types {
        out.extend(markers_for(*ty, diagram_type, id));
    }
    out
}

/// The markers one `markers[name]` call appends.
pub fn markers_for(ty: MarkerType, diagram_type: &str, id: &str) -> Vec<Marker> {
    let b = Builder {
        id,
        ty: diagram_type,
    };
    match ty {
        MarkerType::Extension => b.extension(),
        MarkerType::Composition => b.diamond("composition", 15.0, 3.5, Some(0.0)),
        MarkerType::Aggregation => b.diamond("aggregation", 15.0, 1.0, Some(2.0)),
        MarkerType::Dependency => b.dependency(),
        MarkerType::Lollipop => b.lollipop(),
        MarkerType::Point => b.point(),
        MarkerType::Circle => b.circle(),
        MarkerType::Cross => b.cross(),
        MarkerType::Barb => b.barb(),
        MarkerType::Hierarchy => b.hierarchy(),
        MarkerType::OnlyOne => b.only_one(),
        MarkerType::ZeroOrOne => b.zero_or_one(),
        MarkerType::OneOrMore => b.one_or_more(),
        MarkerType::ZeroOrMore => b.zero_or_more(),
        MarkerType::RequirementArrow => b.requirement_arrow(),
        MarkerType::RequirementContains => b.requirement_contains(),
    }
}

impl Marker {
    /// Applies the stylesheet paint: every field the marker and its children
    /// leave unset takes the value from `paint`.
    pub fn apply_paint(&mut self, paint: &Style) {
        self.style.fill_unset_from(paint);
        for child in &mut self.children {
            apply_to_element(child, paint);
        }
    }
}

fn apply_to_element(element: &mut Element, paint: &Style) {
    match element {
        Element::Group(g) => {
            g.common.style.fill_unset_from(paint);
            for child in &mut g.children {
                apply_to_element(child, paint);
            }
        }
        Element::Rect(e) => e.common.style.fill_unset_from(paint),
        Element::Circle(e) => e.common.style.fill_unset_from(paint),
        Element::Ellipse(e) => e.common.style.fill_unset_from(paint),
        Element::Line(e) => e.common.style.fill_unset_from(paint),
        Element::Polygon(e) => e.common.style.fill_unset_from(paint),
        Element::Polyline(e) => e.common.style.fill_unset_from(paint),
        Element::Path(e) => e.common.style.fill_unset_from(paint),
        Element::Text(e) => e.common.style.fill_unset_from(paint),
        Element::Image(e) => e.common.style.fill_unset_from(paint),
        Element::Use(e) => e.common.style.fill_unset_from(paint),
        Element::Marker(m) => m.apply_paint(paint),
    }
}

struct Builder<'a> {
    id: &'a str,
    ty: &'a str,
}

/// Geometry of one marker before the children go in.
struct Spec<'a> {
    name: &'a str,
    class: Option<String>,
    view_box: Option<ViewBox>,
    ref_x: f64,
    ref_y: f64,
    width: f64,
    height: f64,
    units: MarkerUnits,
}

fn path(d: PathData, style: Style, class: Option<&str>) -> Element {
    Element::Path(Path {
        common: Common {
            class: class.map(str::to_string),
            style,
            ..Common::default()
        },
        d,
    })
}

fn polygon(points: &[(f64, f64)], style: Style, class: Option<&str>) -> Element {
    Element::Polygon(Polygon {
        common: Common {
            class: class.map(str::to_string),
            style,
            ..Common::default()
        },
        points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
    })
}

fn circle(cx: f64, cy: f64, r: f64, style: Style, class: Option<&str>) -> Element {
    Element::Circle(Circle {
        common: Common {
            class: class.map(str::to_string),
            style,
            ..Common::default()
        },
        cx,
        cy,
        r,
    })
}

fn stroke_style(width: f64, dasharray: Option<Vec<f64>>) -> Style {
    Style {
        stroke_width: Some(width),
        stroke_dasharray: dasharray,
        ..Style::default()
    }
}

/// `.style('stroke-dasharray', '1,0')`, the dash pattern mermaid uses to
/// cancel an inherited dashed edge on the arrowhead.
fn solid() -> Option<Vec<f64>> {
    Some(vec![1.0, 0.0])
}

/// A closed polygon path such as `M 18,7 L9,13 L1,7 L9,1 Z`.
fn closed(points: &[(f64, f64)]) -> PathData {
    let mut d = PathData::new();
    for (i, &(x, y)) in points.iter().enumerate() {
        if i == 0 {
            d.move_to(x, y);
        } else {
            d.line_to(x, y);
        }
    }
    d.close();
    d
}

/// Two separate strokes, `M a L b M c L d`.
fn two_lines(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> PathData {
    let mut p = PathData::new();
    p.move_to(a.0, a.1)
        .line_to(b.0, b.1)
        .move_to(c.0, c.1)
        .line_to(d.0, d.1);
    p
}

impl Builder<'_> {
    fn class(&self, kind: &str) -> Option<String> {
        if kind.is_empty() {
            Some(format!("marker {}", self.ty))
        } else {
            Some(format!("marker {kind} {}", self.ty))
        }
    }

    fn marker(&self, spec: Spec<'_>, children: Vec<Element>) -> Marker {
        Marker {
            id: marker_id(self.id, self.ty, spec.name),
            class: spec.class,
            view_box: spec.view_box,
            ref_x: spec.ref_x,
            ref_y: spec.ref_y,
            marker_width: spec.width,
            marker_height: spec.height,
            orient: Orient::Auto,
            marker_units: spec.units,
            style: Style::default(),
            children,
        }
    }

    fn extension(&self) -> Vec<Marker> {
        let class = self.class("extension");
        let spec = |name, ref_x, view_box| Spec {
            name,
            class: class.clone(),
            view_box,
            ref_x,
            ref_y: 7.0,
            width: 20.0,
            height: 28.0,
            units: MarkerUnits::UserSpaceOnUse,
        };
        let vb = Some(ViewBox::new(0.0, 0.0, 20.0, 14.0));
        vec![
            self.marker(
                spec("extensionStart", 18.0, None),
                vec![path(
                    closed(&[(1.0, 7.0), (18.0, 13.0), (18.0, 1.0)]),
                    Style::default(),
                    None,
                )],
            ),
            self.marker(
                spec("extensionEnd", 1.0, None),
                vec![path(
                    closed(&[(1.0, 1.0), (1.0, 13.0), (18.0, 7.0)]),
                    Style::default(),
                    None,
                )],
            ),
            self.marker(
                spec("extensionStart-margin", 18.0, vb),
                vec![polygon(
                    &[(10.0, 7.0), (18.0, 13.0), (18.0, 1.0)],
                    stroke_style(2.0, Some(vec![0.0])),
                    None,
                )],
            ),
            self.marker(
                spec("extensionEnd-margin", 9.0, vb),
                vec![polygon(
                    &[(10.0, 1.0), (10.0, 13.0), (18.0, 7.0)],
                    stroke_style(2.0, Some(vec![0.0])),
                    None,
                )],
            ),
        ]
    }

    /// `composition` and `aggregation` share the diamond
    /// `M 18,7 L9,13 L1,7 L9,1 Z` and differ only in the margin variants'
    /// `refX` and `stroke-width`.
    fn diamond(
        &self,
        kind: &str,
        start_margin_ref_x: f64,
        end_margin_ref_x: f64,
        margin_stroke_width: Option<f64>,
    ) -> Vec<Marker> {
        let class = self.class(kind);
        let diamond = || closed(&[(18.0, 7.0), (9.0, 13.0), (1.0, 7.0), (9.0, 1.0)]);
        let spec = |name, ref_x, w, h| Spec {
            name,
            class: class.clone(),
            view_box: None,
            ref_x,
            ref_y: 7.0,
            width: w,
            height: h,
            units: MarkerUnits::UserSpaceOnUse,
        };
        let margin_style = Style {
            stroke_width: margin_stroke_width,
            ..Style::default()
        };
        let start = format!("{kind}Start");
        let end = format!("{kind}End");
        let start_margin = format!("{kind}Start-margin");
        let end_margin = format!("{kind}End-margin");
        vec![
            self.marker(
                spec(&start, 18.0, 190.0, 240.0),
                vec![path(diamond(), Style::default(), None)],
            ),
            self.marker(
                spec(&end, 1.0, 20.0, 28.0),
                vec![path(diamond(), Style::default(), None)],
            ),
            self.marker(
                spec(&start_margin, start_margin_ref_x, 190.0, 240.0),
                vec![path(diamond(), margin_style.clone(), None)],
            ),
            self.marker(
                spec(&end_margin, end_margin_ref_x, 20.0, 28.0),
                vec![path(diamond(), margin_style, None)],
            ),
        ]
    }

    fn dependency(&self) -> Vec<Marker> {
        let class = self.class("dependency");
        let start_shape = || closed(&[(5.0, 7.0), (9.0, 13.0), (1.0, 7.0), (9.0, 1.0)]);
        let end_shape = || closed(&[(18.0, 7.0), (9.0, 13.0), (14.0, 7.0), (9.0, 1.0)]);
        let spec = |name, ref_x, w, h| Spec {
            name,
            class: class.clone(),
            view_box: None,
            ref_x,
            ref_y: 7.0,
            width: w,
            height: h,
            units: MarkerUnits::UserSpaceOnUse,
        };
        let no_stroke = stroke_style(0.0, None);
        vec![
            self.marker(
                spec("dependencyStart", 6.0, 190.0, 240.0),
                vec![path(start_shape(), Style::default(), None)],
            ),
            self.marker(
                spec("dependencyEnd", 13.0, 20.0, 28.0),
                vec![path(end_shape(), Style::default(), None)],
            ),
            self.marker(
                spec("dependencyStart-margin", 4.0, 190.0, 240.0),
                vec![path(start_shape(), no_stroke.clone(), None)],
            ),
            self.marker(
                spec("dependencyEnd-margin", 16.0, 20.0, 28.0),
                vec![path(end_shape(), no_stroke, None)],
            ),
        ]
    }

    fn lollipop(&self) -> Vec<Marker> {
        let class = self.class("lollipop");
        let spec = |name, ref_x| Spec {
            name,
            class: class.clone(),
            view_box: None,
            ref_x,
            ref_y: 7.0,
            width: 190.0,
            height: 240.0,
            units: MarkerUnits::UserSpaceOnUse,
        };
        // `.attr('fill', 'transparent')`.
        let transparent = Style {
            fill: Some(Paint::None),
            ..Style::default()
        };
        let transparent_margin = Style {
            fill: Some(Paint::None),
            stroke_width: Some(2.0),
            ..Style::default()
        };
        vec![
            self.marker(
                spec("lollipopStart", 13.0),
                vec![circle(7.0, 7.0, 6.0, transparent.clone(), None)],
            ),
            self.marker(
                spec("lollipopEnd", 1.0),
                vec![circle(7.0, 7.0, 6.0, transparent, None)],
            ),
            self.marker(
                spec("lollipopStart-margin", 13.0),
                vec![circle(7.0, 7.0, 6.0, transparent_margin.clone(), None)],
            ),
            self.marker(
                spec("lollipopEnd-margin", 1.0),
                vec![circle(7.0, 7.0, 6.0, transparent_margin, None)],
            ),
        ]
    }

    fn point(&self) -> Vec<Marker> {
        let class = self.class("");
        let arrow = Some("arrowMarkerPath");
        let vb10 = Some(ViewBox::new(0.0, 0.0, 10.0, 10.0));
        let vb_margin = Some(ViewBox::new(0.0, 0.0, 11.5, 14.0));
        let spec = |name, view_box, ref_x, ref_y, w, h| Spec {
            name,
            class: class.clone(),
            view_box,
            ref_x,
            ref_y,
            width: w,
            height: h,
            units: MarkerUnits::UserSpaceOnUse,
        };
        vec![
            self.marker(
                spec("pointEnd", vb10, 5.0, 5.0, 8.0, 8.0),
                vec![path(
                    closed(&[(0.0, 0.0), (10.0, 5.0), (0.0, 10.0)]),
                    stroke_style(1.0, solid()),
                    arrow,
                )],
            ),
            self.marker(
                spec("pointStart", vb10, 4.5, 5.0, 8.0, 8.0),
                vec![path(
                    closed(&[(0.0, 5.0), (10.0, 10.0), (10.0, 0.0)]),
                    stroke_style(1.0, solid()),
                    arrow,
                )],
            ),
            self.marker(
                spec("pointEnd-margin", vb_margin, 11.5, 7.0, 10.5, 14.0),
                vec![path(
                    closed(&[(0.0, 0.0), (11.5, 7.0), (0.0, 14.0)]),
                    stroke_style(0.0, solid()),
                    arrow,
                )],
            ),
            self.marker(
                spec("pointStart-margin", vb_margin, 1.0, 7.0, 11.5, 14.0),
                vec![polygon(
                    &[(0.0, 7.0), (11.5, 14.0), (11.5, 0.0)],
                    stroke_style(0.0, solid()),
                    arrow,
                )],
            ),
        ]
    }

    fn circle(&self) -> Vec<Marker> {
        let class = self.class("");
        let arrow = Some("arrowMarkerPath");
        let vb = Some(ViewBox::new(0.0, 0.0, 10.0, 10.0));
        let spec = |name, ref_x, size| Spec {
            name,
            class: class.clone(),
            view_box: vb,
            ref_x,
            ref_y: 5.0,
            width: size,
            height: size,
            units: MarkerUnits::UserSpaceOnUse,
        };
        vec![
            self.marker(
                spec("circleEnd", 11.0, 11.0),
                vec![circle(5.0, 5.0, 5.0, stroke_style(1.0, solid()), arrow)],
            ),
            self.marker(
                spec("circleStart", -1.0, 11.0),
                vec![circle(5.0, 5.0, 5.0, stroke_style(1.0, solid()), arrow)],
            ),
            self.marker(
                spec("circleEnd-margin", 12.25, 14.0),
                vec![circle(5.0, 5.0, 5.0, stroke_style(0.0, solid()), arrow)],
            ),
            self.marker(
                spec("circleStart-margin", -2.0, 14.0),
                vec![circle(5.0, 5.0, 5.0, stroke_style(0.0, solid()), arrow)],
            ),
        ]
    }

    fn cross(&self) -> Vec<Marker> {
        let class = self.class("cross");
        let arrow = Some("arrowMarkerPath");
        let spec = |name, view_box, ref_x, ref_y, size| Spec {
            name,
            class: class.clone(),
            view_box: Some(view_box),
            ref_x,
            ref_y,
            width: size,
            height: size,
            units: MarkerUnits::UserSpaceOnUse,
        };
        let vb11 = ViewBox::new(0.0, 0.0, 11.0, 11.0);
        let vb15 = ViewBox::new(0.0, 0.0, 15.0, 15.0);
        // `M 1,1 l 9,9 M 10,1 l -9,9` in absolute terms.
        let small = || two_lines((1.0, 1.0), (10.0, 10.0), (10.0, 1.0), (1.0, 10.0));
        let large = || two_lines((1.0, 1.0), (14.0, 14.0), (1.0, 14.0), (14.0, 1.0));
        vec![
            self.marker(
                spec("crossEnd", vb11, 12.0, 5.2, 11.0),
                vec![path(small(), stroke_style(2.0, solid()), arrow)],
            ),
            self.marker(
                spec("crossStart", vb11, -1.0, 5.2, 11.0),
                vec![path(small(), stroke_style(2.0, solid()), arrow)],
            ),
            self.marker(
                spec("crossEnd-margin", vb15, 17.7, 7.5, 12.0),
                vec![path(large(), stroke_style(2.5, None), arrow)],
            ),
            self.marker(
                spec("crossStart-margin", vb15, -3.5, 7.5, 12.0),
                vec![path(large(), stroke_style(2.5, solid()), arrow)],
            ),
        ]
    }

    fn barb(&self) -> Vec<Marker> {
        vec![self.marker(
            Spec {
                name: "barbEnd",
                class: None,
                view_box: None,
                ref_x: 19.0,
                ref_y: 7.0,
                width: 20.0,
                height: 14.0,
                units: MarkerUnits::UserSpaceOnUse,
            },
            vec![path(
                closed(&[(19.0, 7.0), (9.0, 13.0), (14.0, 7.0), (9.0, 1.0)]),
                Style::default(),
                None,
            )],
        )]
    }

    fn hierarchy(&self) -> Vec<Marker> {
        let class = self.class("hierarchy");
        let arrow = Some("arrowMarkerPath");
        let vb = Some(ViewBox::new(0.0, 0.0, 12.0, 10.0));
        let spec = |name, ref_x| Spec {
            name,
            class: class.clone(),
            view_box: vb,
            ref_x,
            ref_y: 5.0,
            width: 12.0,
            height: 10.0,
            units: MarkerUnits::UserSpaceOnUse,
        };
        let style = || Style {
            fill: Some(Paint::None),
            stroke_width: Some(1.0),
            stroke_dasharray: solid(),
            ..Style::default()
        };
        let chevrons = |a: [(f64, f64); 3], b: [(f64, f64); 3]| {
            let mut d = PathData::new();
            d.move_to(a[0].0, a[0].1)
                .line_to(a[1].0, a[1].1)
                .line_to(a[2].0, a[2].1)
                .move_to(b[0].0, b[0].1)
                .line_to(b[1].0, b[1].1)
                .line_to(b[2].0, b[2].1);
            d
        };
        vec![
            self.marker(
                spec("hierarchyEnd", 10.0),
                vec![path(
                    chevrons(
                        [(0.0, 0.0), (6.0, 5.0), (0.0, 10.0)],
                        [(4.0, 0.0), (10.0, 5.0), (4.0, 10.0)],
                    ),
                    style(),
                    arrow,
                )],
            ),
            self.marker(
                spec("hierarchyStart", 2.0),
                vec![path(
                    chevrons(
                        [(12.0, 0.0), (6.0, 5.0), (12.0, 10.0)],
                        [(8.0, 0.0), (2.0, 5.0), (8.0, 10.0)],
                    ),
                    style(),
                    arrow,
                )],
            ),
        ]
    }

    /// The ER markers set no `markerUnits`, so the SVG default
    /// `strokeWidth` applies.
    fn er_spec<'s>(
        &self,
        name: &'s str,
        kind: &str,
        ref_x: f64,
        ref_y: f64,
        w: f64,
        h: f64,
    ) -> Spec<'s> {
        Spec {
            name,
            class: self.class(kind),
            view_box: None,
            ref_x,
            ref_y,
            width: w,
            height: h,
            units: MarkerUnits::StrokeWidth,
        }
    }

    fn only_one(&self) -> Vec<Marker> {
        vec![
            self.marker(
                self.er_spec("onlyOneStart", "onlyOne", 0.0, 9.0, 18.0, 18.0),
                vec![path(
                    two_lines((9.0, 0.0), (9.0, 18.0), (15.0, 0.0), (15.0, 18.0)),
                    Style::default(),
                    None,
                )],
            ),
            self.marker(
                self.er_spec("onlyOneEnd", "onlyOne", 18.0, 9.0, 18.0, 18.0),
                vec![path(
                    two_lines((3.0, 0.0), (3.0, 18.0), (9.0, 0.0), (9.0, 18.0)),
                    Style::default(),
                    None,
                )],
            ),
        ]
    }

    fn zero_or_one(&self) -> Vec<Marker> {
        let white = Style::fill(Color::WHITE);
        let bar = |x: f64| {
            let mut d = PathData::new();
            d.move_to(x, 0.0).line_to(x, 18.0);
            d
        };
        vec![
            self.marker(
                self.er_spec("zeroOrOneStart", "zeroOrOne", 0.0, 9.0, 30.0, 18.0),
                vec![
                    circle(21.0, 9.0, 6.0, white.clone(), None),
                    path(bar(9.0), Style::default(), None),
                ],
            ),
            self.marker(
                self.er_spec("zeroOrOneEnd", "zeroOrOne", 30.0, 9.0, 30.0, 18.0),
                vec![
                    circle(9.0, 9.0, 6.0, white, None),
                    path(bar(21.0), Style::default(), None),
                ],
            ),
        ]
    }

    /// The crow's foot `M x,18 Q x+18,0 x+36,18 Q x+18,36 x,18`.
    fn crow(x: f64) -> PathData {
        let mut d = PathData::new();
        d.move_to(x, 18.0)
            .quad_to(x + 18.0, 0.0, x + 36.0, 18.0)
            .quad_to(x + 18.0, 36.0, x, 18.0);
        d
    }

    fn one_or_more(&self) -> Vec<Marker> {
        let mut start = Self::crow(0.0);
        start.move_to(42.0, 9.0).line_to(42.0, 27.0);
        let mut end = PathData::new();
        end.move_to(3.0, 9.0).line_to(3.0, 27.0);
        end.move_to(9.0, 18.0)
            .quad_to(27.0, 0.0, 45.0, 18.0)
            .quad_to(27.0, 36.0, 9.0, 18.0);
        vec![
            self.marker(
                self.er_spec("oneOrMoreStart", "oneOrMore", 18.0, 18.0, 45.0, 36.0),
                vec![path(start, Style::default(), None)],
            ),
            self.marker(
                self.er_spec("oneOrMoreEnd", "oneOrMore", 27.0, 18.0, 45.0, 36.0),
                vec![path(end, Style::default(), None)],
            ),
        ]
    }

    fn zero_or_more(&self) -> Vec<Marker> {
        let white = Style::fill(Color::WHITE);
        vec![
            self.marker(
                self.er_spec("zeroOrMoreStart", "zeroOrMore", 18.0, 18.0, 57.0, 36.0),
                vec![
                    circle(48.0, 18.0, 6.0, white.clone(), None),
                    path(Self::crow(0.0), Style::default(), None),
                ],
            ),
            self.marker(
                self.er_spec("zeroOrMoreEnd", "zeroOrMore", 39.0, 18.0, 57.0, 36.0),
                vec![
                    circle(9.0, 18.0, 6.0, white, None),
                    path(Self::crow(21.0), Style::default(), None),
                ],
            ),
        ]
    }

    fn requirement_arrow(&self) -> Vec<Marker> {
        let mut d = PathData::new();
        d.move_to(0.0, 0.0)
            .line_to(20.0, 10.0)
            .move_to(20.0, 10.0)
            .line_to(0.0, 20.0);
        vec![self.marker(
            Spec {
                name: "requirement_arrowEnd",
                class: None,
                view_box: None,
                ref_x: 20.0,
                ref_y: 10.0,
                width: 20.0,
                height: 20.0,
                units: MarkerUnits::StrokeWidth,
            },
            vec![path(d, Style::default(), None)],
        )]
    }

    fn requirement_contains(&self) -> Vec<Marker> {
        let line = |x1, y1, x2, y2| {
            Element::Line(Line {
                common: Common::default(),
                x1,
                y1,
                x2,
                y2,
            })
        };
        let group = Element::Group(Group {
            common: Common::default(),
            children: vec![
                circle(
                    10.0,
                    10.0,
                    9.0,
                    Style {
                        fill: Some(Paint::None),
                        ..Style::default()
                    },
                    None,
                ),
                line(1.0, 10.0, 19.0, 10.0),
                line(10.0, 1.0, 10.0, 19.0),
            ],
        });
        vec![self.marker(
            Spec {
                name: "requirement_containsStart",
                class: None,
                view_box: None,
                ref_x: 0.0,
                ref_y: 10.0,
                width: 20.0,
                height: 20.0,
                units: MarkerUnits::StrokeWidth,
            },
            vec![group],
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_follow_mermaid() {
        let m = markers_for(MarkerType::Point, "flowchart-v2", "my-svg");
        let ids: Vec<&str> = m.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "my-svg_flowchart-v2-pointEnd",
                "my-svg_flowchart-v2-pointStart",
                "my-svg_flowchart-v2-pointEnd-margin",
                "my-svg_flowchart-v2-pointStart-margin",
            ]
        );
    }

    #[test]
    fn paint_fills_only_unset_fields() {
        let mut m = markers_for(MarkerType::Lollipop, "classDiagram", "x").remove(0);
        let paint = Style {
            fill: Some(Paint::Color(Color::BLACK)),
            stroke: Some(Paint::Color(Color::BLACK)),
            ..Style::default()
        };
        m.apply_paint(&paint);
        let Element::Circle(c) = &m.children[0] else {
            panic!("lollipop is a circle");
        };
        assert_eq!(c.common.style.fill, Some(Paint::None));
        assert_eq!(c.common.style.stroke, Some(Paint::Color(Color::BLACK)));
    }
}
