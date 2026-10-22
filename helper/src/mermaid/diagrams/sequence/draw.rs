//! Port of `diagrams/sequence/svgDraw.js` (mermaid 12.2.0) to scene
//! elements. Each function builds what its d3 original appends, with the
//! stylesheet already resolved to attributes by `styles::sequence`.
//!
//! Text follows `drawText` to the attribute: the same rounded `x` and `y`,
//! `dy`, `text-anchor`, and `dominant-baseline`, so resvg puts the glyphs
//! where Chrome does. Heights that mermaid reads back with `getBBox()` come
//! from the measurer's `svg_bbox` of the drawn font.

use crate::mermaid::scene::{
    Baseline, Circle, Common, Element, Group, Length, Line, Marker, MarkerUnits, Orient, Path,
    PathData, Point, Polygon, Rect, Style, Text, TextAnchor, TextRun,
};
use crate::mermaid::styles::sequence::SequenceStyles;
use crate::text::TextMetrics;
use crate::text::markdown::split_breaks;

use super::conf::{Conf, FontPair};
use super::layout::{Activation, ActorModel, BoxModel, LoopModel};

/// mermaid-cli renders into `<svg id="my-svg">`; marker ids hang off it.
pub const DIAGRAM_ID: &str = "my-svg";

/// `ACTOR_TYPE_WIDTH` in `svgDraw.js`.
pub const ACTOR_TYPE_WIDTH: f64 = 18.0 * 2.0;

/// The height the classic stick figure reports: `ACTOR_GLYPH_BOTTOM -
/// ACTOR_GLYPH_TOP`.
pub const ACTOR_GLYPH_HEIGHT: f64 = 65.0;

const ZERO_WIDTH_SPACE: &str = "\u{200b}";

/// `Math.round` for the positive coordinates here: half rounds up.
pub fn js_round(v: f64) -> f64 {
    (v + 0.5).floor()
}

pub struct DrawCtx<'a> {
    pub conf: &'a Conf,
    pub styles: &'a SequenceStyles,
    pub metrics: &'a TextMetrics<'a>,
}

impl DrawCtx<'_> {
    /// `getBBox().height` of a drawn `<text>` holding one line.
    pub fn line_height(&self, line: &str, font: &FontPair) -> f64 {
        let text = if line.is_empty() {
            ZERO_WIDTH_SPACE
        } else {
            line
        };
        f64::from(self.metrics.svg_bbox(text, &font.render).height)
    }
}

fn common(class: &str, style: Style) -> Common {
    Common {
        id: None,
        class: Some(class.to_string()),
        transform: Vec::new(),
        style,
    }
}

/// `svgDrawCommon.drawRect` with the class rule already applied.
#[allow(clippy::too_many_arguments)]
pub fn rect(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    rx: Option<f64>,
    ry: Option<f64>,
    class: &str,
    style: Style,
) -> Element {
    Element::Rect(Rect {
        common: common(class, style),
        x,
        y,
        width,
        height,
        rx: rx.filter(|v| *v != 0.0),
        ry: ry.filter(|v| *v != 0.0),
    })
}

pub fn line(x1: f64, y1: f64, x2: f64, y2: f64, class: &str, style: Style) -> Element {
    Element::Line(Line {
        common: common(class, style),
        x1,
        y1,
        x2,
        y2,
    })
}

/// The `textData` object `drawText` reads.
#[derive(Debug, Clone)]
pub struct TextData {
    pub x: f64,
    pub y: f64,
    /// `undefined` on `svgDraw.getTextObj()`, which skips the anchor
    /// adjustment; `svgDrawCommon.getTextObj()` sets 100.
    pub width: Option<f64>,
    pub text: String,
    pub class: &'static str,
    pub anchor: Option<String>,
    pub valign: Option<&'static str>,
    pub text_margin: f64,
    /// `dy="1em"` on the element, as messages and notes set.
    pub dy_em: bool,
    pub tspan: bool,
}

