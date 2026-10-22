//! Port of `kanbanRenderer.ts` `draw` and the `kanbanSection` cluster of
//! `rendering-elements/clusters.js` (mermaid 12.2.0).
//!
//! The renderer lays the sections out as columns `sectionWidth` wide with
//! a 5 px gap, stacks each section's cards from a common top with a 5 px
//! gap, sizes the section rect to its cards, and frames the whole with
//! `setupGraphViewbox` and the mindmap padding. The cards are the
//! `kanbanItem` shape through `shapes::insert_node`.
//!
//! Labels are HTML in the oracle. `draw` sets `conf.htmlLabels = false` on
//! a copy (`getConfig()` returns `assignWithDepth({}, currentConfig)`),
//! so `labelHelper`, `insertLabel`, and `kanbanSection`, which all call
//! `getConfig()` again, measure `foreignObject` labels with `line-height
//! 1.5`. The scene therefore uses the shapes' HTML label mode.

use serde_json::Value;

use super::db::{Item, KanbanDb, Section};
use super::measure::Measurer;
use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::scene::{
    Baseline, Color, Common, Element, Group, Length, Scene, Style, TextAnchor, ViewBox,
};
use crate::mermaid::shapes::util::{build_label_group, rect_element, translate};
use crate::mermaid::shapes::{
    self, KanbanData, LabelBox, LabelMeasurer, LabelOpts, Node, NodeExtra, ShapeCtx,
};
use crate::mermaid::styles::kanban::KanbanStyles;
use crate::text::FontSpec as MeasureSpec;
use crate::text::measure::HTML_LINE_HEIGHT;

/// mermaid-cli's SVG id; `draw` prefixes every `domId` with it.
const SVG_ID: &str = "my-svg";
/// `const padding = 10` in `draw`: the column gap is half of it, the card
/// gap half of it, and the section rect keeps three of it below the cards.
const PADDING: f64 = 10.0;
/// `let maxLabelHeight = 25`: the room above the first card.
const MIN_LABEL_HEIGHT: f64 = 25.0;
/// `Math.max(..., 50)`: the shortest section rect.
const MIN_SECTION_HEIGHT: f64 = 50.0;
/// `kanbanItem`: `kanbanNode.width = (width ?? 200) - 10` for the labels,
/// `padding = kanbanNode.padding || 10`, `labelPaddingX = 10`.
const ITEM_LABEL_INSET: f64 = 10.0;
const ITEM_PADDING: f64 = 10.0;
const ITEM_LABEL_PADDING_X: f64 = 10.0;

/// `svgElem.getBBox()`: the union of the painted boxes, stroke excluded.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bounds {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Bounds {
    fn empty() -> Option<Bounds> {
        None
    }

    fn add(acc: &mut Option<Bounds>, x: f64, y: f64, width: f64, height: f64) {
        let next = Bounds {
            x0: x,
            y0: y,
            x1: x + width,
            y1: y + height,
        };
        *acc = Some(match *acc {
            None => next,
            Some(b) => Bounds {
                x0: b.x0.min(next.x0),
                y0: b.y0.min(next.y0),
                x1: b.x1.max(next.x1),
                y1: b.y1.max(next.y1),
            },
        });
    }
}

/// `conf?.kanban?.sectionWidth || 200`.
fn section_width(resolved: &Resolved) -> f64 {
    resolved
        .config
        .kanban
        .get("sectionWidth")
        .and_then(Value::as_f64)
        .filter(|w| *w != 0.0 && !w.is_nan())
        .unwrap_or(200.0)
}

