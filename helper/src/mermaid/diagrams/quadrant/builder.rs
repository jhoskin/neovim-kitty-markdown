//! Port of `QuadrantBuilder` (`diagrams/quadrant-chart/quadrantBuilder.ts`,
//! mermaid 12.2.0): the geometry of the chart from the parsed data, the
//! `quadrantChart` config section, and the theme variables.
//!
//! The builder measures no text. Every position is arithmetic on the chart
//! size and the paddings, so the output is exact and the SVG snapshot is
//! stable across font sets.

use indexmap::IndexMap;

use crate::mermaid::config::QuadrantChartConfig;
use crate::mermaid::theme::ThemeVariables;

/// `TextVerticalPos`: how a text sits on its anchor along the x axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalPos {
    Left,
    Center,
}

/// `TextHorizontalPos`: how a text sits on its anchor along the y axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalPos {
    Top,
    Middle,
}

/// `StylesObject`: the point styles `parseStyles` accepts. Every field is
/// optional; a point's own style wins over its class, the class over the
/// theme.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StylesObject {
    pub radius: Option<f64>,
    pub color: Option<String>,
    pub stroke_color: Option<String>,
    pub stroke_width: Option<String>,
}

impl StylesObject {
    /// `{ ...classStyles, ...point }`: the point object only carries the
    /// keys the user wrote, so an absent key falls through to the class.
    fn over(&self, class: &StylesObject) -> StylesObject {
        StylesObject {
            radius: self.radius.or(class.radius),
            color: self.color.clone().or_else(|| class.color.clone()),
            stroke_color: self
                .stroke_color
                .clone()
                .or_else(|| class.stroke_color.clone()),
            stroke_width: self
                .stroke_width
                .clone()
                .or_else(|| class.stroke_width.clone()),
        }
    }
}

/// `QuadrantPointInputType`: one `text: [x, y]` statement.
#[derive(Debug, Clone, PartialEq)]
pub struct PointInput {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub class_name: String,
    pub styles: StylesObject,
}

/// `QuadrantTextType`: a label with its anchor and font size.
#[derive(Debug, Clone, PartialEq)]
pub struct TextType {
    pub text: String,
    /// `None` where the theme variable is missing; the renderer then writes
    /// no `fill`, as `d3.attr` does for `undefined`.
    pub fill: Option<String>,
    pub x: f64,
    pub y: f64,
    pub vertical_pos: VerticalPos,
    pub horizontal_pos: HorizontalPos,
    pub font_size: f64,
    pub rotation: f64,
}

/// `QuadrantPointType`.
#[derive(Debug, Clone, PartialEq)]
pub struct PointType {
    pub x: f64,
    pub y: f64,
    pub fill: Option<String>,
    pub radius: f64,
    pub text: TextType,
    pub stroke_color: Option<String>,
    /// A CSS length such as `0px`, as the user wrote it.
    pub stroke_width: String,
}

/// `QuadrantQuadrantsType`.
#[derive(Debug, Clone, PartialEq)]
pub struct QuadrantType {
    pub text: TextType,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub fill: Option<String>,
}

/// `QuadrantLineType`.
#[derive(Debug, Clone, PartialEq)]
pub struct LineType {
    pub stroke_width: f64,
    pub stroke_fill: Option<String>,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

/// `QuadrantBuildType`: what `build()` hands the renderer.
#[derive(Debug, Clone, PartialEq)]
pub struct Build {
    pub points: Vec<PointType>,
    pub quadrants: Vec<QuadrantType>,
    pub axis_labels: Vec<TextType>,
    pub title: Option<TextType>,
    pub border_lines: Vec<LineType>,
}

/// `QuadrantBuilderData`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Data {
    pub title_text: String,
    pub quadrant1_text: String,
    pub quadrant2_text: String,
    pub quadrant3_text: String,
    pub quadrant4_text: String,
    pub x_axis_left_text: String,
    pub x_axis_right_text: String,
    pub y_axis_bottom_text: String,
    pub y_axis_top_text: String,
    pub points: Vec<PointInput>,
}