/// `drawText`: one `<text>` per `<br>` line. Returns the elements and each
/// line's `getBBox().height`, which the callers sum.
pub fn draw_text(
    ctx: &DrawCtx<'_>,
    data: &TextData,
    font: &FontPair,
    style: Style,
) -> (Vec<Element>, Vec<f64>) {
    let lines = split_breaks(&data.text);
    let mut prev_text_height = 0.0;
    let mut text_height = 0.0;
    let has_valign = data.valign.is_some() && data.text_margin > 0.0;

    let mut x = data.x;
    let mut anchor = None;
    let mut baseline = None;
    if let (Some(a), Some(width)) = (&data.anchor, data.width) {
        match a.as_str() {
            "left" | "start" => {
                x = js_round(x + data.text_margin);
                anchor = Some(TextAnchor::Start);
                baseline = Some(Baseline::Middle);
            }
            "middle" | "center" => {
                x = js_round(x + width / 2.0);
                anchor = Some(TextAnchor::Middle);
                baseline = Some(Baseline::Middle);
            }
            "right" | "end" => {
                x = js_round(x + width - data.text_margin);
                anchor = Some(TextAnchor::End);
                baseline = Some(Baseline::Middle);
            }
            _ => {}
        }
    }
    if anchor.is_none() {
        anchor = data.anchor.as_deref().and_then(|a| match a {
            "start" | "left" => Some(TextAnchor::Start),
            "middle" | "center" => Some(TextAnchor::Middle),
            "end" | "right" => Some(TextAnchor::End),
            _ => None,
        });
    }

    let mut elements = Vec::with_capacity(lines.len());
    let mut heights = Vec::with_capacity(lines.len());
    for (i, raw) in lines.iter().enumerate() {
        let y = match data.valign.filter(|_| has_valign) {
            Some("top" | "start") => js_round(data.y + data.text_margin),
            Some("middle" | "center") => {
                js_round(data.y + (prev_text_height + text_height + data.text_margin) / 2.0)
            }
            Some("bottom" | "end") => js_round(
                data.y + (prev_text_height + text_height + 2.0 * data.text_margin)
                    - data.text_margin,
            ),
            _ => data.y,
        };
        let dy = if data.dy_em {
            Some(Length::Em(1.0))
        } else if data.text_margin == 0.0 && i > 0 {
            Some(Length::Px(i as f64 * font.size))
        } else {
            None
        };
        let text = if raw.is_empty() {
            ZERO_WIDTH_SPACE.to_string()
        } else {
            raw.to_string()
        };
        let run = TextRun {
            text: text.clone(),
            x: data.tspan.then_some(x),
            dy,
            ..TextRun::default()
        };
        let mut style = style.clone();
        style.font = font.scene_font();
        elements.push(Element::Text(Text {
            common: common(data.class, style),
            x,
            y,
            anchor,
            baseline,
            font: Default::default(),
            runs: vec![run],
        }));
        let h = ctx.line_height(raw, font);
        heights.push(h);
        if has_valign {
            text_height += h;
            prev_text_height = text_height;
        }
    }
    (elements, heights)
}

/// `_drawTextCandidateFunc` `byTspan`: the actor label and the box title.
/// One `<text>` per line with `dominant-baseline: central`, the lines
/// spread around `y + height / 2` by `dy`.
#[allow(clippy::too_many_arguments)]
pub fn draw_text_by_tspan(
    ctx: &DrawCtx<'_>,
    content: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    class: &str,
    style: Style,
) -> Vec<Element> {
    let font = &ctx.conf.fonts.actor;
    let lines = split_breaks(content);
    let n = lines.len() as f64;
    lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let dy = i as f64 * font.size - (font.size * (n - 1.0)) / 2.0;
            let mut style = style.clone();
            style.font = font.scene_font();
            Element::Text(Text {
                common: common(class, style),
                x: x + width / 2.0,
                y: y + height / 2.0,
                anchor: Some(TextAnchor::Middle),
                baseline: Some(Baseline::Central),
                font: Default::default(),
                runs: vec![TextRun {
                    text: l.to_string(),
                    x: Some(x + width / 2.0),
                    dy: Some(Length::Px(dy)),
                    ..TextRun::default()
                }],
            })
        })
        .collect()
}

