//! Port of mermaid 12.2.0 `rendering-util/rendering-elements/shapes/`: the
//! node shape library the unified renderer draws for flowchart, class,
//! state, ER, requirement, kanban, and usecase diagrams.
//!
//! One Rust module per mermaid shape file, the classic-look branch of each.
//! The handDrawn (`rough.js`) branches are not ported: the config package
//! rejects every `look` but `classic`, and [`insert_node`] refuses the other
//! looks so that branch can never be reached by accident. Where the classic
//! branch still calls `rough.js` with `roughness: 0` and `fillStyle:
//! 'solid'`, the output is the plain geometry, so a path element with the
//! same `d` stands in for the rough group.
//!
//! Colors come from the stylesheet in mermaid (`.node rect, .node circle,
//! .node ellipse, .node polygon, .node path { fill: mainBkg; stroke:
//! nodeBorder; stroke-width: 1px }`). The scene model has no CSS, so the
//! shapes resolve that rule themselves from [`NodeStyleVars`] and apply the
//! user's `style`/`classDef` declarations on top, as `!important` would.
//! Every element keeps mermaid's class names so a later styles pass can
//! still find them.

pub mod intersect;
pub mod measure;
pub mod util;
pub mod vars;

pub mod anchor;
pub mod bang;
pub mod block_arrow;
pub mod bow_tie_rect;
pub mod browser;
pub mod bucket;
pub mod card;
pub mod choice;
pub mod circle;
pub mod class_box;
pub mod cloud;
pub mod collapsed_group;
pub mod composite;
pub mod console;
pub mod crossed_circle;
pub mod curly_brace_left;
pub mod curly_brace_right;
pub mod curly_braces;
pub mod curved_trapezoid;
pub mod cylinder;
pub mod datastore;
pub mod default_mindmap_node;
pub mod divided_rect;
pub mod double_circle;
pub mod draw_rect;
pub mod er_box;
pub mod filled_circle;
pub mod flipped_triangle;
pub mod folder;
pub mod fork_join;
pub mod half_rounded_rectangle;
pub mod hexagon;
pub mod hourglass;
pub mod icon;
pub mod inverted_trapezoid;
pub mod kanban_item;
pub mod label_rect;
pub mod lean_left;
pub mod lean_right;
pub mod lightning_bolt;
pub mod lined_cylinder;
pub mod lined_wave_edged_rect;
pub mod mindmap_circle;
pub mod multi_rect;
pub mod multi_wave_edged_rectangle;
pub mod note;
pub mod person;
pub mod question;
pub mod rect_left_inv_arrow;
pub mod rect_with_title;
pub mod requirement_box;
pub mod rounded_rect;
pub mod shaded_process;
pub mod sloped_rect;
pub mod square_rect;
pub mod stadium;
pub mod state;
pub mod state_end;
pub mod state_start;
pub mod subroutine;
pub mod tagged_rect;
pub mod tagged_wave_edged_rectangle;
pub mod text;
pub mod tilted_cylinder;
pub mod trapezoid;
pub mod trapezoidal_pentagon;
pub mod triangle;
pub mod usecase_actor;
pub mod usecase_business;
pub mod usecase_ellipse;
pub mod usecase_json_table;
pub mod wave_edged_rectangle;
pub mod wave_rectangle;
pub mod window_pane;

use crate::error::HelperError;
use crate::mermaid::scene::{Color, FontSpec, Group, Point};

pub use intersect::Bounds;
pub use measure::{Dims, FixedMeasurer, LabelBox, LabelLine, LabelMeasurer, LabelOpts, LabelRun};

/// mermaid's `rankdir` / `dir` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Dir {
    #[default]
    TB,
    BT,
    LR,
    RL,
}

impl Dir {
    /// Accepts the spellings flowchart uses: `TB`, `TD`, `BT`, `LR`, `RL`.
    pub fn parse(s: &str) -> Option<Dir> {
        match s.trim().to_ascii_uppercase().as_str() {
            "TB" | "TD" => Some(Dir::TB),
            "BT" => Some(Dir::BT),
            "LR" => Some(Dir::LR),
            "RL" => Some(Dir::RL),
            _ => None,
        }
    }