/// `config?.kanban?.ticketBaseUrl` when truthy.
fn ticket_base_url(resolved: &Resolved) -> Option<String> {
    resolved
        .config
        .kanban
        .get("ticketBaseUrl")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// `conf.mindmap?.padding ?? defaultConfig.kanban.padding`: the renderer
/// reads the mindmap section, not its own.
fn diagram_padding(resolved: &Resolved) -> f64 {
    resolved
        .config
        .mindmap
        .get("padding")
        .and_then(Value::as_f64)
        .unwrap_or(8.0)
}

fn html_opts(ctx: &ShapeCtx, width: f64) -> LabelOpts {
    LabelOpts {
        font: ctx.font(),
        html: ctx.html_labels,
        width: Some(width),
        min_width: 0.0,
        markdown: true,
    }
}

/// One placed label of a card: its measured box and the top-left corner of
/// its `foreignObject` in the card's own coordinates.
struct PlacedLabel {
    bbox: LabelBox,
    x: f64,
    y: f64,
}

/// A card's three labels, placed as `kanbanItem` places them.
struct CardLabels {
    title: PlacedLabel,
    ticket: PlacedLabel,
    assigned: PlacedLabel,
}

impl CardLabels {
    fn measure(item: &Item, node_width: f64, measurer: &dyn LabelMeasurer, ctx: &ShapeCtx) -> Self {
        let opts = html_opts(ctx, node_width - ITEM_LABEL_INSET);
        let title = measurer.measure(&item.label, &opts);
        let ticket = measurer.measure(item.ticket.as_deref().unwrap_or(""), &opts);
        let assigned = measurer.measure(item.assigned.as_deref().unwrap_or(""), &opts);
        let total_width = node_width;
        let height_adj = ticket.height.max(assigned.height) / 2.0;
        let left = ITEM_PADDING - total_width / 2.0;
        let below_title = -height_adj + title.height / 2.0;
        let title_y = -height_adj - title.height / 2.0;
        let assigned_x =
            ITEM_PADDING + total_width / 2.0 - assigned.width - 2.0 * ITEM_LABEL_PADDING_X;
        CardLabels {
            title: PlacedLabel {
                bbox: title,
                x: left,
                y: title_y,
            },
            ticket: PlacedLabel {
                bbox: ticket,
                x: left,
                y: below_title,
            },
            assigned: PlacedLabel {
                bbox: assigned,
                x: assigned_x,
                y: below_title,
            },
        }
    }

    /// The `foreignObject` boxes for the diagram bounds. A label with no
    /// text has no box.
    fn boxes(&self) -> impl Iterator<Item = (f64, f64, f64, f64)> + '_ {
        [&self.title, &self.ticket, &self.assigned]
            .into_iter()
            .filter(|l| !l.bbox.is_empty())
            .map(|l| (l.x, l.y, l.bbox.width, l.bbox.height))
    }
}

/// `.kanban-ticket-link { text-decoration: underline }` on the `<a>` that
/// wraps the ticket label when `ticketBaseUrl` is set. The decoration
/// propagates into the HTML label in the `<a>`'s own color, black, as a
/// 1 px line one pixel below each line's baseline; both values are read
/// off the oracle renders, where Blink keeps a 1 px gap under the baseline
/// (`ComputeUnderlineOffsetAuto`). The scene has no link, so the line
/// stands in for the decoration.
fn ticket_underlines(ticket: &PlacedLabel, baseline: f64) -> Vec<Element> {
    const THICKNESS: f64 = 1.0;
    const GAP: f64 = 1.0;
    ticket
        .bbox
        .lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.width > 0.0)
        .map(|(i, line)| {
            let y = ticket.y + i as f64 * ticket.bbox.line_height + baseline + GAP;
            rect_element(
                ticket.x,
                y,
                line.width,
                THICKNESS,
                None,
                None,
                "kanban-ticket-link",
                Style::fill(Color::BLACK),
            )
        })
        .collect()
}

/// `getData()`'s `kanbanItem` node for one card.
fn item_node(item: &Item, x: f64, width: f64) -> Node {
    Node {
        id: item.id.clone(),
        dom_id: Some(format!("{SVG_ID}-{}", item.id)),
        label: Some(item.label.clone()),
        label_type: Some("markdown".to_string()),
        shape: "kanbanItem".to_string(),
        rx: Some(5.0),
        ry: Some(5.0),
        css_styles: vec!["text-align: left".to_string()],
        width: Some(width),
        x,
        extra: NodeExtra::Kanban(KanbanData {
            ticket: item.ticket.clone(),
            assigned: item.assigned.clone(),
            priority: item.priority.clone(),
        }),
        ..Node::default()
    }
}

