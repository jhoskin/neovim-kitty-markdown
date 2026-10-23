//! Port of `cynefinRenderer.ts` (`draw`, `getDomainLayouts`) and
//! `cynefinBoundaries.ts` (`seededRandom`, `hashString`, `resolveSeed`,
//! `generateFoldPath`, `generateHorizontalBoundary`, `generateCliffPath`,
//! `generateConfusionPath`).
//!
//! The layout is fixed by the config: four quadrants of `width` by `height`
//! inside the padding, the confusion ellipse over their meeting point, and
//! the item badges stacked under each domain label. Only the badge widths
//! are measured. The boundary waves come from mulberry32 seeded with the
//! configured seed, or with the hash of the SVG id when the seed is 0. The
//! oracle renders with the id `my-svg`, so the helper hashes that id and
//! draws the same waves.

use serde_json::Value;

use super::db::CynefinDb;
use super::parser::DomainName;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::scene::{
    Baseline, Color, Common, Element, FontSpec, FontStyle, FontWeight, Group, Marker, MarkerUnits,
    Orient, Paint, Path, PathData, Rect, Scene, Style, Text, TextAnchor, TextRun, TransformOp,
    ViewBox,
};
use crate::mermaid::styles::cynefin::CynefinStyles;
use crate::text::measure::FontSpec as MeasureSpec;

/// The id mermaid-cli renders the SVG with; `draw` hashes it into the
/// boundary seed and names the arrow marker after it.
pub const SVG_ID: &str = "my-svg";

/// `MAX_CONFUSION_ITEMS`: items in the confusion ellipse before the
/// overflow badge.
const MAX_CONFUSION_ITEMS: usize = 3;
/// `itemHeight` of a badge.
const ITEM_HEIGHT: f64 = 26.0;
/// `itemPaddingX`, left and right of the badge text.
const ITEM_PADDING_X: f64 = 10.0;
/// The gap between stacked badges (`itemHeight + 4`).
const ITEM_GAP: f64 = 4.0;
/// The `rx` and `ry` of a badge.
const ITEM_RADIUS: f64 = 4.0;

/// `CynefinDiagramConfig`: `DEFAULT_CONFIG.cynefin` with
/// `getConfig().cynefin` merged over it.
#[derive(Debug, Clone, PartialEq)]
pub struct CynefinConfig {
    pub width: f64,
    pub height: f64,
    pub padding: f64,
    pub show_domain_descriptions: bool,
    pub boundary_amplitude: f64,
    /// `seed` when it is a JS number; anything else is hashed away.
    pub seed: Option<f64>,
}

impl CynefinConfig {
    /// The `cynefin` section of the effective config; a value of the wrong
    /// type keeps the schema default.
    pub fn from_raw(raw: &Value) -> CynefinConfig {
        let section = &raw["cynefin"];
        let number =
            |key: &str, default: f64| section.get(key).and_then(Value::as_f64).unwrap_or(default);
        CynefinConfig {
            width: number("width", 800.0),
            height: number("height", 600.0),
            padding: number("padding", 40.0),
            show_domain_descriptions: section
                .get("showDomainDescriptions")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            boundary_amplitude: number("boundaryAmplitude", 8.0),
            seed: section.get("seed").and_then(Value::as_f64),
        }
    }
}

/// JS `ToInt32`, the `| 0` and `^` operand conversion.
fn to_int32(x: f64) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    x.trunc().rem_euclid(4_294_967_296.0) as u32 as i32
}

/// `seededRandom`: one step of mulberry32 from `seed`, in `[0, 1)`.
pub fn seeded_random(seed: f64) -> f64 {
    // `let t = (seed + 0x6d2b79f5) | 0`.
    let mut t = to_int32(seed + f64::from(0x6d2b_79f5_u32));
    let ushr = |v: i32, n: u32| ((v as u32) >> n) as i32;
    // `t = Math.imul(t ^ (t >>> 15), t | 1)`.
    t = (t ^ ushr(t, 15)).wrapping_mul(t | 1);
    // `t ^= t + Math.imul(t ^ (t >>> 7), t | 61)`: the sum is a JS number
    // that `^` wraps back to 32 bits.
    t ^= t.wrapping_add((t ^ ushr(t, 7)).wrapping_mul(t | 61));
    f64::from((t ^ ushr(t, 14)) as u32) / 4_294_967_296.0
}

