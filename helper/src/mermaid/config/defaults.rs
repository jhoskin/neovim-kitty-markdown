//! `MermaidConfig` with every default of mermaid 12.2.0's
//! `config.schema.yaml` as `defaultConfig.ts` ships it. The values are the
//! ones `mermaidAPI.defaultConfig` holds in the oracle install
//! (`tests/goldens/themes/default-config.json`); a test keeps the two equal.
//!
//! Phase 1 and 2 sections are typed. The others stay JSON until their phase
//! lands, so a directive can still reach them through the merge.

use std::fmt;
use std::ops::Deref;

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Map, Value, json};

/// A JS number: prints as an integer when integral, so `50` stays `50`
/// and a user's `50.5` survives. Every numeric config value uses it.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Num(pub f64);

impl Deref for Num {
    type Target = f64;
    fn deref(&self) -> &f64 {
        &self.0
    }
}

impl From<f64> for Num {
    fn from(v: f64) -> Num {
        Num(v)
    }
}

impl From<i32> for Num {
    fn from(v: i32) -> Num {
        Num(f64::from(v))
    }
}

impl fmt::Display for Num {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", crate::mermaid::theme::khroma::js_number(self.0))
    }
}

impl Serialize for Num {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if self.0.fract() == 0.0 && self.0.abs() < 9007199254740992.0 {
            s.serialize_i64(self.0 as i64)
        } else {
            s.serialize_f64(self.0)
        }
    }
}

impl<'de> Deserialize<'de> for Num {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Num, D::Error> {
        let v = Value::deserialize(d)?;
        match v.as_f64() {
            Some(f) => Ok(Num(f)),
            None => Err(de::Error::custom(format!("expected a number, got {v}"))),
        }
    }
}

fn n(v: f64) -> Num {
    Num(v)
}

fn s(v: &str) -> String {
    v.to_string()
}

/// `flowchart`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FlowchartConfig {
    pub use_max_width: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub look: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    pub title_top_margin: Num,
    pub sub_graph_title_margin: SubGraphTitleMargin,
    pub diagram_padding: Num,
    /// `null` in the schema: deprecated in favor of the top-level
    /// `htmlLabels`, read through `getEffectiveHtmlLabels`.
    pub html_labels: Value,
    pub node_spacing: Num,
    pub rank_spacing: Num,
    pub curve: String,
    pub padding: Num,
    pub wrapping_width: Num,
    pub min_node_width: Num,
    pub inherit_dir: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SubGraphTitleMargin {
    pub top: Num,
    pub bottom: Num,
}

impl Default for SubGraphTitleMargin {
    fn default() -> Self {
        SubGraphTitleMargin {
            top: n(0.0),
            bottom: n(0.0),
        }
    }
}

impl Default for FlowchartConfig {
    fn default() -> Self {
        FlowchartConfig {
            use_max_width: true,
            theme: Some(s("redux-color")),
            look: Some(s("neo")),
            layout: None,
            title_top_margin: n(25.0),
            sub_graph_title_margin: SubGraphTitleMargin::default(),
            diagram_padding: n(8.0),
            html_labels: Value::Null,
            node_spacing: n(50.0),
            rank_spacing: n(50.0),
            curve: s("basis"),
            padding: n(15.0),
            wrapping_width: n(120.0),
            min_node_width: n(120.0),
            inherit_dir: false,
            extra: Map::new(),
        }
    }
}

/// `sequence`. The `messageFont`, `noteFont`, and `actorFont` functions of
/// `defaultConfig.ts` are [`SequenceConfig::message_font`] and friends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SequenceConfig {
    pub use_max_width: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub look: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    pub hide_unused_participants: bool,
    pub activation_width: Num,
    pub diagram_margin_x: Num,
    pub diagram_margin_y: Num,
    pub actor_margin: Num,
    pub width: Num,
    pub height: Num,
    pub box_margin: Num,
    pub box_text_margin: Num,
    pub note_margin: Num,
    pub message_margin: Num,
    pub message_align: String,
    pub mirror_actors: bool,
    pub force_menus: bool,
    pub bottom_margin_adj: Num,
    pub right_angles: bool,
    pub show_sequence_numbers: bool,
    pub actor_font_size: Num,
    pub actor_font_family: String,
    pub actor_font_weight: Value,
    pub note_font_size: Num,
    pub note_font_family: String,
    pub note_font_weight: Value,
    pub note_align: String,
    pub message_font_size: Num,
    pub message_font_family: String,
    pub message_font_weight: Value,
    pub wrap: bool,
    pub wrap_padding: Num,
    pub label_box_width: Num,
    pub label_box_height: Num,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A font triple as the `*Font()` functions return it.
