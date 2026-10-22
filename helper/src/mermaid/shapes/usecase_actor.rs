//! `shapes/usecaseActor.ts`, `usecaseActorHollow.ts`,
//! `usecaseActorAwesome.ts`, `usecaseActorIcon.ts`: the actor figures.
//! The icon variant needs an icon pack and reports `unsupported_feature`.

use super::util::{
    assemble, circle_element, get_node_classes, group_element, insert_label, label_helper,
    node_style, parse_svg_path, path_element, rect_element, translate,
};
use super::{Intersector, Node, NodeExtra, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Element, Paint, PathData, Style};

const ACTOR_FIGURE_WIDTH: f64 = 56.0;
const ACTOR_FIGURE_HEIGHT: f64 = 72.0;
const ACTOR_LABEL_GAP: f64 = 8.0;
const STEREOTYPE_LABEL_GAP: f64 = 2.0;
const DEFAULT_ACTOR_PADDING: f64 = 8.0;

const STICK_FIGURE_PATH: &str = "M 0 -12 C 6.627 -12 12 -17.373 12 -24 C 12 -30.627 6.627 -36 0 -36 C -6.627 -36 -12 -30.627 -12 -24 C -12 -17.373 -6.627 -12 0 -12 Z M 0 -12 V 8 M -17 -5 H 17 M 0 8 L -15 28 M 0 8 L 15 28";
const AWESOME_SILHOUETTE_PATH: &str = "M 0 -34 C 7.18 -34 13 -28.18 13 -21 C 13 -13.82 7.18 -8 0 -8 C -7.18 -8 -13 -13.82 -13 -21 C -13 -28.18 -7.18 -34 0 -34 Z M -24 25 C -24 7 -14 -3 0 -3 C 14 -3 24 7 24 25 C 24 28 21 30 18 30 H -18 C -21 30 -24 28 -24 25 Z";
const HOLLOW_BODY_PATH: &str =
    "M -22 -10 H 22 V 0 H 6 L 22 17 L 13 28 L 0 13 L -13 28 L -22 17 L -6 0 H -22 Z";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Variant {
    Normal,
    Hollow,
    Awesome,
}

impl Variant {
    fn name(self) -> &'static str {
        match self {
            Variant::Normal => "normal",
            Variant::Hollow => "hollow",
            Variant::Awesome => "awesome",
        }
    }

    fn class(self) -> &'static str {
        match self {
            Variant::Normal => "usecase-actor-shape usecase-actor-normal",
            Variant::Hollow => "usecase-actor-hollow",
            Variant::Awesome => "usecase-actor-awesome",
        }
    }
}

/// `businessMarkerPathForCircle(centerY, radius)`.
fn business_marker_path_for_circle(center_y: f64, radius: f64) -> PathData {
    let angle = std::f64::consts::PI / 3.0;
    let ratio = 0.6;
    let center_offset = radius * ratio;
    let half_chord = radius * (1.0 - ratio * ratio).sqrt();
    let direction_x = angle.cos();
    let direction_y = -angle.sin();
    let center_x = center_offset * -direction_y;
    let marker_center_y = center_y + center_offset * direction_x;
    let delta_x = half_chord * direction_x;
    let delta_y = half_chord * direction_y;
    let mut d = PathData::new();
    d.move_to(center_x - delta_x, marker_center_y - delta_y)
        .line_to(center_x + delta_x, marker_center_y + delta_y);
    d
}

fn business_marker(variant: Variant) -> PathData {
    match variant {
        Variant::Normal => business_marker_path_for_circle(-24.0, 12.0),
        Variant::Hollow => business_marker_path_for_circle(-23.0, 9.0),
        Variant::Awesome => business_marker_path_for_circle(-21.0, 13.0),
    }
}

fn parsed(path: &str) -> Result<PathData, HelperError> {
    parse_svg_path(path).ok_or_else(|| HelperError::internal("bad actor glyph path"))
}

fn glyph(variant: Variant, style: &Style) -> Result<Vec<Element>, HelperError> {
    let hollow = Style {
        fill: Some(Paint::None),
        ..style.clone()
    };
    Ok(match variant {
        Variant::Normal => vec![path_element(
            parsed(STICK_FIGURE_PATH)?,
            "usecase-actor-stick",
            style.clone(),
            Vec::new(),
        )],
        Variant::Awesome => vec![path_element(
            parsed(AWESOME_SILHOUETTE_PATH)?,
            "usecase-actor-awesome-silhouette",
            style.clone(),
            Vec::new(),
        )],
        Variant::Hollow => vec![
            circle_element(0.0, -23.0, 9.0, "usecase-actor-hollow-head", hollow.clone()),
            path_element(
                parsed(HOLLOW_BODY_PATH)?,
                "usecase-actor-hollow-body",
                hollow,
                Vec::new(),
            ),
        ],
    })
}