/// `QuadrantBuilderConfig`: the `quadrantChart` config section plus the
/// three `show*` flags, which nothing in mermaid 12.2.0 turns off.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub show_x_axis: bool,
    pub show_y_axis: bool,
    pub show_title: bool,
    pub chart_height: f64,
    pub chart_width: f64,
    pub title_padding: f64,
    pub title_font_size: f64,
    pub quadrant_padding: f64,
    pub x_axis_label_padding: f64,
    pub y_axis_label_padding: f64,
    pub x_axis_label_font_size: f64,
    pub y_axis_label_font_size: f64,
    pub quadrant_label_font_size: f64,
    pub quadrant_text_top_padding: f64,
    pub point_text_padding: f64,
    pub point_label_font_size: f64,
    pub point_radius: f64,
    pub x_axis_position: String,
    pub y_axis_position: String,
    pub quadrant_internal_border_stroke_width: f64,
    pub quadrant_external_border_stroke_width: f64,
}

impl Config {
    /// `setConfig(quadrantChartConfig)` in `getQuadrantData`: the effective
    /// section replaces every default.
    pub fn from_config(c: &QuadrantChartConfig) -> Config {
        Config {
            show_x_axis: true,
            show_y_axis: true,
            show_title: true,
            chart_height: *c.chart_height,
            chart_width: *c.chart_width,
            title_padding: *c.title_padding,
            title_font_size: *c.title_font_size,
            quadrant_padding: *c.quadrant_padding,
            x_axis_label_padding: *c.x_axis_label_padding,
            y_axis_label_padding: *c.y_axis_label_padding,
            x_axis_label_font_size: *c.x_axis_label_font_size,
            y_axis_label_font_size: *c.y_axis_label_font_size,
            quadrant_label_font_size: *c.quadrant_label_font_size,
            quadrant_text_top_padding: *c.quadrant_text_top_padding,
            point_text_padding: *c.point_text_padding,
            point_label_font_size: *c.point_label_font_size,
            point_radius: *c.point_radius,
            x_axis_position: c.x_axis_position.clone(),
            y_axis_position: c.y_axis_position.clone(),
            quadrant_internal_border_stroke_width: *c.quadrant_internal_border_stroke_width,
            quadrant_external_border_stroke_width: *c.quadrant_external_border_stroke_width,
        }
    }
}

/// `QuadrantBuilderThemeConfig`: the fifteen `quadrant*` theme variables.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThemeConfig {
    pub quadrant_title_fill: Option<String>,
    pub quadrant1_fill: Option<String>,
    pub quadrant2_fill: Option<String>,
    pub quadrant3_fill: Option<String>,
    pub quadrant4_fill: Option<String>,
    pub quadrant1_text_fill: Option<String>,
    pub quadrant2_text_fill: Option<String>,
    pub quadrant3_text_fill: Option<String>,
    pub quadrant4_text_fill: Option<String>,
    pub quadrant_point_fill: Option<String>,
    pub quadrant_point_text_fill: Option<String>,
    pub quadrant_x_axis_text_fill: Option<String>,
    pub quadrant_y_axis_text_fill: Option<String>,
    pub quadrant_internal_border_stroke_fill: Option<String>,
    pub quadrant_external_border_stroke_fill: Option<String>,
}

impl ThemeConfig {
    /// `setThemeConfig({...})` in `getQuadrantData`: every variable is
    /// copied, a missing one included, so it overrides the default theme's
    /// value with `undefined`.
    pub fn from_theme(tv: &ThemeVariables) -> ThemeConfig {
        let get = |name: &str| tv.str(name).map(str::to_string);
        ThemeConfig {
            quadrant_title_fill: get("quadrantTitleFill"),
            quadrant1_fill: get("quadrant1Fill"),
            quadrant2_fill: get("quadrant2Fill"),
            quadrant3_fill: get("quadrant3Fill"),
            quadrant4_fill: get("quadrant4Fill"),
            quadrant1_text_fill: get("quadrant1TextFill"),
            quadrant2_text_fill: get("quadrant2TextFill"),
            quadrant3_text_fill: get("quadrant3TextFill"),
            quadrant4_text_fill: get("quadrant4TextFill"),
            quadrant_point_fill: get("quadrantPointFill"),
            quadrant_point_text_fill: get("quadrantPointTextFill"),
            quadrant_x_axis_text_fill: get("quadrantXAxisTextFill"),
            quadrant_y_axis_text_fill: get("quadrantYAxisTextFill"),
            quadrant_internal_border_stroke_fill: get("quadrantInternalBorderStrokeFill"),
            quadrant_external_border_stroke_fill: get("quadrantExternalBorderStrokeFill"),
        }
    }
}