#[derive(Debug, Clone, PartialEq)]
pub struct Font {
    pub font_family: String,
    pub font_size: Num,
    pub font_weight: Value,
}

impl SequenceConfig {
    pub fn message_font(&self) -> Font {
        Font {
            font_family: self.message_font_family.clone(),
            font_size: self.message_font_size,
            font_weight: self.message_font_weight.clone(),
        }
    }

    pub fn note_font(&self) -> Font {
        Font {
            font_family: self.note_font_family.clone(),
            font_size: self.note_font_size,
            font_weight: self.note_font_weight.clone(),
        }
    }

    pub fn actor_font(&self) -> Font {
        Font {
            font_family: self.actor_font_family.clone(),
            font_size: self.actor_font_size,
            font_weight: self.actor_font_weight.clone(),
        }
    }
}

impl Default for SequenceConfig {
    fn default() -> Self {
        SequenceConfig {
            use_max_width: true,
            theme: Some(s("redux-color")),
            look: Some(s("neo")),
            layout: None,
            hide_unused_participants: false,
            activation_width: n(10.0),
            diagram_margin_x: n(50.0),
            diagram_margin_y: n(10.0),
            actor_margin: n(50.0),
            width: n(150.0),
            height: n(65.0),
            box_margin: n(10.0),
            box_text_margin: n(5.0),
            note_margin: n(10.0),
            message_margin: n(35.0),
            message_align: s("center"),
            mirror_actors: true,
            force_menus: false,
            bottom_margin_adj: n(1.0),
            right_angles: false,
            show_sequence_numbers: false,
            actor_font_size: n(14.0),
            actor_font_family: s("\"Open Sans\", sans-serif"),
            actor_font_weight: json!(400),
            note_font_size: n(14.0),
            note_font_family: s("\"trebuchet ms\", verdana, arial, sans-serif"),
            note_font_weight: json!(400),
            note_align: s("center"),
            message_font_size: n(16.0),
            message_font_family: s("\"trebuchet ms\", verdana, arial, sans-serif"),
            message_font_weight: json!(400),
            wrap: false,
            wrap_padding: n(10.0),
            label_box_width: n(50.0),
            label_box_height: n(20.0),
            extra: Map::new(),
        }
    }
}

/// `class`, as `defaultConfig.ts` builds it by hand: no `padding`, no
/// `useMaxWidth`, an explicit `layout: undefined`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClassConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub look: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    pub hide_empty_members_box: bool,
    pub hierarchical_namespaces: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for ClassConfig {
    fn default() -> Self {
        ClassConfig {
            theme: Some(s("redux-color")),
            look: Some(s("neo")),
            layout: None,
            hide_empty_members_box: false,
            hierarchical_namespaces: true,
            extra: Map::new(),
        }
    }
}

/// `state`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StateConfig {
    pub use_max_width: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub look: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    pub wrapping_width: Num,
    pub min_node_width: Num,
    pub title_top_margin: Num,
    pub divider_margin: Num,
    pub size_unit: Num,
    pub padding: Num,
    pub text_height: Num,
    pub title_shift: Num,
    pub note_margin: Num,
    pub fork_width: Num,
    pub fork_height: Num,
    pub mini_padding: Num,
    pub font_size_factor: Num,
    pub font_size: Num,
    pub label_height: Num,
    pub edge_length_factor: String,
    pub composit_title_size: Num,
    pub radius: Num,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for StateConfig {
    fn default() -> Self {
        StateConfig {
            use_max_width: true,
            theme: Some(s("redux-color")),
            look: Some(s("neo")),
            layout: Some(s("elk")),
            wrapping_width: n(120.0),
            min_node_width: n(120.0),
            title_top_margin: n(25.0),
            divider_margin: n(10.0),
            size_unit: n(5.0),
            padding: n(8.0),
            text_height: n(10.0),
            title_shift: n(-15.0),
            note_margin: n(10.0),
            fork_width: n(70.0),
            fork_height: n(7.0),
            mini_padding: n(2.0),
            font_size_factor: n(5.02),
            font_size: n(24.0),
            label_height: n(16.0),
            edge_length_factor: s("20"),
            composit_title_size: n(35.0),
            radius: n(5.0),
            extra: Map::new(),
        }
    }
}

