//! Port of `diagrams/c4/c4Renderer.ts` with `svgDraw.ts` folded in
//! (mermaid 12.2.0).
//!
//! The layout is mermaid's grid: a [`Bounds`] per boundary level places
//! the shapes left to right, `c4ShapeInRow` to a row and wrapping at the
//! level's width limit; boundaries sit `c4BoundaryInRow` to a row, each
//! one drawn after its contents. The elements are drawn by
//! [`super::shapes`], the relationships last, as one straight line for the
//! first and quadratic curves for the rest.
//!
//! Text the renderer measures goes through `calculateTextWidth` and
//! `calculateTextHeight` ([`TextMetrics::calculate_text_dimensions`]).
//! The boundary and relationship texts are `byTspan` texts with
//! `dominant-baseline: middle` on the `text` and `alignment-baseline:
//! mathematical` on the `tspan`; the scene has no alignment baseline, so
//! the baseline is written where Blink puts it: half the font's ascent
//! below `y`.

use crate::error::HelperError;
use crate::mermaid::scene::{
    Common, Element, FontSpec, FontStyle, FontWeight, Line, Marker, MarkerUnits, Orient, Paint,
    Path, PathData, Point, Rect, Style, Text, TextAnchor, TextRun,
};
use crate::mermaid::shapes::intersect::{self, Bounds as RectBounds};
use crate::mermaid::styles::c4::{BOUNDARY_COLOR, C4Styles};
use crate::mermaid::styles::sequence::css_paint;
use crate::text::TextMetrics;
use crate::text::fonts::{Style as FaceStyle, Weight};
use crate::text::markdown::split_breaks;
use crate::text::wrap::wrap_label;

use super::conf::{C4Font, Conf};
use super::db::{C4Db, Field};
use super::shapes::{self, DrawnShape};

/// The id mermaid-cli gives the diagram; markers and shapes are named
/// after it.
pub const DIAGRAM_ID: &str = "my-svg";

/// `screen.availWidth` in the oracle's headless Chrome, the width limit of
/// the outermost level.
const SCREEN_AVAIL_WIDTH: f64 = 800.0;
/// Nested boundaries deeper than this are a boundary inside itself, which
/// recurses without end in mermaid.
const MAX_DEPTH: usize = 64;

/// `Bounds.data` and `Bounds.nextData`; `NaN` stands for `undefined`.
#[derive(Debug, Clone, Copy)]
struct Extent {
    startx: f64,
    stopx: f64,
    starty: f64,
    stopy: f64,
}

impl Extent {
    const UNSET: Extent = Extent {
        startx: f64::NAN,
        stopx: f64::NAN,
        starty: f64::NAN,
        stopy: f64::NAN,
    };

    /// `updateVal` for the four keys.
    fn grow(&mut self, startx: f64, starty: f64, stopx: f64, stopy: f64) {
        let update = |slot: &mut f64, val: f64, max: bool| {
            *slot = if slot.is_nan() {
                val
            } else if max {
                val.max(*slot)
            } else {
                val.min(*slot)
            };
        };
        update(&mut self.startx, startx, false);
        update(&mut self.starty, starty, false);
        update(&mut self.stopx, stopx, true);
        update(&mut self.stopy, stopy, true);
    }
}

/// The `Bounds` class of `c4Renderer.ts`.
#[derive(Debug, Clone)]
struct Bounds {
    data: Extent,
    width_limit: f64,
    next: Extent,
    cnt: i64,
}

impl Bounds {
    fn new() -> Bounds {
        Bounds {
            data: Extent::UNSET,
            width_limit: f64::NAN,
            next: Extent::UNSET,
            cnt: 0,
        }
    }

    /// `setData(startx, stopx, starty, stopy)`; the row count survives.
    fn set_data(&mut self, startx: f64, stopx: f64, starty: f64, stopy: f64) {
        let e = Extent {
            startx,
            stopx,
            starty,
            stopy,
        };
        self.data = e;
        self.next = e;
    }