    /// The upper-case `rankdir` string dagre receives.
    pub fn as_str(self) -> &'static str {
        match self {
            Dir::TB => "TB",
            Dir::BT => "BT",
            Dir::LR => "LR",
            Dir::RL => "RL",
        }
    }

    pub fn is_horizontal(self) -> bool {
        matches!(self, Dir::LR | Dir::RL)
    }
}

/// mermaid's `look`. Only `Classic` renders; the others are reported as
/// `unsupported_feature` (SPEC.md R-112).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Look {
    #[default]
    Classic,
    HandDrawn,
    Neo,
}

impl Look {
    pub fn parse(s: &str) -> Option<Look> {
        match s {
            "classic" | "default" => Some(Look::Classic),
            "handDrawn" => Some(Look::HandDrawn),
            "neo" => Some(Look::Neo),
            _ => None,
        }
    }
}

/// `ClassMember.parseClassifier()` of `diagrams/class/classTypes.ts`:
/// `*` marks an abstract member (italic) and `$` a static one (underline).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Classifier {
    #[default]
    None,
    Abstract,
    Static,
}

/// One class member or method line of `classBox`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClassMember {
    pub text: String,
    pub classifier: Classifier,
}

/// One row of an `erBox`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ErAttribute {
    pub type_name: String,
    pub name: String,
    pub keys: Vec<String>,
    pub comment: String,
}

/// The requirement diagram node kinds `requirementBox` tells apart with
/// `'verifyMethod' in node`.
#[derive(Debug, Clone, PartialEq)]
pub enum RequirementData {
    Requirement {
        type_name: String,
        name: String,
        requirement_id: String,
        text: String,
        risk: String,
        verify_method: String,
    },
    Element {
        name: String,
        type_name: String,
        doc_ref: String,
    },
}

/// `KanbanNode` fields of `types.ts`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct KanbanData {
    pub ticket: Option<String>,
    pub assigned: Option<String>,
    pub priority: Option<String>,
}

/// Diagram-specific node payloads. mermaid casts `Node` to the diagram's
/// own type inside the shape; this enum carries the same fields typed.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum NodeExtra {
    #[default]
    None,
    Class {
        annotations: Vec<String>,
        members: Vec<ClassMember>,
        methods: Vec<ClassMember>,
    },
    Entity {
        alias: Option<String>,
        attributes: Vec<ErAttribute>,
    },
    Requirement(RequirementData),
    Kanban(KanbanData),
    /// `usecaseActor*` and `usecaseBusiness` read `stereotype` and
    /// `business`; `usecaseJsonTable` reads `rows`.
    Usecase {
        stereotype: Option<String>,
        business: bool,
        rows: Vec<(String, String)>,
    },
}

/// The geometry `node.intersect` closes over. mermaid stores a closure on
/// the node after drawing; this enum stores the same data and
/// [`Node::intersect`] replays the closure body.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Intersector {
    /// No shape drawn yet: `intersectNode` would throw; we return the centre.
    #[default]
    None,
    Rect,
    Circle(f64),
    Ellipse {
        rx: f64,
        ry: f64,
    },
    Polygon(Vec<Point>),
    /// `cylinder.ts`, `document.ts`, `linedCylinder.ts`: a rect intersection
    /// pulled onto the elliptical cap.
    Cylinder {
        rx: f64,
        ry: f64,
    },
    /// `tiltedCylinder.ts`: the same on the horizontal axis.
    TiltedCylinder {
        rx: f64,
        ry: f64,
    },
}

