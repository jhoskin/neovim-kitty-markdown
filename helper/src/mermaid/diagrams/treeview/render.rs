//! Port of `diagrams/treeView/renderer.ts` (`draw`, `drawTree`,
//! `positionLabel`, `positionLine`), mermaid 12.2.0.
//!
//! Every node, the root `/` included, is one row: its label sits
//! `paddingX` in from the row's indent (`depth * (rowIndent + paddingX)`),
//! after a 14 px icon and a 4 px gap when the node has one, and the row is
//! the label's `getBBox()` height plus `paddingY` above and below. A short
//! horizontal line leads into each row, and a parent with children gets a
//! vertical line from the bottom of its row to the middle of its last
//! child's row. Descriptions line up 16 px right of the widest label, and a
//! `highlight` node gets a rounded background as wide as the tree. The
//! viewBox is `-lineThickness / 2 0 totalWidth totalHeight`, so the natural
//! size is the tree's width and height. The diagram title is not drawn.

use super::db::{BUILTIN_PACK, Node, NodeType, TreeViewConfig, TreeViewDb, node_icon};
use crate::error::HelperError;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::scene::{
    Baseline, Common, Element, FontSpec, FontStyle, FontWeight, Group, Line, Paint, Path, Rect,
    Scene, Style, Text, TextRun, TransformOp, ViewBox, XmlSpace,
};
use crate::mermaid::shapes::util::parse_svg_path;
use crate::mermaid::styles::treeview::TreeViewStyles;
use crate::text::measure::{FontSpec as MeasureSpec, FontStyle as MeasureStyle};

/// `ICON_SIZE`, `ICON_GAP`, `DESC_GAP`.
const ICON_SIZE: f64 = 14.0;
const ICON_GAP: f64 = 4.0;
const DESC_GAP: f64 = 16.0;
/// The iconify box of the built-in pack (`height: 24, width: 24`).
const ICON_VIEW_BOX: f64 = 24.0;

/// The bodies of `treeViewIcons` in `icons.ts`, rewritten in absolute
/// commands for the scene's path parser. The file icon's fold is a second
/// subpath that winds against the outline, so the `evenodd` fill rule of
/// the original and the default `nonzero` paint the same.
const FOLDER_PATH: &str = "M10.59 4.59 A2 2 0 0 0 9.17 4 L4 4 A2 2 0 0 0 2 6 L2 18 A2 2 0 0 0 4 20 L20 20 A2 2 0 0 0 22 18 L22 8 A2 2 0 0 0 20 6 L12.83 6 Z";
const FILE_PATH: &str = "M6 2 A2 2 0 0 0 4 4 L4 20 A2 2 0 0 0 6 22 L18 22 A2 2 0 0 0 20 20 L20 8.83 A2 2 0 0 0 19.41 7.41 L14.59 2.59 A2 2 0 0 0 13.17 2 L6 2 Z M13.5 3.9 L18.1 8.5 L14.5 8.5 A1 1 0 0 1 13.5 7.5 L13.5 3.9 Z";

/// `node.BBox`.
#[derive(Debug, Clone, Copy, Default)]
struct BBox {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// `RenderInfo`: the node, the index of its group among the tree's
/// elements, the right edge of its label, and the middle of its row.
struct RenderInfo {
    node: usize,
    group: usize,
    label_right_edge: f64,
    center_y: f64,
}

/// The built-in icon an icon reference names, or an error for any other
/// pack: icon packs are not supported (R-112), and an unknown name in the
/// built-in pack would draw iconify's question mark.
pub fn builtin_icon(reference: &str) -> Result<&'static str, HelperError> {
    let unsupported = || HelperError::UnsupportedFeature {
        feature: "icon pack".to_string(),
    };
    let (pack, name) = reference.split_once(':').ok_or_else(unsupported)?;
    if pack != BUILTIN_PACK {
        return Err(unsupported());
    }
    match name {
        "folder" => Ok(FOLDER_PATH),
        "file" => Ok(FILE_PATH),
        _ => Err(unsupported()),
    }
}

struct Painter<'a> {
    ctx: &'a RenderCtx<'a>,
    cfg: &'a TreeViewConfig,
    styles: &'a TreeViewStyles,
    db: &'a TreeViewDb,
    elements: Vec<Element>,
    infos: Vec<RenderInfo>,
    boxes: Vec<BBox>,
    total_height: f64,
    total_width: f64,
}