/// `drawLabel`: the notched polygon and its text.
pub fn draw_label(ctx: &DrawCtx<'_>, data: &mut TextData, height: f64, width: f64) -> Vec<Element> {
    let cut = 7.0;
    let (x, y) = (data.x, data.y);
    let points = vec![
        Point::new(x, y),
        Point::new(x + width, y),
        Point::new(x + width, y + height - cut),
        Point::new(x + width - cut * 1.2, y + height),
        Point::new(x, y + height),
    ];
    let mut out = vec![Element::Polygon(Polygon {
        common: common("labelBox", ctx.styles.label_box()),
        points,
    })];
    data.y += height / 2.0;
    let (texts, _) = draw_text(ctx, data, &ctx.conf.fonts.message, ctx.styles.label_text());
    out.extend(texts);
    out
}

/// The lifeline every top actor gets: `y2` is patched by
/// `fixLifeLineHeights` once the diagram height is known.
fn lifeline(ctx: &DrawCtx<'_>, actor: &ActorModel, center: f64, center_y: f64) -> Element {
    let mut el = line(
        center,
        center_y,
        center,
        2000.0,
        "actor-line 200",
        ctx.styles.actor_line(),
    );
    if let Element::Line(l) = &mut el {
        l.common.id = actor.actor_cnt.map(|c| format!("actor{c}"));
    }
    el
}

/// What `drawActor` appends: the group d3 `.lower()`s to the front, the
/// group that stays in document order (the stick figure), and the height
/// the function returns.
pub struct DrawnActor {
    pub lowered: Group,
    pub appended: Option<Group>,
    pub height: f64,
}

/// `drawActorTypeParticipant`.
pub fn draw_actor_participant(
    ctx: &DrawCtx<'_>,
    actor: &mut ActorModel,
    is_footer: bool,
) -> DrawnActor {
    let actor_y = if is_footer {
        actor.stopy.unwrap_or(0.0)
    } else {
        actor.starty
    };
    let center = actor.x + actor.width / 2.0;
    let center_y = actor_y + actor.height;
    let class = if is_footer {
        "actor actor-bottom"
    } else {
        "actor actor-top"
    };
    let mut inner = vec![rect(
        actor.x,
        actor_y,
        actor.width,
        actor.height,
        Some(3.0),
        Some(3.0),
        class,
        ctx.styles.actor_rect(),
    )];
    inner.extend(draw_text_by_tspan(
        ctx,
        &actor.description,
        actor.x,
        actor_y,
        actor.width,
        actor.height,
        "actor actor-box",
        ctx.styles.actor_text(),
    ));
    let mut outer = Group::default();
    if is_footer {
        outer.children = inner;
    } else {
        outer.children.push(lifeline(ctx, actor, center, center_y));
        outer.children.push(Element::Group(Group {
            common: Common::default(),
            children: inner,
        }));
    }
    // `rectElem.getBBox().height` is the rect's own height.
    DrawnActor {
        lowered: outer,
        appended: None,
        height: actor.height,
    }
}

