//! `draw` of `architectureRenderer.ts` with `drawEdges`, `drawGroups`,
//! `drawServices`, and `drawJunctions` of `svgDraw.ts` (mermaid 12.2.0).
//!
//! The SVG holds three groups in this order: the edges, the services and
//! junctions, and the groups, so a group's dashed box lies over its
//! services. A service is drawn with its 80 px icon's top-left corner at
//! the cytoscape position and its title centred below the icon; a group
//! box is the cytoscape compound box moved by half an icon to match. The
//! diagram title is set in the db but `draw` never renders it.
//! `setupGraphViewbox` frames the `getBBox()` of everything with the
//! configured padding.

use crate::error::HelperError;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::scene::{
    Baseline, Color, Common, Element, FontSpec, FontStyle, FontWeight, Group, Length, Paint, Path,
    PathData, Point, Polygon, Rect, Scene, Style, Text, TextAnchor, TextRun, TransformOp, ViewBox,
};
use crate::mermaid::shapes::measure::{LabelBox, LabelLine, LabelOpts, measure_with_metrics};
use crate::mermaid::styles::architecture::{ArchitectureStyles, ROOT_FONT_SIZE};
use crate::text::fonts::Weight;
use crate::text::measure::FontSpec as MeasureSpec;

use super::db::{self, ArchitectureDb, Dir, NodeType};
use super::icons::{self, Icon};
use super::layout::{self, ArchConfig, CYTOSCAPE_FONT_FAMILY, Cy, CyKind};

/// A box in SVG user units, for `getBBox()`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Bounds {
    fn rect(x: f64, y: f64, w: f64, h: f64) -> Bounds {
        Bounds {
            min_x: x,
            min_y: y,
            max_x: x + w,
            max_y: y + h,
        }
    }

    fn union(self, other: Bounds) -> Bounds {
        Bounds {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }

    fn translated(self, dx: f64, dy: f64) -> Bounds {
        Bounds {
            min_x: self.min_x + dx,
            min_y: self.min_y + dy,
            max_x: self.max_x + dx,
            max_y: self.max_y + dy,
        }
    }

    fn of_points(points: &[(f64, f64)]) -> Option<Bounds> {
        let first = points.first()?;
        let mut b = Bounds::rect(first.0, first.1, 0.0, 0.0);
        for p in &points[1..] {
            b = b.union(Bounds::rect(p.0, p.1, 0.0, 0.0));
        }
        Some(b)
    }

    /// The four corners after a rotation by `degrees` about the origin.
    fn rotated(self, degrees: f64) -> Bounds {
        let (s, c) = degrees.to_radians().sin_cos();
        let corners = [
            (self.min_x, self.min_y),
            (self.max_x, self.min_y),
            (self.min_x, self.max_y),
            (self.max_x, self.max_y),
        ]
        .map(|(x, y)| (x * c - y * s, x * s + y * c));
        Bounds::of_points(&corners).unwrap_or(self)
    }

    fn width(&self) -> f64 {
        self.max_x - self.min_x
    }

    fn height(&self) -> f64 {
        self.max_y - self.min_y
    }
}

/// The resolved icon of every service and group, checked before drawing.
fn icon_of(reference: &Option<String>) -> Result<Option<Icon>, HelperError> {
    reference.as_deref().map(icons::resolve).transpose()
}

struct Painter<'a, 'f> {
    ctx: &'a RenderCtx<'f>,
    styles: &'a ArchitectureStyles,
    cfg: &'a ArchConfig,
    bounds: Option<Bounds>,
}

impl Painter<'_, '_> {
    fn extend(&mut self, b: Bounds) {
        if !(b.min_x.is_finite()
            && b.min_y.is_finite()
            && b.max_x.is_finite()
            && b.max_y.is_finite())
        {
            return;
        }
        self.bounds = Some(match self.bounds {
            Some(old) => old.union(b),
            None => b,
        });
    }

    /// cytoscape's label width: `Math.ceil(measureText(text).width)` in
    /// its default font at the configured size.
    fn cytoscape_label_width(&self, text: &str) -> f64 {
        let spec = MeasureSpec::new(CYTOSCAPE_FONT_FAMILY, self.cfg.font_size as f32);
        f64::from(self.ctx.metrics.shape(text, &spec).width).ceil()
    }