/// `CalculateSpaceData`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Space {
    x_axis_top: f64,
    x_axis_bottom: f64,
    y_axis_left: f64,
    y_axis_right: f64,
    title_top: f64,
    quadrant_left: f64,
    quadrant_top: f64,
    quadrant_width: f64,
    quadrant_half_width: f64,
    quadrant_height: f64,
    quadrant_half_height: f64,
}

/// `scaleLinear().domain([0, 1]).range([r0, r1])` applied to `t`:
/// d3's `interpolateNumber`, kept in its `a * (1 - t) + b * t` form so the
/// rounding matches the oracle's coordinates digit for digit.
fn scale_unit(r0: f64, r1: f64, t: f64) -> f64 {
    r0 * (1.0 - t) + r1 * t
}

pub struct Builder {
    config: Config,
    theme: ThemeConfig,
    data: Data,
    classes: IndexMap<String, StylesObject>,
}

impl Builder {
    pub fn new(config: Config, theme: ThemeConfig, data: Data) -> Builder {
        Builder {
            config,
            theme,
            data,
            classes: IndexMap::new(),
        }
    }

    pub fn add_class(&mut self, class_name: &str, styles: StylesObject) {
        self.classes.insert(class_name.to_string(), styles);
    }

    /// `calculateSpace`.
    fn calculate_space(
        &self,
        x_axis_position: &str,
        show_x_axis: bool,
        show_y_axis: bool,
        show_title: bool,
    ) -> Space {
        let c = &self.config;
        let x_axis_space = c.x_axis_label_padding * 2.0 + c.x_axis_label_font_size;
        let x_axis_top = if x_axis_position == "top" && show_x_axis {
            x_axis_space
        } else {
            0.0
        };
        let x_axis_bottom = if x_axis_position == "bottom" && show_x_axis {
            x_axis_space
        } else {
            0.0
        };

        let y_axis_space = c.y_axis_label_padding * 2.0 + c.y_axis_label_font_size;
        let y_axis_left = if c.y_axis_position == "left" && show_y_axis {
            y_axis_space
        } else {
            0.0
        };
        let y_axis_right = if c.y_axis_position == "right" && show_y_axis {
            y_axis_space
        } else {
            0.0
        };

        let title_top = if show_title {
            c.title_font_size + c.title_padding * 2.0
        } else {
            0.0
        };

        let quadrant_left = c.quadrant_padding + y_axis_left;
        let quadrant_top = c.quadrant_padding + x_axis_top + title_top;
        let quadrant_width = c.chart_width - c.quadrant_padding * 2.0 - y_axis_left - y_axis_right;
        let quadrant_height =
            c.chart_height - c.quadrant_padding * 2.0 - x_axis_top - x_axis_bottom - title_top;
        Space {
            x_axis_top,
            x_axis_bottom,
            y_axis_left,
            y_axis_right,
            title_top,
            quadrant_left,
            quadrant_top,
            quadrant_width,
            quadrant_half_width: quadrant_width / 2.0,
            quadrant_height,
            quadrant_half_height: quadrant_height / 2.0,
        }
    }