/// `drawActorTypeActor`: the stick figure under the classic look.
pub fn draw_actor_actor(ctx: &DrawCtx<'_>, actor: &mut ActorModel, is_footer: bool) -> DrawnActor {
    let actor_y = if is_footer {
        actor.stopy.unwrap_or(0.0)
    } else {
        actor.starty
    };
    let center = actor.x + actor.width / 2.0;
    let center_y = actor_y + 80.0;
    let mut lowered = Group::default();
    if !is_footer {
        lowered
            .children
            .push(lifeline(ctx, actor, center, center_y));
    }
    let class = if is_footer {
        "actor-man actor-bottom"
    } else {
        "actor-man actor-top"
    };
    let man = ctx.styles.actor_man();
    let gy = |offset: f64| actor_y + offset;
    let half = ACTOR_TYPE_WIDTH / 2.0;
    let mut children = vec![
        line(center, gy(25.0), center, gy(45.0), "", man.clone()),
        line(
            center - half,
            gy(33.0),
            center + half,
            gy(33.0),
            "",
            man.clone(),
        ),
        line(center - half, gy(60.0), center, gy(45.0), "", man.clone()),
        line(
            center,
            gy(45.0),
            center + half - 2.0,
            gy(60.0),
            "",
            man.clone(),
        ),
        Element::Circle(Circle {
            common: Common::with_style(man.clone()),
            cx: center,
            cy: gy(10.0),
            r: 15.0,
        }),
    ];
    actor.height = ACTOR_GLYPH_HEIGHT;
    children.extend(draw_text_by_tspan(
        ctx,
        &actor.description,
        actor.x,
        actor_y + 35.0,
        actor.width,
        actor.height,
        "actor actor-man",
        ctx.styles.actor_text(),
    ));
    // The group's inline `stroke: actorBorder` is what the lines and the
    // circle inherit; the text's `stroke: none` beats it.
    let group = Group {
        common: Common {
            class: Some(class.to_string()),
            style: Style {
                stroke: man.stroke.clone(),
                ..Style::default()
            },
            ..Common::default()
        },
        children,
    };
    DrawnActor {
        lowered,
        appended: Some(group),
        height: actor.height,
    }
}

/// `drawBox`: the background rect and the centered title.
pub fn draw_box(ctx: &DrawCtx<'_>, b: &BoxModel) -> Group {
    let mut children = vec![rect(
        b.startx,
        b.starty,
        b.stopx - b.startx,
        b.stopy - b.starty,
        None,
        None,
        "rect",
        ctx.styles.box_rect(&b.fill),
    )];
    if let Some(name) = b.name.as_deref().filter(|n| !n.is_empty()) {
        children.extend(draw_text_by_tspan(
            ctx,
            name,
            b.x,
            b.y + ctx.conf.box_text_margin + b.text_max_height / 2.0,
            b.width,
            0.0,
            "text",
            ctx.styles.box_text(),
        ));
    }
    Group {
        common: Common::default(),
        children,
    }
}

/// `drawActivation`: the rect that lands in the anchor group.
pub fn draw_activation(
    ctx: &DrawCtx<'_>,
    activation: &Activation,
    vertical_pos: f64,
    count: usize,
) -> Element {
    let class = format!("activation{}", count % 3);
    rect(
        activation.startx,
        activation.starty,
        activation.stopx - activation.startx,
        vertical_pos - activation.starty,
        None,
        None,
        &class,
        ctx.styles.activation(),
    )
}