/// `er`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ErConfig {
    pub use_max_width: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub look: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    pub title_top_margin: Num,
    pub diagram_padding: Num,
    pub layout_direction: String,
    pub min_entity_width: Num,
    pub min_entity_height: Num,
    pub entity_padding: Num,
    pub node_spacing: Num,
    pub rank_spacing: Num,
    pub stroke: String,
    pub fill: String,
    pub font_size: Num,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for ErConfig {
    fn default() -> Self {
        ErConfig {
            use_max_width: true,
            theme: Some(s("redux-color")),
            look: Some(s("neo")),
            layout: None,
            title_top_margin: n(25.0),
            diagram_padding: n(20.0),
            layout_direction: s("TB"),
            min_entity_width: n(100.0),
            min_entity_height: n(75.0),
            entity_padding: n(15.0),
            node_spacing: n(140.0),
            rank_spacing: n(80.0),
            stroke: s("gray"),
            fill: s("honeydew"),
            font_size: n(12.0),
            extra: Map::new(),
        }
    }
}

/// `pie`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PieConfig {
    pub use_max_width: bool,
    pub text_position: Num,
    pub donut_hole: Num,
    pub legend_position: String,
    pub highlight_slice: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_width: Option<Num>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for PieConfig {
    fn default() -> Self {
        PieConfig {
            use_max_width: true,
            text_position: n(0.75),
            donut_hole: n(0.0),
            legend_position: s("right"),
            highlight_slice: s(""),
            use_width: Some(n(984.0)),
            extra: Map::new(),
        }
    }
}

/// `gantt`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GanttConfig {
    pub use_max_width: bool,
    pub title_top_margin: Num,
    pub bar_height: Num,
    pub bar_gap: Num,
    pub top_padding: Num,
    pub right_padding: Num,
    pub left_padding: Num,
    pub grid_line_start_padding: Num,
    pub font_size: Num,
    pub section_font_size: Num,
    pub number_section_styles: Num,
    pub axis_format: String,
    pub top_axis: bool,
    pub display_mode: String,
    pub weekday: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tick_interval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_width: Option<Num>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for GanttConfig {
    fn default() -> Self {
        GanttConfig {
            use_max_width: true,
            title_top_margin: n(25.0),
            bar_height: n(20.0),
            bar_gap: n(4.0),
            top_padding: n(50.0),
            right_padding: n(75.0),
            left_padding: n(75.0),
            grid_line_start_padding: n(35.0),
            font_size: n(11.0),
            section_font_size: n(11.0),
            number_section_styles: n(4.0),
            axis_format: s("%Y-%m-%d"),
            top_axis: false,
            display_mode: s(""),
            weekday: s("sunday"),
            tick_interval: None,
            use_width: None,
            extra: Map::new(),
        }
    }
}

/// `xyChart.xAxis` and `xyChart.yAxis`. The `$ref` key is what the
/// schema-to-defaults build leaves in `defaultConfig.ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XyChartAxisConfig {
    #[serde(rename = "$ref")]
    pub schema_ref: String,
    pub show_label: bool,
    pub label_font_size: Num,
    pub label_padding: Num,
    pub show_title: bool,
    pub title_font_size: Num,
    pub title_padding: Num,
    pub show_tick: bool,
    pub tick_length: Num,
    pub tick_width: Num,
    pub show_axis_line: bool,
    pub axis_line_width: Num,
    pub label_rotation: Num,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for XyChartAxisConfig {
    fn default() -> Self {
        XyChartAxisConfig {
            schema_ref: s("#/$defs/XYChartAxisConfig"),
            show_label: true,
            label_font_size: n(14.0),
            label_padding: n(5.0),
            show_title: true,
            title_font_size: n(16.0),
            title_padding: n(5.0),
            show_tick: true,
            tick_length: n(5.0),
            tick_width: n(2.0),
            show_axis_line: true,
            axis_line_width: n(2.0),
            label_rotation: n(0.0),
            extra: Map::new(),
        }
    }
}