/// `hashString`: the Java string hash over UTF-16 code units.
pub fn hash_string(s: &str) -> i32 {
    let mut hash: i32 = 0;
    for unit in s.encode_utf16() {
        hash = (hash << 5).wrapping_sub(hash).wrapping_add(i32::from(unit));
    }
    hash
}

/// `resolveSeed`: a finite non-zero configured seed, else the id hash.
pub fn resolve_seed(configured: Option<f64>, id: &str) -> f64 {
    match configured {
        Some(seed) if seed.is_finite() && seed != 0.0 => seed,
        _ => f64::from(hash_string(id)),
    }
}

/// `generateFoldPath`: the vertical wave through the centre, seven cubic
/// segments whose ends jitter by up to `amplitude` sideways.
pub fn fold_path(width: f64, height: f64, seed: f64, amplitude: f64) -> PathData {
    let cx = width / 2.0;
    let segments = 7;
    let seg_height = height / f64::from(segments);
    let points: Vec<(f64, f64)> = (0..=segments)
        .map(|i| {
            let i = f64::from(i);
            let jitter = seeded_random(seed + i * 17.0) * amplitude * 2.0 - amplitude;
            (cx + jitter, i * seg_height)
        })
        .collect();
    let mut d = PathData::new();
    d.move_to(points[0].0, points[0].1);
    for (i, pair) in points.windows(2).enumerate() {
        let (p0, p1) = (pair[0], pair[1]);
        let mid_y = (p0.1 + p1.1) / 2.0;
        let dir = if i % 2 == 0 { 1.0 } else { -1.0 };
        let offset = amplitude * 1.5 * dir * seeded_random(seed + i as f64 * 31.0 + 7.0);
        d.cubic_to(p0.0 + offset, mid_y, p1.0 - offset, mid_y, p1.0, p1.1);
    }
    d
}

/// `generateHorizontalBoundary`: the horizontal wave through the centre.
pub fn horizontal_boundary(width: f64, height: f64, seed: f64, amplitude: f64) -> PathData {
    let cy = height / 2.0;
    let segments = 7;
    let seg_width = width / f64::from(segments);
    let points: Vec<(f64, f64)> = (0..=segments)
        .map(|i| {
            let i = f64::from(i);
            let jitter = seeded_random(seed + i * 23.0) * amplitude * 2.0 - amplitude;
            (i * seg_width, cy + jitter)
        })
        .collect();
    let mut d = PathData::new();
    d.move_to(points[0].0, points[0].1);
    for (i, pair) in points.windows(2).enumerate() {
        let (p0, p1) = (pair[0], pair[1]);
        let mid_x = (p0.0 + p1.0) / 2.0;
        let dir = if i % 2 == 0 { 1.0 } else { -1.0 };
        let offset = amplitude * 1.5 * dir * seeded_random(seed + i as f64 * 37.0 + 11.0);
        d.cubic_to(mid_x, p0.1 + offset, mid_x, p1.1 - offset, p1.0, p1.1);
    }
    d
}

/// `generateCliffPath`: the steep S-curve from the centre down to the
/// bottom edge between Chaotic and Clear.
pub fn cliff_path(width: f64, height: f64) -> PathData {
    let cx = width / 2.0;
    let top = height * 0.5;
    let bottom = height;
    let span = bottom - top;
    let amplitude = width * 0.03;
    let mut d = PathData::new();
    d.move_to(cx, top)
        .cubic_to(
            cx + amplitude,
            top + span * 0.2,
            cx - amplitude * 1.5,
            top + span * 0.55,
            cx + amplitude * 0.5,
            top + span * 0.75,
        )
        .cubic_to(
            cx - amplitude,
            top + span * 0.85,
            cx + amplitude * 0.3,
            top + span * 0.95,
            cx,
            bottom,
        );
    d
}

