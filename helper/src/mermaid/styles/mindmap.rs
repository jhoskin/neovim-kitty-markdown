//! `diagrams/mindmap/styles.ts` (mermaid 12.2.0) resolved to attributes.
//!
//! The classic-look rules of the sheet, for `i` in `0..THEME_COLOR_LIMIT`
//! and the class suffix `N = i - 1`:
//!
//! - `.section-N rect, path, circle, polygon { fill: cScale<i> }`, no
//!   stroke: the mindmap sheet has no `.node rect` rule, so shapes are
//!   unstroked.
//! - `.section-N text { fill }` and `.section-N span { color }`:
//!   `cScaleLabel<i>`.
//! - `.section-N line { stroke: cScaleInv<i>; stroke-width: 3 }`: the base
//!   line of `defaultMindmapNode`.
//! - `.section-edge-N { stroke: cScale<i> }` and `.edge-depth-N {
//!   stroke-width: 17 - 3 * i }`. A negative width is invalid CSS and is
//!   dropped, so deeper edges keep `.edge { stroke-width: 3 }`, which
//!   follows `.edge-thickness-normal { stroke-width: 1px }` in the sheet.
//!
//! The `.section-root` rules follow the loop and win over `.section--1`:
//! `fill: git0`, `text { fill }` and `span { color }`: `gitBranchLabel0`.
//! The `[data-look="neo"]` rules never match the classic look, but
//! `genGradient`, appended when the theme sets `useGradient` (`base`,
//! `dark`, `forest`, `neutral`), also emits `.section-N line {
//! stroke-width: 0 }`, which hides the base line in those themes.

use crate::mermaid::scene::{Color, DEFAULT_FONT_FAMILY, Element, Group, Paint};
use crate::mermaid::shapes::NodeStyleVars;
use crate::mermaid::styles::gantt::css_color;
use crate::mermaid::theme::ThemeVariables;
use crate::mermaid::theme::vars::js_truthy;

/// `THEME_COLOR_LIMIT` of every built-in theme.
const THEME_COLOR_LIMIT: usize = 12;
/// `.edge { stroke-width: 3 }`.
const EDGE_STROKE_WIDTH: f64 = 3.0;
/// `.section-N line { stroke-width: 3 }`.
const LINE_STROKE_WIDTH: f64 = 3.0;

#[derive(Debug, Clone, PartialEq)]
pub struct MindmapStyles {
    pub font_family: String,
    pub font_size: f64,
    pub text_color: Color,
    c_scale: Vec<Color>,
    c_scale_label: Vec<Color>,
    c_scale_inv: Vec<Color>,
    git0: Color,
    git_branch_label0: Color,
    /// `options.useGradient && svgId && options.mainBkg`: mermaid-cli
    /// always passes an `svgId`.
    gradient: bool,
}

/// What a node group's classes select.
#[derive(Debug, Clone, Copy, PartialEq)]
struct NodePaint {
    fill: Option<Color>,
    label: Option<Color>,
    line: Option<Color>,
}

fn has_class(classes: &str, name: &str) -> bool {
    classes.split_whitespace().any(|c| c == name)
}

/// The `N` of every `<prefix>N` class, in class order.
fn suffixes<'a>(classes: &'a str, prefix: &'a str) -> impl Iterator<Item = i64> + 'a {
    classes
        .split_whitespace()
        .filter_map(move |c| c.strip_prefix(prefix))
        .filter_map(|n| n.parse::<i64>().ok())
}

impl MindmapStyles {
    pub fn from_theme(tv: &ThemeVariables) -> MindmapStyles {
        let color = |name: &str| css_color(tv.str(name).unwrap_or("black"));
        let scale = |prefix: &str| -> Vec<Color> {
            (0..THEME_COLOR_LIMIT)
                .map(|i| color(&format!("{prefix}{i}")))
                .collect()
        };
        let font_size = tv
            .str("fontSize")
            .and_then(|s| s.trim().trim_end_matches("px").parse::<f64>().ok())
            .unwrap_or(16.0);
        MindmapStyles {
            font_family: tv
                .str("fontFamily")
                .unwrap_or(DEFAULT_FONT_FAMILY)
                .to_string(),
            font_size,
            text_color: color("textColor"),
            c_scale: scale("cScale"),
            c_scale_label: scale("cScaleLabel"),
            c_scale_inv: scale("cScaleInv"),
            git0: color("git0"),
            git_branch_label0: color("gitBranchLabel0"),
            gradient: js_truthy(tv.get("useGradient")) && js_truthy(tv.get("mainBkg")),
        }
    }