/// `renderUsecaseActor(parent, node, variant, drawGlyph)`.
fn render_actor(
    node: &mut Node,
    ctx: &ShapeCtx,
    variant: Variant,
) -> Result<ShapeOut, HelperError> {
    let (stereotype, business) = match &node.extra {
        NodeExtra::Usecase {
            stereotype,
            business,
            ..
        } => (stereotype.clone(), *business),
        _ => (None, false),
    };
    let classes = get_node_classes(
        node,
        Some(&format!(
            "usecase-actor-variant usecase-actor-{}",
            variant.name()
        )),
    );
    let mut out = label_helper(node, ctx, Some(&classes), false);
    out.label.common.class = Some("label actor-label usecase-actor-label".to_string());
    let stereo = stereotype.map(|s| {
        let (mut g, b) = insert_label(&format!("«{s}»"), ctx, node, node.wrapping_width, true);
        g.common.class = Some("label usecase-stereotype".to_string());
        (g, b)
    });

    let style = node_style(ctx, node);
    let mut glyph_children = glyph(variant, &style)?;
    if business {
        glyph_children.push(path_element(
            business_marker(variant),
            "usecase-business-marker usecase-actor-business-marker",
            Style {
                fill: Some(Paint::None),
                stroke: style.stroke.clone(),
                stroke_width: style.stroke_width,
                ..Style::default()
            },
            Vec::new(),
        ));
    }
    let padding = node.padding.unwrap_or(DEFAULT_ACTOR_PADDING);
    let stereo_h = stereo.as_ref().map_or(0.0, |(_, b)| b.height);
    let stereo_w = stereo.as_ref().map_or(0.0, |(_, b)| b.width);
    let content_height = ACTOR_FIGURE_HEIGHT
        + ACTOR_LABEL_GAP
        + out.bbox.height
        + if stereo.is_some() {
            stereo_h + STEREOTYPE_LABEL_GAP
        } else {
            0.0
        };
    let total_width = ACTOR_FIGURE_WIDTH.max(out.bbox.width).max(stereo_w) + padding * 2.0;
    let total_height = content_height + padding * 2.0;
    let content_top = -content_height / 2.0;
    let actor_group = group_element(
        &format!("usecase-actor-glyph {}", variant.class()),
        translate(0.0, content_top + ACTOR_FIGURE_HEIGHT / 2.0),
        glyph_children,
    );
    let mut next_label_center = content_top + ACTOR_FIGURE_HEIGHT + ACTOR_LABEL_GAP;
    let mut stereo_el = None;
    if let Some((mut g, b)) = stereo {
        next_label_center += stereo_h / 2.0;
        g.common.transform = translate(
            -b.width / 2.0 - b.x,
            next_label_center - b.height / 2.0 - b.y,
        );
        next_label_center += stereo_h / 2.0 + STEREOTYPE_LABEL_GAP;
        stereo_el = Some(Element::Group(g));
    }
    let label_center = next_label_center + out.bbox.height / 2.0;
    out.label.common.transform = translate(
        -out.bbox.width / 2.0 - out.bbox.x,
        label_center - out.bbox.height / 2.0 - out.bbox.y,
    );
    let outline = rect_element(
        -total_width / 2.0,
        -total_height / 2.0,
        total_width,
        total_height,
        None,
        None,
        "usecase-actor-outline",
        Style {
            opacity: Some(0.0),
            ..Style::default()
        },
    );
    let label = std::mem::take(&mut out.label);
    let mut group = assemble(out.shape_svg, vec![outline], label);
    if let Some(el) = stereo_el {
        group.children.push(el);
    }
    group.children.push(actor_group);
    node.width = Some(total_width);
    node.height = Some(total_height);
    node.intersector = Intersector::Rect;
    Ok(ShapeOut {
        group,
        width: total_width,
        height: total_height,
    })
}

pub struct UsecaseActor;

impl Shape for UsecaseActor {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        render_actor(node, ctx, Variant::Normal)
    }
}

pub struct UsecaseActorHollow;

impl Shape for UsecaseActorHollow {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        render_actor(node, ctx, Variant::Hollow)
    }
}

pub struct UsecaseActorAwesome;

impl Shape for UsecaseActorAwesome {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        render_actor(node, ctx, Variant::Awesome)
    }
}

pub struct UsecaseActorIcon;

impl Shape for UsecaseActorIcon {
    fn draw(&self, _node: &mut Node, _ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        Err(HelperError::UnsupportedFeature {
            feature: "icon pack".to_string(),
        })
    }
}
