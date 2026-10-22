//! `diagrams/gantt/styles.js` (mermaid 12.2.0) resolved to attributes.
//!
//! The renderer hands each element its class list in mermaid's words
//! (section number, task state, text placement) and gets back the `Style`
//! the browser would compute from the stylesheet: the cascade of the
//! `getStyles` rules, mermaid's root rule `#id { fill: textColor }`, and
//! the SVG defaults. `!important` ordering and specificity are resolved
//! here once, so the renderer never sees a class name.

use crate::mermaid::scene::{
    Color, FontSpec, FontStyle, FontWeight, Paint, Style, TextAnchor, TransformOp,
};
use crate::mermaid::theme::ThemeVariables;
use crate::mermaid::theme::khroma::{self, Channel};

/// The section-specific rules exist for `section0` to `section3` only.
const STYLED_SECTIONS: usize = 4;

/// A CSS color as the theme writes it: hex, `rgb()`, `rgba()`, `hsl()`, or
/// a keyword. An unparsable value is treated as `black`, which is what a
/// browser paints for an invalid `fill` in an inherited chain of defaults.
pub fn css_color(text: &str) -> Color {
    let Ok(mut channels) = khroma::parse(text.trim()) else {
        return Color::BLACK;
    };
    let byte = |ch: Channel, c: &mut khroma::Channels| c.get(ch).round().clamp(0.0, 255.0) as u8;
    let r = byte(Channel::R, &mut channels);
    let g = byte(Channel::G, &mut channels);
    let b = byte(Channel::B, &mut channels);
    let a = channels.get(Channel::A).clamp(0.0, 1.0);
    Color::rgba(r, g, b, a)
}

/// The state classes a task rect carries: `taskN`, `activeN`, `doneN`,
/// `critN`, `activeCritN`, `doneCritN`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Plain,
    Active,
    Done,
    Crit,
    ActiveCrit,
    DoneCrit,
}

impl TaskState {
    /// The `taskClass` branch of `drawRects`.
    pub fn of(active: bool, done: bool, crit: bool) -> TaskState {
        if active {
            if crit {
                TaskState::ActiveCrit
            } else {
                TaskState::Active
            }
        } else if done {
            if crit {
                TaskState::DoneCrit
            } else {
                TaskState::Done
            }
        } else if crit {
            TaskState::Crit
        } else {
            TaskState::Plain
        }
    }
}

/// Where `drawRects` put the label: `taskText`, `taskTextOutsideRight`,
/// or `taskTextOutsideLeft`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextPlacement {
    Inside,
    OutsideRight,
    OutsideLeft,
}

/// The `taskType` classes of a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LabelFlags {
    pub active: bool,
    pub done: bool,
    pub crit: bool,
    pub milestone: bool,
    pub vert: bool,
    pub clickable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GanttStyles {
    pub font_family: String,
    pub section_bkg: Color,
    pub alt_section_bkg: Color,
    pub section_bkg2: Color,
    pub exclude_bkg: Color,
    pub task_border: Color,
    pub task_bkg: Color,
    pub task_text: Color,
    pub task_text_outside: Color,
    pub task_text_clickable: Color,
    pub task_text_dark: Color,
    pub active_task_border: Color,
    pub active_task_bkg: Color,
    pub grid: Color,
    pub done_task_bkg: Color,
    pub done_task_border: Color,
    pub crit_border: Color,
    pub crit_bkg: Color,
    pub today_line: Color,
    pub vert_line: Color,
    pub title: Option<Color>,
    /// `#id { fill: textColor }`: what every element without a fill rule
    /// inherits.
    pub text: Color,
}