/// `xyChart`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XyChartConfig {
    pub use_max_width: bool,
    pub width: Num,
    pub height: Num,
    pub title_font_size: Num,
    pub title_padding: Num,
    pub show_data_label: bool,
    pub show_data_label_outside_bar: bool,
    pub show_title: bool,
    pub show_legend: bool,
    pub legend_font_size: Num,
    pub legend_padding: Num,
    pub x_axis: XyChartAxisConfig,
    pub y_axis: XyChartAxisConfig,
    pub chart_orientation: String,
    pub plot_reserved_space_percent: Num,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_width: Option<Num>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for XyChartConfig {
    fn default() -> Self {
        XyChartConfig {
            use_max_width: true,
            width: n(700.0),
            height: n(500.0),
            title_font_size: n(20.0),
            title_padding: n(10.0),
            show_data_label: false,
            show_data_label_outside_bar: false,
            show_title: true,
            show_legend: true,
            legend_font_size: n(14.0),
            legend_padding: n(10.0),
            x_axis: XyChartAxisConfig::default(),
            y_axis: XyChartAxisConfig::default(),
            chart_orientation: s("vertical"),
            plot_reserved_space_percent: n(50.0),
            use_width: None,
            extra: Map::new(),
        }
    }
}

/// `quadrantChart`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct QuadrantChartConfig {
    pub use_max_width: bool,
    pub chart_width: Num,
    pub chart_height: Num,
    pub title_font_size: Num,
    pub title_padding: Num,
    pub quadrant_padding: Num,
    pub x_axis_label_padding: Num,
    pub y_axis_label_padding: Num,
    pub x_axis_label_font_size: Num,
    pub y_axis_label_font_size: Num,
    pub quadrant_label_font_size: Num,
    pub quadrant_text_top_padding: Num,
    pub point_text_padding: Num,
    pub point_label_font_size: Num,
    pub point_radius: Num,
    pub x_axis_position: String,
    pub y_axis_position: String,
    pub quadrant_internal_border_stroke_width: Num,
    pub quadrant_external_border_stroke_width: Num,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for QuadrantChartConfig {
    fn default() -> Self {
        QuadrantChartConfig {
            use_max_width: true,
            chart_width: n(500.0),
            chart_height: n(500.0),
            title_font_size: n(20.0),
            title_padding: n(10.0),
            quadrant_padding: n(5.0),
            x_axis_label_padding: n(5.0),
            y_axis_label_padding: n(5.0),
            x_axis_label_font_size: n(16.0),
            y_axis_label_font_size: n(16.0),
            quadrant_label_font_size: n(16.0),
            quadrant_text_top_padding: n(5.0),
            point_text_padding: n(5.0),
            point_label_font_size: n(12.0),
            point_radius: n(5.0),
            x_axis_position: s("top"),
            y_axis_position: s("left"),
            quadrant_internal_border_stroke_width: n(1.0),
            quadrant_external_border_stroke_width: n(2.0),
            extra: Map::new(),
        }
    }
}

