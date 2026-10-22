//! Label measurement seam between the shapes and the text package.
//!
//! mermaid measures a label by inserting it into the DOM and reading
//! `getBoundingClientRect()` (HTML labels) or `getBBox()` (SVG text) in
//! `rendering-elements/shapes/util.ts` `labelHelper`. The shapes only need
//! the resulting box and the lines it holds, so that is the whole contract.
//! The text package implements [`LabelMeasurer`] with rustybuzz; the tests
//! use [`FixedMeasurer`], which charges a fixed advance per character.

use crate::mermaid::scene::FontSpec;

/// A width and a height in CSS pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Dims {
    pub width: f64,
    pub height: f64,
}

impl Dims {
    pub const fn new(width: f64, height: f64) -> Dims {
        Dims { width, height }
    }
}

/// One styled run inside a label line: mermaid's `MarkdownWord`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LabelRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
}

impl LabelRun {
    pub fn plain(text: impl Into<String>) -> LabelRun {
        LabelRun {
            text: text.into(),
            ..LabelRun::default()
        }
    }
}

/// One line of a label after wrapping.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LabelLine {
    pub runs: Vec<LabelRun>,
    /// Advance width of the whole line.
    pub width: f64,
}

/// The measured label: the `DOMRect` mermaid reads plus the wrapped lines
/// the renderer has to draw.
///
/// `x` and `y` reproduce the difference between the two measuring modes
/// that `labelHelper` relies on. An HTML label's `getBoundingClientRect()`
/// has `left`, so `bbox.x - bbox.left` is `0`; SVG text's `getBBox()` is
/// centred on `x = 0`, so `x` is `-width / 2`. Shapes place labels with
/// `pos - (bbox.x - (bbox.left ?? 0))`, which is `pos - x` here.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LabelBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Line height in pixels, so the renderer can place each line.
    pub line_height: f64,
    pub lines: Vec<LabelLine>,
}

impl LabelBox {
    pub fn dims(&self) -> Dims {
        Dims::new(self.width, self.height)
    }

    pub fn is_empty(&self) -> bool {
        self.lines
            .iter()
            .all(|l| l.runs.iter().all(|r| r.text.is_empty()))
    }
}

/// What `createText` was called with.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelOpts {
    pub font: FontSpec,
    /// `useHtmlLabels`: HTML measuring (`line-height: 1.5`) or SVG text
    /// (`getBBox`, `1.1em` tspans).
    pub html: bool,
    /// The wrapping width; `None` is `Number.POSITIVE_INFINITY`.
    pub width: Option<f64>,
    /// `minWidth` of `createText`; `0` disables it.
    pub min_width: f64,
    /// `labelType === 'markdown'`: `markdownToLines`, else
    /// `nonMarkdownToLines`.
    pub markdown: bool,
}

impl Default for LabelOpts {
    fn default() -> LabelOpts {
        LabelOpts {
            font: FontSpec::default(),
            html: true,
            width: None,
            min_width: 0.0,
            markdown: true,
        }
    }
}

/// The text package implements this. The lead wires
/// `crate::text::measure::TextMetrics` to it at integration.
pub trait LabelMeasurer {
    /// Measures `text` as `createText` would and returns the box with its
    /// wrapped lines. An empty `text` returns a zero box with no lines.
    fn measure(&self, text: &str, opts: &LabelOpts) -> LabelBox;
}

/// Test fake: every character advances `char_width`, every line is
/// `line_height` tall, `<br>` and newlines break lines, `**x**` and `*x*`
/// mark bold and italic runs, and no wrapping happens. Deterministic and
/// font-free, so snapshots do not depend on the installed fonts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FixedMeasurer {
    pub char_width: f64,
    pub line_height: f64,
}

impl FixedMeasurer {
    /// The fake the shape snapshots use: 8 px per character, 16 px lines.
    pub const SNAPSHOT: FixedMeasurer = FixedMeasurer {
        char_width: 8.0,
        line_height: 16.0,
    };
}

impl Default for FixedMeasurer {
    fn default() -> FixedMeasurer {
        FixedMeasurer::SNAPSHOT
    }
}

impl LabelMeasurer for FixedMeasurer {
    fn measure(&self, text: &str, opts: &LabelOpts) -> LabelBox {
        if text.is_empty() {
            return LabelBox {
                line_height: self.line_height,
                ..LabelBox::default()
            };
        }
        let normalized = text
            .replace("<br/>", "\n")
            .replace("<br />", "\n")
            .replace("<br>", "\n")
            .replace("\\n", "\n");
        let lines: Vec<LabelLine> = normalized
            .split('\n')
            .map(|line| {
                let runs = if opts.markdown {
                    split_runs(line)
                } else {
                    vec![LabelRun::plain(line)]
                };
                let width = runs
                    .iter()
                    .map(|r| r.text.chars().count() as f64 * self.char_width)
                    .sum();
                LabelLine { runs, width }
            })
            .collect();
        let mut width = lines.iter().map(|l| l.width).fold(0.0, f64::max);
        if width < opts.min_width {
            width = opts.min_width;
        }
        let height = lines.len() as f64 * self.line_height;
        let x = if opts.html { 0.0 } else { -width / 2.0 };
        LabelBox {
            x,
            y: 0.0,
            width,
            height,
            line_height: self.line_height,
            lines,
        }
    }
}

/// Splits `**bold**` and `*italic*` spans into runs. Enough markdown for
/// the fake; the text package has the real `markdownToLines` port.
fn split_runs(line: &str) -> Vec<LabelRun> {
    let mut runs = Vec::new();
    let mut rest = line;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("**")
            && let Some(end) = after.find("**")
        {
            runs.push(LabelRun {
                text: after[..end].to_string(),
                bold: true,
                italic: false,
            });
            rest = &after[end + 2..];
            continue;
        }
        if let Some(after) = rest.strip_prefix('*')
            && let Some(end) = after.find('*')
        {
            runs.push(LabelRun {
                text: after[..end].to_string(),
                bold: false,
                italic: true,
            });
            rest = &after[end + 1..];
            continue;
        }
        let first_len = rest.chars().next().map_or(1, char::len_utf8);
        let next = rest[first_len..]
            .find('*')
            .map(|i| i + first_len)
            .unwrap_or(rest.len());
        runs.push(LabelRun::plain(&rest[..next]));
        rest = &rest[next..];
    }
    if runs.is_empty() {
        runs.push(LabelRun::plain(""));
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_measurer_counts_characters_and_lines() {
        let m = FixedMeasurer::SNAPSHOT;
        let b = m.measure("Hello<br/>World!", &LabelOpts::default());
        assert_eq!(b.width, 48.0);
        assert_eq!(b.height, 32.0);
        assert_eq!(b.lines.len(), 2);
        assert_eq!(b.x, 0.0);
    }

    #[test]
    fn fixed_measurer_splits_emphasis() {
        let m = FixedMeasurer::SNAPSHOT;
        let b = m.measure("a **b** *c*", &LabelOpts::default());
        let runs = &b.lines[0].runs;
        assert_eq!(runs.len(), 4);
        assert!(runs[1].bold);
        assert!(runs[3].italic);
        assert_eq!(b.width, 5.0 * 8.0);
    }

    #[test]
    fn svg_mode_centres_the_box() {
        let m = FixedMeasurer::SNAPSHOT;
        let opts = LabelOpts {
            html: false,
            ..LabelOpts::default()
        };
        let b = m.measure("abcd", &opts);
        assert_eq!(b.x, -16.0);
    }
}