    fn text_spec(&self) -> MeasureSpec {
        MeasureSpec::new(self.styles.font_family.clone(), ROOT_FONT_SIZE as f32)
    }

    /// Half the x-height: how far `dominant-baseline: middle` moves the
    /// alphabetic baseline down.
    fn middle_shift(&self) -> f64 {
        let spec = self.text_spec();
        let fonts = self.ctx.fonts;
        let Some(face) = fonts.resolve(&spec.family, Weight(spec.weight), spec.style.into()) else {
            return 0.0;
        };
        fonts
            .with_face(face, |f| {
                let upem = f64::from(f.units_per_em());
                let units = f.x_height().filter(|v| *v > 0).map(f64::from).or_else(|| {
                    f.glyph_index('x')
                        .and_then(|g| f.glyph_bounding_box(g))
                        .map(|r| f64::from(r.y_max))
                });
                units.map_or(0.0, |u| u / upem * f64::from(spec.size)) / 2.0
            })
            .unwrap_or(0.0)
    }

    /// `createText(el, text, { useHtmlLabels: false, width })`: the
    /// `createFormattedText` tspans, one per wrapped line, and the
    /// `getBBox()` of the `<text>` relative to the label group.
    fn svg_label(
        &self,
        text: &str,
        width: f64,
        anchor: TextAnchor,
        middle: bool,
    ) -> Option<(Element, Bounds, LabelBox)> {
        let opts = LabelOpts {
            font: FontSpec {
                size: Some(ROOT_FONT_SIZE),
                ..FontSpec::default()
            },
            html: false,
            width: Some(width),
            min_width: 0.0,
            markdown: true,
        };
        let label = measure_with_metrics(&self.ctx.metrics, text, &opts, &self.styles.font_family);
        if label.is_empty() {
            return None;
        }
        let mut runs = Vec::new();
        for (i, line) in label.lines.iter().enumerate() {
            for (j, run) in line.runs.iter().enumerate() {
                let (content, x, dy) = if j == 0 {
                    (
                        run.text.clone(),
                        Some(0.0),
                        Some(Length::Em(if i == 0 { 1.0 } else { 1.1 })),
                    )
                } else {
                    (format!(" {}", run.text), None, None)
                };
                runs.push(TextRun {
                    text: content,
                    x,
                    dy,
                    font: FontSpec {
                        weight: run.bold.then_some(FontWeight::Bold),
                        style: run.italic.then_some(FontStyle::Italic),
                        ..FontSpec::default()
                    },
                    fill: None,
                });
            }
        }
        let shift = if middle { self.middle_shift() } else { 0.0 };
        let x = match anchor {
            TextAnchor::Middle => -label.width / 2.0,
            TextAnchor::End => -label.width,
            TextAnchor::Start => 0.0,
        };
        let bounds = Bounds::rect(x, label.y + shift, label.width, label.height);
        let element = Element::Text(Text {
            common: Common::default(),
            x: 0.0,
            y: 0.0,
            anchor: Some(anchor),
            baseline: Some(if middle {
                Baseline::Middle
            } else {
                Baseline::Auto
            }),
            font: FontSpec::default(),
            runs,
        });
        Some((element, bounds, label))
    }

    fn icon(&mut self, icon: Icon, size: f64, at: (f64, f64)) -> Element {
        self.extend(Bounds::rect(at.0, at.1, size, size));
        icons::elements(icon, size)
    }
}

fn group(class: &str, transform: Vec<TransformOp>, children: Vec<Element>) -> Element {
    Element::Group(Group {
        common: Common {
            class: (!class.is_empty()).then(|| class.to_string()),
            transform,
            ..Common::default()
        },
        children,
    })
}

/// `ArchitectureDirectionArrow[dir](scale)`.
fn arrow_points(dir: Dir, s: f64) -> Vec<Point> {
    let p = |x: f64, y: f64| Point::new(x, y);
    match dir {
        'L' => vec![p(s, s / 2.0), p(0.0, s), p(0.0, 0.0)],
        'R' => vec![p(0.0, s / 2.0), p(s, 0.0), p(s, s)],
        'T' => vec![p(0.0, 0.0), p(s, 0.0), p(s / 2.0, s)],
        _ => vec![p(s / 2.0, 0.0), p(s, s), p(0.0, s)],
    }
}