/// `generateConfusionPath`: an ellipse as two arcs.
pub fn confusion_path(cx: f64, cy: f64, rx: f64, ry: f64) -> PathData {
    let mut d = PathData::new();
    d.move_to(cx - rx, cy)
        .arc_to(rx, ry, 0.0, true, true, cx + rx, cy)
        .arc_to(rx, ry, 0.0, true, true, cx - rx, cy)
        .close();
    d
}

/// One entry of `getDomainLayouts`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct DomainLayout {
    cx: f64,
    cy: f64,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// `getDomainLayouts`.
fn domain_layout(name: DomainName, width: f64, height: f64) -> DomainLayout {
    let hw = width / 2.0;
    let hh = height / 2.0;
    let quadrant = |x: f64, y: f64| DomainLayout {
        cx: x + hw / 2.0,
        cy: y + hh / 2.0,
        x,
        y,
        w: hw,
        h: hh,
    };
    match name {
        DomainName::Complex => quadrant(0.0, 0.0),
        DomainName::Complicated => quadrant(hw, 0.0),
        DomainName::Chaotic => quadrant(0.0, hh),
        DomainName::Clear => quadrant(hw, hh),
        DomainName::Confusion => DomainLayout {
            cx: hw,
            cy: hh,
            x: hw * 0.7,
            y: hh * 0.7,
            w: hw * 0.6,
            h: hh * 0.6,
        },
    }
}

/// The four quadrant domains in drawing order.
const QUADRANTS: [DomainName; 4] = [
    DomainName::Complex,
    DomainName::Complicated,
    DomainName::Chaotic,
    DomainName::Clear,
];

/// `DOMAIN_META`: the decision model and practice of each domain.
fn domain_meta(name: DomainName) -> (&'static str, &'static str) {
    match name {
        DomainName::Complex => (
            "Probe \u{2192} Sense \u{2192} Respond",
            "Emergent Practices",
        ),
        DomainName::Complicated => ("Sense \u{2192} Analyse \u{2192} Respond", "Good Practices"),
        DomainName::Clear => (
            "Sense \u{2192} Categorise \u{2192} Respond",
            "Best Practices",
        ),
        DomainName::Chaotic => ("Act \u{2192} Sense \u{2192} Respond", "Novel Practices"),
        DomainName::Confusion => ("", "Disorder"),
    }
}

/// `domainName.charAt(0).toUpperCase() + domainName.slice(1)`.
fn domain_label(name: DomainName) -> String {
    let s = name.as_str();
    let mut out = s[..1].to_uppercase();
    out.push_str(&s[1..]);
    out
}

/// SVG text collapses runs of space, tab, CR, and LF and drops them at the
/// ends before `getBBox` measures it.
fn collapse(text: &str) -> String {
    text.split([' ', '\t', '\n', '\r'])
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

struct Painter<'a> {
    ctx: &'a RenderCtx<'a>,
    styles: &'a CynefinStyles,
}