impl GanttStyles {
    pub fn from_theme(tv: &ThemeVariables) -> GanttStyles {
        let color = |name: &str| css_color(tv.str(name).unwrap_or("black"));
        GanttStyles {
            font_family: tv
                .str("fontFamily")
                .unwrap_or(crate::mermaid::scene::DEFAULT_FONT_FAMILY)
                .to_string(),
            section_bkg: color("sectionBkgColor"),
            alt_section_bkg: color("altSectionBkgColor"),
            section_bkg2: color("sectionBkgColor2"),
            exclude_bkg: color("excludeBkgColor"),
            task_border: color("taskBorderColor"),
            task_bkg: color("taskBkgColor"),
            task_text: color("taskTextColor"),
            task_text_outside: color("taskTextOutsideColor"),
            task_text_clickable: color("taskTextClickableColor"),
            task_text_dark: color("taskTextDarkColor"),
            active_task_border: color("activeTaskBorderColor"),
            active_task_bkg: color("activeTaskBkgColor"),
            grid: color("gridColor"),
            done_task_bkg: color("doneTaskBkgColor"),
            done_task_border: color("doneTaskBorderColor"),
            crit_border: color("critBorderColor"),
            crit_bkg: color("critBkgColor"),
            today_line: color("todayLineColor"),
            vert_line: color("vertLineColor"),
            title: tv
                .str("titleColor")
                .filter(|s| !s.is_empty())
                .map(css_color),
            text: color("textColor"),
        }
    }

    /// `.titleText { fill: titleColor || textColor }`.
    pub fn title_fill(&self) -> Color {
        self.title.unwrap_or(self.text)
    }

    /// `rect.section.sectionN`: `.section { stroke: none; opacity: 0.2 }`
    /// with the fill of `section0`, `section2`, or `section1, section3`.
    /// A number past the styled four inherits the root fill.
    pub fn section_row(&self, n: usize) -> Style {
        let fill = match n {
            0 => self.section_bkg,
            2 => self.section_bkg2,
            1 | 3 => self.alt_section_bkg,
            _ => self.text,
        };
        Style {
            fill: Some(Paint::Color(fill)),
            stroke: Some(Paint::None),
            opacity: Some(0.2),
            ..Style::default()
        }
    }

    /// `rect.exclude-range`.
    pub fn exclude_range(&self) -> Style {
        Style::fill(self.exclude_bkg)
    }

    /// `rect.task ...`: `.task { stroke-width: 2 }` plus the state rule for
    /// section `n`, then `.vert { stroke: vertLineColor }`.
    pub fn task_rect(&self, state: TaskState, n: usize, vert: bool) -> Style {
        let mut style = Style {
            stroke_width: Some(2.0),
            ..Style::default()
        };
        if n < STYLED_SECTIONS {
            let (fill, stroke) = match state {
                TaskState::Plain => (self.task_bkg, self.task_border),
                TaskState::Active => (self.active_task_bkg, self.active_task_border),
                TaskState::Done => (self.done_task_bkg, self.done_task_border),
                TaskState::Crit => (self.crit_bkg, self.crit_border),
                TaskState::ActiveCrit => (self.active_task_bkg, self.crit_border),
                TaskState::DoneCrit => (self.done_task_bkg, self.crit_border),
            };
            style.fill = Some(Paint::Color(fill));
            style.stroke = Some(Paint::Color(stroke));
        } else {
            style.fill = Some(Paint::Color(self.text));
        }
        if vert {
            style.stroke = Some(Paint::Color(self.vert_line));
        }
        style
    }

    /// `.milestone { transform: rotate(45deg) scale(0.8, 0.8) }` about the
    /// `transform-origin` mermaid writes on the rect.
    pub fn milestone_transform(origin_x: f64, origin_y: f64) -> Vec<TransformOp> {
        vec![
            TransformOp::Translate(origin_x, origin_y),
            TransformOp::Rotate(45.0, 0.0, 0.0),
            TransformOp::Scale(0.8, 0.8),
            TransformOp::Translate(-origin_x, -origin_y),
        ]
    }