/// `ArchitectureDirectionArrowShift[dir](orig, arrowSize)`.
fn arrow_shift(dir: Dir, orig: f64, arrow_size: f64) -> f64 {
    match dir {
        'L' | 'T' => orig - arrow_size + 2.0,
        _ => orig - 2.0,
    }
}

/// `draw(text, id, version, diagObj)`.
pub fn draw(
    db: &ArchitectureDb,
    cfg: &ArchConfig,
    styles: &ArchitectureStyles,
    ctx: &RenderCtx<'_>,
) -> Result<Scene, HelperError> {
    // Resolve every icon first: an icon pack fails the whole diagram.
    let mut service_icons = Vec::new();
    for n in db.nodes.values().filter(|n| n.kind == NodeType::Service) {
        service_icons.push(icon_of(&n.icon)?);
    }
    let mut group_icons = Vec::new();
    for g in db.groups.values() {
        group_icons.push(icon_of(&g.icon)?);
    }

    let mut painter = Painter {
        ctx,
        styles,
        cfg,
        bounds: None,
    };
    let measure = |t: &str| painter.cytoscape_label_width(t);
    let cy = layout::layout(db, cfg, &measure)?;

    let edges = draw_edges(&mut painter, db, &cy);
    let mut services = draw_services(&mut painter, db, &cy, &service_icons);
    services.extend(draw_junctions(&mut painter, db, &cy));
    let groups = draw_groups(&mut painter, db, &cy, &group_icons);

    // `setupGraphViewbox(undefined, svg, padding, useMaxWidth)`.
    let padding = cfg.padding;
    let bounds = painter.bounds.unwrap_or(Bounds::rect(0.0, 0.0, 0.0, 0.0));
    let width = bounds.width() + 2.0 * padding;
    let height = bounds.height() + 2.0 * padding;
    let mut scene = Scene::new(width, height);
    scene.view_box = ViewBox::new(
        bounds.min_x - padding,
        bounds.min_y - padding,
        width,
        height,
    );
    scene.font_family = styles.font_family.clone();
    scene.root = Group {
        common: Common::with_style(Style {
            fill: Some(Paint::Color(styles.text_color)),
            font: FontSpec {
                family: Some(styles.font_family.clone()),
                size: Some(ROOT_FONT_SIZE),
                ..FontSpec::default()
            },
            ..Style::default()
        }),
        children: vec![
            group("architecture-edges", Vec::new(), edges),
            group("architecture-services", Vec::new(), services),
            group("architecture-groups", Vec::new(), groups),
        ],
    };
    Ok(scene)
}

