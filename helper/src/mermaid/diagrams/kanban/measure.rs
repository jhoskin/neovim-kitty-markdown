//! A [`LabelMeasurer`] over the text package, so the `kanbanItem` shape and
//! the section titles measure with the same rustybuzz metrics the rest of
//! the diagram uses.
//!
//! Temporary: `TextMetrics` implements `LabelMeasurer` on main. The lead
//! swaps this adapter for that implementation at integration.

use crate::mermaid::scene::{FontStyle as SceneStyle, FontWeight};
use crate::mermaid::shapes::{LabelBox, LabelLine, LabelMeasurer, LabelOpts, LabelRun};
use crate::text::markdown::{self, Line, WordType};
use crate::text::measure::{HTML_LINE_HEIGHT, SVG_LINE_HEIGHT_EM};
use crate::text::{FontSpec, FontStyle, TextMetrics};

pub struct Measurer<'a, 'f> {
    metrics: &'a TextMetrics<'f>,
    /// The CSS `font-family` chain every label inherits from the SVG root.
    family: String,
}

impl<'a, 'f> Measurer<'a, 'f> {
    pub fn new(metrics: &'a TextMetrics<'f>, family: impl Into<String>) -> Measurer<'a, 'f> {
        Measurer {
            metrics,
            family: family.into(),
        }
    }

    fn spec(&self, opts: &LabelOpts) -> FontSpec {
        let size = opts.font.size.unwrap_or(16.0) as f32;
        let mut spec = FontSpec::new(self.family.clone(), size);
        match opts.font.weight {
            Some(FontWeight::Bold) => spec = spec.weight(700),
            Some(FontWeight::Number(n)) => spec = spec.weight(n),
            Some(FontWeight::Normal) | None => {}
        }
        if matches!(
            opts.font.style,
            Some(SceneStyle::Italic | SceneStyle::Oblique)
        ) {
            spec = spec.style(FontStyle::Italic);
        }
        spec
    }
}

fn runs(line: &Line) -> Vec<LabelRun> {
    line.iter()
        .map(|word| LabelRun {
            text: word.content.clone(),
            bold: word.kind == WordType::Strong,
            italic: word.kind == WordType::Em,
        })
        .collect()
}

/// The runs of an HTML line: the browser lays the words out as one text
/// flow, so adjacent words of one style merge into one run. The shared
/// label builder joins runs with a space, as mermaid's SVG tspans do, so a
/// run after the first drops the space it starts with, and the line loses
/// the leading and trailing spaces the browser collapses.
fn html_runs(line: &Line) -> Vec<LabelRun> {
    let mut runs: Vec<LabelRun> = Vec::new();
    for word in line {
        let bold = word.kind == WordType::Strong;
        let italic = word.kind == WordType::Em;
        match runs.last_mut() {
            Some(last) if last.bold == bold && last.italic == italic => {
                last.text.push_str(&word.content);
            }
            _ => runs.push(LabelRun {
                text: word.content.clone(),
                bold,
                italic,
            }),
        }
    }
    for (i, run) in runs.iter_mut().enumerate() {
        if i == 0 {
            run.text = run.text.trim_start().to_string();
        } else if let Some(rest) = run.text.strip_prefix(' ') {
            run.text = rest.to_string();
        }
    }
    if let Some(last) = runs.last_mut() {
        last.text = last.text.trim_end().to_string();
    }
    runs
}