/// The top-level `MermaidConfig`. `themeVariables` is not a field: the
/// derived variables travel as [`crate::mermaid::theme::ThemeVariables`]
/// next to the config.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MermaidConfig {
    pub theme: String,
    #[serde(rename = "themeCSS", skip_serializing_if = "Option::is_none")]
    pub theme_css: Option<String>,
    pub look: String,
    pub hand_drawn_seed: Num,
    pub layout: String,
    pub max_text_size: Num,
    pub max_edges: Num,
    pub dark_mode: bool,
    pub font_family: String,
    pub log_level: Value,
    pub security_level: String,
    pub start_on_load: bool,
    pub arrow_marker_absolute: bool,
    pub secure: Vec<String>,
    #[serde(rename = "legacyMathML")]
    pub legacy_math_ml: bool,
    #[serde(rename = "forceLegacyMathML")]
    pub force_legacy_math_ml: bool,
    pub deterministic_ids: bool,
    #[serde(
        rename = "deterministicIDSeed",
        skip_serializing_if = "Option::is_none"
    )]
    pub deterministic_id_seed: Option<String>,
    pub font_size: Num,
    pub markdown_auto_wrap: bool,
    pub suppress_error_rendering: bool,
    /// Not in the defaults; set by a user and read through
    /// `getEffectiveHtmlLabels`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html_labels: Option<bool>,
    /// Not in the defaults; `%%{wrap}%%` sets it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap: Option<bool>,
    pub flowchart: FlowchartConfig,
    pub sequence: SequenceConfig,
    pub class: ClassConfig,
    pub state: StateConfig,
    pub er: ErConfig,
    pub pie: PieConfig,
    pub gantt: GanttConfig,
    pub xy_chart: XyChartConfig,
    pub quadrant_chart: QuadrantChartConfig,
    pub elk: Value,
    pub swimlane: Value,
    pub agentflow: Value,
    pub journey: Value,
    pub requirement: Value,
    pub mindmap: Value,
    pub ishikawa: Value,
    pub kanban: Value,
    pub timeline: Value,
    pub git_graph: Value,
    pub c4: Value,
    pub sankey: Value,
    pub block: Value,
    pub packet: Value,
    pub tree_view: Value,
    pub architecture: Value,
    pub eventmodeling: Value,
    pub radar: Value,
    pub usecase: Value,
    pub venn: Value,
    pub cynefin: Value,
    pub railroad: Value,
    pub treemap: Value,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for MermaidConfig {
    fn default() -> Self {
        let u = untyped_defaults();
        let section = |k: &str| u.get(k).cloned().unwrap_or(Value::Object(Map::new()));
        MermaidConfig {
            theme: s("default"),
            theme_css: None,
            look: s("classic"),
            hand_drawn_seed: n(0.0),
            layout: s("elk"),
            max_text_size: n(50000.0),
            max_edges: n(500.0),
            dark_mode: false,
            font_family: s("\"trebuchet ms\", verdana, arial, sans-serif;"),
            log_level: json!(5),
            security_level: s("strict"),
            start_on_load: true,
            arrow_marker_absolute: false,
            secure: [
                "secure",
                "securityLevel",
                "startOnLoad",
                "maxTextSize",
                "suppressErrorRendering",
                "maxEdges",
            ]
            .into_iter()
            .map(s)
            .collect(),
            legacy_math_ml: false,
            force_legacy_math_ml: false,
            deterministic_ids: false,
            deterministic_id_seed: None,
            font_size: n(16.0),
            markdown_auto_wrap: true,
            suppress_error_rendering: false,
            html_labels: None,
            wrap: None,
            flowchart: FlowchartConfig::default(),
            sequence: SequenceConfig::default(),
            class: ClassConfig::default(),
            state: StateConfig::default(),
            er: ErConfig::default(),
            pie: PieConfig::default(),
            gantt: GanttConfig::default(),
            xy_chart: XyChartConfig::default(),
            quadrant_chart: QuadrantChartConfig::default(),
            elk: section("elk"),
            swimlane: section("swimlane"),
            agentflow: section("agentflow"),
            journey: section("journey"),
            requirement: section("requirement"),
            mindmap: section("mindmap"),
            ishikawa: section("ishikawa"),
            kanban: section("kanban"),
            timeline: section("timeline"),
            git_graph: section("gitGraph"),
            c4: section("c4"),
            sankey: section("sankey"),
            block: section("block"),
            packet: section("packet"),
            tree_view: section("treeView"),
            architecture: section("architecture"),
            eventmodeling: section("eventmodeling"),
            radar: section("radar"),
            usecase: section("usecase"),
            venn: section("venn"),
            cynefin: section("cynefin"),
            railroad: section("railroad"),
            treemap: section("treemap"),
            extra: Map::new(),
        }
    }
}

impl MermaidConfig {
    /// The config as JSON, with the same keys `JSON.stringify` prints for
    /// `mermaidAPI.defaultConfig`.
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    /// `getEffectiveHtmlLabels`: the top-level `htmlLabels`, then the
    /// deprecated `flowchart.htmlLabels`, then `true`, through mermaid's
    /// `evaluate` (`'false'`, `'null'`, `'0'`, and `false` are off).
    pub fn effective_html_labels(&self) -> bool {
        let raw = match self.html_labels {
            Some(b) => Value::Bool(b),
            None => self.flowchart.html_labels.clone(),
        };
        match raw {
            Value::Null => true,
            Value::Bool(b) => b,
            other => {
                let text = match &other {
                    Value::String(t) => t.clone(),
                    v => v.to_string(),
                };
                !matches!(text.trim().to_lowercase().as_str(), "false" | "null" | "0")
            }
        }
    }
}