/// `Node` of `rendering-util/types.ts`, the fields the shapes read, plus the
/// layout results and the intersection geometry the shape leaves behind.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Node {
    pub id: String,
    pub dom_id: Option<String>,
    pub label: Option<String>,
    /// `labelType`: `markdown`, `text`, or `string`.
    pub label_type: Option<String>,
    pub label_style: Option<String>,
    /// The font a diagram stylesheet gives this node's labels, such as
    /// `.usecase-actor .nodeLabel { font-size: 14px }`. Unset fields keep
    /// the theme font; the inline `label_style` fonts override it.
    pub label_font: FontSpec,
    /// `description` of `rectWithTitle`.
    pub description: Vec<String>,
    pub parent_id: Option<String>,
    /// `cssStyles`: `style A fill:#f9f` declarations, one per entry.
    pub css_styles: Vec<String>,
    /// `cssCompiledStyles`: declarations from the `classDef`s the node uses.
    pub css_compiled_styles: Vec<String>,
    /// `cssClasses`: space separated class names.
    pub css_classes: String,
    /// The key in `shapes.ts`.
    pub shape: String,
    pub dir: Option<Dir>,
    pub padding: Option<f64>,
    pub is_group: bool,
    /// Width hint in, measured width out (`updateNodeBounds`).
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub wrapping_width: Option<f64>,
    /// `minWidth` (`flowchart.minNodeWidth`).
    pub min_width: Option<f64>,
    pub label_bbox: Option<Dims>,
    pub rx: Option<f64>,
    pub ry: Option<f64>,
    pub use_html_labels: Option<bool>,
    pub center_label: bool,
    pub look: Look,
    pub label_padding_x: Option<f64>,
    pub label_padding_y: Option<f64>,
    /// Layout output: the node centre.
    pub x: f64,
    pub y: f64,
    pub extra: NodeExtra,
    pub intersector: Intersector,
}

impl Node {
    /// A node with an id, a label, and a shape; everything else default.
    pub fn new(id: &str, label: &str, shape: &str) -> Node {
        Node {
            id: id.to_string(),
            label: Some(label.to_string()),
            shape: shape.to_string(),
            ..Node::default()
        }
    }

    /// A flowchart node as `flowDb.addNodeFromVertex` builds it: padding
    /// from `flowchart.padding` (15), `cssClasses` `default`, minimum width
    /// from `flowchart.minNodeWidth`.
    pub fn flowchart(id: &str, label: &str, shape: &str) -> Node {
        Node {
            padding: Some(15.0),
            css_classes: "default".to_string(),
            label_style: Some(String::new()),
            label_type: Some("text".to_string()),
            min_width: Some(120.0),
            ..Node::new(id, label, shape)
        }
    }

    pub fn dom_id(&self) -> &str {
        self.dom_id.as_deref().unwrap_or(&self.id)
    }

    pub fn padding(&self) -> f64 {
        self.padding.unwrap_or(0.0)
    }

    pub fn width(&self) -> f64 {
        self.width.unwrap_or(0.0)
    }

    pub fn height(&self) -> f64 {
        self.height.unwrap_or(0.0)
    }

    /// The `{x, y, width, height}` the intersect functions read.
    pub fn bounds(&self) -> Bounds {
        Bounds {
            x: self.x,
            y: self.y,
            width: self.width(),
            height: self.height(),
        }
    }

    /// `node.intersect(point)`: the shape's intersection closure.
    pub fn intersect(&self, point: Point) -> Point {
        let b = self.bounds();
        match &self.intersector {
            Intersector::None => Point::new(self.x, self.y),
            Intersector::Rect => intersect::rect(&b, point),
            Intersector::Circle(r) => intersect::circle(&b, *r, point),
            Intersector::Ellipse { rx, ry } => intersect::ellipse(&b, *rx, *ry, point),
            Intersector::Polygon(points) => intersect::polygon(&b, points, point),
            Intersector::Cylinder { rx, ry } => cylinder::intersect_cylinder(&b, *rx, *ry, point),
            Intersector::TiltedCylinder { rx, ry } => {
                tilted_cylinder::intersect_tilted(&b, *rx, *ry, point)
            }
        }
    }
}