    /// The variables the shapes read. The fills and strokes they set are
    /// replaced by [`MindmapStyles::apply`].
    pub fn node_vars(&self) -> NodeStyleVars {
        NodeStyleVars {
            text_color: self.text_color,
            node_text_color: self.text_color,
            font_family: self.font_family.clone(),
            font_size: self.font_size,
            ..NodeStyleVars::default()
        }
    }

    /// The slot `i = N + 1` of a `section-N` class, when the sheet has it.
    fn slot(n: i64) -> Option<usize> {
        let i = n + 1;
        (0..THEME_COLOR_LIMIT as i64)
            .contains(&i)
            .then_some(i as usize)
    }

    fn node_paint(&self, classes: &str) -> NodePaint {
        let mut paint = NodePaint {
            fill: None,
            label: None,
            line: None,
        };
        // Equal specificity: the later rule in the sheet wins, and the
        // sheet orders the sections by `N`.
        let mut sections: Vec<i64> = suffixes(classes, "section-").collect();
        sections.sort_unstable();
        for i in sections.into_iter().filter_map(Self::slot) {
            paint.fill = Some(self.c_scale[i]);
            paint.label = Some(self.c_scale_label[i]);
            paint.line = (!self.gradient).then_some(self.c_scale_inv[i]);
        }
        if has_class(classes, "section-root") {
            paint.fill = Some(self.git0);
            paint.label = Some(self.git_branch_label0);
        }
        paint
    }

    /// `.section-edge-N { stroke }` and the width cascade.
    fn edge_paint(&self, classes: &str) -> (Option<Color>, f64) {
        let mut stroke = None;
        let mut sections: Vec<i64> = suffixes(classes, "section-edge-").collect();
        sections.sort_unstable();
        for i in sections.into_iter().filter_map(Self::slot) {
            stroke = Some(self.c_scale[i]);
        }
        let mut width = EDGE_STROKE_WIDTH;
        let mut depths: Vec<i64> = suffixes(classes, "edge-depth-").collect();
        depths.sort_unstable();
        for i in depths.into_iter().filter_map(Self::slot) {
            let sw = 17.0 - 3.0 * i as f64;
            if sw >= 0.0 {
                width = sw;
            }
        }
        (stroke, width)
    }

    /// Applies the sheet to the unified renderer's `g.root`.
    pub fn apply(&self, root: &mut Group) {
        self.walk(root, None, false);
    }

    fn walk(&self, group: &mut Group, node: Option<NodePaint>, in_edges: bool) {
        let class = group.common.class.clone().unwrap_or_default();
        let node = if has_class(&class, "mindmap-node") {
            Some(self.node_paint(&class))
        } else {
            node
        };
        let in_edges = in_edges || has_class(&class, "edgePaths");
        for child in &mut group.children {
            match child {
                Element::Group(g) => self.walk(g, node, in_edges),
                Element::Path(p) if in_edges => {
                    let class = p.common.class.clone().unwrap_or_default();
                    if has_class(&class, "edge") {
                        let (stroke, width) = self.edge_paint(&class);
                        let style = &mut p.common.style;
                        style.fill = Some(Paint::None);
                        if let Some(stroke) = stroke {
                            style.stroke = Some(Paint::Color(stroke));
                        }
                        style.stroke_width = Some(width);
                    }
                }
                Element::Rect(_) | Element::Path(_) | Element::Circle(_) | Element::Polygon(_) => {
                    let Some(paint) = node else { continue };
                    let style = match child {
                        Element::Rect(e) => &mut e.common.style,
                        Element::Path(e) => &mut e.common.style,
                        Element::Circle(e) => &mut e.common.style,
                        Element::Polygon(e) => &mut e.common.style,
                        _ => continue,
                    };
                    // A section past the sheet keeps the SVG default fill.
                    style.fill = Some(Paint::Color(paint.fill.unwrap_or(Color::BLACK)));
                    style.stroke = Some(Paint::None);
                }
                Element::Line(l) => {
                    if let Some(line) = node.and_then(|p| p.line) {
                        l.common.style.stroke = Some(Paint::Color(line));
                        l.common.style.stroke_width = Some(LINE_STROKE_WIDTH);
                    }
                }
                Element::Text(t) => {
                    if let Some(label) = node.and_then(|p| p.label) {
                        t.common.style.fill = Some(Paint::Color(label));
                    }
                }
                _ => {}
            }
        }
    }
}