    /// `insert(c4Shape)`: the top-left corner of the next shape.
    fn insert(
        &mut self,
        width: f64,
        height: f64,
        margin: f64,
        shape_in_row: i64,
        next_line_padding_x: f64,
    ) -> (f64, f64) {
        self.cnt += 1;
        let mut startx = if self.next.startx == self.next.stopx {
            self.next.stopx + margin
        } else {
            self.next.stopx + margin * 2.0
        };
        let mut stopx = startx + width;
        let mut starty = self.next.starty + margin * 2.0;
        let mut stopy = starty + height;
        if startx >= self.width_limit || stopx >= self.width_limit || self.cnt > shape_in_row {
            startx = self.next.startx + margin + next_line_padding_x;
            starty = self.next.stopy + margin * 2.0;
            stopx = startx + width;
            self.next.stopx = stopx;
            self.next.starty = self.next.stopy;
            stopy = starty + height;
            self.next.stopy = stopy;
            self.cnt = 1;
        }
        self.data.grow(startx, starty, stopx, stopy);
        self.next.grow(startx, starty, stopx, stopy);
        (startx, starty)
    }

    /// `bumpLastMargin(margin)`.
    fn bump_last_margin(&mut self, margin: f64) {
        self.data.stopx += margin;
        self.data.stopy += margin;
    }
}

/// A measured `C4Text`: the text after any wrapping, its size, and the `Y`
/// offset the boundary layout gives it.
#[derive(Debug, Clone, PartialEq)]
struct Measured {
    text: String,
    width: f64,
    height: f64,
    y: f64,
}

/// The boundary texts `drawInsideBoundary` measures and `drawBoundary`
/// draws.
#[derive(Debug, Clone, PartialEq)]
struct BoundaryTexts {
    label: Measured,
    kind: Option<Measured>,
    descr: Option<Measured>,
}

/// A relationship endpoint: a drawn shape or a drawn boundary.
enum Endpoint<'a> {
    Shape(&'a DrawnShape, f64, f64),
    Boundary(RectBounds),
}

impl Endpoint<'_> {
    fn center(&self) -> Point {
        match self {
            Endpoint::Shape(d, x, y) => Point::new(x + d.width / 2.0, y + d.height / 2.0),
            Endpoint::Boundary(b) => Point::new(b.x + b.width / 2.0, b.y + b.height / 2.0),
        }
    }

    fn intersect(&self, point: Point) -> Point {
        match self {
            Endpoint::Shape(d, ..) => {
                let c = self.center();
                d.intersect(c.x, c.y, point)
            }
            Endpoint::Boundary(b) => {
                let centered =
                    RectBounds::new(b.x + b.width / 2.0, b.y + b.height / 2.0, b.width, b.height);
                intersect::rect(&centered, point)
            }
        }
    }
}

/// What `draw` leaves behind for the diagram module to wrap in a scene.
#[derive(Debug, Clone, PartialEq)]
pub struct Drawn {
    /// `[x, y, width, height]` of the `viewBox` attribute.
    pub view_box: [f64; 4],
    /// The `width` and `height` `configureSvgSize` gives the svg.
    pub width: f64,
    pub height: f64,
    pub defs: Vec<Element>,
    pub children: Vec<Element>,
}

pub struct Renderer<'a> {
    conf: &'a Conf,
    styles: &'a C4Styles,
    metrics: &'a TextMetrics<'a>,
    db: &'a C4Db,
    /// The drawn element and its top-left corner, per shape index.
    shapes: Vec<Option<(DrawnShape, f64, f64)>>,
    /// The drawn box, per boundary index.
    boundaries: Vec<Option<RectBounds>>,
    children: Vec<Element>,
    global_max_x: f64,
    global_max_y: f64,
}