/// The theme variables the shapes and edges read directly. The theme
/// package fills this from `ThemeVariables`; [`NodeStyleVars::default`] is
/// mermaid's `default` theme (`themes/theme-default.js`).
#[derive(Debug, Clone, PartialEq)]
pub struct NodeStyleVars {
    pub main_bkg: Color,
    pub node_border: Color,
    pub line_color: Color,
    pub text_color: Color,
    pub node_text_color: Color,
    pub title_color: Color,
    pub cluster_bkg: Color,
    pub cluster_border: Color,
    pub edge_label_background: Color,
    pub note_bkg_color: Color,
    pub note_border_color: Color,
    pub note_text_color: Color,
    pub background: Color,
    pub composite_background: Color,
    pub composite_title_background: Color,
    pub alt_background: Color,
    /// `stateBorder` is unset in the default theme; shapes fall back to
    /// `nodeBorder`.
    pub state_border: Option<Color>,
    pub special_state_color: Color,
    pub inner_end_background: Color,
    pub row_even: Color,
    pub row_odd: Color,
    /// `themeVariables.radius`, the rounded rect radius (default 5).
    pub radius: f64,
    /// `strokeWidth` (default 1).
    pub stroke_width: f64,
    pub font_family: String,
    pub font_size: f64,
    /// The root `config.fontFamily` and `config.fontSize`, which
    /// `calculateTextWidth(text, config)` measures in rather than the label
    /// font. The default family keeps its trailing `;`, so Chrome measures
    /// it in the page default font.
    pub config_font_family: String,
    pub config_font_size: f64,
}

impl Default for NodeStyleVars {
    fn default() -> NodeStyleVars {
        let hex = |s: &str| Color::from_hex(s).unwrap_or(Color::BLACK);
        NodeStyleVars {
            main_bkg: hex("#ECECFF"),
            node_border: hex("#9370DB"),
            line_color: hex("#333333"),
            text_color: hex("#333"),
            node_text_color: hex("#333"),
            title_color: hex("#333"),
            cluster_bkg: hex("#ffffde"),
            cluster_border: hex("#aaaa33"),
            edge_label_background: Color::rgba(232, 232, 232, 0.8),
            note_bkg_color: hex("#fff5ad"),
            note_border_color: hex("#aaaa33"),
            note_text_color: Color::BLACK,
            background: Color::WHITE,
            composite_background: Color::WHITE,
            composite_title_background: hex("#ECECFF"),
            alt_background: hex("#f0f0f0"),
            state_border: None,
            special_state_color: hex("#333333"),
            inner_end_background: hex("#9370DB"),
            // theme-default derives `rowEven` as hsl(240, 100%, 97.27%).
            row_even: Color::rgb(241, 241, 255),
            row_odd: Color::WHITE,
            radius: 5.0,
            stroke_width: 1.0,
            font_family: crate::mermaid::scene::DEFAULT_FONT_FAMILY.to_string(),
            font_size: 16.0,
            // The `fontFamily` default of `config.schema.yaml`.
            config_font_family: format!("{};", crate::mermaid::scene::DEFAULT_FONT_FAMILY),
            config_font_size: 16.0,
        }
    }
}

