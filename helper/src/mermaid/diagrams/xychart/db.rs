//! Port of `xychartDb.ts`: the state the grammar actions fill and the
//! chart builder reads.
//!
//! The db owns the effective `xyChart` config (the typed config section,
//! which mermaid's `getChartDefaultConfig` merges the same way) and the
//! theme config from `styles::xychart`. `setTmpSVGG` has no counterpart:
//! the builder measures text through the render context.

use crate::mermaid::config::XyChartConfig;
use crate::mermaid::jison::lexer::js_trim;
use crate::mermaid::scene::Color;
use crate::mermaid::styles::xychart::XyChartTheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

/// `AxisDataType`.
#[derive(Debug, Clone, PartialEq)]
pub enum AxisData {
    Band {
        title: String,
        categories: Vec<String>,
    },
    Linear {
        title: String,
        min: f64,
        max: f64,
    },
}

impl AxisData {
    pub fn title(&self) -> &str {
        match self {
            AxisData::Band { title, .. } | AxisData::Linear { title, .. } => title,
        }
    }

    fn set_title(&mut self, text: String) {
        match self {
            AxisData::Band { title, .. } | AxisData::Linear { title, .. } => *title = text,
        }
    }
}

/// The first element of a `SimplePlotDataType` entry: a category for a
/// band x axis, a number for a linear one (mermaid keeps the number as the
/// string `${min + index * step}`; the scale coerces it back).
#[derive(Debug, Clone, PartialEq)]
pub enum XValue {
    Category(String),
    Number(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlotKind {
    Line {
        stroke_fill: Color,
        stroke_width: f64,
        /// Per-point labels, present when at least one point has one.
        point_labels: Option<Vec<String>>,
    },
    Bar {
        fill: Color,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plot {
    pub kind: PlotKind,
    pub title: String,
    pub data: Vec<(XValue, f64)>,
}

impl Plot {
    pub fn is_bar(&self) -> bool {
        matches!(self.kind, PlotKind::Bar { .. })
    }
}

/// A parsed `dataPoint`: the number and its optional quoted label.
#[derive(Debug, Clone, PartialEq)]
pub struct DataPoint {
    pub value: f64,
    pub label: String,
}

/// `NormalTextType` without the `markdown` flag, which nothing reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextValue {
    pub text: String,
}

pub struct Db {
    pub config: XyChartConfig,
    pub theme: XyChartTheme,
    pub title: String,
    pub x_axis: AxisData,
    pub y_axis: AxisData,
    pub plots: Vec<Plot>,
    plot_index: usize,
    has_set_x_axis: bool,
    has_set_y_axis: bool,
}

/// `sanitizeText(text.trim(), config)` at `securityLevel: strict`:
/// `removeScript` leaves plain text alone and DOMPurify re-serializes it as
/// HTML, so `&`, `<`, and `>` come back as entities, which d3's `.text()`
/// then shows verbatim. A `<` that opens a tag stays, as DOMPurify keeps
/// the element.
pub fn sanitize_text(text: &str) -> String {
    let text = js_trim(text);
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut in_tag = false;
    for (i, c) in chars.iter().enumerate() {
        match c {
            '&' => {
                let rest: String = chars[i + 1..].iter().take(12).collect();
                let entity = rest.find(';').map(|end| &rest[..end]).is_some_and(|name| {
                    !name.is_empty()
                        && (name.chars().all(|c| c.is_ascii_alphanumeric())
                            || (name.starts_with('#')
                                && name[1..].chars().all(|c| c.is_ascii_hexdigit() || c == 'x')))
                });
                if entity {
                    out.push('&');
                } else {
                    out.push_str("&amp;");
                }
            }
            '<' => {
                let next = chars.get(i + 1);
                if next.is_some_and(|n| n.is_ascii_alphabetic() || *n == '/') {
                    in_tag = true;
                    out.push('<');
                } else {
                    out.push_str("&lt;");
                }
            }
            '>' if in_tag => {
                in_tag = false;
                out.push('>');
            }
            '>' => out.push_str("&gt;"),
            c => out.push(*c),
        }
    }
    out
}

impl Db {
    pub fn new(config: &XyChartConfig, theme: XyChartTheme) -> Db {
        Db {
            config: config.clone(),
            theme,
            title: String::new(),
            x_axis: AxisData::Band {
                title: String::new(),
                categories: Vec::new(),
            },
            y_axis: AxisData::Linear {
                title: String::new(),
                min: f64::INFINITY,
                max: f64::NEG_INFINITY,
            },
            plots: Vec::new(),
            plot_index: 0,
            has_set_x_axis: false,
            has_set_y_axis: false,
        }
    }

    pub fn orientation(&self) -> Orientation {
        if self.config.chart_orientation == "horizontal" {
            Orientation::Horizontal
        } else {
            Orientation::Vertical
        }
    }

    /// `setOrientation`: anything but `horizontal` is vertical.
    pub fn set_orientation(&mut self, orientation: &str) {
        self.config.chart_orientation = if orientation == "horizontal" {
            "horizontal".to_string()
        } else {
            "vertical".to_string()
        };
    }

    /// `commonDb.setDiagramTitle`.
    pub fn set_diagram_title(&mut self, title: &str) {
        self.title = sanitize_text(title);
    }

    pub fn set_x_axis_title(&mut self, title: &TextValue) {
        self.x_axis.set_title(sanitize_text(&title.text));
    }

    pub fn set_x_axis_range_data(&mut self, min: f64, max: f64) {
        self.x_axis = AxisData::Linear {
            title: self.x_axis.title().to_string(),
            min,
            max,
        };
        self.has_set_x_axis = true;
    }

    pub fn set_x_axis_band(&mut self, categories: &[TextValue]) {
        self.x_axis = AxisData::Band {
            title: self.x_axis.title().to_string(),
            categories: categories.iter().map(|c| sanitize_text(&c.text)).collect(),
        };
        self.has_set_x_axis = true;
    }

    pub fn set_y_axis_title(&mut self, title: &TextValue) {
        self.y_axis.set_title(sanitize_text(&title.text));
    }

    pub fn set_y_axis_range_data(&mut self, min: f64, max: f64) {
        self.y_axis = AxisData::Linear {
            title: self.y_axis.title().to_string(),
            min,
            max,
        };
        self.has_set_y_axis = true;
    }

    /// `setYAxisRangeFromPlotData`: widens the auto range; it never sets
    /// `hasSetYAxis`, so every plot takes part.
    fn set_y_axis_range_from_plot_data(&mut self, data: &[f64]) {
        let min_value = data.iter().copied().fold(f64::INFINITY, f64::min);
        let max_value = data.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let (prev_min, prev_max) = match &self.y_axis {
            AxisData::Linear { min, max, .. } => (*min, *max),
            AxisData::Band { .. } => (f64::INFINITY, f64::NEG_INFINITY),
        };
        self.y_axis = AxisData::Linear {
            title: self.y_axis.title().to_string(),
            min: prev_min.min(min_value),
            max: prev_max.max(max_value),
        };
    }

    /// `transformDataWithoutCategory`.
    fn transform_data_without_category(&mut self, data: &[f64]) -> Vec<(XValue, f64)> {
        if data.is_empty() {
            return Vec::new();
        }
        if !self.has_set_x_axis {
            let (prev_min, prev_max) = match &self.x_axis {
                AxisData::Linear { min, max, .. } => (*min, *max),
                AxisData::Band { .. } => (f64::INFINITY, f64::NEG_INFINITY),
            };
            self.set_x_axis_range_data(prev_min.min(1.0), prev_max.max(data.len() as f64));
        }
        // A band axis truncates the data to its categories, so the y range
        // comes from visible points only.
        let data = match &self.x_axis {
            AxisData::Band { categories, .. } if data.len() > categories.len() => {
                &data[..categories.len()]
            }
            _ => data,
        };
        if !self.has_set_y_axis {
            self.set_y_axis_range_from_plot_data(data);
        }
        match &self.x_axis {
            // `categories.map((c, i) => [c, data[i]])`: a category past the
            // data gets `undefined`, which draws nothing, so it is left out.
            AxisData::Band { categories, .. } => categories
                .iter()
                .zip(data)
                .map(|(c, v)| (XValue::Category(c.clone()), *v))
                .collect(),
            AxisData::Linear { min, max, .. } => {
                if data.len() == 1 {
                    vec![(XValue::Number(*min), data[0])]
                } else {
                    let step = (max - min) / (data.len() as f64 - 1.0);
                    data.iter()
                        .enumerate()
                        .map(|(i, v)| (XValue::Number(min + i as f64 * step), *v))
                        .collect()
                }
            }
        }
    }

    /// `getPlotColorFromPalette`.
    fn plot_color(&self) -> Color {
        let palette = &self.theme.plot_color_palette;
        if palette.is_empty() {
            return Color::BLACK;
        }
        let i = if self.plot_index == 0 {
            0
        } else {
            self.plot_index % palette.len()
        };
        palette[i]
    }

    pub fn set_line_data(&mut self, title: &TextValue, data: &[DataPoint]) {
        let values: Vec<f64> = data.iter().map(|d| d.value).collect();
        let labels: Vec<String> = data
            .iter()
            .map(|d| {
                if d.label.is_empty() {
                    String::new()
                } else {
                    sanitize_text(&d.label)
                }
            })
            .collect();
        let plot_data = self.transform_data_without_category(&values);
        let has_any_label = labels.iter().any(|l| !l.is_empty());
        let stroke_fill = self.plot_color();
        self.plots.push(Plot {
            kind: PlotKind::Line {
                stroke_fill,
                stroke_width: 2.0,
                point_labels: has_any_label.then_some(labels),
            },
            title: sanitize_text(&title.text),
            data: plot_data,
        });
        self.plot_index += 1;
    }

    pub fn set_bar_data(&mut self, title: &TextValue, data: &[DataPoint]) {
        let values: Vec<f64> = data.iter().map(|d| d.value).collect();
        let plot_data = self.transform_data_without_category(&values);
        let fill = self.plot_color();
        self.plots.push(Plot {
            kind: PlotKind::Bar { fill },
            title: sanitize_text(&title.text),
            data: plot_data,
        });
        self.plot_index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::styles::xychart::theme_config;
    use crate::mermaid::theme;

    fn db() -> Db {
        let tv = theme::resolve("default", None).unwrap();
        Db::new(&XyChartConfig::default(), theme_config(&tv))
    }

    fn points(values: &[f64]) -> Vec<DataPoint> {
        values
            .iter()
            .map(|v| DataPoint {
                value: *v,
                label: String::new(),
            })
            .collect()
    }

    #[test]
    fn auto_ranges_from_the_first_plot_length_and_all_values() {
        let mut db = db();
        let t = TextValue {
            text: String::new(),
        };
        db.set_line_data(&t, &points(&[3.0, 1.0, 2.0]));
        db.set_bar_data(&t, &points(&[10.0, -1.0]));
        assert_eq!(
            db.x_axis,
            AxisData::Linear {
                title: String::new(),
                min: 1.0,
                max: 3.0
            }
        );
        assert_eq!(
            db.y_axis,
            AxisData::Linear {
                title: String::new(),
                min: -1.0,
                max: 10.0
            }
        );
        assert_eq!(db.plots[0].data[1], (XValue::Number(2.0), 1.0));
        assert_eq!(
            db.plots[1].data,
            vec![(XValue::Number(1.0), 10.0), (XValue::Number(3.0), -1.0)]
        );
    }

    #[test]
    fn band_axis_truncates_and_colors_cycle() {
        let mut db = db();
        db.set_x_axis_band(&[
            TextValue { text: " a ".into() },
            TextValue { text: "b".into() },
        ]);
        let t = TextValue { text: "s".into() };
        db.set_bar_data(&t, &points(&[1.0, 2.0, 99.0]));
        assert_eq!(
            db.plots[0].data,
            vec![
                (XValue::Category("a".into()), 1.0),
                (XValue::Category("b".into()), 2.0)
            ]
        );
        assert_eq!(
            db.y_axis,
            AxisData::Linear {
                title: String::new(),
                min: 1.0,
                max: 2.0
            }
        );
        assert_eq!(
            db.plots[0].kind,
            PlotKind::Bar {
                fill: Color::rgb(0xEC, 0xEC, 0xFF)
            }
        );
    }

    #[test]
    fn sanitizer_matches_dompurify_serialization() {
        assert_eq!(sanitize_text(" R&D "), "R&amp;D");
        assert_eq!(sanitize_text("a &amp; b"), "a &amp; b");
        assert_eq!(sanitize_text("1 < 2 > 0"), "1 &lt; 2 &gt; 0");
        assert_eq!(sanitize_text("<b>x</b>"), "<b>x</b>");
    }
}