/// `drawLoop`: the frame, the section dividers, the label tag, the
/// title, and the section titles.
pub fn draw_loop(ctx: &DrawCtx<'_>, model: &mut LoopModel, label_text: &str) -> Group {
    let conf = ctx.conf;
    let startx = model.startx.unwrap_or(0.0);
    let stopx = model.stopx.unwrap_or(0.0);
    let starty = model.starty;
    let stopy = model.stopy.unwrap_or(starty);
    let loop_line = |x1, y1, x2, y2| line(x1, y1, x2, y2, "loopLine", ctx.styles.loop_line(false));
    let mut children = vec![
        loop_line(startx, starty, stopx, starty),
        loop_line(stopx, starty, stopx, stopy),
        loop_line(startx, stopy, stopx, stopy),
        loop_line(startx, starty, startx, stopy),
    ];
    for section in &model.sections {
        children.push(line(
            startx,
            section.y,
            stopx,
            section.y,
            "loopLine",
            ctx.styles.loop_line(true),
        ));
    }

    let mut label = TextData {
        x: startx,
        y: starty,
        width: Some(conf.label_box_width.max(50.0)),
        text: label_text.to_string(),
        class: "labelText",
        anchor: Some("middle".to_string()),
        valign: Some("middle"),
        text_margin: conf.box_text_margin,
        dy_em: false,
        tspan: false,
    };
    let height = if conf.label_box_height != 0.0 {
        conf.label_box_height
    } else {
        20.0
    };
    children.extend(draw_label(
        ctx,
        &mut label,
        height,
        conf.label_box_width.max(50.0),
    ));

    let title = TextData {
        x: startx + conf.label_box_width / 2.0 + (stopx - startx) / 2.0,
        y: starty + conf.box_margin + conf.box_text_margin,
        width: None,
        text: model.title.clone().unwrap_or_default(),
        class: "loopText",
        anchor: Some("middle".to_string()),
        valign: Some("middle"),
        text_margin: conf.box_text_margin,
        dy_em: false,
        tspan: true,
    };
    let (title_elements, title_heights) =
        draw_text(ctx, &title, &conf.fonts.message, ctx.styles.loop_text());
    children.extend(title_elements);
    let title_height = js_round(title_heights.iter().sum());

    for (idx, item) in model.section_titles.iter().enumerate() {
        let Some(message) = item.as_deref().filter(|m| !m.is_empty()) else {
            continue;
        };
        let Some(section) = model.sections.get(idx) else {
            continue;
        };
        let data = TextData {
            x: startx + (stopx - startx) / 2.0,
            y: section.y + conf.box_margin + conf.box_text_margin,
            width: None,
            text: message.to_string(),
            class: "sectionTitle",
            anchor: Some("middle".to_string()),
            valign: Some("middle"),
            text_margin: conf.box_text_margin,
            dy_em: false,
            tspan: false,
        };
        let (els, _) = draw_text(ctx, &data, &conf.fonts.message, ctx.styles.loop_text());
        children.extend(els);
        // mermaid reads the title's elements here, not the section's.
        model.sections[idx].height += title_height - (conf.box_margin + conf.box_text_margin);
    }
    model.height = js_round(stopy - starty);
    Group {
        common: Common::default(),
        children,
    }
}

/// `drawBackgroundRect` for a `rect` block: no stroke, class `rect`.
pub fn draw_background_rect(ctx: &DrawCtx<'_>, model: &LoopModel) -> Element {
    let startx = model.startx.unwrap_or(0.0);
    let stopx = model.stopx.unwrap_or(0.0);
    let stopy = model.stopy.unwrap_or(model.starty);
    rect(
        startx,
        model.starty,
        stopx - startx,
        stopy - model.starty,
        None,
        None,
        "rect",
        ctx.styles
            .background_rect(model.fill.as_deref().unwrap_or("")),
    )
}

fn path(d: PathData, style: Style) -> Element {
    Element::Path(Path {
        common: Common::with_style(style),
        d,
    })
}

/// A marker's `refX`/`refY` and `markerWidth`/`markerHeight`.
struct MarkerGeom {
    reference: (f64, f64),
    size: (f64, f64),
}

const fn geom(ref_x: f64, ref_y: f64, width: f64, height: f64) -> MarkerGeom {
    MarkerGeom {
        reference: (ref_x, ref_y),
        size: (width, height),
    }
}

fn marker(
    id: &str,
    geom: MarkerGeom,
    orient: Orient,
    units: MarkerUnits,
    style: Style,
    children: Vec<Element>,
) -> Element {
    Element::Marker(Marker {
        id: format!("{DIAGRAM_ID}-{id}"),
        class: None,
        view_box: None,
        ref_x: geom.reference.0,
        ref_y: geom.reference.1,
        marker_width: geom.size.0,
        marker_height: geom.size.1,
        orient,
        marker_units: units,
        style,
        children,
    })
}

pub fn marker_ref(name: &str) -> String {
    format!("{DIAGRAM_ID}-{name}")
}