/// Everything a shape handler reads besides the node: the measurer, the
/// theme variables, and the config bits `getConfig()` would answer.
pub struct ShapeCtx<'a> {
    pub measurer: &'a dyn LabelMeasurer,
    pub vars: NodeStyleVars,
    /// The effective `htmlLabels` (root config, default `true`).
    pub html_labels: bool,
    /// `flowchart.wrappingWidth` (default 120).
    pub wrapping_width: f64,
    /// `ShapeRenderOptions.dir`: the direction of the enclosing graph, which
    /// `forkJoin` reads.
    pub dir: Option<Dir>,
    /// `class.padding` (default 12).
    pub class_padding: f64,
    /// `class.hideEmptyMembersBox` (default false).
    pub hide_empty_members_box: bool,
    /// `er.diagramPadding` (10), `er.entityPadding` (6), `er.minEntityWidth`
    /// (100).
    pub er_diagram_padding: f64,
    pub er_entity_padding: f64,
    pub er_min_entity_width: f64,
    /// `state.padding` (8).
    pub state_padding: f64,
    /// `kanban.ticketBaseUrl`; only affects the link, which the scene omits.
    pub kanban_ticket_base_url: Option<String>,
    /// `layout === 'elk'` switches requirement body text to `start`; always
    /// false here because ELK is unsupported.
    pub elk: bool,
    /// The font the diagram stylesheet gives edge labels, such as ER's
    /// `.edgeLabel .label { font-size: 14px }`. Unset fields keep the theme
    /// font; the edge's inline label styles override it.
    pub edge_label_font: FontSpec,
    /// Horizontal padding of the HTML edge label box on each side, from a
    /// `.labelBkg { padding: 0 2px }` rule: `createText` puts that class on
    /// the div it measures with `getBoundingClientRect()`.
    pub edge_label_padding_x: f64,
    /// The font of the edge terminal labels (`g.edgeTerminals`), such as
    /// class diagram's `.edgeTerminals { font-size: 11px }`.
    pub edge_terminal_font: FontSpec,
}

impl<'a> ShapeCtx<'a> {
    /// Defaults of mermaid's config schema with the default theme.
    pub fn new(measurer: &'a dyn LabelMeasurer) -> ShapeCtx<'a> {
        ShapeCtx {
            measurer,
            vars: NodeStyleVars::default(),
            html_labels: true,
            wrapping_width: 120.0,
            dir: None,
            class_padding: 12.0,
            hide_empty_members_box: false,
            er_diagram_padding: 10.0,
            er_entity_padding: 6.0,
            er_min_entity_width: 100.0,
            state_padding: 8.0,
            kanban_ticket_base_url: None,
            elk: false,
            edge_label_font: FontSpec::default(),
            edge_label_padding_x: 0.0,
            edge_terminal_font: FontSpec::default(),
        }
    }

    /// The font every label starts from: the theme's family and size.
    pub fn font(&self) -> FontSpec {
        // The `.label { font-family }` rule of every unified stylesheet, so
        // labels measure and draw in the theme family.
        FontSpec {
            family: Some(self.vars.font_family.clone()),
            size: Some(self.vars.font_size),
            weight: None,
            style: None,
        }
    }

    /// `node.useHtmlLabels || evaluate(getConfig()?.htmlLabels)`.
    pub fn use_html_labels(&self, node: &Node) -> bool {
        node.use_html_labels.unwrap_or(false) || self.html_labels
    }
}

/// What a shape handler returns: the node's `g` (shape geometry first,
/// label group last, untranslated) and the bounds `updateNodeBounds` wrote.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeOut {
    pub group: Group,
    pub width: f64,
    pub height: f64,
}

/// One entry of the `shapes` map in `shapes.ts`.
pub trait Shape: Sync + Send {
    /// The handler body. Mutates `node` like mermaid does (`width`,
    /// `height`, `intersect`, sometimes `label`) and returns the group.
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError>;

    /// `node.intersect(point)` for a node this shape drew.
    fn intersect(&self, node: &Node, point: Point) -> Point {
        node.intersect(point)
    }
}