impl Painter<'_> {
    /// The fill of a domain: the `fill` attribute, or the inherited root
    /// fill when the browser rejects the color.
    fn domain_fill(&self, name: DomainName) -> Color {
        let d = &self.styles.domains;
        match name {
            DomainName::Complex => d.complex,
            DomainName::Complicated => d.complicated,
            DomainName::Clear => d.clear,
            DomainName::Chaotic => d.chaotic,
            DomainName::Confusion => d.confusion,
        }
        .unwrap_or(self.styles.root_text_color)
    }

    /// `textNode.getBBox().width`, or `label.length * 7` when the box is
    /// empty.
    fn badge_text_width(&self, label: &str) -> f64 {
        let spec = MeasureSpec::new(
            self.styles.font_family.clone(),
            self.styles.item_font_size as f32,
        );
        let width = f64::from(self.ctx.metrics.shape(&collapse(label), &spec).bbox().width);
        if width > 0.0 {
            width
        } else {
            label.encode_utf16().count() as f64 * 7.0
        }
    }

    /// One item badge: a rounded rect and its centred label, translated so
    /// the badge is centred on `cx`.
    fn badge(&self, label: &str, cx: f64, y: f64, fill: Color, overflow: bool) -> Element {
        let badge_width = self.badge_text_width(label) + ITEM_PADDING_X * 2.0;
        let opacity = if overflow { 0.6 } else { 0.95 };
        let rect = Element::Rect(Rect {
            common: Common {
                class: Some(
                    if overflow {
                        "cynefinItemOverflow"
                    } else {
                        "cynefinItem"
                    }
                    .to_string(),
                ),
                style: Style {
                    fill: Some(Paint::Color(fill)),
                    fill_opacity: Some(opacity),
                    stroke: self.styles.boundary_color.map(Paint::Color),
                    stroke_width: Some(1.0),
                    stroke_dasharray: overflow.then(|| vec![3.0, 2.0]),
                    ..Style::default()
                },
                ..Common::default()
            },
            x: 0.0,
            y: 0.0,
            width: badge_width,
            height: ITEM_HEIGHT,
            rx: Some(ITEM_RADIUS),
            ry: Some(ITEM_RADIUS),
        });
        let text = text(
            "cynefinItemText",
            (badge_width / 2.0, ITEM_HEIGHT / 2.0),
            Baseline::Central,
            self.styles.text_color,
            font(self.styles.item_font_size, None, None),
            label,
        );
        Element::Group(Group {
            common: Common {
                transform: vec![TransformOp::Translate(cx - badge_width / 2.0, y)],
                ..Common::default()
            },
            children: vec![rect, text],
        })
    }
}

fn font(size: f64, weight: Option<FontWeight>, style: Option<FontStyle>) -> FontSpec {
    FontSpec {
        size: Some(size),
        weight,
        style,
        ..FontSpec::default()
    }
}

/// A `text-anchor="middle"` text.
fn text(
    class: &str,
    at: (f64, f64),
    baseline: Baseline,
    fill: Color,
    font: FontSpec,
    content: &str,
) -> Element {
    Element::Text(Text {
        common: Common {
            class: Some(class.to_string()),
            style: Style {
                fill: Some(Paint::Color(fill)),
                ..Style::default()
            },
            ..Common::default()
        },
        x: at.0,
        y: at.1,
        anchor: Some(TextAnchor::Middle),
        baseline: Some(baseline),
        font,
        runs: vec![TextRun::new(content)],
    })
}

fn group(class: &str, children: Vec<Element>) -> Element {
    Element::Group(Group {
        common: Common {
            class: Some(class.to_string()),
            ..Common::default()
        },
        children,
    })
}

fn path(class: &str, d: PathData, style: Style) -> Element {
    Element::Path(Path {
        common: Common {
            class: Some(class.to_string()),
            style,
            ..Common::default()
        },
        d,
    })
}

/// The `cynefin-arrow-<id>` marker: a triangle in a 10 by 10 box, six
/// stroke widths wide.
fn arrow_marker(styles: &CynefinStyles) -> Element {
    let mut d = PathData::new();
    d.move_to(0.0, 0.0)
        .line_to(10.0, 5.0)
        .line_to(0.0, 10.0)
        .close();
    Element::Marker(Marker {
        id: format!("cynefin-arrow-{SVG_ID}"),
        class: None,
        view_box: Some(ViewBox::new(0.0, 0.0, 10.0, 10.0)),
        ref_x: 9.0,
        ref_y: 5.0,
        marker_width: 6.0,
        marker_height: 6.0,
        orient: Orient::AutoStartReverse,
        marker_units: MarkerUnits::StrokeWidth,
        style: Style::default(),
        children: vec![path(
            "cynefinArrowHead",
            d,
            Style {
                fill: Some(Paint::Color(
                    styles.arrow_color.unwrap_or(styles.root_text_color),
                )),
                ..Style::default()
            },
        )],
    })
}