/// The keys of `defaultConfig.ts` whose values are functions or
/// `undefined`, which `JSON.stringify` drops but `configKeys` keeps.
pub const UNSERIALIZED_KEYS: &[&str] = &[
    "actorFont",
    "boundaryFont",
    "commentFill",
    "commentStroke",
    "commentTextColor",
    "componentFont",
    "component_dbFont",
    "component_queueFont",
    "containerFont",
    "container_dbFont",
    "container_queueFont",
    "cycleBreakingStrategy",
    "deterministicIDSeed",
    "external_componentFont",
    "external_component_dbFont",
    "external_component_queueFont",
    "external_containerFont",
    "external_container_dbFont",
    "external_container_queueFont",
    "external_personFont",
    "external_systemFont",
    "external_system_dbFont",
    "external_system_queueFont",
    "layeringStrategy",
    "markerFill",
    "messageFont",
    "nodeColors",
    "nodePlacementAlignment",
    "nodePlacementStrategy",
    "nonTerminalFill",
    "nonTerminalStroke",
    "nonTerminalTextColor",
    "noteFont",
    "personFont",
    "ruleNameColor",
    "specialFill",
    "specialStroke",
    "systemFont",
    "system_dbFont",
    "system_queueFont",
    "terminalFill",
    "terminalStroke",
    "terminalTextColor",
    "themeCSS",
    "tickInterval",
];

/// `mermaidAPI.defaultConfig` as JSON (without `themeVariables`).
pub fn default_config_value() -> Value {
    MermaidConfig::default().to_value()
}

