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
use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::scene::{Common, Element, Group, Scene, ViewBox};
use crate::mermaid::shapes::util::{build_label_group, rect_element, translate};
use crate::mermaid::shapes::{
    self, KanbanData, LabelBox, LabelMeasurer, LabelOpts, Node, NodeExtra, ShapeCtx,
};
use crate::mermaid::styles::kanban::KanbanStyles;

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

/// `conf.mindmap?.useMaxWidth ?? defaultConfig.kanban.useMaxWidth`, the
/// mindmap section again.
fn use_max_width(resolved: &Resolved) -> bool {
    resolved
        .config
        .mindmap
        .get("useMaxWidth")
        .and_then(Value::as_bool)
        .unwrap_or(true)
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
        Self::place(title, ticket, assigned, node_width)
    }

    fn place(title: LabelBox, ticket: LabelBox, assigned: LabelBox, node_width: f64) -> Self {
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

    /// `nodeEl.getBBox().height`: the card rect united with its label
    /// boxes. The rect is `title + 20 + heightAdj` tall around 0, but the
    /// title starts at `-heightAdj - title / 2` and the metadata ends at
    /// `title / 2 + heightAdj`, so a ticket or assignee label taller than
    /// 40 px (two HTML lines) leaves the rect by `heightAdj / 2 - 10` at
    /// the top and the bottom. `draw` stacks the cards by this height.
    fn group_height(&self, rect_height: f64) -> f64 {
        let mut y0 = -rect_height / 2.0;
        let mut y1 = rect_height / 2.0;
        for (_, ly, _, lh) in self.boxes() {
            y0 = y0.min(ly);
            y1 = y1.max(ly + lh);
        }
        y1 - y0
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
    let measurer: &dyn LabelMeasurer = &ctx.metrics;
    let mut shape_ctx = ShapeCtx::new(measurer);
    shape_ctx.vars = styles.node_vars();
    shape_ctx.kanban_ticket_base_url = ticket_base_url(resolved);

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
            let labels = CardLabels::measure(item, item_width, measurer, &shape_ctx);
            let height = labels.group_height(out.height);
            node.y = y + height / 2.0;
            let mut group = out.group;
            group.common.transform = translate(node.x, node.y);
            Bounds::add(
                &mut bounds,
                node.x - out.width / 2.0,
                node.y - out.height / 2.0,
                out.width,
                out.height,
            );
            for (lx, ly, lw, lh) in labels.boxes() {
                Bounds::add(&mut bounds, node.x + lx, node.y + ly, lw, lh);
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
    let mut scene_width = (b.x1 - b.x0) + padding * 2.0;
    let mut scene_height = (b.y1 - b.y0) + padding * 2.0;
    // A fixed-size SVG (`useMaxWidth: false`) is drawn by the browser at
    // scale 1 and the screenshot clip rounds its size up to whole pixels,
    // so the fraction is page background on the right and bottom. Growing
    // the frame to whole pixels from the same origin gives the raster that
    // picture; a `useMaxWidth` SVG is scaled to its clip width instead.
    if !use_max_width(resolved) {
        scene_width = scene_width.ceil();
        scene_height = scene_height.ceil();
    }
    let root = Group {
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
    Ok(Scene {
        width: scene_width,
        height: scene_height,
        view_box: ViewBox::new(b.x0 - padding, b.y0 - padding, scene_width, scene_height),
        background: None,
        font_family: styles.font_family.clone(),
        defs: Vec::new(),
        linear_gradients: Vec::new(),
        root,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::shapes::{LabelLine, LabelRun};

    /// A one-run label box of the given size, as the HTML measurer returns.
    fn label(width: f64, height: f64) -> LabelBox {
        LabelBox {
            width,
            height,
            lines: vec![LabelLine {
                runs: vec![LabelRun::plain("x")],
                width,
            }],
            ..LabelBox::default()
        }
    }

    /// `kanbanItem`'s rect height: `title + 20 + heightAdj`.
    fn rect_height(title: f64, ticket: f64, assigned: f64) -> f64 {
        title + 20.0 + ticket.max(assigned) / 2.0
    }

    #[test]
    fn one_line_metadata_stays_inside_the_card() {
        for (title, ticket, assigned) in [(24.0, 24.0, 24.0), (48.0, 0.0, 24.0), (72.0, 24.0, 0.0)]
        {
            let labels = CardLabels::place(
                label(80.0, title),
                label(60.0, ticket),
                label(60.0, assigned),
                185.0,
            );
            let rect = rect_height(title, ticket, assigned);
            assert_eq!(
                labels.group_height(rect),
                rect,
                "{title} {ticket} {assigned}"
            );
        }
    }

    /// The oracle's `overflow_long_assignee` second card: a two-line title
    /// and a two-line ticket give a 92 px rect whose labels span -48 to 48.
    /// The labels pass the rect by `heightAdj / 2 - 10` at each end.
    #[test]
    fn wrapped_metadata_grows_the_stacking_height() {
        for (ticket, assigned, extra) in [
            (48.0, 24.0, 4.0),
            (24.0, 48.0, 4.0),
            (72.0, 0.0, 16.0),
            (48.0, 72.0, 16.0),
        ] {
            let labels = CardLabels::place(
                label(175.0, 48.0),
                label(175.0, ticket),
                label(195.8, assigned),
                185.0,
            );
            let rect = rect_height(48.0, ticket, assigned);
            assert_eq!(
                labels.group_height(rect),
                rect + extra,
                "{ticket} {assigned}"
            );
        }
    }
}
