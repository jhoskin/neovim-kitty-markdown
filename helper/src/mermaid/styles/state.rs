//! `diagrams/state/styles.js` (mermaid 12.2.0) resolved to attributes.
//!
//! The shapes and edges paint themselves from [`NodeStyleVars`]; the state
//! stylesheet differs from the flowchart one the shapes default to, so
//! [`StateStyles::node_vars`] fills those variables with the state values
//! (`.node rect { fill: stateBkg || mainBkg; stroke: stateBorder ||
//! nodeBorder }`, `.nodeLabel { color: stateLabelColor }`, `.transition {
//! stroke: transitionColor }`). [`StateStyles::apply`] then walks the
//! rendered tree for the rules no shape knows about: the rounded composite
//! title rect, the dashed concurrency regions, the dashed note edges, the
//! transition label color, and the note text color.
//!
//! The palette rules of `genColor` only exist for the `redux-color` themes
//! and the `neo` look rules only for `look: neo`; the helper renders
//! neither, so they are not ported.

use std::collections::HashMap;

use crate::mermaid::config::MermaidConfig;
use crate::mermaid::scene::{Color, Element, Group, Paint, Text, TextAnchor};
use crate::mermaid::shapes::NodeStyleVars;
use crate::mermaid::shapes::vars::parse_color;
use crate::mermaid::theme::ThemeVariables;

/// The stroke width `rough.js` draws a classic-look shape outline with
/// (`options.strokeWidth` of `userNodeOverrides`), which the oracle shows
/// on the choice, fork, join, and note outlines.
pub const ROUGH_STROKE_WIDTH: f64 = 1.3;

/// `.statediagram-state rect.divider { stroke-dasharray: 10,10 }`.
const DIVIDER_DASH: [f64; 2] = [10.0, 10.0];
/// `.note-edge { stroke-dasharray: 5 }`.
const NOTE_EDGE_DASH: f64 = 5.0;
/// `.statediagram-cluster rect.outer { rx: 5px; ry: 5px }` and
/// `.statediagram-state .title-state { rx: 5px; ry: 5px }`.
const TITLE_RADIUS: f64 = 5.0;
/// `.statediagramTitleText { font-size: 18px }`.
pub const TITLE_FONT_SIZE: f64 = 18.0;

/// What the walk needs to know about one node group: the shape that drew
/// it and the label declarations of its `classDef`s and `style`s.
#[derive(Debug, Clone, Default)]
pub struct NodeInfo {
    pub shape: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateStyles {
    pub vars: NodeStyleVars,
    pub text_color: Color,
    pub transition_color: Color,
    pub transition_label_color: Color,
    pub note_text_color: Color,
    pub alt_background: Color,
    pub state_border: Color,
}

impl StateStyles {
    pub fn resolve(tv: &ThemeVariables, config: &MermaidConfig) -> StateStyles {
        let base = NodeStyleVars::from_theme(tv, config);
        let color = |names: &[&str], fallback: Color| -> Color {
            names
                .iter()
                .find_map(|name| tv.str(name).and_then(parse_color))
                .unwrap_or(fallback)
        };
        let state_border = color(&["stateBorder", "nodeBorder"], base.node_border);
        let transition_color = color(&["transitionColor"], base.line_color);
        let state_label_color = color(&["stateLabelColor"], base.node_text_color);
        let alt_background = tv
            .str("altBackground")
            .filter(|s| !s.is_empty())
            .and_then(parse_color)
            .unwrap_or(Color::rgb(0xef, 0xef, 0xef));
        let vars = NodeStyleVars {
            main_bkg: color(&["stateBkg", "mainBkg"], base.main_bkg),
            node_border: state_border,
            line_color: transition_color,
            node_text_color: state_label_color,
            title_color: state_label_color,
            alt_background,
            ..base.clone()
        };
        StateStyles {
            text_color: base.text_color,
            transition_color,
            transition_label_color: color(
                &["transitionLabelColor", "tertiaryTextColor"],
                base.text_color,
            ),
            note_text_color: base.note_text_color,
            alt_background,
            state_border,
            vars,
        }
    }

    /// The variables the shapes, clusters, and edges paint with.
    pub fn node_vars(&self) -> NodeStyleVars {
        self.vars.clone()
    }

    /// Applies the rules of the stylesheet no shape handles. `nodes` maps a
    /// node group id to what drew it.
    pub fn apply(&self, root: &mut Group, nodes: &HashMap<String, NodeInfo>) {
        self.walk(root, nodes, Scope::default());
    }