    /// The computed style of a task label from its class list.
    pub fn task_text(&self, placement: TextPlacement, n: usize, flags: LabelFlags) -> Style {
        let styled = n < STYLED_SECTIONS;
        // Normal rules, later in the sheet wins at equal specificity.
        let mut fill = match placement {
            TextPlacement::Inside => {
                if styled {
                    self.task_text
                } else {
                    self.text
                }
            }
            TextPlacement::OutsideRight | TextPlacement::OutsideLeft => {
                if styled {
                    self.task_text_outside
                } else {
                    self.task_text_dark
                }
            }
        };
        let mut anchor = match placement {
            TextPlacement::Inside => TextAnchor::Middle,
            TextPlacement::OutsideRight => TextAnchor::Start,
            TextPlacement::OutsideLeft => TextAnchor::End,
        };
        let mut font = FontSpec::default();
        if flags.vert {
            // `.vertText` comes after the placement rules.
            anchor = TextAnchor::Middle;
            font.size = Some(15.0);
        }
        if flags.milestone {
            font.style = Some(FontStyle::Italic);
        }
        if flags.clickable {
            font.weight = Some(FontWeight::Bold);
        }

        // `!important` rules with one class, in sheet order: the last one
        // that applies wins.
        let state_text = styled && (flags.active || flags.done);
        if state_text {
            fill = self.task_text_dark;
        }
        if flags.vert {
            fill = self.vert_line;
        }
        if styled && flags.active && flags.crit {
            fill = self.task_text_dark;
        }
        // `!important` rules with two classes beat the ones above; among
        // them the done-outside rules come last in the sheet.
        let outside = placement != TextPlacement::Inside;
        if flags.clickable {
            fill = self.task_text_clickable;
        }
        if styled && flags.done && outside {
            fill = self.task_text_outside;
        }

        Style {
            fill: Some(Paint::Color(fill)),
            text_anchor: Some(anchor),
            font,
            ..Style::default()
        }
    }

    /// `text.sectionTitle.sectionTitleN`.
    pub fn section_title(&self, n: usize) -> Style {
        let fill = if n < STYLED_SECTIONS {
            self.title_fill()
        } else {
            self.text
        };
        Style {
            fill: Some(Paint::Color(fill)),
            text_anchor: Some(TextAnchor::Start),
            ..Style::default()
        }
    }

    /// `.titleText`.
    pub fn title_text(&self) -> Style {
        Style {
            fill: Some(Paint::Color(self.title_fill())),
            text_anchor: Some(TextAnchor::Middle),
            font: FontSpec {
                size: Some(18.0),
                ..FontSpec::default()
            },
            ..Style::default()
        }
    }

    /// `g.tick`: `.grid .tick { stroke: gridColor; opacity: 0.8 }`. The
    /// tick line's own `stroke="currentColor"` wins over the inherited
    /// grid color, and nothing sets `color`, so the line is black.
    pub fn tick_group(&self) -> Style {
        Style {
            stroke: Some(Paint::Color(self.grid)),
            opacity: Some(0.8),
            ..Style::default()
        }
    }

    pub fn tick_line(&self) -> Style {
        Style {
            stroke: Some(Paint::Color(Color::BLACK)),
            ..Style::default()
        }
    }

    /// `.grid .tick text { fill: textColor }` over the `fill="#000"`
    /// attribute, `stroke="none"`, `text-anchor: middle`.
    pub fn tick_text(&self) -> Style {
        Style {
            fill: Some(Paint::Color(self.text)),
            stroke: Some(Paint::None),
            text_anchor: Some(TextAnchor::Middle),
            font: FontSpec {
                size: Some(10.0),
                ..FontSpec::default()
            },
            ..Style::default()
        }
    }

    /// `.grid path { stroke-width: 0 }` with d3's `stroke="currentColor"`.
    pub fn domain_path(&self) -> Style {
        Style {
            stroke: Some(Paint::Color(Color::BLACK)),
            stroke_width: Some(0.0),
            fill: Some(Paint::None),
            ..Style::default()
        }
    }

    /// `.today { fill: none; stroke: todayLineColor; stroke-width: 2px }`
    /// overridden by the inline style the `todayMarker` statement carries.
    pub fn today_line(&self, inline: &str) -> Style {
        let mut style = Style {
            fill: Some(Paint::None),
            stroke: Some(Paint::Color(self.today_line)),
            stroke_width: Some(2.0),
            ..Style::default()
        };
        for declaration in inline.split(';') {
            let Some((name, value)) = declaration.split_once(':') else {
                continue;
            };
            let value = value.trim();
            match name.trim() {
                "stroke" => style.stroke = Some(Paint::Color(css_color(value))),
                "stroke-width" => {
                    if let Ok(w) = value.trim_end_matches("px").parse::<f64>() {
                        style.stroke_width = Some(w);
                    }
                }
                "fill" => style.fill = Some(Paint::Color(css_color(value))),
                "opacity" => style.opacity = value.parse().ok(),
                "stroke-opacity" => style.stroke_opacity = value.parse().ok(),
                _ => {}
            }
        }
        style
    }
}