/// `cssStyles: ['text-align: left']` on every card: the style lands on the
/// label span, and the `<p>` that `markdownToHTML` wraps the text in
/// inherits it, so a wrapped card line starts at the label's left edge
/// while the shared label builder centres HTML lines in their box.
fn left_align_labels(group: &mut Group) {
    for child in &mut group.children {
        match child {
            Element::Group(g) => left_align_labels(g),
            Element::Text(text) => {
                text.x = 0.0;
                text.anchor = Some(TextAnchor::Start);
                for run in &mut text.runs {
                    if run.x.is_some() {
                        run.x = Some(0.0);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Where Chrome puts the first baseline of an HTML label line, measured
/// from the top of its line box: the rounded ascent plus half the leading
/// that `line-height: 1.5` (`addHtmlSpan` in `createText.ts`) adds around
/// the rounded ascent and descent.
fn html_baseline(ctx: &RenderCtx<'_>, family: &str, size: f64) -> f64 {
    let spec = MeasureSpec::new(family.to_string(), size as f32);
    let shaped = ctx.metrics.shape("x", &spec);
    let ascent = f64::from(shaped.ascent);
    let descent = f64::from(shaped.descent);
    let line_height = f64::from(HTML_LINE_HEIGHT) * size;
    // The oracle puts the baseline on a whole pixel: an odd leading of 5
    // px at 16 px Trebuchet MS gives 2 px above, not 2.5.
    ((line_height - ascent - descent) / 2.0).floor() + ascent
}

/// The shared label builder centres an HTML line with `dominant-baseline:
/// central`, which resvg places from the unrounded font metrics, 1 px
/// below where Chrome draws it. This pass puts the first baseline at
/// `baseline` below the box top on an alphabetic baseline; later lines
/// keep their `dy` of one line height.
fn place_html_baselines(group: &mut Group, baseline: f64) {
    for child in &mut group.children {
        match child {
            Element::Group(g) => place_html_baselines(g, baseline),
            Element::Text(text) if text.baseline == Some(Baseline::Central) => {
                text.baseline = Some(Baseline::Alphabetic);
                if let Some(first) = text.runs.first_mut() {
                    first.dy = Some(Length::Px(baseline));
                }
            }
            _ => {}
        }
    }
}

/// A measured section: what the first loop of `draw` keeps for the second.
struct Column<'s> {
    section: &'s Section,
    /// 1-based, the `section-N` class and the column index.
    cnt: usize,
    x: f64,
    label_bbox: LabelBox,
}

pub fn draw(db: &KanbanDb, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
    let width = section_width(resolved);
    let styles = KanbanStyles::from_theme(&resolved.theme_variables);
    let measurer = Measurer::new(&ctx.metrics, styles.font_family.clone());
    let mut shape_ctx = ShapeCtx::new(&measurer);
    shape_ctx.vars = styles.node_vars();
    shape_ctx.kanban_ticket_base_url = ticket_base_url(resolved);
    let baseline = html_baseline(ctx, &styles.font_family, styles.font_size);

    let sections = db.get_data();
    // Pass one: `insertCluster` for every section, which measures the
    // title and raises `maxLabelHeight`.
    let mut max_label_height = MIN_LABEL_HEIGHT;
    let mut columns: Vec<Column<'_>> = Vec::with_capacity(sections.len());
    for (i, section) in sections.iter().enumerate() {
        let cnt = i + 1;
        let x = width * cnt as f64 + ((cnt - 1) as f64 * PADDING) / 2.0;
        // `kanbanSection`: `createText(labelEl, node.label, { useHtmlLabels,
        // isNode: true, width: node.width })`.
        let label_bbox = measurer.measure(&section.label, &html_opts(&shape_ctx, width));
        max_label_height = max_label_height.max(label_bbox.height);
        columns.push(Column {
            section,
            cnt,
            x,
            label_bbox,
        });
    }

    // Pass two: the cards, then the section rect sized to them.
    let section_height = width * 3.0;
    let top = -section_height / 2.0 + max_label_height;
    let item_width = width - 1.5 * PADDING;
    let mut bounds = Bounds::empty();
    let mut section_elements = Vec::with_capacity(columns.len());
    let mut item_elements = Vec::new();
    for column in &columns {
        let mut y = top;
        for item in &column.section.items {
            let mut node = item_node(item, column.x, item_width);
            let out = shapes::insert_node(&mut node, &shape_ctx)?;
            // `nodeEl.getBBox().height`: the card rect, which the labels
            // never leave vertically.
            let height = out.height;
            node.y = y + height / 2.0;
            let mut group = out.group;
            left_align_labels(&mut group);
            group.common.transform = translate(node.x, node.y);
            Bounds::add(
                &mut bounds,
                node.x - out.width / 2.0,
                node.y - height / 2.0,
                out.width,
                height,
            );
            let labels = CardLabels::measure(item, item_width, &measurer, &shape_ctx);
            for (lx, ly, lw, lh) in labels.boxes() {
                Bounds::add(&mut bounds, node.x + lx, node.y + ly, lw, lh);
            }
            if shape_ctx.kanban_ticket_base_url.is_some() {
                group
                    .children
                    .extend(ticket_underlines(&labels.ticket, baseline));
            }
            item_elements.push(Element::Group(group));
            y = node.y + height / 2.0 + PADDING / 2.0;
        }

        let rect_height = (y - top + 3.0 * PADDING).max(MIN_SECTION_HEIGHT)
            + (max_label_height - MIN_LABEL_HEIGHT);
        let rect_x = column.x - width / 2.0;
        let rect_y = -section_height / 2.0;
        let rect = rect_element(
            rect_x,
            rect_y,
            width,
            rect_height,
            Some(5.0),
            Some(5.0),
            "",
            styles.section_rect(column.cnt),
        );
        Bounds::add(&mut bounds, rect_x, rect_y, width, rect_height);
        // `labelEl.attr('transform', translate(node.x - bbox.width / 2,
        // node.y - node.height / 2 + subGraphTitleTopMargin))` with the
        // flowchart `subGraphTitleMargin.top` of 0.
        let label_x = column.x - column.label_bbox.width / 2.0;
        let label = build_label_group(
            &column.label_bbox,
            &shape_ctx,
            styles.text_color,
            "cluster-label",
            translate(label_x, rect_y),
        );
        if !column.label_bbox.is_empty() {
            Bounds::add(
                &mut bounds,
                label_x,
                rect_y,
                column.label_bbox.width,
                column.label_bbox.height,
            );
        }
        section_elements.push(Element::Group(Group {
            common: Common {
                id: Some(format!("{SVG_ID}-{}", column.section.id)),
                class: Some(format!("cluster section-{}", column.cnt)),
                ..Common::default()
            },
            children: vec![rect, Element::Group(label)],
        }));
    }

    // `setupGraphViewbox(undefined, svg, mindmap.padding, mindmap.useMaxWidth)`.
    let padding = diagram_padding(resolved);
    let b = bounds.unwrap_or(Bounds {
        x0: 0.0,
        y0: 0.0,
        x1: 0.0,
        y1: 0.0,
    });
    let scene_width = (b.x1 - b.x0) + padding * 2.0;
    let scene_height = (b.y1 - b.y0) + padding * 2.0;
    let mut root = Group {
        common: Common::default(),
        children: vec![
            Element::Group(Group {
                common: Common {
                    class: Some("sections".to_string()),
                    ..Common::default()
                },
                children: section_elements,
            }),
            Element::Group(Group {
                common: Common {
                    class: Some("items".to_string()),
                    ..Common::default()
                },
                children: item_elements,
            }),
        ],
    };
    place_html_baselines(&mut root, baseline);
    Ok(Scene {
        width: scene_width,
        height: scene_height,
        view_box: ViewBox::new(b.x0 - padding, b.y0 - padding, scene_width, scene_height),
        background: None,
        font_family: styles.font_family.clone(),
        defs: Vec::new(),
        root,
    })
}