/// Resolves a key of the `shapes` map in `shapes.ts`: every `shortName`,
/// `aliases` and `internalAliases` entry of `shapesDefs` plus the
/// `undocumentedShapes`. `None` is mermaid's `No such shape` error.
pub fn lookup(name: &str) -> Option<&'static dyn Shape> {
    let shape: &'static dyn Shape = match name {
        "rect" | "proc" | "process" | "rectangle" | "squareRect" => &square_rect::SquareRect,
        "rounded" | "event" | "roundedRect" => &rounded_rect::RoundedRect,
        "stadium" | "terminal" | "pill" => &stadium::Stadium,
        "fr-rect" | "subprocess" | "subproc" | "framed-rectangle" | "subroutine" => {
            &subroutine::Subroutine
        }
        "cyl" | "db" | "database" | "cylinder" => &cylinder::Cylinder,
        "datastore" | "data-store" => &datastore::Datastore,
        "folder" | "directory" => &folder::Folder,
        "bucket" => &bucket::Bucket,
        "console" => &console::ConsoleWindow,
        "browser" => &browser::Browser,
        "person" => &person::Person,
        "circle" | "circ" => &circle::Circle,
        "bang" => &bang::Bang,
        "cloud" => &cloud::Cloud,
        "diam" | "decision" | "diamond" | "question" => &question::Question,
        "hex" | "hexagon" | "prepare" => &hexagon::Hexagon,
        "lean-r" | "lean-right" | "in-out" | "lean_right" => &lean_right::LeanRight,
        "lean-l" | "lean-left" | "out-in" | "lean_left" => &lean_left::LeanLeft,
        "trap-b" | "priority" | "trapezoid-bottom" | "trapezoid" => &trapezoid::Trapezoid,
        "trap-t" | "manual" | "trapezoid-top" | "inv-trapezoid" | "inv_trapezoid" => {
            &inverted_trapezoid::InvTrapezoid
        }
        "dbl-circ" | "double-circle" | "doublecircle" => &double_circle::DoubleCircle,
        "text" => &text::TextShape,
        "notch-rect" | "card" | "notched-rectangle" => &card::Card,
        "lin-rect" | "lined-rectangle" | "lined-process" | "lin-proc" | "shaded-process" => {
            &shaded_process::ShadedProcess
        }
        "sm-circ" | "start" | "small-circle" | "stateStart" => &state_start::StateStart,
        "fr-circ" | "stop" | "framed-circle" | "stateEnd" => &state_end::StateEnd,
        "fork" | "join" | "forkJoin" => &fork_join::ForkJoin,
        "hourglass" | "collate" => &hourglass::Hourglass,
        "brace" | "comment" | "brace-l" => &curly_brace_left::CurlyBraceLeft,
        "brace-r" => &curly_brace_right::CurlyBraceRight,
        "braces" => &curly_braces::CurlyBraces,
        "bolt" | "com-link" | "lightning-bolt" => &lightning_bolt::LightningBolt,
        "doc" | "document" => &wave_edged_rectangle::WaveEdgedRectangle,
        "delay" | "half-rounded-rectangle" => &half_rounded_rectangle::HalfRoundedRectangle,
        "h-cyl" | "das" | "horizontal-cylinder" => &tilted_cylinder::TiltedCylinder,
        "lin-cyl" | "disk" | "lined-cylinder" => &lined_cylinder::LinedCylinder,
        "curv-trap" | "curved-trapezoid" | "display" => &curved_trapezoid::CurvedTrapezoid,
        "div-rect" | "div-proc" | "divided-rectangle" | "divided-process" => {
            &divided_rect::DividedRectangle
        }
        "tri" | "extract" | "triangle" => &triangle::Triangle,
        "win-pane" | "internal-storage" | "window-pane" => &window_pane::WindowPane,
        "f-circ" | "junction" | "filled-circle" => &filled_circle::FilledCircle,
        "notch-pent" | "loop-limit" | "notched-pentagon" => {
            &trapezoidal_pentagon::TrapezoidalPentagon
        }
        "flip-tri" | "manual-file" | "flipped-triangle" => &flipped_triangle::FlippedTriangle,
        "sl-rect" | "manual-input" | "sloped-rectangle" => &sloped_rect::SlopedRect,
        "docs" | "documents" | "st-doc" | "stacked-document" => {
            &multi_wave_edged_rectangle::MultiWaveEdgedRectangle
        }
        "st-rect" | "procs" | "processes" | "stacked-rectangle" => &multi_rect::MultiRect,
        "bow-rect" | "stored-data" | "bow-tie-rectangle" => &bow_tie_rect::BowTieRect,
        "cross-circ" | "summary" | "crossed-circle" => &crossed_circle::CrossedCircle,
        "tag-doc" | "tagged-document" => &tagged_wave_edged_rectangle::TaggedWaveEdgedRectangle,
        "tag-rect" | "tagged-rectangle" | "tag-proc" | "tagged-process" => &tagged_rect::TaggedRect,
        "flag" | "paper-tape" => &wave_rectangle::WaveRectangle,
        "odd" | "rect_left_inv_arrow" => &rect_left_inv_arrow::RectLeftInvArrow,
        "lin-doc" | "lined-document" => &lined_wave_edged_rect::LinedWaveEdgedRect,
        "state" => &state::State,
        "choice" => &choice::Choice,
        "note" => &note::Note,
        "composite" => &composite::Composite,
        "rectWithTitle" => &rect_with_title::RectWithTitle,
        "labelRect" => &label_rect::LabelRect,
        "block_arrow" => &block_arrow::BlockArrow,
        "collapsedGroup" => &collapsed_group::CollapsedGroup,
        "iconSquare" | "iconCircle" | "icon" | "iconRounded" => &icon::IconShape,
        "imageSquare" => &icon::ImageSquare,
        "anchor" => &anchor::Anchor,
        "kanbanItem" => &kanban_item::KanbanItem,
        "mindmapCircle" => &mindmap_circle::MindmapCircle,
        "defaultMindmapNode" => &default_mindmap_node::DefaultMindmapNode,
        "classBox" => &class_box::ClassBox,
        "erBox" => &er_box::ErBox,
        "requirementBox" => &requirement_box::RequirementBox,
        "usecaseActor" => &usecase_actor::UsecaseActor,
        "usecaseActorHollow" => &usecase_actor::UsecaseActorHollow,
        "usecaseActorAwesome" => &usecase_actor::UsecaseActorAwesome,
        "usecaseActorIcon" => &usecase_actor::UsecaseActorIcon,
        "usecaseBusiness" => &usecase_business::UsecaseBusiness,
        "usecaseEllipse" => &usecase_ellipse::UsecaseEllipse,
        "usecaseJsonTable" => &usecase_json_table::UsecaseJsonTable,
        _ => return None,
    };
    Some(shape)
}

