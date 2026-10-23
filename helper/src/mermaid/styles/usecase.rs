//! `diagrams/usecase/styles.ts` (mermaid 12.2.0) resolved to attribute
//! values.
//!
//! `roleColors` picks one colour per kind of element. The `genColor`
//! palette rules apply only under the `redux-color` themes, which the
//! helper does not render (R-175), so they are left out. The renderer
//! applies these values to the scene after the unified render:
//!
//! - `.usecase-actor-shape, .usecase-actor-hollow, ...` and
//!   `.node.usecase-actor .usecase-actor-glyph path, circle`: the glyph
//!   fill, the glyph stroke, and `stroke-width: 2px`.
//! - `.usecase-element ellipse, rect`: the use case body, `2px` wide.
//! - `.usecase-actor .nodeLabel` and `.usecase-element .nodeLabel`: the
//!   label colours and the `--mermaid-usecase-*` fonts.
//! - `.system-boundary rect.boundary-body`: the boundary colours.
//! - `.relationship-include`, `-extend`, `-note`: `stroke-dasharray: 3`
//!   and the include and extend line colours.
//! - `.edgeLabel`: 10 px text in `primaryTextColor`.
//! - `.marker.extension`: the generalization marker filled with
//!   `mainBkg`.

use crate::mermaid::scene::Color;
use crate::mermaid::shapes::NodeStyleVars;
use crate::mermaid::shapes::vars::parse_color;
use crate::mermaid::theme::ThemeVariables;

/// `.edgeLabel { font-size: 10px }`.
pub const EDGE_LABEL_FONT_SIZE: f64 = 10.0;
/// `.labelBkg { padding: 0 2px }`: the HTML edge label box is 4 px wider
/// than its text.
pub const EDGE_LABEL_PADDING_X: f64 = 2.0;
/// `stroke-width: 2px` of the actor glyph and the use case body.
pub const SHAPE_STROKE_WIDTH: f64 = 2.0;
/// `.relationship-include, .relationship-extend, .relationship-note {
/// stroke-dasharray: 3 }`.
pub const DASHED_RELATIONSHIP: f64 = 3.0;

/// The use case stylesheet values.
#[derive(Debug, Clone, PartialEq)]
pub struct UsecaseStyles {
    pub actor_bkg: Color,
    pub actor_border: Color,
    /// `actorTextColor ?? primaryTextColor`.
    pub actor_text: Color,
    pub bkg: Color,
    pub border: Color,
    pub boundary_bkg: Color,
    pub boundary_border: Color,
    pub include_line: Color,
    pub extend_line: Color,
    pub line_color: Color,
    pub primary_text: Color,
    /// `titleColor ?? primaryTextColor`.
    pub title_color: Color,
    pub note_text: Color,
    pub main_bkg: Color,
    /// `nodeBorder ?? primaryColor`.
    pub node_border: Color,
    pub edge_label_background: Color,
    /// `fontFamily`, the edge label family.
    pub font_family: String,
}

impl UsecaseStyles {
    /// `getStyles(options)` for the resolved theme; `vars` supplies the
    /// values the shapes already derived.
    pub fn resolve(tv: &ThemeVariables, vars: &NodeStyleVars) -> UsecaseStyles {
        let get = |name: &str| tv.str(name).and_then(parse_color);
        let first =
            |names: &[&str], fallback: Color| names.iter().find_map(|n| get(n)).unwrap_or(fallback);
        let primary = get("primaryColor").unwrap_or(vars.main_bkg);
        let primary_text = get("primaryTextColor").unwrap_or(vars.node_text_color);
        let main_bkg = get("mainBkg").unwrap_or(vars.main_bkg);
        let line_color = get("lineColor").unwrap_or(vars.line_color);
        let cluster_bkg = get("clusterBkg").unwrap_or(vars.cluster_bkg);
        let cluster_border = get("clusterBorder").unwrap_or(vars.cluster_border);
        UsecaseStyles {
            actor_bkg: first(&["usecaseActorBkg", "actorBkg"], main_bkg),
            actor_border: first(&["usecaseActorBorder", "actorBorder"], primary),
            actor_text: first(&["actorTextColor"], primary_text),
            bkg: first(&["usecaseBkg"], main_bkg),
            border: first(&["usecaseBorder", "nodeBorder"], primary),
            boundary_bkg: first(&["usecaseBoundaryBkg"], cluster_bkg),
            boundary_border: first(&["usecaseBoundaryBorder"], cluster_border),
            include_line: first(&["usecaseIncludeLine"], line_color),
            extend_line: first(&["usecaseExtendLine"], line_color),
            line_color,
            primary_text,
            title_color: first(&["titleColor"], primary_text),
            note_text: get("noteTextColor").unwrap_or(vars.note_text_color),
            main_bkg,
            node_border: first(&["nodeBorder"], primary),
            edge_label_background: get("edgeLabelBackground").unwrap_or(vars.edge_label_background),
            font_family: vars.font_family.clone(),
        }
    }

    /// Writes the boundary and title colours into the shapes' variables,
    /// which the shared cluster code paints with.
    pub fn apply_to(&self, vars: &mut NodeStyleVars) {
        vars.cluster_bkg = self.boundary_bkg;
        vars.cluster_border = self.boundary_border;
        vars.title_color = self.title_color;
        vars.edge_label_background = self.edge_label_background;
    }
}