/// `drawEdges`.
fn draw_edges(p: &mut Painter<'_, '_>, db: &ArchitectureDb, cy: &Cy) -> Vec<Element> {
    let padding = p.cfg.padding;
    let icon_size = p.cfg.icon_size;
    let half_icon = icon_size / 2.0;
    let arrow_size = icon_size / 6.0;
    let half_arrow = arrow_size / 2.0;
    let mut out = Vec::new();
    for (edge, cy_edge) in db.edges.iter().zip(&cy.edges) {
        let points = cy.edge_points(cy_edge);
        let (mut start_x, mut start_y) = points.start;
        let (mid_x, mid_y) = points.mid;
        let (mut end_x, mut end_y) = points.end;
        let shift = padding + 4.0;
        let (sd, td) = (edge.lhs_dir, edge.rhs_dir);
        if edge.lhs_group {
            if db::is_x(sd) {
                start_x += if sd == 'L' { -shift } else { shift };
            } else {
                start_y += if sd == 'T' { -shift } else { shift + 18.0 };
            }
        }
        if edge.rhs_group {
            if db::is_x(td) {
                end_x += if td == 'L' { -shift } else { shift };
            } else {
                end_y += if td == 'T' { -shift } else { shift + 18.0 };
            }
        }
        let is_junction = |id: &str| {
            db.nodes
                .get(id)
                .is_some_and(|n| n.kind == NodeType::Junction)
        };
        if !edge.lhs_group && is_junction(&edge.lhs_id) {
            if db::is_x(sd) {
                start_x += if sd == 'L' { half_icon } else { -half_icon };
            } else {
                start_y += if sd == 'T' { half_icon } else { -half_icon };
            }
        }
        if !edge.rhs_group && is_junction(&edge.rhs_id) {
            if db::is_x(td) {
                end_x += if td == 'L' { half_icon } else { -half_icon };
            } else {
                end_y += if td == 'T' { half_icon } else { -half_icon };
            }
        }

        let mut children = Vec::new();
        let mut d = PathData::new();
        d.move_to(start_x, start_y)
            .line_to(mid_x, mid_y)
            .line_to(end_x, end_y);
        children.push(Element::Path(Path {
            common: Common {
                class: Some("edge".to_string()),
                style: Style {
                    fill: Some(Paint::None),
                    stroke: Some(p.styles.edge_color.clone()),
                    stroke_width: Some(p.styles.edge_width),
                    ..Style::default()
                },
                ..Common::default()
            },
            d,
        }));
        if let Some(b) = Bounds::of_points(&[(start_x, start_y), (mid_x, mid_y), (end_x, end_y)]) {
            p.extend(b);
        }
        let mut arrow = |p: &mut Painter<'_, '_>, dir: Dir, x: f64, y: f64| {
            let x_shift = if db::is_x(dir) {
                arrow_shift(dir, x, arrow_size)
            } else {
                x - half_arrow
            };
            let y_shift = if db::is_y(dir) {
                arrow_shift(dir, y, arrow_size)
            } else {
                y - half_arrow
            };
            let points = arrow_points(dir, arrow_size);
            let pts: Vec<(f64, f64)> = points.iter().map(|q| (q.x, q.y)).collect();
            if let Some(b) = Bounds::of_points(&pts) {
                p.extend(b.translated(x_shift, y_shift));
            }
            children.push(Element::Polygon(Polygon {
                common: Common {
                    class: Some("arrow".to_string()),
                    transform: vec![TransformOp::Translate(x_shift, y_shift)],
                    style: Style {
                        fill: Some(p.styles.arrow_color.clone()),
                        ..Style::default()
                    },
                    ..Common::default()
                },
                points,
            }));
        };
        if edge.lhs_into {
            arrow(p, sd, start_x, start_y);
        }
        if edge.rhs_into {
            arrow(p, td, end_x, end_y);
        }
        if let Some(title) = &edge.title {
            let xy = db::is_xy(sd, td);
            let axis_x = !xy && db::is_x(sd);
            let width = if xy {
                (start_x - end_x).abs() / 2.0
            } else if axis_x {
                (start_x - end_x).abs()
            } else {
                (start_y - end_y).abs() / 1.5
            };
            if xy {
                if let Some((_, orig, _)) = p.svg_label(title, width, TextAnchor::Middle, true)
                    && let Some(pair) = db::direction_pair(sd, td)
                {
                    // `getBoundingClientRect` before and after the 45 degree
                    // rotation with `dominant-baseline: auto`.
                    let (x, y) = db::xy_factors(pair);
                    let angle = -x * y * 45.0;
                    let Some((auto_text, auto_box, _)) =
                        p.svg_label(title, width, TextAnchor::Middle, false)
                    else {
                        continue;
                    };
                    let rotated = auto_box.rotated(angle);
                    let tx = mid_x + x * rotated.width() / 2.0;
                    let ty = mid_y - orig.height() / 2.0 + y * rotated.height() / 2.0;
                    let pivot = orig.height() / 2.0;
                    let transform = vec![
                        TransformOp::Translate(tx, ty),
                        TransformOp::Rotate(angle, 0.0, pivot),
                    ];
                    // The final box: the auto-baseline text rotated about
                    // the pivot, then moved.
                    let placed = auto_box
                        .translated(0.0, -pivot)
                        .rotated(angle)
                        .translated(tx, ty + pivot);
                    p.extend(placed);
                    children.push(group("", transform, vec![auto_text]));
                }
            } else if let Some((text, b, _)) = p.svg_label(title, width, TextAnchor::Middle, true) {
                let transform = if axis_x {
                    p.extend(b.translated(mid_x, mid_y));
                    vec![TransformOp::Translate(mid_x, mid_y)]
                } else {
                    p.extend(b.rotated(-90.0).translated(mid_x, mid_y));
                    vec![
                        TransformOp::Translate(mid_x, mid_y),
                        TransformOp::Rotate(-90.0, 0.0, 0.0),
                    ]
                };
                children.push(group("", transform, vec![text]));
            }
        }
        out.push(group("", Vec::new(), children));
    }
    out
}