/// Every key [`lookup`] accepts, for the completeness test.
pub const SHAPE_NAMES: &[&str] = &[
    "rect",
    "proc",
    "process",
    "rectangle",
    "squareRect",
    "rounded",
    "event",
    "roundedRect",
    "stadium",
    "terminal",
    "pill",
    "fr-rect",
    "subprocess",
    "subproc",
    "framed-rectangle",
    "subroutine",
    "cyl",
    "db",
    "database",
    "cylinder",
    "datastore",
    "data-store",
    "folder",
    "directory",
    "bucket",
    "console",
    "browser",
    "person",
    "circle",
    "circ",
    "bang",
    "cloud",
    "diam",
    "decision",
    "diamond",
    "question",
    "hex",
    "hexagon",
    "prepare",
    "lean-r",
    "lean-right",
    "in-out",
    "lean_right",
    "lean-l",
    "lean-left",
    "out-in",
    "lean_left",
    "trap-b",
    "priority",
    "trapezoid-bottom",
    "trapezoid",
    "trap-t",
    "manual",
    "trapezoid-top",
    "inv-trapezoid",
    "inv_trapezoid",
    "dbl-circ",
    "double-circle",
    "doublecircle",
    "text",
    "notch-rect",
    "card",
    "notched-rectangle",
    "lin-rect",
    "lined-rectangle",
    "lined-process",
    "lin-proc",
    "shaded-process",
    "sm-circ",
    "start",
    "small-circle",
    "stateStart",
    "fr-circ",
    "stop",
    "framed-circle",
    "stateEnd",
    "fork",
    "join",
    "forkJoin",
    "hourglass",
    "collate",
    "brace",
    "comment",
    "brace-l",
    "brace-r",
    "braces",
    "bolt",
    "com-link",
    "lightning-bolt",
    "doc",
    "document",
    "delay",
    "half-rounded-rectangle",
    "h-cyl",
    "das",
    "horizontal-cylinder",
    "lin-cyl",
    "disk",
    "lined-cylinder",
    "curv-trap",
    "curved-trapezoid",
    "display",
    "div-rect",
    "div-proc",
    "divided-rectangle",
    "divided-process",
    "tri",
    "extract",
    "triangle",
    "win-pane",
    "internal-storage",
    "window-pane",
    "f-circ",
    "junction",
    "filled-circle",
    "notch-pent",
    "loop-limit",
    "notched-pentagon",
    "flip-tri",
    "manual-file",
    "flipped-triangle",
    "sl-rect",
    "manual-input",
    "sloped-rectangle",
    "docs",
    "documents",
    "st-doc",
    "stacked-document",
    "st-rect",
    "procs",
    "processes",
    "stacked-rectangle",
    "bow-rect",
    "stored-data",
    "bow-tie-rectangle",
    "cross-circ",
    "summary",
    "crossed-circle",
    "tag-doc",
    "tagged-document",
    "tag-rect",
    "tagged-rectangle",
    "tag-proc",
    "tagged-process",
    "flag",
    "paper-tape",
    "odd",
    "rect_left_inv_arrow",
    "lin-doc",
    "lined-document",
    "state",
    "choice",
    "note",
    "composite",
    "rectWithTitle",
    "labelRect",
    "block_arrow",
    "collapsedGroup",
    "iconSquare",
    "iconCircle",
    "icon",
    "iconRounded",
    "imageSquare",
    "anchor",
    "kanbanItem",
    "mindmapCircle",
    "defaultMindmapNode",
    "classBox",
    "erBox",
    "requirementBox",
    "usecaseActor",
    "usecaseActorHollow",
    "usecaseActorAwesome",
    "usecaseActorIcon",
    "usecaseBusiness",
    "usecaseEllipse",
    "usecaseJsonTable",
];