/// The sections that stay JSON until their phase lands.
fn untyped_defaults() -> Map<String, Value> {
    let open_sans = "\"Open Sans\", sans-serif";
    let actor_colours = json!([
        "#8FBC8F", "#7CFC00", "#00FFFF", "#20B2AA", "#B0E0E6", "#FFFFE0"
    ]);
    let section_fills = json!([
        "#191970", "#8B008B", "#4B0082", "#2F4F4F", "#800000", "#8B4513", "#00008B"
    ]);
    let c4_fonts = |m: &mut Map<String, Value>| {
        for shape in [
            "person",
            "external_person",
            "system",
            "external_system",
            "system_db",
            "external_system_db",
            "system_queue",
            "external_system_queue",
            "boundary",
            "message",
            "container",
            "external_container",
            "container_db",
            "external_container_db",
            "container_queue",
            "external_container_queue",
            "component",
            "external_component",
            "component_db",
            "external_component_db",
            "component_queue",
            "external_component_queue",
        ] {
            let size = if shape == "message" { 12 } else { 14 };
            m.insert(format!("{shape}FontSize"), json!(size));
            m.insert(format!("{shape}FontFamily"), json!(open_sans));
            m.insert(format!("{shape}FontWeight"), json!("normal"));
        }
    };
    let mut c4 = Map::new();
    for (k, v) in [
        ("useMaxWidth", json!(true)),
        ("diagramMarginX", json!(50)),
        ("diagramMarginY", json!(10)),
        ("c4ShapeMargin", json!(50)),
        ("c4ShapePadding", json!(20)),
        ("width", json!(216)),
        ("height", json!(60)),
        ("boxMargin", json!(10)),
        ("c4ShapeInRow", json!(4)),
        ("nextLinePaddingX", json!(0)),
        ("c4BoundaryInRow", json!(2)),
    ] {
        c4.insert(k.to_string(), v);
    }
    c4_fonts(&mut c4);
    for (k, v) in [
        ("wrap", json!(true)),
        ("wrapPadding", json!(10)),
        ("person_bg_color", json!("#08427B")),
        ("person_border_color", json!("#073B6F")),
        ("external_person_bg_color", json!("#686868")),
        ("external_person_border_color", json!("#8A8A8A")),
        ("system_bg_color", json!("#1168BD")),
        ("system_border_color", json!("#3C7FC0")),
        ("system_db_bg_color", json!("#1168BD")),
        ("system_db_border_color", json!("#3C7FC0")),
        ("system_queue_bg_color", json!("#1168BD")),
        ("system_queue_border_color", json!("#3C7FC0")),
        ("external_system_bg_color", json!("#999999")),
        ("external_system_border_color", json!("#8A8A8A")),
        ("external_system_db_bg_color", json!("#999999")),
        ("external_system_db_border_color", json!("#8A8A8A")),
        ("external_system_queue_bg_color", json!("#999999")),
        ("external_system_queue_border_color", json!("#8A8A8A")),
        ("container_bg_color", json!("#438DD5")),
        ("container_border_color", json!("#3C7FC0")),
        ("container_db_bg_color", json!("#438DD5")),
        ("container_db_border_color", json!("#3C7FC0")),
        ("container_queue_bg_color", json!("#438DD5")),
        ("container_queue_border_color", json!("#3C7FC0")),
        ("external_container_bg_color", json!("#B3B3B3")),
        ("external_container_border_color", json!("#A6A6A6")),
        ("external_container_db_bg_color", json!("#B3B3B3")),
        ("external_container_db_border_color", json!("#A6A6A6")),
        ("external_container_queue_bg_color", json!("#B3B3B3")),
        ("external_container_queue_border_color", json!("#A6A6A6")),
        ("component_bg_color", json!("#85BBF0")),
        ("component_border_color", json!("#78A8D8")),
        ("component_db_bg_color", json!("#85BBF0")),
        ("component_db_border_color", json!("#78A8D8")),
        ("component_queue_bg_color", json!("#85BBF0")),
        ("component_queue_border_color", json!("#78A8D8")),
        ("external_component_bg_color", json!("#CCCCCC")),
        ("external_component_border_color", json!("#BFBFBF")),
        ("external_component_db_bg_color", json!("#CCCCCC")),
        ("external_component_db_border_color", json!("#BFBFBF")),
        ("external_component_queue_bg_color", json!("#CCCCCC")),
        ("external_component_queue_border_color", json!("#BFBFBF")),
        // `defaultConfig.ts` nests a full flowchart config with
        // `inheritDir: false` under `c4`.
        (
            "flowchart",
            serde_json::to_value(FlowchartConfig::default()).unwrap_or(Value::Null),
        ),
    ] {
        c4.insert(k.to_string(), v);
    }

    let mut out = Map::new();
    let sections = json!({
        "elk": {
            "mergeEdges": false,
            "straightenEdges": true,
            "lineHops": true,
            "preset": "default",
            "layeringLayerBound": 4,
            "forceNodeModelOrder": false,
            "considerModelOrder": "NODES_AND_EDGES",
            "keepEntryNodeOnTop": false,
            "orientFeedbackEdges": true
        },
        "swimlane": {
            "useMaxWidth": true,
            "theme": "redux-color",
            "look": "neo",
            "layout": "swimlane",
            "lineHops": "arc",
            "ignoreCrossLaneEdges": true,
            "optimizeRanksByCrossings": true,
            "automaticLaneOrdering": false
        },
        "agentflow": {
            "useMaxWidth": true,
            "theme": "redux-color",
            "look": "neo",
            "titleTopMargin": 25,
            "diagramPadding": 8,
            "nodeSpacing": 50,
            "rankSpacing": 50,
            "wrappingWidth": 120,
            "minNodeWidth": 120
        },
        "journey": {
            "useMaxWidth": true,
            "diagramMarginX": 50,
            "diagramMarginY": 10,
            "leftMargin": 150,
            "maxLabelWidth": 360,
            "width": 150,
            "height": 50,
            "boxMargin": 10,
            "boxTextMargin": 5,
            "noteMargin": 10,
            "messageMargin": 35,
            "messageAlign": "center",
            "bottomMarginAdj": 1,
            "rightAngles": false,
            "taskFontSize": 14,
            "taskFontFamily": open_sans,
            "taskMargin": 50,
            "activationWidth": 10,
            "textPlacement": "fo",
            "actorColours": actor_colours,
            "sectionFills": section_fills,
            "sectionColours": ["#fff"],
            "titleColor": "",
            "titleFontFamily": "\"trebuchet ms\", verdana, arial, sans-serif",
            "titleFontSize": "4ex"
        },
        "requirement": {
            "useMaxWidth": true,
            "theme": "redux-color",
            "look": "neo",
            "rect_fill": "#f9f9f9",
            "text_color": "#333",
            "rect_border_size": "0.5px",
            "rect_border_color": "#bbb",
            "rect_min_width": 200,
            "rect_min_height": 200,
            "fontSize": 14,
            "rect_padding": 10,
            "line_height": 20
        },
        "mindmap": {
            "useMaxWidth": true,
            "colorBy": "branch",
            "padding": 10,
            "maxNodeWidth": 200,
            "layoutAlgorithm": "cose-bilkent"
        },
        "ishikawa": { "useMaxWidth": true, "diagramPadding": 20 },
        "kanban": { "useMaxWidth": true, "padding": 8, "sectionWidth": 200, "ticketBaseUrl": "" },
        "timeline": {
            "useMaxWidth": true,
            "diagramMarginX": 50,
            "diagramMarginY": 10,
            "leftMargin": 150,
            "width": 150,
            "height": 50,
            "boxMargin": 10,
            "boxTextMargin": 5,
            "noteMargin": 10,
            "messageMargin": 35,
            "messageAlign": "center",
            "bottomMarginAdj": 1,
            "rightAngles": false,
            "taskFontSize": 14,
            "taskFontFamily": open_sans,
            "taskMargin": 50,
            "activationWidth": 10,
            "textPlacement": "fo",
            "actorColours": actor_colours,
            "sectionFills": section_fills,
            "sectionColours": ["#fff"],
            "disableMulticolor": false
        },
        "gitGraph": {
            "useMaxWidth": true,
            "titleTopMargin": 25,
            "diagramPadding": 8,
            "nodeLabel": { "width": 75, "height": 100, "x": -25, "y": 0 },
            "mainBranchName": "main",
            "mainBranchOrder": 0,
            "showCommitLabel": true,
            "showBranches": true,
            "rotateCommitLabel": true,
            "parallelCommits": false,
            "arrowMarkerAbsolute": false
        },
        "c4": c4,
        "sankey": {
            "useMaxWidth": true,
            "width": 600,
            "height": 400,
            "linkColor": "gradient",
            "nodeAlignment": "justify",
            "showValues": true,
            "prefix": "",
            "suffix": "",
            "nodeWidth": 10,
            "nodePadding": 12,
            "labelStyle": "legacy"
        },
        "block": { "useMaxWidth": true, "padding": 8 },
        "packet": {
            "useMaxWidth": true,
            "rowHeight": 32,
            "bitWidth": 32,
            "bitsPerRow": 32,
            "showBits": true,
            "bitOrder": "ascending",
            "paddingX": 5,
            "paddingY": 5
        },
        "treeView": {
            "useMaxWidth": true,
            "rowIndent": 10,
            "paddingX": 5,
            "paddingY": 5,
            "lineThickness": 1,
            "showIcons": false,
            "defaultIconPack": "",
            "filenameIcons": {},
            "extensionIcons": {}
        },
        "architecture": {
            "useMaxWidth": true,
            "padding": 40,
            "iconSize": 80,
            "fontSize": 16,
            "randomize": false,
            "nodeSeparation": 75,
            "idealEdgeLengthMultiplier": 1.5,
            "edgeElasticity": 0.45,
            "numIter": 2500,
            "seed": 1
        },
        "eventmodeling": { "useMaxWidth": true, "padding": 30, "rowHeight": 32 },
        "radar": {
            "useMaxWidth": true,
            "width": 600,
            "height": 600,
            "marginTop": 50,
            "marginRight": 50,
            "marginBottom": 50,
            "marginLeft": 50,
            "axisScaleFactor": 1,
            "axisLabelFactor": 1.05,
            "curveTension": 0.17
        },
        "usecase": {
            "useMaxWidth": true,
            "theme": "redux-color",
            "look": "neo",
            "wrappingWidth": 120,
            "minNodeWidth": 120,
            "actorFontSize": 14,
            "actorFontFamily": open_sans,
            "actorFontWeight": "normal",
            "usecaseFontSize": 12,
            "usecaseFontFamily": open_sans,
            "usecaseFontWeight": "normal",
            "nodeSpacing": 50,
            "rankSpacing": 50,
            "diagramPadding": 20,
            "colorScheme": "role"
        },
        "venn": {
            "useMaxWidth": true,
            "theme": "redux-color",
            "look": "neo",
            "width": 800,
            "height": 450,
            "padding": 8,
            "useDebugLayout": false
        },
        "cynefin": {
            "useMaxWidth": true,
            "width": 800,
            "height": 600,
            "padding": 40,
            "showDomainDescriptions": true,
            "boundaryAmplitude": 8,
            "seed": 0
        },
        "railroad": {},
        "treemap": {
            "useMaxWidth": true,
            "padding": 10,
            "diagramPadding": 8,
            "showValues": true,
            "nodeWidth": 100,
            "nodeHeight": 40,
            "borderWidth": 1,
            "valueFontSize": 12,
            "labelFontSize": 14,
            "valueFormat": ","
        }
    });
    if let Value::Object(map) = sections {
        out = map;
    }
    out
}