/// The eight `insert*Head` and `insertSequenceNumber` definitions, in
/// the order `draw` appends them.
pub fn markers(styles: &SequenceStyles) -> Vec<Element> {
    let mut arrowhead = PathData::new();
    arrowhead
        .move_to(-1.0, 0.0)
        .line_to(10.0, 5.0)
        .line_to(0.0, 10.0)
        .close();
    let mut crosshead = PathData::new();
    crosshead
        .move_to(1.0, 2.0)
        .line_to(6.0, 7.0)
        .move_to(6.0, 2.0)
        .line_to(1.0, 7.0);
    let mut filled = PathData::new();
    filled
        .move_to(18.0, 7.0)
        .line_to(9.0, 13.0)
        .line_to(14.0, 7.0)
        .line_to(9.0, 1.0)
        .close();
    let mut solid_top = PathData::new();
    solid_top
        .move_to(0.0, 0.0)
        .line_to(10.0, 8.0)
        .line_to(0.0, 8.0)
        .close();
    let mut solid_bottom = PathData::new();
    solid_bottom
        .move_to(0.0, 0.0)
        .line_to(10.0, 0.0)
        .line_to(0.0, 8.0)
        .close();
    let mut stick_top = PathData::new();
    stick_top.move_to(0.0, 0.0).line_to(7.0, 7.0);
    let mut stick_bottom = PathData::new();
    stick_bottom.move_to(0.0, 7.0).line_to(7.0, 0.0);
    vec![
        marker(
            "arrowhead",
            geom(7.9, 5.0, 12.0, 12.0),
            Orient::AutoStartReverse,
            MarkerUnits::UserSpaceOnUse,
            Style::default(),
            vec![path(arrowhead, styles.arrowhead())],
        ),
        marker(
            "crosshead",
            geom(4.0, 4.5, 15.0, 8.0),
            Orient::Auto,
            MarkerUnits::StrokeWidth,
            Style::default(),
            vec![path(crosshead, styles.crosshead())],
        ),
        marker(
            "filled-head",
            geom(15.5, 7.0, 20.0, 28.0),
            Orient::Auto,
            MarkerUnits::StrokeWidth,
            Style::default(),
            vec![path(filled, styles.root_fill())],
        ),
        marker(
            "sequencenumber",
            geom(15.0, 15.0, 60.0, 40.0),
            Orient::Auto,
            MarkerUnits::StrokeWidth,
            styles.sequence_number_marker(),
            vec![Element::Circle(Circle {
                common: Common::default(),
                cx: 15.0,
                cy: 15.0,
                r: 6.0,
            })],
        ),
        marker(
            "solidTopArrowHead",
            geom(7.9, 7.25, 12.0, 12.0),
            Orient::AutoStartReverse,
            MarkerUnits::UserSpaceOnUse,
            Style::default(),
            vec![path(solid_top, styles.root_fill())],
        ),
        marker(
            "solidBottomArrowHead",
            geom(7.9, 0.75, 12.0, 12.0),
            Orient::AutoStartReverse,
            MarkerUnits::UserSpaceOnUse,
            Style::default(),
            vec![path(solid_bottom, styles.root_fill())],
        ),
        marker(
            "stickTopArrowHead",
            geom(7.5, 7.0, 12.0, 12.0),
            Orient::AutoStartReverse,
            MarkerUnits::UserSpaceOnUse,
            Style::default(),
            vec![path(stick_top, styles.stick_head())],
        ),
        marker(
            "stickBottomArrowHead",
            geom(7.5, 0.0, 12.0, 12.0),
            Orient::AutoStartReverse,
            MarkerUnits::UserSpaceOnUse,
            Style::default(),
            vec![path(stick_bottom, styles.stick_head())],
        ),
    ]
}

/// The self-message curve of `drawMessage`:
/// `M lsx,y C lsx+60,y-10 sx+60,y+30 sx,y+20`.
pub fn self_message_curve(line_start_x: f64, startx: f64, y: f64) -> PathData {
    let mut d = PathData::new();
    d.move_to(line_start_x, y).cubic_to(
        line_start_x + 60.0,
        y - 10.0,
        startx + 60.0,
        y + 30.0,
        startx,
        y + 20.0,
    );
    d
}

/// The `rightAngles` self message: `M lsx,y H right V y+25 H sx`.
pub fn self_message_right_angles(line_start_x: f64, startx: f64, right: f64, y: f64) -> PathData {
    let mut d = PathData::new();
    d.move_to(line_start_x, y)
        .line_to(right, y)
        .line_to(right, y + 25.0)
        .line_to(startx, y + 25.0);
    d
}