/// `drawServices`, placed by `positionNodes`.
fn draw_services(
    p: &mut Painter<'_, '_>,
    db: &ArchitectureDb,
    cy: &Cy,
    service_icons: &[Option<Icon>],
) -> Vec<Element> {
    let icon_size = p.cfg.icon_size;
    let mut out = Vec::new();
    let services = db.nodes.values().filter(|n| n.kind == NodeType::Service);
    for (service, icon) in services.zip(service_icons) {
        let Some(&index) = cy.index.get(&service.id) else {
            continue;
        };
        let (x, y) = cy.nodes[index].pos;
        let mut children = Vec::new();
        if let Some(title) = &service.title
            && let Some((text, b, _)) =
                p.svg_label(title, icon_size * 1.5, TextAnchor::Middle, true)
        {
            p.extend(b.translated(x + icon_size / 2.0, y + icon_size));
            children.push(group(
                "",
                vec![TransformOp::Translate(icon_size / 2.0, icon_size)],
                vec![text],
            ));
        }
        let background = match (icon, &service.icon_text) {
            (Some(icon), _) => p.icon(*icon, icon_size, (x, y)),
            (None, Some(text)) => icon_text(p, text, (x, y)),
            (None, None) => {
                p.extend(Bounds::rect(x, y, icon_size, icon_size));
                let mut d = PathData::new();
                d.move_to(0.0, icon_size)
                    .line_to(0.0, 5.0)
                    .quad_to(0.0, 0.0, 5.0, 0.0)
                    .line_to(icon_size - 5.0, 0.0)
                    .quad_to(icon_size, 0.0, icon_size, 5.0)
                    .line_to(icon_size, icon_size)
                    .close();
                Element::Path(Path {
                    common: Common {
                        class: Some("node-bkg".to_string()),
                        style: node_bkg_style(p.styles),
                        ..Common::default()
                    },
                    d,
                })
            }
        };
        children.push(group("", Vec::new(), vec![background]));
        out.push(group(
            "architecture-service",
            vec![TransformOp::Translate(x, y)],
            children,
        ));
    }
    out
}

/// A service with `iconText`: the blank icon and the text in a
/// `foreignObject` div (`.node-icon-text`: white, centred, clamped to
/// `floor((iconSize - 2) / 16)` lines), drawn as SVG text centred in the
/// icon.
fn icon_text(p: &mut Painter<'_, '_>, text: &str, at: (f64, f64)) -> Element {
    let icon_size = p.cfg.icon_size;
    let blank = p.icon(Icon::Blank, icon_size, at);
    let mut children = vec![blank];
    let opts = LabelOpts {
        font: FontSpec {
            size: Some(ROOT_FONT_SIZE),
            ..FontSpec::default()
        },
        html: true,
        width: Some(icon_size - 2.0),
        min_width: 0.0,
        markdown: false,
    };
    let label = measure_with_metrics(&p.ctx.metrics, text, &opts, &p.styles.font_family);
    let clamp = ((icon_size - 2.0) / ROOT_FONT_SIZE).floor().max(1.0) as usize;
    let lines: Vec<&LabelLine> = label.lines.iter().take(clamp).collect();
    if !lines.is_empty() {
        let spec = MeasureSpec::new(p.styles.font_family.clone(), ROOT_FONT_SIZE as f32);
        let shaped = p.ctx.metrics.shape("x", &spec);
        let line_height = f64::from(shaped.ascent + shaped.descent);
        let total = line_height * lines.len() as f64;
        let top = (icon_size - total) / 2.0;
        let runs = lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let content: String = line
                    .runs
                    .iter()
                    .map(|r| r.text.as_str())
                    .collect::<Vec<_>>()
                    .join("");
                TextRun {
                    text: content,
                    x: Some(icon_size / 2.0),
                    dy: Some(Length::Px(if i == 0 {
                        top + f64::from(shaped.ascent)
                    } else {
                        line_height
                    })),
                    ..TextRun::default()
                }
            })
            .collect();
        children.push(Element::Text(Text {
            common: Common::with_style(Style::fill(Color::WHITE)),
            x: icon_size / 2.0,
            y: 0.0,
            anchor: Some(TextAnchor::Middle),
            baseline: Some(Baseline::Alphabetic),
            font: FontSpec::default(),
            runs,
        }));
    }
    group("", Vec::new(), children)
}