    fn walk(&self, group: &mut Group, nodes: &HashMap<String, NodeInfo>, scope: Scope) {
        let class = group.common.class.clone().unwrap_or_default();
        let mut scope = scope;
        if has_class(&class, "edgeLabels") {
            scope.edge_labels = true;
        }
        if has_class(&class, "statediagram-note") {
            scope.note = true;
        }
        if has_class(&class, "statediagram-state") {
            scope.state = true;
        }
        if has_class(&class, "statediagram-cluster") {
            scope.cluster = true;
        }
        if has_class(&class, "node")
            && let Some(info) = group.common.id.as_deref().and_then(|id| nodes.get(id))
        {
            scope.rough = matches!(info.shape.as_str(), "choice" | "fork" | "join" | "note");
            scope.left = info.shape == "rectWithTitle";
        }
        for child in &mut group.children {
            match child {
                Element::Group(g) => self.walk(g, nodes, scope),
                Element::Rect(r) => {
                    let class = r.common.class.clone().unwrap_or_default();
                    if scope.cluster && has_class(&class, "outer") {
                        r.rx = Some(TITLE_RADIUS);
                        r.ry = Some(TITLE_RADIUS);
                    }
                    if scope.state && has_class(&class, "title-state") {
                        r.rx = Some(TITLE_RADIUS);
                        r.ry = Some(TITLE_RADIUS);
                    }
                    if scope.state && has_class(&class, "divider") {
                        r.common.style.stroke_dasharray = Some(DIVIDER_DASH.to_vec());
                        r.common.style.fill = Some(Paint::Color(self.alt_background));
                        r.common.style.stroke = Some(Paint::Color(self.state_border));
                    }
                    if scope.rough {
                        r.common.style.stroke_width = Some(ROUGH_STROKE_WIDTH);
                    }
                }
                Element::Path(p) => {
                    let class = p.common.class.clone().unwrap_or_default();
                    if has_class(&class, "note-edge") {
                        p.common.style.stroke_dasharray = Some(vec![NOTE_EDGE_DASH]);
                    }
                    if scope.rough {
                        p.common.style.stroke_width = Some(ROUGH_STROKE_WIDTH);
                    }
                }
                Element::Text(t) => {
                    if scope.edge_labels {
                        t.common.style.fill = Some(Paint::Color(self.transition_label_color));
                    } else if scope.note {
                        t.common.style.fill = Some(Paint::Color(self.note_text_color));
                    }
                    if scope.left && t.anchor == Some(TextAnchor::Middle) {
                        left_align(t);
                    }
                }
                _ => {}
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Scope {
    edge_labels: bool,
    note: bool,
    state: bool,
    cluster: bool,
    rough: bool,
    /// Inside a `rectWithTitle`, whose label `div`s set no `text-align`.
    left: bool,
}

/// `rectWithTitle` builds its two label `div`s without the `text-align:
/// center` `createText` gives other labels, so each line starts at the
/// left edge of its block; the shared builder centres them.
fn left_align(t: &mut Text) {
    t.anchor = Some(TextAnchor::Start);
    t.x = 0.0;
    for run in &mut t.runs {
        if run.x.is_some() {
            run.x = Some(0.0);
        }
    }
}

fn has_class(classes: &str, name: &str) -> bool {
    classes.split_whitespace().any(|c| c == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::theme;

    #[test]
    fn default_theme_values() {
        let tv = theme::resolve("default", None).expect("theme");
        let s = StateStyles::resolve(&tv, &MermaidConfig::default());
        let hex = |v: &str| Color::from_hex(v).expect("hex");
        assert_eq!(s.vars.node_text_color, hex("#131300"));
        assert_eq!(s.vars.main_bkg, hex("#ECECFF"));
        assert_eq!(s.vars.node_border, hex("#9370DB"));
        assert_eq!(s.transition_color, hex("#333333"));
        assert_eq!(s.transition_label_color, hex("#333"));
        assert_eq!(s.alt_background, hex("#f0f0f0"));
        assert_eq!(s.note_text_color, Color::BLACK);
    }

    #[test]
    fn dark_theme_uses_the_state_variables() {
        let tv = theme::resolve("dark", None).expect("theme");
        let s = StateStyles::resolve(&tv, &MermaidConfig::default());
        let hex = |v: &str| Color::from_hex(v).expect("hex");
        assert_eq!(s.vars.node_text_color, hex("#e0dfdf"));
        assert_eq!(s.vars.composite_background, hex("#333"));
        assert_eq!(s.transition_label_color, hex("#ccc"));
        assert_eq!(s.alt_background, hex("#555"));
    }
}