impl LabelMeasurer for Measurer<'_, '_> {
    fn measure(&self, text: &str, opts: &LabelOpts) -> LabelBox {
        let spec = self.spec(opts);
        let size = f64::from(spec.size);
        if opts.html {
            // `addHtmlSpan`: a `line-height: 1.5` div measured with
            // `getBoundingClientRect`; an empty span has no line box.
            let line_height = f64::from(HTML_LINE_HEIGHT) * size;
            if text.is_empty() {
                return LabelBox {
                    line_height,
                    ..LabelBox::default()
                };
            }
            let html = if opts.markdown {
                markdown::markdown_to_html(text)
            } else {
                markdown::non_markdown_to_html(text)
            };
            let lines = markdown::html_to_lines(&html);
            let label = self
                .metrics
                .html_label(&lines, &spec, opts.width.map(|w| w as f32));
            // `createText`'s `minWidth`: a narrower div becomes a table of
            // that width.
            let width = f64::from(label.width).max(opts.min_width);
            let lines = label
                .lines
                .iter()
                .map(|line| LabelLine {
                    runs: html_runs(line),
                    width: f64::from(self.metrics.words_width(line, &spec)),
                })
                .collect();
            LabelBox {
                x: 0.0,
                y: 0.0,
                width,
                height: f64::from(label.height),
                line_height,
                lines,
            }
        } else {
            // `createFormattedText`: tspans 1.1 em apart, `getBBox` on the
            // `<text>`, centred on x = 0.
            let line_height = f64::from(SVG_LINE_HEIGHT_EM) * size;
            if text.is_empty() {
                return LabelBox {
                    line_height,
                    ..LabelBox::default()
                };
            }
            let lines = if opts.markdown {
                markdown::markdown_to_lines(text)
            } else {
                markdown::non_markdown_to_lines(text)
            };
            let max_width = opts.width.map_or(f32::INFINITY, |w| w as f32);
            let label = self.metrics.svg_label(&lines, &spec, max_width);
            let width = f64::from(label.width);
            let lines = label
                .lines
                .iter()
                .map(|line| LabelLine {
                    runs: runs(line),
                    width: f64::from(self.metrics.compute_width_of_text(line, &spec)),
                })
                .collect();
            LabelBox {
                x: -width / 2.0,
                y: 0.0,
                width,
                height: f64::from(label.height),
                line_height,
                lines,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::scene::FontSpec as SceneFont;
    use crate::text;

    fn opts(html: bool, width: Option<f64>) -> LabelOpts {
        LabelOpts {
            font: SceneFont {
                size: Some(16.0),
                ..SceneFont::default()
            },
            html,
            width,
            min_width: 0.0,
            markdown: true,
        }
    }

    #[test]
    fn html_labels_are_one_and_a_half_lines_tall_and_wrap_at_the_width() {
        let fonts = text::font_system();
        let metrics = TextMetrics::new(fonts);
        let m = Measurer::new(&metrics, "\"Open Sans\", sans-serif");
        let one = m.measure("Design grammar", &opts(true, Some(175.0)));
        assert_eq!(one.height, 24.0);
        assert_eq!(one.lines.len(), 1);
        assert!(one.width > 0.0 && one.width < 175.0);
        let long = m.measure(
            "Create renderer so that it works in all cases. We also add some extra text here for testing purposes.",
            &opts(true, Some(175.0)),
        );
        assert!(long.lines.len() > 1, "{long:?}");
        assert_eq!(long.width, 175.0);
        // One run per line, without the spaces the browser collapses.
        assert!(long.lines.iter().all(|l| l.runs.len() == 1), "{long:?}");
        let joined: Vec<String> = long.lines.iter().map(|l| l.runs[0].text.clone()).collect();
        assert_eq!(
            joined.join(" "),
            "Create renderer so that it works in all cases. We also add some extra text here for testing purposes."
        );
        let styled = m.measure("**Second** card", &opts(true, Some(175.0)));
        let texts: Vec<&str> = styled.lines[0]
            .runs
            .iter()
            .map(|r| r.text.as_str())
            .collect();
        assert_eq!(texts, ["Second", "card"]);
        assert_eq!(long.height, 24.0 * long.lines.len() as f64);
        assert!(m.measure("", &opts(true, Some(175.0))).is_empty());
    }

    #[test]
    fn svg_labels_are_centred_on_the_origin() {
        let fonts = text::font_system();
        let metrics = TextMetrics::new(fonts);
        let m = Measurer::new(&metrics, "\"Open Sans\", sans-serif");
        let b = m.measure("Hello **there**", &opts(false, None));
        assert_eq!(b.x, -b.width / 2.0);
        assert_eq!(b.height, 22.0);
        assert!(b.lines[0].runs[1].bold);
    }
}