impl<'a> Renderer<'a> {
    pub fn new(
        conf: &'a Conf,
        styles: &'a C4Styles,
        metrics: &'a TextMetrics<'a>,
        db: &'a C4Db,
    ) -> Renderer<'a> {
        Renderer {
            conf,
            styles,
            metrics,
            db,
            shapes: vec![None; db.shapes.len()],
            boundaries: vec![None; db.boundaries.len()],
            children: Vec::new(),
            global_max_x: 0.0,
            global_max_y: 0.0,
        }
    }

    fn text_width(&self, text: &str, font: &C4Font) -> f64 {
        f64::from(self.metrics.calculate_text_width(text, &font.text_font()))
    }

    fn text_height(&self, text: &str, font: &C4Font) -> f64 {
        f64::from(self.metrics.calculate_text_height(text, &font.text_font()))
    }

    /// `calcC4ShapeTextWH(textType, c4Shape, wrap, textConf,
    /// textLimitWidth)`.
    fn measure(&self, text: &str, wrap: bool, font: &C4Font, limit: f64) -> Measured {
        if wrap {
            let wrapped = wrap_label(text, limit as f32, "<br/>", &|s| {
                self.metrics.calculate_text_width(s, &font.text_font())
            });
            let height = self.text_height(&wrapped, font);
            Measured {
                text: wrapped,
                width: limit,
                height,
                y: 0.0,
            }
        } else {
            let mut width: f64 = 0.0;
            let mut height = 0.0;
            for line in split_breaks(text) {
                width = width.max(self.text_width(line, font));
                height += self.text_height(line, font);
            }
            Measured {
                text: text.to_string(),
                width,
                height,
                y: 0.0,
            }
        }
    }

    /// `boundaryFont(conf)`.
    fn boundary_font(&self) -> C4Font {
        self.conf.font("boundary")
    }

    /// `draw(text, id, version, diagObj)`.
    pub fn draw(mut self) -> Result<Drawn, HelperError> {
        let conf = self.conf;
        let mut screen = Bounds::new();
        screen.set_data(
            conf.diagram_margin_x,
            conf.diagram_margin_x,
            conf.diagram_margin_y,
            conf.diagram_margin_y,
        );
        screen.width_limit = SCREEN_AVAIL_WIDTH;
        self.global_max_x = conf.diagram_margin_x;
        self.global_max_y = conf.diagram_margin_y;

        let top = self.db.boundaries_in("");
        self.draw_inside_boundary(&mut screen, &top, 0)?;
        self.draw_rels()?;

        screen.data.stopx = self.global_max_x;
        screen.data.stopy = self.global_max_y;
        let box_startx = screen.data.startx;
        let box_starty = screen.data.starty;
        let box_height = self.global_max_y - box_starty;
        let height = box_height + 2.0 * conf.diagram_margin_y;
        let box_width = self.global_max_x - box_startx;
        let width = box_width + 2.0 * conf.diagram_margin_x;

        let title = self.db.title.as_str();
        if !title.is_empty() {
            self.children.push(Element::Text(Text {
                common: Common::default(),
                x: box_width / 2.0 - 4.0 * conf.diagram_margin_x,
                y: box_starty + conf.diagram_margin_y,
                anchor: None,
                baseline: None,
                font: FontSpec::default(),
                runs: vec![TextRun::new(title)],
            }));
        }
        let extra_vert_for_title = if title.is_empty() { 0.0 } else { 60.0 };
        Ok(Drawn {
            view_box: [
                box_startx - conf.diagram_margin_x,
                -(conf.diagram_margin_y + extra_vert_for_title),
                width,
                height + extra_vert_for_title,
            ],
            width,
            height: if conf.use_max_width {
                height + extra_vert_for_title
            } else {
                height
            },
            defs: self.markers(),
            children: self.children,
        })
    }

    /// `drawInsideBoundary(diagram, parentBoundaryAlias, parentBounds,
    /// currentBoundaries, diagObj)`.
    fn draw_inside_boundary(
        &mut self,
        parent: &mut Bounds,
        current: &[usize],
        depth: usize,
    ) -> Result<(), HelperError> {
        if depth > MAX_DEPTH {
            return Err(HelperError::internal(
                "C4: a boundary is nested inside itself".to_string(),
            ));
        }
        let conf = self.conf;
        let db = self.db;
        let boundary_in_row = db.c4_boundary_in_row.max(1);
        let mut bounds = Bounds::new();
        bounds.width_limit =
            parent.width_limit / (boundary_in_row.min(current.len() as i64).max(1) as f64);
        for (i, &index) in current.iter().enumerate() {
            let boundary = &db.boundaries[index];
            let mut y = 0.0;
            if boundary.fields.get("sprite").is_some_and(Field::truthy) {
                y = 48.0;
            }
            let wrap = boundary.wrap && conf.wrap;
            let label_font = self.boundary_font().bold();
            let label_font = label_font.clone().sized(label_font.size + 2.0);
            let mut label = self.measure(
                boundary.fields.text("label").unwrap_or(""),
                wrap,
                &label_font,
                bounds.width_limit,
            );
            label.y = y + 8.0;
            y = label.y + label.height;

            let kind = match boundary.fields.text("type").filter(|t| !t.is_empty()) {
                Some(t) => {
                    let mut kind = self.measure(
                        &format!("[{t}]"),
                        wrap,
                        &self.boundary_font(),
                        bounds.width_limit,
                    );
                    kind.y = y + 5.0;
                    y = kind.y + kind.height;
                    Some(kind)
                }
                None => None,
            };
            let descr = match boundary.fields.text("descr").filter(|t| !t.is_empty()) {
                Some(t) => {
                    let font = self.boundary_font();
                    let font = font.clone().sized(font.size - 2.0);
                    let mut descr = self.measure(t, wrap, &font, bounds.width_limit);
                    descr.y = y + 20.0;
                    y = descr.y + descr.height;
                    Some(descr)
                }
                None => None,
            };

            if i == 0 || (i as i64) % boundary_in_row == 0 {
                let x = parent.data.startx + conf.diagram_margin_x;
                let y0 = parent.data.stopy + conf.diagram_margin_y + y;
                bounds.set_data(x, x, y0, y0);
            } else {
                let x = if bounds.data.stopx != bounds.data.startx {
                    bounds.data.stopx + conf.diagram_margin_x
                } else {
                    bounds.data.startx
                };
                let y0 = bounds.data.starty;
                bounds.set_data(x, x, y0, y0);
            }

            let alias = boundary.alias.as_str();
            let members = db.shapes_in(alias);
            if !members.is_empty() {
                self.draw_shape_array(&mut bounds, &members);
            }
            let nested = db.boundaries_in(alias);
            if !nested.is_empty() {
                self.draw_inside_boundary(&mut bounds, &nested, depth + 1)?;
            }
            if alias != "global" {
                let texts = BoundaryTexts { label, kind, descr };
                self.draw_boundary(index, &bounds, &texts);
            }
            parent.data.stopy = (bounds.data.stopy + conf.c4_shape_margin).max(parent.data.stopy);
            parent.data.stopx = (bounds.data.stopx + conf.c4_shape_margin).max(parent.data.stopx);
            self.global_max_x = self.global_max_x.max(parent.data.stopx);
            self.global_max_y = self.global_max_y.max(parent.data.stopy);
        }
        Ok(())
    }

    /// `drawC4ShapeArray`: measure every shape, place them on the grid, then
    /// draw each at its place.
    fn draw_shape_array(&mut self, bounds: &mut Bounds, members: &[usize]) {
        let conf = self.conf;
        let drawn: Vec<DrawnShape> = members
            .iter()
            .map(|&i| shapes::draw(&self.db.shapes[i], conf, self.styles, self.metrics))
            .collect();
        for (&i, shape) in members.iter().zip(drawn) {
            let (x, y) = bounds.insert(
                shape.width,
                shape.height,
                conf.c4_shape_margin,
                self.db.c4_shape_in_row,
                conf.next_line_padding_x,
            );
            self.children.push(shapes::positioned(&shape, x, y));
            self.shapes[i] = Some((shape, x, y));
        }
        bounds.bump_last_margin(conf.c4_shape_margin);
    }

    /// `drawBoundary` of the renderer and of `svgDraw.ts`.
    fn draw_boundary(&mut self, index: usize, bounds: &Bounds, texts: &BoundaryTexts) {
        let boundary = &self.db.boundaries[index];
        let x = bounds.data.startx;
        let y = bounds.data.starty;
        let width = bounds.data.stopx - x;
        let height = bounds.data.stopy - y;
        self.boundaries[index] = Some(RectBounds::new(x, y, width, height));

        let fill = boundary
            .fields
            .str("bgColor")
            .filter(|c| !c.is_empty())
            .map_or(Paint::None, css_paint);
        let stroke = css_paint(
            boundary
                .fields
                .str("borderColor")
                .filter(|c| !c.is_empty())
                .unwrap_or(BOUNDARY_COLOR),
        );
        let dash = if boundary.node_type.is_some() {
            None
        } else {
            Some(vec![7.0, 7.0])
        };
        let mut children = vec![Element::Rect(Rect {
            common: Common::with_style(Style {
                fill: Some(fill),
                stroke: Some(stroke),
                stroke_width: Some(1.0),
                stroke_dasharray: dash,
                ..Style::default()
            }),
            x,
            y,
            width,
            height,
            rx: Some(2.5),
            ry: Some(2.5),
        })];
        let text_fill = css_paint(BOUNDARY_COLOR);
        let label_font = self.boundary_font().bold();
        let label_font = label_font.clone().sized(label_font.size + 2.0);
        children.extend(self.by_tspan(
            &texts.label.text,
            x,
            y + texts.label.y,
            width,
            &text_fill,
            &label_font,
            false,
        ));
        if let Some(kind) = &texts.kind {
            children.extend(self.by_tspan(
                &kind.text,
                x,
                y + kind.y,
                width,
                &text_fill,
                &self.boundary_font(),
                false,
            ));
        }
        if let Some(descr) = &texts.descr {
            let font = self.boundary_font();
            let font = font.clone().sized(font.size - 2.0);
            children.extend(self.by_tspan(
                &descr.text,
                x,
                y + descr.y,
                width,
                &text_fill,
                &font,
                false,
            ));
        }
        self.children
            .push(Element::Group(crate::mermaid::scene::Group {
                common: Common::default(),
                children,
            }));
    }

    /// Half the unrounded ascent of the font's primary face: where Blink
    /// puts the alphabetic baseline of a `tspan` with `alignment-baseline:
    /// mathematical` in a `text` with `dominant-baseline: middle`.
    fn mathematical_shift(&self, font: &C4Font, italic: bool) -> f64 {
        let style = if italic {
            FaceStyle::Italic
        } else {
            FaceStyle::Normal
        };
        let fonts = self.metrics.fonts();
        match fonts.resolve(&font.family, Weight(font.weight_number()), style) {
            Some(face) => {
                let (ascent, _) = self.metrics.face_float_metrics(face, font.size as f32);
                f64::from(ascent) / 2.0
            }
            None => font.size * 0.5,
        }
    }

    /// `byTspan(content, g, x, y, width, height, textAttrs, conf)`: one
    /// `text` per `<br>` line, centered on `x + width / 2`.
    #[allow(clippy::too_many_arguments)]
    fn by_tspan(
        &self,
        content: &str,
        x: f64,
        y: f64,
        width: f64,
        fill: &Paint,
        font: &C4Font,
        italic: bool,
    ) -> Vec<Element> {
        let lines = split_breaks(content);
        let n = lines.len() as f64;
        let shift = self.mathematical_shift(font, italic);
        let weight = match font.weight_number() {
            400 => FontWeight::Normal,
            700 => FontWeight::Bold,
            w => FontWeight::Number(w),
        };
        lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let dy = i as f64 * font.size - (font.size * (n - 1.0)) / 2.0;
                Element::Text(Text {
                    common: Common::with_style(shapes::text_style(fill)),
                    x: x + width / 2.0,
                    y: y + dy + shift,
                    anchor: Some(TextAnchor::Middle),
                    baseline: None,
                    font: FontSpec {
                        family: Some(font.family.clone()),
                        size: Some(font.size),
                        weight: Some(weight),
                        style: italic.then_some(FontStyle::Italic),
                    },
                    runs: vec![TextRun::new(*line)],
                })
            })
            .collect()
    }

    /// `getC4Shape(alias)` as a drawn endpoint: shapes first, then
    /// boundaries.
    fn endpoint(&self, alias: &str) -> Result<Endpoint<'_>, HelperError> {
        if let Some(i) = self.db.shapes.iter().position(|s| s.alias == alias) {
            return match &self.shapes[i] {
                Some((drawn, x, y)) => Ok(Endpoint::Shape(drawn, *x, *y)),
                None => Err(no_intersect(alias)),
            };
        }
        if let Some(i) = self.db.boundaries.iter().position(|b| b.alias == alias) {
            return match self.boundaries[i] {
                Some(b) => Ok(Endpoint::Boundary(b)),
                None => Err(no_intersect(alias)),
            };
        }
        Err(HelperError::internal(format!(
            "C4 rel references an unknown shape or boundary: \"{alias}\""
        )))
    }

    /// `drawRels` of the renderer and of `svgDraw.ts`.
    fn draw_rels(&mut self) -> Result<(), HelperError> {
        let conf = self.conf;
        let font = conf.font("message");
        let dynamic = self.db.c4_type.as_deref() == Some("C4Dynamic");
        let mut children = Vec::new();
        for (n, rel) in self.db.rels.iter().enumerate() {
            let wrap = rel.wrap && conf.wrap;
            let mut label_text = rel.fields.text("label").unwrap_or("").to_string();
            if dynamic {
                label_text = format!("{}: {label_text}", n + 1);
            }
            let label = self.measure(
                &label_text,
                wrap,
                &font,
                self.text_width(&label_text, &font),
            );
            let techn = rel
                .fields
                .text("techn")
                .filter(|t| !t.is_empty())
                .map(|t| self.measure(t, wrap, &font, self.text_width(t, &font)));

            let from = self.endpoint(&rel.from)?;
            let to = self.endpoint(&rel.to)?;
            let start = from.intersect(to.center());
            let end = to.intersect(from.center());

            let text_color = rel
                .fields
                .get("textColor")
                .filter(|f| f.truthy())
                .map_or_else(|| BOUNDARY_COLOR.to_string(), Field::js_string);
            let line_color = rel
                .fields
                .get("lineColor")
                .filter(|f| f.truthy())
                .map_or_else(|| BOUNDARY_COLOR.to_string(), Field::js_string);
            let offset = |key: &str| -> f64 {
                match rel.fields.get(key).filter(|f| f.truthy()) {
                    Some(f) => {
                        let v = super::db::js_parse_int(&f.js_string());
                        if v.is_nan() { 0.0 } else { v }
                    }
                    None => 0.0,
                }
            };
            // `rel.offsetX ? parseInt(String(rel.offsetX)) : 0`; a string
            // that is not a number gives `NaN`, for which `0` stands in.
            let (offset_x, offset_y) = (offset("offsetX"), offset("offsetY"));

            let mut style = Style {
                fill: Some(Paint::None),
                stroke: Some(css_paint(&line_color)),
                stroke_width: Some(1.0),
                ..Style::default()
            };
            if rel.kind != "rel_b" {
                style.marker_end = Some(format!("{DIAGRAM_ID}-arrowhead"));
            }
            if rel.kind == "birel" || rel.kind == "rel_b" {
                style.marker_start = Some(format!("{DIAGRAM_ID}-arrowend"));
            }
            if n == 0 {
                children.push(Element::Line(Line {
                    common: Common::with_style(style),
                    x1: start.x,
                    y1: start.y,
                    x2: end.x,
                    y2: end.y,
                }));
            } else {
                let mut d = PathData::new();
                d.move_to(start.x, start.y).quad_to(
                    start.x + (end.x - start.x) / 2.0 - (end.x - start.x) / 4.0,
                    start.y + (end.y - start.y) / 2.0,
                    end.x,
                    end.y,
                );
                children.push(Element::Path(Path {
                    common: Common::with_style(style),
                    d,
                }));
            }

            let text_fill = css_paint(&text_color);
            let mid_x = start.x.min(end.x) + (end.x - start.x).abs() / 2.0 + offset_x;
            let mid_y = start.y.min(end.y) + (end.y - start.y).abs() / 2.0 + offset_y;
            children.extend(self.by_tspan(
                &label.text,
                mid_x,
                mid_y,
                label.width,
                &text_fill,
                &font,
                false,
            ));
            if let Some(techn) = techn {
                children.extend(self.by_tspan(
                    &format!("[{}]", techn.text),
                    mid_x,
                    mid_y + font.size + 5.0,
                    label.width.max(techn.width),
                    &text_fill,
                    &font,
                    true,
                ));
            }
        }
        self.children
            .push(Element::Group(crate::mermaid::scene::Group {
                common: Common::default(),
                children,
            }));
        Ok(())
    }

    /// `insertArrowHead` and `insertArrowEnd`. Their paths set no fill and
    /// take the root rule's `fill: textColor` through the `svg`.
    fn markers(&self) -> Vec<Element> {
        let marker = |name: &str, ref_x: f64, d: PathData| {
            Element::Marker(Marker {
                id: format!("{DIAGRAM_ID}-{name}"),
                class: None,
                view_box: None,
                ref_x,
                ref_y: 5.0,
                marker_width: 12.0,
                marker_height: 12.0,
                orient: Orient::Auto,
                marker_units: MarkerUnits::UserSpaceOnUse,
                style: Style::default(),
                children: vec![Element::Path(Path {
                    common: Common::with_style(Style {
                        fill: Some(self.styles.text_color.clone()),
                        ..Style::default()
                    }),
                    d,
                })],
            })
        };
        let mut head = PathData::new();
        head.move_to(0.0, 0.0)
            .line_to(10.0, 5.0)
            .line_to(0.0, 10.0)
            .close();
        let mut end = PathData::new();
        end.move_to(10.0, 0.0)
            .line_to(0.0, 5.0)
            .line_to(10.0, 10.0)
            .close();
        vec![marker("arrowhead", 9.0, head), marker("arrowend", 1.0, end)]
    }
}

fn no_intersect(alias: &str) -> HelperError {
    HelperError::internal(format!("C4 element \"{alias}\" has no intersect function"))
}
