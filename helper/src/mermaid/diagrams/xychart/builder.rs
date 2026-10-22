//! Port of `chartBuilder/`: the orchestrator and its components (title,
//! legend, band and linear axes, bar and line plots) down to the
//! `DrawableElem` list the renderer turns into SVG.
//!
//! Every method keeps the name and the arithmetic of its TypeScript
//! original, so a behavior question is answered by reading the two side by
//! side. Text is measured through [`TextDimensionCalculator`], the port of
//! `TextDimensionCalculatorWithFont`.

use std::f64::consts::PI;

use crate::mermaid::config::{XyChartAxisConfig, XyChartConfig};
use crate::mermaid::layout::d3::scale_band::ScaleBand;
use crate::mermaid::layout::d3::scale_linear::ScaleLinear;
use crate::mermaid::scene::{Color, PathData, Point, curves};
use crate::mermaid::styles::xychart::{AxisTheme, XyChartTheme};
use crate::mermaid::theme::khroma::js_number;

use super::db::{AxisData, Db, Orientation, Plot, PlotKind, XValue};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Dimension {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BoundingRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalPos {
    Top,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalPos {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RectElem {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub fill: Color,
    pub stroke_width: f64,
    pub stroke_fill: Color,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextElem {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub fill: Color,
    pub vertical_pos: VerticalPos,
    pub horizontal_pos: HorizontalPos,
    pub font_size: f64,
    pub rotation: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PathElem {
    pub path: PathData,
    pub fill: Option<Color>,
    pub stroke_width: f64,
    pub stroke_fill: Color,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DrawableData {
    Rect(Vec<RectElem>),
    Text(Vec<TextElem>),
    Path(Vec<PathElem>),
}

/// One `DrawableElem`: a group path and the elements that go in it.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawableElem {
    pub group_texts: Vec<String>,
    pub data: DrawableData,
}

impl DrawableElem {
    fn new(groups: &[&str], data: DrawableData) -> DrawableElem {
        DrawableElem {
            group_texts: groups.iter().map(|g| g.to_string()).collect(),
            data,
        }
    }

    pub fn is_empty(&self) -> bool {
        match &self.data {
            DrawableData::Rect(v) => v.is_empty(),
            DrawableData::Text(v) => v.is_empty(),
            DrawableData::Path(v) => v.is_empty(),
        }
    }
}

/// `TextDimensionCalculator.getMaxDimension`: the widest and tallest
/// `getBoundingClientRect` over the texts at `font_size`, in user units.
pub trait TextDimensionCalculator {
    fn get_max_dimension(&self, texts: &[String], font_size: f64) -> Dimension;
}

/// `ChartTitle`.
struct ChartTitle<'c> {
    calc: &'c dyn TextDimensionCalculator,
    config: &'c XyChartConfig,
    title: &'c str,
    title_color: Color,
    bounding: BoundingRect,
    show: bool,
    title_width: f64,
}

impl<'c> ChartTitle<'c> {
    fn new(
        calc: &'c dyn TextDimensionCalculator,
        config: &'c XyChartConfig,
        title: &'c str,
        theme: &XyChartTheme,
    ) -> ChartTitle<'c> {
        ChartTitle {
            calc,
            config,
            title,
            title_color: theme.title_color,
            bounding: BoundingRect::default(),
            show: false,
            title_width: 0.0,
        }
    }

    fn calculate_space(&mut self, available: Dimension) -> Dimension {
        let dim = self
            .calc
            .get_max_dimension(&[self.title.to_string()], *self.config.title_font_size);
        let width_required = dim.width.max(available.width);
        let height_required = dim.height + 2.0 * *self.config.title_padding;
        if height_required <= available.height && self.config.show_title && !self.title.is_empty() {
            self.bounding.width = width_required;
            self.bounding.height = height_required;
            self.title_width = dim.width;
            self.show = true;
        }
        Dimension {
            width: self.bounding.width,
            height: self.bounding.height,
        }
    }

    /// Centers the title over `[start, end]`, kept inside the chart.
    fn align_to(&mut self, start: f64, end: f64) {
        let half_width = self.title_width.min(*self.config.width) / 2.0;
        let center = ((start + end) / 2.0)
            .max(half_width)
            .min(*self.config.width - half_width);
        self.bounding.x = center - half_width;
        self.bounding.width = half_width * 2.0;
    }

    fn drawable_elements(&self) -> Vec<DrawableElem> {
        if !self.show {
            return Vec::new();
        }
        vec![DrawableElem::new(
            &["chart-title"],
            DrawableData::Text(vec![TextElem {
                font_size: *self.config.title_font_size,
                text: self.title.to_string(),
                vertical_pos: VerticalPos::Middle,
                horizontal_pos: HorizontalPos::Center,
                x: self.bounding.x + self.bounding.width / 2.0,
                y: self.bounding.y + self.bounding.height / 2.0,
                fill: self.title_color,
                rotation: 0.0,
            }]),
        )]
    }
}

const LEGEND_MARKER_TO_FONT_RATIO: f64 = 0.75;
const LEGEND_ITEM_SPACING_TO_FONT_RATIO: f64 = 0.5;
const LEGEND_MARKER_SPACING_TO_FONT_RATIO: f64 = 0.35;

struct LegendLayout {
    font_size: f64,
    marker_size: f64,
    marker_spacing: f64,
    item_spacing: f64,
}

fn legend_layout(font_size: f64) -> LegendLayout {
    LegendLayout {
        font_size,
        marker_size: font_size * LEGEND_MARKER_TO_FONT_RATIO,
        marker_spacing: font_size * LEGEND_MARKER_SPACING_TO_FONT_RATIO,
        item_spacing: font_size * LEGEND_ITEM_SPACING_TO_FONT_RATIO,
    }
}

/// `ChartLegend`: one row per titled plot, to the right of the plot area.
struct ChartLegend<'c> {
    calc: &'c dyn TextDimensionCalculator,
    config: &'c XyChartConfig,
    plots: &'c [Plot],
    text_color: Color,
    bounding: BoundingRect,
    visible: Vec<usize>,
}

impl<'c> ChartLegend<'c> {
    fn new(
        calc: &'c dyn TextDimensionCalculator,
        config: &'c XyChartConfig,
        plots: &'c [Plot],
        theme: &XyChartTheme,
    ) -> ChartLegend<'c> {
        ChartLegend {
            calc,
            config,
            plots,
            text_color: theme.legend_text_color,
            bounding: BoundingRect::default(),
            visible: Vec::new(),
        }
    }

    fn set_bounding_box_xy(&mut self, x: f64, y: f64) {
        self.bounding.x = x;
        self.bounding.y = y;
    }

    fn calculate_space(&mut self, available: Dimension) -> Dimension {
        self.visible = if self.config.show_legend {
            self.plots
                .iter()
                .enumerate()
                .filter(|(_, p)| !p.title.is_empty())
                .map(|(i, _)| i)
                .collect()
        } else {
            Vec::new()
        };
        if self.visible.is_empty() {
            self.bounding.width = 0.0;
            self.bounding.height = 0.0;
            return Dimension::default();
        }
        let layout = legend_layout(*self.config.legend_font_size);
        let titles: Vec<String> = self
            .visible
            .iter()
            .map(|i| self.plots[*i].title.clone())
            .collect();
        let text_dim = self.calc.get_max_dimension(&titles, layout.font_size);
        let padding = *self.config.legend_padding;
        let n = self.visible.len() as f64;
        let width_required =
            padding * 2.0 + layout.marker_size + layout.marker_spacing + text_dim.width;
        let height_required =
            padding * 2.0 + n * layout.font_size + (n - 1.0) * layout.item_spacing;
        if width_required <= available.width && height_required <= available.height {
            self.bounding.width = width_required;
            self.bounding.height = height_required;
        } else {
            self.visible.clear();
            self.bounding.width = 0.0;
            self.bounding.height = 0.0;
        }
        Dimension {
            width: self.bounding.width,
            height: self.bounding.height,
        }
    }

    fn drawable_elements(&self) -> Vec<DrawableElem> {
        if self.visible.is_empty() {
            return Vec::new();
        }
        let layout = legend_layout(*self.config.legend_font_size);
        let row_height = layout.font_size + layout.item_spacing;
        let start_x = self.bounding.x + *self.config.legend_padding;
        let start_y = self.bounding.y + *self.config.legend_padding;
        let mut bar_markers = Vec::new();
        let mut line_markers = Vec::new();
        let mut labels = Vec::new();
        for (index, plot_index) in self.visible.iter().enumerate() {
            let plot = &self.plots[*plot_index];
            let row_y = start_y + index as f64 * row_height;
            match &plot.kind {
                PlotKind::Bar { fill } => bar_markers.push(RectElem {
                    x: start_x,
                    y: row_y,
                    width: layout.marker_size,
                    height: layout.marker_size,
                    fill: *fill,
                    stroke_fill: *fill,
                    stroke_width: 0.0,
                }),
                PlotKind::Line {
                    stroke_fill,
                    stroke_width,
                    ..
                } => {
                    let marker_y = row_y + layout.marker_size / 2.0;
                    let mut path = PathData::new();
                    path.move_to(start_x, marker_y)
                        .line_to(start_x + layout.marker_size, marker_y);
                    line_markers.push(PathElem {
                        path,
                        fill: None,
                        stroke_fill: *stroke_fill,
                        stroke_width: *stroke_width,
                    });
                }
            }
            labels.push(TextElem {
                text: plot.title.clone(),
                x: start_x + layout.marker_size + layout.marker_spacing,
                y: row_y + layout.marker_size / 2.0,
                fill: self.text_color,
                font_size: layout.font_size,
                rotation: 0.0,
                vertical_pos: VerticalPos::Middle,
                horizontal_pos: HorizontalPos::Left,
            });
        }
        vec![
            DrawableElem::new(&["legend", "markers"], DrawableData::Rect(bar_markers)),
            DrawableElem::new(&["legend", "markers"], DrawableData::Path(line_markers)),
            DrawableElem::new(&["legend", "label"], DrawableData::Text(labels)),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisPosition {
    Left,
    Right,
    Top,
    Bottom,
}

/// A tick as `getTickValues` returns it: a category or a number.
#[derive(Debug, Clone, PartialEq)]
pub enum TickValue {
    Str(String),
    Num(f64),
}

impl TickValue {
    /// `tick.toString()`.
    fn label(&self) -> String {
        match self {
            TickValue::Str(s) => s.clone(),
            TickValue::Num(n) => js_number(*n),
        }
    }
}

enum AxisScale {
    Band {
        categories: Vec<String>,
        scale: ScaleBand,
    },
    Linear {
        domain: [f64; 2],
        scale: ScaleLinear,
    },
}

const BAR_WIDTH_TO_TICK_WIDTH_RATIO: f64 = 0.7;
const MAX_OUTER_PADDING_PERCENT_FOR_WRT_LABEL: f64 = 0.2;

/// `BaseAxis` with the `BandAxis` or `LinearAxis` scale inside.
pub struct Axis<'c> {
    config: &'c XyChartAxisConfig,
    title: &'c str,
    calc: &'c dyn TextDimensionCalculator,
    theme: AxisTheme,
    scale: AxisScale,
    bounding: BoundingRect,
    position: AxisPosition,
    range: [f64; 2],
    show_title: bool,
    show_label: bool,
    show_tick: bool,
    show_axis_line: bool,
    outer_padding: f64,
    title_text_height: f64,
    label_text_height: f64,
    rotation_rad: f64,
}

impl<'c> Axis<'c> {
    /// `getAxis`.
    fn new(
        data: &'c AxisData,
        config: &'c XyChartAxisConfig,
        theme: AxisTheme,
        calc: &'c dyn TextDimensionCalculator,
    ) -> Axis<'c> {
        let rotation = *config.label_rotation;
        let rotation_rad = if (-90.0..=90.0).contains(&rotation) {
            rotation * PI / 180.0
        } else {
            0.0
        };
        let range = [0.0, 10.0];
        let scale = match data {
            AxisData::Band { categories, .. } => AxisScale::Band {
                categories: categories.clone(),
                scale: ScaleBand::new().domain(categories.clone()).range(range),
            },
            AxisData::Linear { min, max, .. } => AxisScale::Linear {
                domain: [*min, *max],
                scale: ScaleLinear::new().domain([*min, *max]).range(range),
            },
        };
        Axis {
            config,
            title: data.title(),
            calc,
            theme,
            scale,
            bounding: BoundingRect::default(),
            position: AxisPosition::Left,
            range,
            show_title: false,
            show_label: false,
            show_tick: false,
            show_axis_line: false,
            outer_padding: 0.0,
            title_text_height: 0.0,
            label_text_height: 0.0,
            rotation_rad,
        }
    }

    fn is_vertical(&self) -> bool {
        matches!(self.position, AxisPosition::Left | AxisPosition::Right)
    }

    pub fn set_range(&mut self, range: [f64; 2]) {
        self.range = range;
        if self.is_vertical() {
            self.bounding.height = range[1] - range[0];
        } else {
            self.bounding.width = range[1] - range[0];
        }
        self.recalculate_scale();
    }

    pub fn get_range(&self) -> [f64; 2] {
        [
            self.range[0] + self.outer_padding,
            self.range[1] - self.outer_padding,
        ]
    }

    pub fn set_axis_position(&mut self, position: AxisPosition) {
        self.position = position;
        self.set_range(self.range);
    }

    fn recalculate_scale(&mut self) {
        let range = self.get_range();
        match &mut self.scale {
            AxisScale::Band { categories, scale } => {
                *scale = ScaleBand::new()
                    .domain(categories.clone())
                    .range(range)
                    .padding_inner(1.0)
                    .padding_outer(0.0)
                    .align(0.5);
            }
            AxisScale::Linear { domain, scale } => {
                // The y axis of an SVG starts at the top, so the left axis
                // runs its domain backwards.
                let d = if self.position == AxisPosition::Left {
                    [domain[1], domain[0]]
                } else {
                    *domain
                };
                *scale = ScaleLinear::new().domain(d).range(range);
            }
        }
    }

    pub fn tick_values(&self) -> Vec<TickValue> {
        match &self.scale {
            AxisScale::Band { categories, .. } => {
                categories.iter().cloned().map(TickValue::Str).collect()
            }
            AxisScale::Linear { scale, .. } => {
                scale.ticks(None).into_iter().map(TickValue::Num).collect()
            }
        }
    }

    fn tick_labels(&self) -> Vec<String> {
        self.tick_values().iter().map(TickValue::label).collect()
    }

    /// `getScaleValue` for a plot's x or y value.
    pub fn scale_value(&self, value: &XValue) -> f64 {
        match (&self.scale, value) {
            (AxisScale::Band { scale, .. }, XValue::Category(c)) => {
                scale.get(c).unwrap_or(self.get_range()[0])
            }
            (AxisScale::Band { scale, .. }, XValue::Number(n)) => {
                scale.get(&js_number(*n)).unwrap_or(self.get_range()[0])
            }
            (AxisScale::Linear { scale, .. }, XValue::Number(n)) => scale.scale(*n),
            (AxisScale::Linear { scale, .. }, XValue::Category(c)) => {
                scale.scale(c.parse().unwrap_or(f64::NAN))
            }
        }
    }

    fn tick_scale_value(&self, tick: &TickValue) -> f64 {
        match tick {
            TickValue::Str(s) => self.scale_value(&XValue::Category(s.clone())),
            TickValue::Num(n) => self.scale_value(&XValue::Number(*n)),
        }
    }

    pub fn tick_distance(&self) -> f64 {
        let range = self.get_range();
        (range[0] - range[1]).abs() / self.tick_values().len() as f64
    }

    pub fn axis_outer_padding(&self) -> f64 {
        self.outer_padding
    }

    fn label_dimension(&self) -> Dimension {
        self.calc
            .get_max_dimension(&self.tick_labels(), *self.config.label_font_size)
    }

    pub fn recalculate_outer_padding_to_draw_bar(&mut self) {
        if BAR_WIDTH_TO_TICK_WIDTH_RATIO * self.tick_distance() > self.outer_padding * 2.0 {
            self.outer_padding =
                ((BAR_WIDTH_TO_TICK_WIDTH_RATIO * self.tick_distance()) / 2.0).floor();
        }
        self.recalculate_scale();
    }

    fn calculate_space_if_drawn_horizontally(&mut self, available: Dimension) {
        let mut available_height = available.height;
        if self.config.show_axis_line && available_height > *self.config.axis_line_width {
            available_height -= *self.config.axis_line_width;
            self.show_axis_line = true;
        }
        if self.config.show_label {
            let space_required = self.label_dimension();
            let max_padding = MAX_OUTER_PADDING_PERCENT_FOR_WRT_LABEL * available.width;
            self.outer_padding = (space_required.width / 2.0).min(max_padding);
            let mut height_required = space_required.height;
            if self.position == AxisPosition::Bottom && self.rotation_rad != 0.0 {
                height_required = height_required.max(
                    (self.rotation_rad.sin() * space_required.width).abs()
                        + (self.rotation_rad.cos() * space_required.height).abs(),
                );
            }
            height_required += *self.config.label_padding * 2.0;
            self.label_text_height = space_required.height;
            if height_required <= available_height {
                available_height -= height_required;
                self.show_label = true;
            }
        }
        if self.config.show_tick && available_height >= *self.config.tick_length {
            self.show_tick = true;
            available_height -= *self.config.tick_length;
        }
        if self.config.show_title && !self.title.is_empty() {
            let space_required = self
                .calc
                .get_max_dimension(&[self.title.to_string()], *self.config.title_font_size);
            let height_required = space_required.height + *self.config.title_padding * 2.0;
            self.title_text_height = space_required.height;
            if height_required <= available_height {
                available_height -= height_required;
                self.show_title = true;
            }
        }
        self.bounding.width = available.width;
        self.bounding.height = available.height - available_height;
    }

    fn calculate_space_if_drawn_vertical(&mut self, available: Dimension) {
        let mut available_width = available.width;
        if self.config.show_axis_line && available_width > *self.config.axis_line_width {
            available_width -= *self.config.axis_line_width;
            self.show_axis_line = true;
        }
        if self.config.show_label {
            let space_required = self.label_dimension();
            let max_padding = MAX_OUTER_PADDING_PERCENT_FOR_WRT_LABEL * available.height;
            self.outer_padding = (space_required.height / 2.0).min(max_padding);
            let width_required = space_required.width + *self.config.label_padding * 2.0;
            if width_required <= available_width {
                available_width -= width_required;
                self.show_label = true;
            }
        }
        if self.config.show_tick && available_width >= *self.config.tick_length {
            self.show_tick = true;
            available_width -= *self.config.tick_length;
        }
        if self.config.show_title && !self.title.is_empty() {
            let space_required = self
                .calc
                .get_max_dimension(&[self.title.to_string()], *self.config.title_font_size);
            let width_required = space_required.height + *self.config.title_padding * 2.0;
            self.title_text_height = space_required.height;
            if width_required <= available_width {
                available_width -= width_required;
                self.show_title = true;
            }
        }
        self.bounding.width = available.width - available_width;
        self.bounding.height = available.height;
    }

    pub fn calculate_space(&mut self, available: Dimension) -> Dimension {
        if self.is_vertical() {
            self.calculate_space_if_drawn_vertical(available);
        } else {
            self.calculate_space_if_drawn_horizontally(available);
        }
        self.recalculate_scale();
        Dimension {
            width: self.bounding.width,
            height: self.bounding.height,
        }
    }

    pub fn set_bounding_box_xy(&mut self, x: f64, y: f64) {
        self.bounding.x = x;
        self.bounding.y = y;
    }

    fn offset_by_rotation(&self, use_width: bool) -> f64 {
        if self.rotation_rad == 0.0 {
            return 0.0;
        }
        let dim = self.label_dimension();
        let reference = if use_width { dim.width } else { dim.height };
        self.rotation_rad.sin() * reference / 2.0
    }

    fn line_path(x1: f64, y1: f64, x2: f64, y2: f64) -> PathData {
        let mut path = PathData::new();
        path.move_to(x1, y1).line_to(x2, y2);
        path
    }

    fn label_x(&self) -> f64 {
        self.bounding.x + self.bounding.width
            - if self.show_label {
                *self.config.label_padding
            } else {
                0.0
            }
            - if self.show_tick {
                *self.config.tick_length
            } else {
                0.0
            }
            - if self.show_axis_line {
                *self.config.axis_line_width
            } else {
                0.0
            }
    }

    fn axis_line_width_if_shown(&self) -> f64 {
        if self.show_axis_line {
            *self.config.axis_line_width
        } else {
            0.0
        }
    }

    fn tick_length_if_shown(&self) -> f64 {
        if self.show_tick {
            *self.config.tick_length
        } else {
            0.0
        }
    }

    fn drawable_elements_for_left_axis(&self) -> Vec<DrawableElem> {
        let mut out = Vec::new();
        if self.show_axis_line {
            let x = self.bounding.x + self.bounding.width - *self.config.axis_line_width / 2.0;
            out.push(DrawableElem::new(
                &["left-axis", "axisl-line"],
                DrawableData::Path(vec![PathElem {
                    path: Self::line_path(
                        x,
                        self.bounding.y,
                        x,
                        self.bounding.y + self.bounding.height,
                    ),
                    fill: None,
                    stroke_fill: self.theme.axis_line_color,
                    stroke_width: *self.config.axis_line_width,
                }]),
            ));
        }
        if self.show_label {
            let x = self.label_x();
            out.push(DrawableElem::new(
                &["left-axis", "label"],
                DrawableData::Text(
                    self.tick_values()
                        .iter()
                        .map(|tick| TextElem {
                            text: tick.label(),
                            x,
                            y: self.tick_scale_value(tick),
                            fill: self.theme.label_color,
                            font_size: *self.config.label_font_size,
                            rotation: 0.0,
                            vertical_pos: VerticalPos::Middle,
                            horizontal_pos: HorizontalPos::Right,
                        })
                        .collect(),
                ),
            ));
        }
        if self.show_tick {
            let x = self.bounding.x + self.bounding.width - self.axis_line_width_if_shown();
            out.push(DrawableElem::new(
                &["left-axis", "ticks"],
                DrawableData::Path(
                    self.tick_values()
                        .iter()
                        .map(|tick| {
                            let y = self.tick_scale_value(tick);
                            PathElem {
                                path: Self::line_path(x, y, x - *self.config.tick_length, y),
                                fill: None,
                                stroke_fill: self.theme.tick_color,
                                stroke_width: *self.config.tick_width,
                            }
                        })
                        .collect(),
                ),
            ));
        }
        if self.show_title {
            out.push(DrawableElem::new(
                &["left-axis", "title"],
                DrawableData::Text(vec![TextElem {
                    text: self.title.to_string(),
                    x: self.bounding.x + *self.config.title_padding,
                    y: self.bounding.y + self.bounding.height / 2.0,
                    fill: self.theme.title_color,
                    font_size: *self.config.title_font_size,
                    rotation: 270.0,
                    vertical_pos: VerticalPos::Top,
                    horizontal_pos: HorizontalPos::Center,
                }]),
            ));
        }
        out
    }

    fn drawable_elements_for_bottom_axis(&self) -> Vec<DrawableElem> {
        let mut out = Vec::new();
        if self.show_axis_line {
            let y = self.bounding.y + *self.config.axis_line_width / 2.0;
            out.push(DrawableElem::new(
                &["bottom-axis", "axis-line"],
                DrawableData::Path(vec![PathElem {
                    path: Self::line_path(
                        self.bounding.x,
                        y,
                        self.bounding.x + self.bounding.width,
                        y,
                    ),
                    fill: None,
                    stroke_fill: self.theme.axis_line_color,
                    stroke_width: *self.config.axis_line_width,
                }]),
            ));
        }
        if self.show_label {
            let y = self.bounding.y
                + *self.config.label_padding
                + self.tick_length_if_shown()
                + self.axis_line_width_if_shown()
                + self.offset_by_rotation(true).abs();
            let dx = self.offset_by_rotation(false);
            out.push(DrawableElem::new(
                &["bottom-axis", "label"],
                DrawableData::Text(
                    self.tick_values()
                        .iter()
                        .map(|tick| TextElem {
                            text: tick.label(),
                            x: self.tick_scale_value(tick) + dx,
                            y,
                            fill: self.theme.label_color,
                            font_size: *self.config.label_font_size,
                            rotation: self.rotation_rad * 180.0 / PI,
                            vertical_pos: VerticalPos::Top,
                            horizontal_pos: HorizontalPos::Center,
                        })
                        .collect(),
                ),
            ));
        }
        if self.show_tick {
            let y = self.bounding.y + self.axis_line_width_if_shown();
            out.push(DrawableElem::new(
                &["bottom-axis", "ticks"],
                DrawableData::Path(
                    self.tick_values()
                        .iter()
                        .map(|tick| {
                            let x = self.tick_scale_value(tick);
                            PathElem {
                                path: Self::line_path(x, y, x, y + *self.config.tick_length),
                                fill: None,
                                stroke_fill: self.theme.tick_color,
                                stroke_width: *self.config.tick_width,
                            }
                        })
                        .collect(),
                ),
            ));
        }
        if self.show_title {
            out.push(DrawableElem::new(
                &["bottom-axis", "title"],
                DrawableData::Text(vec![TextElem {
                    text: self.title.to_string(),
                    x: self.range[0] + (self.range[1] - self.range[0]) / 2.0,
                    y: self.bounding.y + self.bounding.height
                        - *self.config.title_padding
                        - self.title_text_height,
                    fill: self.theme.title_color,
                    font_size: *self.config.title_font_size,
                    rotation: 0.0,
                    vertical_pos: VerticalPos::Top,
                    horizontal_pos: HorizontalPos::Center,
                }]),
            ));
        }
        out
    }

    fn drawable_elements_for_top_axis(&self) -> Vec<DrawableElem> {
        let mut out = Vec::new();
        if self.show_axis_line {
            let y = self.bounding.y + self.bounding.height - *self.config.axis_line_width / 2.0;
            out.push(DrawableElem::new(
                &["top-axis", "axis-line"],
                DrawableData::Path(vec![PathElem {
                    path: Self::line_path(
                        self.bounding.x,
                        y,
                        self.bounding.x + self.bounding.width,
                        y,
                    ),
                    fill: None,
                    stroke_fill: self.theme.axis_line_color,
                    stroke_width: *self.config.axis_line_width,
                }]),
            ));
        }
        if self.show_label {
            let y = self.bounding.y
                + if self.show_title {
                    self.title_text_height + *self.config.title_padding * 2.0
                } else {
                    0.0
                }
                + *self.config.label_padding;
            out.push(DrawableElem::new(
                &["top-axis", "label"],
                DrawableData::Text(
                    self.tick_values()
                        .iter()
                        .map(|tick| TextElem {
                            text: tick.label(),
                            x: self.tick_scale_value(tick),
                            y,
                            fill: self.theme.label_color,
                            font_size: *self.config.label_font_size,
                            rotation: 0.0,
                            vertical_pos: VerticalPos::Top,
                            horizontal_pos: HorizontalPos::Center,
                        })
                        .collect(),
                ),
            ));
        }
        if self.show_tick {
            let y = self.bounding.y;
            let y1 = y + self.bounding.height - self.axis_line_width_if_shown();
            let y2 = y1 - *self.config.tick_length;
            out.push(DrawableElem::new(
                &["top-axis", "ticks"],
                DrawableData::Path(
                    self.tick_values()
                        .iter()
                        .map(|tick| {
                            let x = self.tick_scale_value(tick);
                            PathElem {
                                path: Self::line_path(x, y1, x, y2),
                                fill: None,
                                stroke_fill: self.theme.tick_color,
                                stroke_width: *self.config.tick_width,
                            }
                        })
                        .collect(),
                ),
            ));
        }
        if self.show_title {
            out.push(DrawableElem::new(
                &["top-axis", "title"],
                DrawableData::Text(vec![TextElem {
                    text: self.title.to_string(),
                    x: self.bounding.x + self.bounding.width / 2.0,
                    y: self.bounding.y + *self.config.title_padding,
                    fill: self.theme.title_color,
                    font_size: *self.config.title_font_size,
                    rotation: 0.0,
                    vertical_pos: VerticalPos::Top,
                    horizontal_pos: HorizontalPos::Center,
                }]),
            ));
        }
        out
    }

    pub fn drawable_elements(&self) -> Vec<DrawableElem> {
        match self.position {
            AxisPosition::Left => self.drawable_elements_for_left_axis(),
            // mermaid throws for a right axis; the orchestrator never asks.
            AxisPosition::Right => Vec::new(),
            AxisPosition::Bottom => self.drawable_elements_for_bottom_axis(),
            AxisPosition::Top => self.drawable_elements_for_top_axis(),
        }
    }
}

/// `BasePlot` with `BarPlot` and `LinePlot` inlined.
struct PlotComponent {
    bounding: BoundingRect,
}

impl PlotComponent {
    fn calculate_space(&mut self, available: Dimension) -> Dimension {
        self.bounding.width = available.width;
        self.bounding.height = available.height;
        available
    }

    fn set_bounding_box_xy(&mut self, x: f64, y: f64) {
        self.bounding.x = x;
        self.bounding.y = y;
    }

    fn drawable_elements(
        &self,
        plots: &[Plot],
        orientation: Orientation,
        x_axis: &Axis<'_>,
        y_axis: &Axis<'_>,
    ) -> Vec<DrawableElem> {
        let mut out = Vec::new();
        for (i, plot) in plots.iter().enumerate() {
            let final_data: Vec<(f64, f64)> = plot
                .data
                .iter()
                .map(|(x, y)| {
                    (
                        x_axis.scale_value(x),
                        y_axis.scale_value(&XValue::Number(*y)),
                    )
                })
                .collect();
            match &plot.kind {
                PlotKind::Line {
                    stroke_fill,
                    stroke_width,
                    point_labels,
                } => {
                    let points: Vec<Point> = final_data
                        .iter()
                        .map(|(x, y)| match orientation {
                            Orientation::Horizontal => Point::new(*y, *x),
                            Orientation::Vertical => Point::new(*x, *y),
                        })
                        .collect();
                    let path = curves::linear(&points);
                    if path.is_empty() {
                        continue;
                    }
                    let group = format!("line-plot-{i}");
                    out.push(DrawableElem::new(
                        &["plot", &group],
                        DrawableData::Path(vec![PathElem {
                            path,
                            fill: None,
                            stroke_fill: *stroke_fill,
                            stroke_width: *stroke_width,
                        }]),
                    ));
                    if let Some(labels) = point_labels
                        && !labels.is_empty()
                    {
                        let label_offset = 10.0;
                        let font_size = 12.0;
                        let mut texts = Vec::new();
                        for (j, (px, py)) in final_data.iter().enumerate() {
                            let Some(label) = labels.get(j).filter(|l| !l.is_empty()) else {
                                continue;
                            };
                            texts.push(match orientation {
                                Orientation::Horizontal => TextElem {
                                    x: py + label_offset,
                                    y: *px,
                                    text: label.clone(),
                                    fill: *stroke_fill,
                                    vertical_pos: VerticalPos::Middle,
                                    horizontal_pos: HorizontalPos::Left,
                                    font_size,
                                    rotation: 0.0,
                                },
                                Orientation::Vertical => TextElem {
                                    x: *px,
                                    y: py - label_offset,
                                    text: label.clone(),
                                    fill: *stroke_fill,
                                    vertical_pos: VerticalPos::Middle,
                                    horizontal_pos: HorizontalPos::Center,
                                    font_size,
                                    rotation: 0.0,
                                },
                            });
                        }
                        if !texts.is_empty() {
                            out.push(DrawableElem::new(
                                &["plot", &group, "labels"],
                                DrawableData::Text(texts),
                            ));
                        }
                    }
                }
                PlotKind::Bar { fill } => {
                    let bar_padding_percent = 0.05;
                    let bar_width = (x_axis.axis_outer_padding() * 2.0).min(x_axis.tick_distance())
                        * (1.0 - bar_padding_percent);
                    let half = bar_width / 2.0;
                    let group = format!("bar-plot-{i}");
                    let rects = final_data
                        .iter()
                        .map(|(x, y)| match orientation {
                            Orientation::Horizontal => RectElem {
                                x: self.bounding.x,
                                y: x - half,
                                height: bar_width,
                                width: y - self.bounding.x,
                                fill: *fill,
                                stroke_width: 0.0,
                                stroke_fill: *fill,
                            },
                            Orientation::Vertical => RectElem {
                                x: x - half,
                                y: *y,
                                width: bar_width,
                                height: self.bounding.y + self.bounding.height - y,
                                fill: *fill,
                                stroke_width: 0.0,
                                stroke_fill: *fill,
                            },
                        })
                        .collect();
                    out.push(DrawableElem::new(
                        &["plot", &group],
                        DrawableData::Rect(rects),
                    ));
                }
            }
        }
        out
    }
}

/// `Orchestrator.getDrawableElement()`: lays the components out and
/// returns their elements in `componentStore` order (title, plot, legend,
/// x axis, y axis).
pub fn build(db: &Db, calc: &dyn TextDimensionCalculator) -> Vec<DrawableElem> {
    let config = &db.config;
    let mut title = ChartTitle::new(calc, config, &db.title, &db.theme);
    let mut plot = PlotComponent {
        bounding: BoundingRect::default(),
    };
    let mut legend = ChartLegend::new(calc, config, &db.plots, &db.theme);
    let mut x_axis = Axis::new(&db.x_axis, &config.x_axis, db.theme.x_axis(), calc);
    let mut y_axis = Axis::new(&db.y_axis, &config.y_axis, db.theme.y_axis(), calc);
    let has_bar = db.plots.iter().any(Plot::is_bar);
    let orientation = db.orientation();

    let mut available_width = *config.width;
    let mut available_height = *config.height;
    let reserved = *config.plot_reserved_space_percent;
    let mut chart_width = ((available_width * reserved) / 100.0).floor();
    let mut chart_height = ((available_height * reserved) / 100.0).floor();
    let mut space_used = plot.calculate_space(Dimension {
        width: chart_width,
        height: chart_height,
    });
    available_width -= space_used.width;
    available_height -= space_used.height;
    space_used = title.calculate_space(Dimension {
        width: *config.width,
        height: available_height,
    });
    let title_y_end = space_used.height;
    available_height -= space_used.height;

    let (plot_x, plot_y);
    match orientation {
        Orientation::Vertical => {
            plot_y = title_y_end;
            x_axis.set_axis_position(AxisPosition::Bottom);
            space_used = x_axis.calculate_space(Dimension {
                width: available_width,
                height: available_height,
            });
            available_height -= space_used.height;
            y_axis.set_axis_position(AxisPosition::Left);
            space_used = y_axis.calculate_space(Dimension {
                width: available_width,
                height: available_height,
            });
            plot_x = space_used.width;
            available_width -= space_used.width;
        }
        Orientation::Horizontal => {
            x_axis.set_axis_position(AxisPosition::Left);
            space_used = x_axis.calculate_space(Dimension {
                width: available_width,
                height: available_height,
            });
            available_width -= space_used.width;
            plot_x = space_used.width;
            y_axis.set_axis_position(AxisPosition::Top);
            space_used = y_axis.calculate_space(Dimension {
                width: available_width,
                height: available_height,
            });
            available_height -= space_used.height;
            plot_y = title_y_end + space_used.height;
        }
    }
    let legend_space = legend.calculate_space(Dimension {
        width: available_width,
        height: chart_height,
    });
    available_width -= legend_space.width;
    if available_width > 0.0 {
        chart_width += available_width;
    }
    if available_height > 0.0 {
        chart_height += available_height;
    }
    plot.calculate_space(Dimension {
        width: chart_width,
        height: chart_height,
    });
    plot.set_bounding_box_xy(plot_x, plot_y);
    title.align_to(plot_x, plot_x + chart_width);
    legend.set_bounding_box_xy(
        plot_x + chart_width,
        plot_y + ((chart_height - legend_space.height) / 2.0).max(0.0),
    );
    match orientation {
        Orientation::Vertical => {
            x_axis.set_range([plot_x, plot_x + chart_width]);
            x_axis.set_bounding_box_xy(plot_x, plot_y + chart_height);
            y_axis.set_range([plot_y, plot_y + chart_height]);
            y_axis.set_bounding_box_xy(0.0, plot_y);
        }
        Orientation::Horizontal => {
            y_axis.set_range([plot_x, plot_x + chart_width]);
            y_axis.set_bounding_box_xy(plot_x, title_y_end);
            x_axis.set_range([plot_y, plot_y + chart_height]);
            x_axis.set_bounding_box_xy(0.0, plot_y);
        }
    }
    if has_bar {
        x_axis.recalculate_outer_padding_to_draw_bar();
    }

    let mut out = title.drawable_elements();
    out.extend(plot.drawable_elements(&db.plots, orientation, &x_axis, &y_axis));
    out.extend(legend.drawable_elements());
    out.extend(x_axis.drawable_elements());
    out.extend(y_axis.drawable_elements());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::styles::xychart::theme_config;
    use crate::mermaid::theme;

    /// `TextDimensionCalculatorWithFont` without a group: character count
    /// times the font size, one line tall.
    struct Approx;

    impl TextDimensionCalculator for Approx {
        fn get_max_dimension(&self, texts: &[String], font_size: f64) -> Dimension {
            Dimension {
                width: texts.iter().map(|t| t.chars().count()).max().unwrap_or(0) as f64
                    * font_size,
                height: font_size,
            }
        }
    }

    fn db() -> Db {
        let tv = theme::resolve("default", None).unwrap();
        Db::new(&XyChartConfig::default(), theme_config(&tv))
    }

    fn text(s: &str) -> super::super::db::TextValue {
        super::super::db::TextValue { text: s.into() }
    }

    fn points(values: &[f64]) -> Vec<super::super::db::DataPoint> {
        values
            .iter()
            .map(|v| super::super::db::DataPoint {
                value: *v,
                label: String::new(),
            })
            .collect()
    }

    #[test]
    fn vertical_chart_groups_in_component_order() {
        let mut db = db();
        db.set_diagram_title("T");
        db.set_x_axis_band(&[text("a"), text("b"), text("c")]);
        db.set_y_axis_title(&text("y"));
        db.set_y_axis_range_data(0.0, 10.0);
        db.set_bar_data(&text(""), &points(&[1.0, 5.0, 10.0]));
        db.set_line_data(&text(""), &points(&[1.0, 5.0, 10.0]));
        let elems = build(&db, &Approx);
        let groups: Vec<String> = elems.iter().map(|e| e.group_texts.join("/")).collect();
        assert_eq!(
            groups,
            vec![
                "chart-title",
                "plot/bar-plot-0",
                "plot/line-plot-1",
                "bottom-axis/axis-line",
                "bottom-axis/label",
                "bottom-axis/ticks",
                "left-axis/axisl-line",
                "left-axis/label",
                "left-axis/ticks",
                "left-axis/title",
            ]
        );
        // The y axis runs downwards: the top tick (10) is at the plot's top.
        let DrawableData::Text(labels) = &elems[7].data else {
            panic!("left-axis label");
        };
        assert_eq!(labels[0].text, "10");
        assert!(labels[0].y < labels.last().unwrap().y);
        // Bars stand on the plot's bottom edge.
        let DrawableData::Rect(rects) = &elems[1].data else {
            panic!("bar rects");
        };
        let bottom = rects[0].y + rects[0].height;
        assert!(bottom < 500.0);
        assert!((bottom - (rects[2].y + rects[2].height)).abs() < 1e-9);
        assert!(rects[2].y < rects[0].y);
    }

    #[test]
    fn horizontal_chart_uses_left_and_top_axes() {
        let mut db = db();
        db.set_orientation("horizontal");
        db.set_x_axis_band(&[text("a"), text("b")]);
        db.set_bar_data(&text("s"), &points(&[1.0, 2.0]));
        let elems = build(&db, &Approx);
        let groups: Vec<String> = elems.iter().map(|e| e.group_texts.join("/")).collect();
        assert!(groups.contains(&"legend/label".to_string()));
        assert!(groups.iter().any(|g| g.starts_with("left-axis")));
        assert!(groups.iter().any(|g| g.starts_with("top-axis")));
        let DrawableData::Rect(rects) = &elems[0].data else {
            panic!("bar rects");
        };
        assert!(rects[1].width > rects[0].width);
    }

    #[test]
    fn rotated_labels_carry_the_angle() {
        let mut db = db();
        db.config.x_axis.label_rotation = 45.0.into();
        db.set_x_axis_band(&[text("one"), text("two")]);
        db.set_line_data(&text(""), &points(&[1.0, 2.0]));
        let elems = build(&db, &Approx);
        let labels = elems
            .iter()
            .find(|e| e.group_texts == ["bottom-axis", "label"])
            .expect("labels");
        let DrawableData::Text(texts) = &labels.data else {
            panic!("text");
        };
        assert!((texts[0].rotation - 45.0).abs() < 1e-9);
    }
}