    /// `getAxisLabels`.
    fn axis_labels(
        &self,
        x_axis_position: &str,
        show_x_axis: bool,
        show_y_axis: bool,
        s: &Space,
    ) -> Vec<TextType> {
        let c = &self.config;
        let d = &self.data;
        let x_in_middle = !d.x_axis_right_text.is_empty();
        let y_in_middle = !d.y_axis_top_text.is_empty();
        let mut labels = Vec::new();

        let x_axis_y = if x_axis_position == "top" {
            c.x_axis_label_padding + s.title_top
        } else {
            c.x_axis_label_padding + s.quadrant_top + s.quadrant_height + c.quadrant_padding
        };
        let x_pos = if x_in_middle {
            VerticalPos::Center
        } else {
            VerticalPos::Left
        };
        if !d.x_axis_left_text.is_empty() && show_x_axis {
            labels.push(TextType {
                text: d.x_axis_left_text.clone(),
                fill: self.theme.quadrant_x_axis_text_fill.clone(),
                x: s.quadrant_left
                    + if x_in_middle {
                        s.quadrant_half_width / 2.0
                    } else {
                        0.0
                    },
                y: x_axis_y,
                font_size: c.x_axis_label_font_size,
                vertical_pos: x_pos,
                horizontal_pos: HorizontalPos::Top,
                rotation: 0.0,
            });
        }
        if !d.x_axis_right_text.is_empty() && show_x_axis {
            labels.push(TextType {
                text: d.x_axis_right_text.clone(),
                fill: self.theme.quadrant_x_axis_text_fill.clone(),
                x: s.quadrant_left
                    + s.quadrant_half_width
                    + if x_in_middle {
                        s.quadrant_half_width / 2.0
                    } else {
                        0.0
                    },
                y: x_axis_y,
                font_size: c.x_axis_label_font_size,
                vertical_pos: x_pos,
                horizontal_pos: HorizontalPos::Top,
                rotation: 0.0,
            });
        }

        let y_axis_x = if c.y_axis_position == "left" {
            c.y_axis_label_padding
        } else {
            c.y_axis_label_padding + s.quadrant_left + s.quadrant_width + c.quadrant_padding
        };
        let y_pos = if y_in_middle {
            VerticalPos::Center
        } else {
            VerticalPos::Left
        };
        let y_shift = if y_in_middle {
            s.quadrant_half_height / 2.0
        } else {
            0.0
        };
        if !d.y_axis_bottom_text.is_empty() && show_y_axis {
            labels.push(TextType {
                text: d.y_axis_bottom_text.clone(),
                fill: self.theme.quadrant_y_axis_text_fill.clone(),
                x: y_axis_x,
                y: s.quadrant_top + s.quadrant_height - y_shift,
                font_size: c.y_axis_label_font_size,
                vertical_pos: y_pos,
                horizontal_pos: HorizontalPos::Top,
                rotation: -90.0,
            });
        }
        if !d.y_axis_top_text.is_empty() && show_y_axis {
            labels.push(TextType {
                text: d.y_axis_top_text.clone(),
                fill: self.theme.quadrant_y_axis_text_fill.clone(),
                x: y_axis_x,
                y: s.quadrant_top + s.quadrant_half_height - y_shift,
                font_size: c.y_axis_label_font_size,
                vertical_pos: y_pos,
                horizontal_pos: HorizontalPos::Top,
                rotation: -90.0,
            });
        }
        labels
    }

    /// `getQuadrants`: quadrant 1 is top right, then counterclockwise.
    fn quadrants(&self, s: &Space) -> Vec<QuadrantType> {
        let c = &self.config;
        let d = &self.data;
        let t = &self.theme;
        let text = |text: &str, fill: &Option<String>| TextType {
            text: text.to_string(),
            fill: fill.clone(),
            x: 0.0,
            y: 0.0,
            font_size: c.quadrant_label_font_size,
            vertical_pos: VerticalPos::Center,
            horizontal_pos: HorizontalPos::Middle,
            rotation: 0.0,
        };
        let corners = [
            (
                &d.quadrant1_text,
                &t.quadrant1_text_fill,
                &t.quadrant1_fill,
                s.quadrant_left + s.quadrant_half_width,
                s.quadrant_top,
            ),
            (
                &d.quadrant2_text,
                &t.quadrant2_text_fill,
                &t.quadrant2_fill,
                s.quadrant_left,
                s.quadrant_top,
            ),
            (
                &d.quadrant3_text,
                &t.quadrant3_text_fill,
                &t.quadrant3_fill,
                s.quadrant_left,
                s.quadrant_top + s.quadrant_half_height,
            ),
            (
                &d.quadrant4_text,
                &t.quadrant4_text_fill,
                &t.quadrant4_fill,
                s.quadrant_left + s.quadrant_half_width,
                s.quadrant_top + s.quadrant_half_height,
            ),
        ];
        corners
            .into_iter()
            .map(|(label, text_fill, fill, x, y)| {
                let mut q = QuadrantType {
                    text: text(label, text_fill),
                    x,
                    y,
                    width: s.quadrant_half_width,
                    height: s.quadrant_half_height,
                    fill: fill.clone(),
                };
                q.text.x = q.x + q.width / 2.0;
                if d.points.is_empty() {
                    q.text.y = q.y + q.height / 2.0;
                    q.text.horizontal_pos = HorizontalPos::Middle;
                } else {
                    q.text.y = q.y + c.quadrant_text_top_padding;
                    q.text.horizontal_pos = HorizontalPos::Top;
                }
                q
            })
            .collect()
    }