/// `insertNode` of `rendering-elements/nodes.ts` without the DOM: the
/// `rect` special case (`rx && ry` selects `roundedRect`, else
/// `squareRect`), the shape lookup, and the look guard.
pub fn insert_node(node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
    match node.look {
        Look::Classic => {}
        Look::HandDrawn => {
            return Err(HelperError::UnsupportedFeature {
                feature: "handDrawn look".to_string(),
            });
        }
        Look::Neo => {
            return Err(HelperError::UnsupportedFeature {
                feature: "neo look".to_string(),
            });
        }
    }
    if node.shape == "rect" {
        node.shape = if node.rx.is_some_and(|v| v != 0.0) && node.ry.is_some_and(|v| v != 0.0) {
            "roundedRect".to_string()
        } else {
            "squareRect".to_string()
        };
    }
    let shape = lookup(&node.shape).ok_or_else(|| {
        HelperError::internal(format!(
            "No such shape: {}. Please check your syntax.",
            node.shape
        ))
    })?;
    shape.draw(node, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_name_resolves() {
        for name in SHAPE_NAMES {
            assert!(lookup(name).is_some(), "{name} is not registered");
        }
        assert!(lookup("no-such-shape").is_none());
    }

    #[test]
    fn rect_special_case_picks_rounded_when_both_radii_set() {
        let m = FixedMeasurer::SNAPSHOT;
        let ctx = ShapeCtx::new(&m);
        let mut node = Node::flowchart("A", "A", "rect");
        node.rx = Some(5.0);
        node.ry = Some(5.0);
        insert_node(&mut node, &ctx).expect("draws");
        assert_eq!(node.shape, "roundedRect");
        let mut node = Node::flowchart("A", "A", "rect");
        insert_node(&mut node, &ctx).expect("draws");
        assert_eq!(node.shape, "squareRect");
    }

    #[test]
    fn hand_drawn_is_unsupported() {
        let m = FixedMeasurer::SNAPSHOT;
        let ctx = ShapeCtx::new(&m);
        let mut node = Node::flowchart("A", "A", "rect");
        node.look = Look::HandDrawn;
        let err = insert_node(&mut node, &ctx).expect_err("refused");
        assert_eq!(err.code(), "unsupported_feature");
    }
}