/// `draw`.
pub fn draw(
    db: &CynefinDb,
    cfg: &CynefinConfig,
    styles: &CynefinStyles,
    ctx: &RenderCtx<'_>,
) -> Scene {
    let painter = Painter { ctx, styles };
    let (width, height, padding) = (cfg.width, cfg.height, cfg.padding);
    let show_descriptions = cfg.show_domain_descriptions;
    let seed = resolve_seed(cfg.seed, SVG_ID);
    let layout = |name| domain_layout(name, width, height);
    let mut root = Vec::new();

    // 1. Domain background rectangles.
    let backgrounds = QUADRANTS
        .iter()
        .map(|&name| {
            let l = layout(name);
            Element::Rect(Rect {
                common: Common {
                    class: Some("cynefinDomain".to_string()),
                    style: Style {
                        fill: Some(Paint::Color(painter.domain_fill(name))),
                        fill_opacity: Some(0.4),
                        ..Style::default()
                    },
                    ..Common::default()
                },
                x: l.x,
                y: l.y,
                width: l.w,
                height: l.h,
                rx: None,
                ry: None,
            })
        })
        .collect();
    root.push(group("cynefin-backgrounds", backgrounds));

    // 2. The wavy boundaries and 3. the cliff.
    let boundary = Style {
        stroke: styles.boundary_color.map(Paint::Color),
        stroke_width: Some(styles.boundary_width),
        stroke_dasharray: Some(vec![6.0, 3.0]),
        fill: Some(Paint::None),
        ..Style::default()
    };
    root.push(group(
        "cynefin-boundaries",
        vec![
            path(
                "cynefinBoundary",
                fold_path(width, height, seed, cfg.boundary_amplitude),
                boundary.clone(),
            ),
            path(
                "cynefinBoundary",
                horizontal_boundary(width, height, seed + 100.0, cfg.boundary_amplitude),
                boundary,
            ),
            path(
                "cynefinCliff",
                cliff_path(width, height),
                Style {
                    stroke: styles.cliff_color.map(Paint::Color),
                    stroke_width: Some(styles.cliff_width),
                    fill: Some(Paint::None),
                    ..Style::default()
                },
            ),
        ],
    ));

    // 4. The confusion ellipse.
    root.push(path(
        "cynefinConfusion",
        confusion_path(width / 2.0, height / 2.0, width * 0.15, height * 0.15),
        Style {
            fill: Some(Paint::Color(painter.domain_fill(DomainName::Confusion))),
            fill_opacity: Some(0.5),
            stroke: styles.boundary_color.map(Paint::Color),
            stroke_width: Some(1.5),
            stroke_dasharray: Some(vec![4.0, 2.0]),
            ..Style::default()
        },
    ));

    // 5. Domain name labels.
    let label_font = || font(styles.domain_font_size, Some(FontWeight::Bold), None);
    let mut labels: Vec<Element> = QUADRANTS
        .iter()
        .map(|&name| {
            let l = layout(name);
            let y = if show_descriptions { l.cy - 30.0 } else { l.cy };
            text(
                "cynefinDomainLabel",
                (l.cx, y),
                Baseline::Middle,
                styles.label_color,
                label_font(),
                &domain_label(name),
            )
        })
        .collect();
    labels.push(text(
        "cynefinDomainLabel",
        (
            width / 2.0,
            if show_descriptions {
                height / 2.0 - 10.0
            } else {
                height / 2.0
            },
        ),
        Baseline::Middle,
        styles.label_color,
        label_font(),
        "Confusion",
    ));
    root.push(group("cynefin-labels", labels));

    // 6. Domain description subtitles.
    if show_descriptions {
        let subtitle = |at: (f64, f64), content: &str| {
            text(
                "cynefinSubtitle",
                at,
                Baseline::Middle,
                styles.text_color,
                font(styles.subtitle_font_size, None, Some(FontStyle::Italic)),
                content,
            )
        };
        let mut subtitles = Vec::new();
        for &name in &QUADRANTS {
            let l = layout(name);
            let (model, practice) = domain_meta(name);
            subtitles.push(subtitle((l.cx, l.cy - 10.0), model));
            subtitles.push(subtitle((l.cx, l.cy + 5.0), practice));
        }
        subtitles.push(subtitle(
            (width / 2.0, height / 2.0 + 8.0),
            domain_meta(DomainName::Confusion).1,
        ));
        root.push(group("cynefin-subtitles", subtitles));
    }

    // 7. Items as badges within each domain.
    let mut items = Vec::new();
    let all_domains = [
        DomainName::Complex,
        DomainName::Complicated,
        DomainName::Chaotic,
        DomainName::Clear,
        DomainName::Confusion,
    ];
    for name in all_domains {
        let Some(domain_items) = db.items(name).filter(|i| !i.is_empty()) else {
            continue;
        };
        let l = layout(name);
        let is_confusion = name == DomainName::Confusion;
        let (shown, overflow) = if is_confusion && domain_items.len() > MAX_CONFUSION_ITEMS {
            (
                &domain_items[..MAX_CONFUSION_ITEMS],
                domain_items.len() - MAX_CONFUSION_ITEMS,
            )
        } else {
            (domain_items, 0)
        };
        let start_y = l.cy
            + match (is_confusion, show_descriptions) {
                (true, true) => 22.0,
                (true, false) => 14.0,
                (false, true) => 25.0,
                (false, false) => 15.0,
            };
        let fill = painter.domain_fill(name);
        for (idx, label) in shown.iter().enumerate() {
            let y = start_y + idx as f64 * (ITEM_HEIGHT + ITEM_GAP);
            items.push(painter.badge(label, l.cx, y, fill, false));
        }
        if overflow > 0 {
            let y = start_y + shown.len() as f64 * (ITEM_HEIGHT + ITEM_GAP);
            items.push(painter.badge(&format!("+{overflow} more"), l.cx, y, fill, true));
        }
    }
    root.push(group("cynefin-items", items));

    // 8. Transition arrows between domain centres.
    let mut defs = Vec::new();
    if !db.transitions().is_empty() {
        defs.push(arrow_marker(styles));
        let mut arrows = Vec::new();
        for t in db.transitions() {
            let from = layout(t.from);
            let to = layout(t.to);
            let (x1, y1, x2, y2) = (from.cx, from.cy, to.cx, to.cy);
            // A quadratic Bézier bent off the chord by 15 % of its length.
            let (mx, my) = ((x1 + x2) / 2.0, (y1 + y2) / 2.0);
            let (dx, dy) = (x2 - x1, y2 - y1);
            let len = (dx * dx + dy * dy).sqrt();
            let offset = len * 0.15;
            let (nx, ny) = (-dy / len, dx / len);
            let (cpx, cpy) = (mx + nx * offset, my + ny * offset);
            let mut d = PathData::new();
            d.move_to(x1, y1).quad_to(cpx, cpy, x2, y2);
            arrows.push(path(
                "cynefinArrowLine",
                d,
                Style {
                    stroke: styles.arrow_color.map(Paint::Color),
                    stroke_width: Some(styles.arrow_width),
                    fill: Some(Paint::None),
                    marker_end: Some(format!("cynefin-arrow-{SVG_ID}")),
                    ..Style::default()
                },
            ));
            if let Some(label) = &t.label {
                arrows.push(text(
                    "cynefinArrowLabel",
                    (cpx, cpy - 6.0),
                    Baseline::Auto,
                    styles.text_color,
                    font(styles.subtitle_font_size, None, None),
                    label,
                ));
            }
        }
        root.push(group("cynefin-arrows", arrows));
    }

    // The title, centred in the top padding.
    let title = db.diagram_title();
    if !title.is_empty() {
        root.push(text(
            "cynefinTitle",
            (width / 2.0, -padding / 2.0),
            Baseline::Middle,
            styles.label_color,
            font(styles.title_font_size, Some(FontWeight::Bold), None),
            title,
        ));
    }

    // `configureSvgSize` with the padded size, and the same `viewBox`.
    let total_width = width + padding * 2.0;
    let total_height = height + padding * 2.0;
    let mut scene = Scene::new(total_width, total_height);
    scene.view_box = ViewBox::new(0.0, 0.0, total_width, total_height);
    scene.font_family = styles.font_family.clone();
    scene.defs = defs;
    scene.root.children.push(Element::Group(Group {
        common: Common {
            transform: vec![TransformOp::Translate(padding, padding)],
            ..Common::default()
        },
        children: root,
    }));
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The `d` attribute as the oracle writes it: JS number printing, a
    /// space before each command.
    fn js_path(d: &PathData) -> String {
        use crate::mermaid::scene::path::Command;
        use crate::mermaid::theme::khroma::js_number as n;
        let mut out = Vec::new();
        for c in d.commands() {
            out.push(match *c {
                Command::MoveTo(p) => format!("M{},{}", n(p.x), n(p.y)),
                Command::CubicTo { c1, c2, to } => format!(
                    "C{},{} {},{} {},{}",
                    n(c1.x),
                    n(c1.y),
                    n(c2.x),
                    n(c2.y),
                    n(to.x),
                    n(to.y)
                ),
                ref other => format!("{other:?}"),
            });
        }
        out.join(" ")
    }

    #[test]
    fn the_prng_and_hash_match_js() {
        assert_eq!(hash_string("my-svg"), -1_061_188_925);
        assert_eq!(hash_string(""), 0);
        // mulberry32 values from node.
        assert_eq!(seeded_random(0.0), 0.26642920868471265);
        assert_eq!(seeded_random(42.0), 0.6011037519201636);
        assert_eq!(seeded_random(-50.0), 0.2312965493183583);
        assert_eq!(seeded_random(1.5), 0.6270739405881613);
        // The `| 0` wraps a sum past 2^31.
        assert_eq!(seeded_random(-1_068_255_563.0 + 3e9), 0.9264082338195294);
    }

    #[test]
    fn boundaries_match_the_oracle_paths() {
        let seed = resolve_seed(Some(0.0), SVG_ID);
        let fold = js_path(&fold_path(800.0, 600.0, seed, 8.0));
        assert!(
            fold.starts_with(
                "M404.8574703261256,0 C415.3262379048392,42.857142857142854 383.91018708143383,42.857142857142854 394.37895466014743,85.71428571428571"
            ),
            "{fold}"
        );
        assert!(fold.ends_with(" 405.5261822901666,600"), "{fold}");
        let horizontal = js_path(&horizontal_boundary(800.0, 600.0, seed + 100.0, 8.0));
        assert!(
            horizontal.starts_with(
                "M0,303.14857986569405 C57.142857142857146,315.1003727382049 57.142857142857146,280.0498515153304 114.28571428571429,292.0016443878412"
            ),
            "{horizontal}"
        );
        assert!(
            horizontal.ends_with(" 800,298.6103892251849"),
            "{horizontal}"
        );
        assert_eq!(
            js_path(&cliff_path(800.0, 600.0)),
            "M400,300 C424,360 364,465 412,525 C376,555 407.2,585 400,600"
        );
    }

    #[test]
    fn a_configured_seed_wins_over_the_id() {
        assert_eq!(resolve_seed(Some(42.0), SVG_ID), 42.0);
        assert_eq!(resolve_seed(None, SVG_ID), -1_061_188_925.0);
        assert_eq!(resolve_seed(Some(f64::NAN), SVG_ID), -1_061_188_925.0);
    }

    #[test]
    fn config_reads_the_section_with_defaults() {
        let cfg = CynefinConfig::from_raw(&json!({ "cynefin": { "width": 1000, "seed": 7 } }));
        assert_eq!(cfg.width, 1000.0);
        assert_eq!(cfg.height, 600.0);
        assert_eq!(cfg.seed, Some(7.0));
        assert!(cfg.show_domain_descriptions);
        let cfg = CynefinConfig::from_raw(&json!({}));
        assert_eq!((cfg.padding, cfg.boundary_amplitude), (40.0, 8.0));
    }

    #[test]
    fn labels_are_capitalised() {
        assert_eq!(domain_label(DomainName::Complicated), "Complicated");
        assert_eq!(collapse("  a \t b\n"), "a b");
    }
}