    /// `getQuadrantPoints`.
    fn points(&self, s: &Space) -> Vec<PointType> {
        let c = &self.config;
        let t = &self.theme;
        let x_axis = |v: f64| scale_unit(s.quadrant_left, s.quadrant_width + s.quadrant_left, v);
        let y_axis = |v: f64| scale_unit(s.quadrant_height + s.quadrant_top, s.quadrant_top, v);
        self.data
            .points
            .iter()
            .map(|point| {
                let styles = match self.classes.get(&point.class_name) {
                    Some(class) => point.styles.over(class),
                    None => point.styles.clone(),
                };
                let x = x_axis(point.x);
                let y = y_axis(point.y);
                PointType {
                    x,
                    y,
                    fill: styles.color.or_else(|| t.quadrant_point_fill.clone()),
                    radius: styles.radius.unwrap_or(c.point_radius),
                    text: TextType {
                        text: point.text.clone(),
                        fill: t.quadrant_point_text_fill.clone(),
                        x,
                        y: y + c.point_text_padding,
                        vertical_pos: VerticalPos::Center,
                        horizontal_pos: HorizontalPos::Top,
                        font_size: c.point_label_font_size,
                        rotation: 0.0,
                    },
                    stroke_color: styles
                        .stroke_color
                        .or_else(|| t.quadrant_point_fill.clone()),
                    stroke_width: styles.stroke_width.unwrap_or_else(|| "0px".to_string()),
                }
            })
            .collect()
    }

    /// `getBorders`: the four outer edges, then the two inner lines. The
    /// outer edges overshoot by half the external width so the corners
    /// close.
    fn borders(&self, s: &Space) -> Vec<LineType> {
        let c = &self.config;
        let t = &self.theme;
        let half = c.quadrant_external_border_stroke_width / 2.0;
        let external = |x1: f64, y1: f64, x2: f64, y2: f64| LineType {
            stroke_fill: t.quadrant_external_border_stroke_fill.clone(),
            stroke_width: c.quadrant_external_border_stroke_width,
            x1,
            y1,
            x2,
            y2,
        };
        let internal = |x1: f64, y1: f64, x2: f64, y2: f64| LineType {
            stroke_fill: t.quadrant_internal_border_stroke_fill.clone(),
            stroke_width: c.quadrant_internal_border_stroke_width,
            x1,
            y1,
            x2,
            y2,
        };
        let left = s.quadrant_left;
        let right = s.quadrant_left + s.quadrant_width;
        let top = s.quadrant_top;
        let bottom = s.quadrant_top + s.quadrant_height;
        vec![
            external(left - half, top, right + half, top),
            external(right, top + half, right, bottom - half),
            external(left - half, bottom, right + half, bottom),
            external(left, top + half, left, bottom - half),
            internal(
                left + s.quadrant_half_width,
                top + half,
                left + s.quadrant_half_width,
                bottom - half,
            ),
            internal(
                left + half,
                top + s.quadrant_half_height,
                right - half,
                top + s.quadrant_half_height,
            ),
        ]
    }

    /// `getTitle`.
    fn title(&self, show_title: bool) -> Option<TextType> {
        show_title.then(|| TextType {
            text: self.data.title_text.clone(),
            fill: self.theme.quadrant_title_fill.clone(),
            font_size: self.config.title_font_size,
            horizontal_pos: HorizontalPos::Top,
            vertical_pos: VerticalPos::Center,
            rotation: 0.0,
            y: self.config.title_padding,
            x: self.config.chart_width / 2.0,
        })
    }