fn node_bkg_style(styles: &ArchitectureStyles) -> Style {
    Style {
        fill: Some(Paint::None),
        stroke: Some(styles.group_border_color.clone()),
        stroke_width: Some(styles.group_border_width),
        stroke_dasharray: Some(vec![8.0]),
        ..Style::default()
    }
}

/// `drawJunctions`: an invisible square the edges meet in.
fn draw_junctions(p: &mut Painter<'_, '_>, db: &ArchitectureDb, cy: &Cy) -> Vec<Element> {
    let icon_size = p.cfg.icon_size;
    let mut out = Vec::new();
    for junction in db.nodes.values().filter(|n| n.kind == NodeType::Junction) {
        let Some(&index) = cy.index.get(&junction.id) else {
            continue;
        };
        let (x, y) = cy.nodes[index].pos;
        p.extend(Bounds::rect(x, y, icon_size, icon_size));
        let rect = Element::Rect(Rect {
            common: Common::with_style(Style {
                fill_opacity: Some(0.0),
                ..Style::default()
            }),
            x: 0.0,
            y: 0.0,
            width: icon_size,
            height: icon_size,
            rx: None,
            ry: None,
        });
        out.push(group(
            "architecture-junction",
            vec![TransformOp::Translate(x, y)],
            vec![group("", Vec::new(), vec![rect])],
        ));
    }
    out
}

/// `drawGroups`.
fn draw_groups(
    p: &mut Painter<'_, '_>,
    db: &ArchitectureDb,
    cy: &Cy,
    group_icons: &[Option<Icon>],
) -> Vec<Element> {
    let padding = p.cfg.padding;
    let group_icon_size = padding * 0.75;
    let font_size = p.cfg.font_size;
    let half_icon = p.cfg.icon_size / 2.0;
    let mut out = Vec::new();
    for (i, node) in cy.nodes.iter().enumerate() {
        if node.kind != CyKind::Group {
            continue;
        }
        let Some(g) = db.groups.get(&node.id) else {
            continue;
        };
        let icon = db
            .groups
            .get_index_of(&node.id)
            .and_then(|k| group_icons.get(k))
            .copied()
            .flatten();
        let bb = cy.node_bounding_box(i);
        let (w, h, x1, y1) = (bb.w(), bb.h(), bb.x1, bb.y1);
        p.extend(Bounds::rect(x1 + half_icon, y1 + half_icon, w, h));
        out.push(Element::Rect(Rect {
            common: Common {
                class: Some("node-bkg".to_string()),
                style: node_bkg_style(p.styles),
                ..Common::default()
            },
            x: x1 + half_icon,
            y: y1 + half_icon,
            width: w,
            height: h,
            rx: None,
            ry: None,
        }));
        let mut label_children = Vec::new();
        let mut shifted_x1 = x1;
        let mut shifted_y1 = y1;
        if let Some(icon) = icon {
            let at = (shifted_x1 + half_icon + 1.0, shifted_y1 + half_icon + 1.0);
            let el = p.icon(icon, group_icon_size, at);
            label_children.push(group(
                "",
                vec![TransformOp::Translate(at.0, at.1)],
                vec![el],
            ));
            shifted_x1 += group_icon_size;
            shifted_y1 += font_size / 2.0 - 1.0 - 2.0;
        }
        if let Some(title) = &g.title
            && let Some((text, b, _)) = p.svg_label(title, w, TextAnchor::Start, false)
        {
            let tx = shifted_x1 + half_icon + 4.0;
            let ty = shifted_y1 + half_icon + 2.0;
            p.extend(b.translated(tx, ty));
            label_children.push(group("", vec![TransformOp::Translate(tx, ty)], vec![text]));
        }
        out.push(group("", Vec::new(), label_children));
    }
    out
}