impl Painter<'_> {
    fn measure_spec(&self, bold: bool, italic: bool) -> MeasureSpec {
        let spec = MeasureSpec::new(
            self.styles.font_family.as_str(),
            self.styles.label_font_size as f32,
        )
        .weight(if bold { 700 } else { 400 });
        if italic {
            spec.style(MeasureStyle::Italic)
        } else {
            spec
        }
    }

    /// `text.node().getBBox()` width and height; an empty text has an
    /// empty box.
    fn bbox(&self, text: &str, spec: &MeasureSpec) -> (f64, f64) {
        if text.is_empty() {
            return (0.0, 0.0);
        }
        let size = self.ctx.metrics.shape(text, spec).bbox();
        (f64::from(size.width), f64::from(size.height))
    }

    fn text(
        &self,
        class: &str,
        x: f64,
        y: f64,
        content: &str,
        font: FontSpec,
        fill: Paint,
    ) -> Element {
        Element::Text(Text {
            common: Common {
                class: Some(class.to_string()),
                style: Style {
                    fill: Some(fill),
                    // The stylesheet's `white-space: pre` on the labels
                    // keeps every space, where SVG's default would drop
                    // the leading and trailing ones and collapse runs.
                    xml_space: Some(XmlSpace::Preserve),
                    ..Style::default()
                },
                ..Common::default()
            },
            x,
            y,
            anchor: None,
            baseline: Some(Baseline::Middle),
            font,
            runs: vec![TextRun::new(content)],
        })
    }

    /// `positionLine`.
    fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        let style = Style {
            stroke: self.styles.line_color.map(Paint::Color),
            stroke_width: Some(self.cfg.line_thickness),
            ..Style::default()
        };
        self.elements.push(Element::Line(Line {
            common: Common {
                class: Some("treeView-node-line".to_string()),
                style,
                ..Common::default()
            },
            x1,
            y1,
            x2,
            y2,
        }));
    }

    /// `positionLabel`.
    fn position_label(&mut self, x: f64, y: f64, id: usize) -> Result<(), HelperError> {
        let db = self.db;
        let node: &Node = &db.nodes[id];
        let cfg = self.cfg;
        let mut class = "treeView-node-label".to_string();
        let directory = node.node_type == NodeType::Directory;
        if directory {
            class.push_str(" treeView-node-dir");
        }
        if let Some(css) = &node.css_class {
            class.push(' ');
            class.push_str(css);
        }
        let mut group = Group::default();
        let icon_offset = ICON_SIZE + ICON_GAP;
        let icon = node_icon(node, cfg);
        if let Some(reference) = &icon {
            let d = parse_svg_path(builtin_icon(reference)?)
                .ok_or_else(|| HelperError::internal("bad treeView icon path"))?;
            let scale = ICON_SIZE / ICON_VIEW_BOX;
            group.children.push(Element::Group(Group {
                common: Common {
                    class: Some("treeView-node-icon".to_string()),
                    transform: vec![
                        TransformOp::Translate(x + cfg.padding_x, y + cfg.padding_y),
                        TransformOp::Scale(scale, scale),
                    ],
                    style: Style::fill(self.styles.icon_color),
                    ..Common::default()
                },
                children: vec![Element::Path(Path {
                    common: Common::default(),
                    d,
                })],
            }));
        }
        let show_icon = icon.is_some();

        let spec = self.measure_spec(directory, false);
        let (label_width, label_height) = self.bbox(&node.name, &spec);
        let height = label_height + cfg.padding_y * 2.0;
        let label_x = x + cfg.padding_x + if show_icon { icon_offset } else { 0.0 };
        let font = FontSpec {
            size: Some(self.styles.label_font_size),
            weight: directory.then_some(FontWeight::Bold),
            ..FontSpec::default()
        };
        group.children.push(self.text(
            &class,
            label_x,
            y + height / 2.0,
            &node.name,
            font,
            Paint::Color(self.styles.label_color),
        ));
        let width = label_width + cfg.padding_x * 2.0 + if show_icon { icon_offset } else { 0.0 };
        self.boxes[id] = BBox {
            x,
            y,
            width,
            height,
        };
        self.elements.push(Element::Group(group));
        self.infos.push(RenderInfo {
            node: id,
            group: self.elements.len() - 1,
            label_right_edge: label_x + label_width,
            center_y: y + height / 2.0,
        });
        Ok(())
    }

    /// `drawNode`.
    fn draw_node(&mut self, id: usize, depth: usize) -> Result<(), HelperError> {
        let cfg = self.cfg;
        let indent = depth as f64 * (cfg.row_indent + cfg.padding_x);
        self.position_label(indent, self.total_height, id)?;
        let BBox { height, width, .. } = self.boxes[id];
        let mid = self.total_height + height / 2.0;
        self.line(indent - cfg.row_indent, mid, indent, mid);
        self.total_width = self.total_width.max(indent + width);
        self.total_height += height;
        Ok(())
    }

    /// `processNode`.
    fn process_node(&mut self, id: usize, depth: usize) -> Result<(), HelperError> {
        self.draw_node(id, depth)?;
        let db = self.db;
        let children = &db.nodes[id].children;
        for &child in children {
            self.process_node(child, depth + 1)?;
        }
        if let Some(&last) = children.last() {
            let BBox { x, y, height, .. } = self.boxes[id];
            let end = self.boxes[last];
            let cfg = self.cfg;
            self.line(
                x + cfg.padding_x,
                y + height,
                x + cfg.padding_x,
                end.y + end.height / 2.0 + cfg.line_thickness / 2.0,
            );
        }
        Ok(())
    }

    /// Phase 2: descriptions at a common column.
    fn descriptions(&mut self) {
        let with_desc: Vec<usize> = (0..self.infos.len())
            .filter(|i| self.db.nodes[self.infos[*i].node].description.is_some())
            .collect();
        if with_desc.is_empty() {
            return;
        }
        let max_label_right = self
            .infos
            .iter()
            .map(|ri| ri.label_right_edge)
            .fold(f64::NEG_INFINITY, f64::max);
        let desc_x = max_label_right + DESC_GAP;
        let spec = self.measure_spec(false, true);
        for i in with_desc {
            let info = &self.infos[i];
            let description = self.db.nodes[info.node]
                .description
                .clone()
                .unwrap_or_default();
            let font = FontSpec {
                size: Some(self.styles.label_font_size),
                style: Some(FontStyle::Italic),
                ..FontSpec::default()
            };
            let text = self.text(
                "treeView-node-description",
                desc_x,
                info.center_y,
                &description,
                font,
                Paint::Color(self.styles.description_color),
            );
            let group = info.group;
            if let Element::Group(g) = &mut self.elements[group] {
                g.children.push(text);
            }
            let (width, _) = self.bbox(&description, &spec);
            self.total_width = self.total_width.max(desc_x + width + self.cfg.padding_x);
        }
    }

    /// Phase 3: highlight backgrounds as wide as the tree. Each one widens
    /// the tree by its stroke allowance, so a later one is wider.
    fn highlights(&mut self) {
        for i in 0..self.infos.len() {
            let id = self.infos[i].node;
            let highlighted = self.db.nodes[id]
                .css_class
                .as_deref()
                .is_some_and(|c| c.split_whitespace().any(|w| w == "highlight"));
            if !highlighted {
                continue;
            }
            let b = self.boxes[id];
            let rect_width = self.total_width - b.x + 8.0;
            let rect = Element::Rect(Rect {
                common: Common {
                    class: Some("treeView-highlight-bg".to_string()),
                    style: Style {
                        fill: self.styles.highlight_bg.map(Paint::Color),
                        stroke: self.styles.highlight_stroke.map(Paint::Color),
                        stroke_width: Some(1.0),
                        ..Style::default()
                    },
                    ..Common::default()
                },
                x: b.x,
                y: b.y + 1.0,
                width: rect_width,
                height: b.height - 2.0,
                rx: Some(3.0),
                ry: None,
            });
            let group = self.infos[i].group;
            if let Element::Group(g) = &mut self.elements[group] {
                g.children.insert(0, rect);
            }
            self.total_width = self.total_width.max(b.x + rect_width + 2.0);
        }
    }
}