    /// `build()`: with points the x axis always moves to the bottom.
    pub fn build(&self) -> Build {
        let c = &self.config;
        let d = &self.data;
        let show_x_axis =
            c.show_x_axis && !(d.x_axis_left_text.is_empty() && d.x_axis_right_text.is_empty());
        let show_y_axis =
            c.show_y_axis && !(d.y_axis_top_text.is_empty() && d.y_axis_bottom_text.is_empty());
        let show_title = c.show_title && !d.title_text.is_empty();
        let x_axis_position = if d.points.is_empty() {
            c.x_axis_position.as_str()
        } else {
            "bottom"
        };
        let space = self.calculate_space(x_axis_position, show_x_axis, show_y_axis, show_title);
        Build {
            points: self.points(&space),
            quadrants: self.quadrants(&space),
            axis_labels: self.axis_labels(x_axis_position, show_x_axis, show_y_axis, &space),
            border_lines: self.borders(&space),
            title: self.title(show_title),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn builder(data: Data) -> Builder {
        let theme = ThemeConfig {
            quadrant_point_fill: Some("#aaa".to_string()),
            ..ThemeConfig::default()
        };
        Builder::new(
            Config::from_config(&QuadrantChartConfig::default()),
            theme,
            data,
        )
    }

    #[test]
    fn default_space_without_axes_or_title() {
        let b = builder(Data::default()).build();
        assert_eq!(b.quadrants.len(), 4);
        let q1 = &b.quadrants[0];
        assert_eq!(
            (q1.x, q1.y, q1.width, q1.height),
            (250.0, 5.0, 245.0, 245.0)
        );
        assert_eq!(q1.text.horizontal_pos, HorizontalPos::Middle);
        assert_eq!((q1.text.x, q1.text.y), (372.5, 127.5));
        assert!(b.title.is_none());
        assert!(b.axis_labels.is_empty());
        assert_eq!(b.border_lines.len(), 6);
        assert_eq!(b.border_lines[0].x1, 4.0);
    }

    #[test]
    fn points_move_the_x_axis_to_the_bottom_and_scale_linearly() {
        let data = Data {
            title_text: "T".into(),
            x_axis_left_text: "L".into(),
            x_axis_right_text: "R".into(),
            y_axis_bottom_text: "B".into(),
            y_axis_top_text: "U".into(),
            points: vec![PointInput {
                x: 0.5,
                y: 0.25,
                text: "P".into(),
                class_name: String::new(),
                styles: StylesObject::default(),
            }],
            ..Data::default()
        };
        let b = builder(data).build();
        // title 40, y axis 26 on the left, x axis 26 at the bottom.
        let q2 = &b.quadrants[1];
        assert_eq!((q2.x, q2.y), (31.0, 45.0));
        assert_eq!(q2.text.horizontal_pos, HorizontalPos::Top);
        assert_eq!(q2.text.y, 50.0);
        let p = &b.points[0];
        assert_eq!(p.x, 31.0 + 464.0 * 0.5);
        assert_eq!(p.y, 45.0 + 424.0 * 0.75);
        assert_eq!(p.stroke_width, "0px");
        assert_eq!(p.fill.as_deref(), Some("#aaa"));
        let x_left = &b.axis_labels[0];
        assert_eq!(x_left.vertical_pos, VerticalPos::Center);
        assert_eq!(x_left.y, 5.0 + 45.0 + 424.0 + 5.0);
        assert_eq!(b.axis_labels[2].rotation, -90.0);
    }

    #[test]
    fn class_styles_fill_what_the_point_leaves_out() {
        let data = Data {
            points: vec![PointInput {
                x: 0.0,
                y: 0.0,
                text: "P".into(),
                class_name: "c".into(),
                styles: StylesObject {
                    radius: Some(9.0),
                    ..StylesObject::default()
                },
            }],
            ..Data::default()
        };
        let mut b = builder(data);
        b.add_class(
            "c",
            StylesObject {
                radius: Some(3.0),
                color: Some("#123456".into()),
                stroke_color: None,
                stroke_width: Some("2px".into()),
            },
        );
        let p = &b.build().points[0];
        assert_eq!(p.radius, 9.0);
        assert_eq!(p.fill.as_deref(), Some("#123456"));
        assert_eq!(p.stroke_color.as_deref(), Some("#aaa"));
        assert_eq!(p.stroke_width, "2px");
    }
}