/// `draw`: the tree, then the viewBox and the natural size.
pub fn draw(
    db: &TreeViewDb,
    cfg: &TreeViewConfig,
    styles: &TreeViewStyles,
    ctx: &RenderCtx<'_>,
) -> Result<Scene, HelperError> {
    let mut painter = Painter {
        ctx,
        cfg,
        styles,
        db,
        elements: Vec::new(),
        infos: Vec::new(),
        boxes: vec![BBox::default(); db.nodes.len()],
        total_height: 0.0,
        total_width: 0.0,
    };
    painter.process_node(0, 0)?;
    painter.descriptions();
    painter.highlights();

    let (width, height) = (painter.total_width, painter.total_height);
    let mut scene = Scene::new(width, height);
    scene.view_box = ViewBox::new(-cfg.line_thickness / 2.0, 0.0, width, height);
    scene.font_family = styles.font_family.clone();
    scene.root.children.push(Element::Group(Group {
        common: Common {
            class: Some("tree-view".to_string()),
            ..Common::default()
        },
        children: painter.elements,
    }));
    Ok(scene)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_builtin_icons_draw() {
        assert!(builtin_icon("mermaid-treeview:folder").is_ok());
        assert!(builtin_icon("mermaid-treeview:file").is_ok());
        for other in ["mermaid-treeview:rust", "logos:react", "file"] {
            match builtin_icon(other) {
                Err(HelperError::UnsupportedFeature { feature }) => {
                    assert_eq!(feature, "icon pack")
                }
                _ => panic!("{other}"),
            }
        }
        assert!(parse_svg_path(FOLDER_PATH).is_some());
        assert!(parse_svg_path(FILE_PATH).is_some());
    }
}
