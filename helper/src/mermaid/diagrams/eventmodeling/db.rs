//! Port of the eventmodeling `db.ts` (mermaid 12.2.0): `setAst` with its
//! duplicate frame check and `getState`, the decider and evolver loop that
//! places every frame in a swimlane and infers the relations.
//!
//! `getState` measures each box with `calculateTextDimensions` on the HTML
//! string it builds, tags and entities included, so the widths follow the
//! SVG text metrics of that markup, not the rendered HTML.

use std::collections::BTreeMap;

use super::parser::{Ast, DataEntity, Frame};
use crate::text::wrap::wrap_label;
use crate::text::{FontSpec, TextMetrics};

/// `diagramProps` in `db.ts`.
pub const SWIMLANE_MIN_HEIGHT: f64 = 70.0;
pub const SWIMLANE_PADDING: f64 = 15.0;
pub const SWIMLANE_GAP: f64 = 10.0;
pub const BOX_PADDING: f64 = 10.0;
pub const BOX_OVERLAP: f64 = 90.0;
pub const BOX_MIN_WIDTH: f64 = 80.0;
pub const BOX_MAX_WIDTH: f64 = 450.0;
pub const BOX_MIN_HEIGHT: f64 = 80.0;
pub const BOX_MAX_HEIGHT: f64 = 750.0;
pub const CONTENT_START_X: f64 = 250.0;
/// `textMaxWidth: 450 - 2 * 10`.
pub const TEXT_MAX_WIDTH: f64 = 430.0;
pub const BOX_TEXT_PADDING: f64 = 10.0;
const LABEL_UI_AUTOMATION: &str = "UI/Automation";
const LABEL_UI_AUTOMATION_PREFIX: &str = "UI/A: ";
const LABEL_COMMAND_READ_MODEL: &str = "Command/Read Model";
const LABEL_COMMAND_READ_MODEL_PREFIX: &str = "C/RM: ";
const LABEL_EVENTS: &str = "Events";
const LABEL_EVENTS_PREFIX: &str = "Stream: ";

/// `wrapLabelConfig` in `calculateTextProps`: the box text is measured in
/// mermaid's default chain at 16 px bold, whatever the theme font.
pub const MEASURE_FONT_FAMILY: &str = "\"trebuchet ms\", verdana, arial, sans-serif";
pub const MEASURE_FONT_SIZE: f32 = 16.0;
pub const MEASURE_FONT_WEIGHT: u16 = 700;

/// The kind of box a frame draws, from `calculateEntityVisualProps`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    Ui,
    Processor,
    ReadModel,
    Command,
    Event,
}

impl EntityKind {
    /// `modelEntityType` to its kind; the grammar allows no other value.
    pub fn from_type(model_entity_type: &str) -> EntityKind {
        match model_entity_type {
            "ui" => EntityKind::Ui,
            "pcr" | "processor" => EntityKind::Processor,
            "rmo" | "readmodel" => EntityKind::ReadModel,
            "cmd" | "command" => EntityKind::Command,
            _ => EntityKind::Event,
        }
    }
}

/// A swimlane; `index` orders them (`0` UI/Automation, `100`
/// Command/Read Model, `200` Events, and the namespaced lanes after each).
#[derive(Debug, Clone, PartialEq)]
pub struct Swimlane {
    pub index: i64,
    pub label: String,
    /// The right edge of the lane's last box.
    pub r: f64,
    pub y: f64,
    pub height: f64,
    pub max_height: f64,
}

/// The text of a box as the renderer's `.html(box.text)` shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxText {
    /// The `<b>` lines: the wrapped entity name split at `<br/>`, entities
    /// decoded.
    pub name_lines: Vec<String>,
    /// The `<code>` content split at `<br/>`, entities decoded, newlines
    /// kept for the browser to collapse; `None` without data. A data block
    /// ends with a `<br/>`, so its last segment is empty.
    pub code: Option<Vec<String>>,
    /// The HTML string `getState` measured, for the snapshots.
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EmBox {
    pub x: f64,
    /// `x + width + boxPadding`.
    pub r: f64,
    pub width: f64,
    pub height: f64,
    /// The key of the box's swimlane in [`State::swimlanes`].
    pub swimlane: i64,
    pub kind: EntityKind,
    pub text: BoxText,
    pub frame_name: String,
}

/// A relation from one box to another, by box index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Relation {
    pub source: usize,
    pub target: usize,
}

/// `Context` after `getState`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    pub boxes: Vec<EmBox>,
    /// Keyed by index; a `BTreeMap` iterates in the ascending order that JS
    /// gives integer-like object keys.
    pub swimlanes: BTreeMap<i64, Swimlane>,
    pub relations: Vec<Relation>,
    pub max_r: f64,
}

impl State {
    /// `sortedSwimlanesArray`.
    pub fn sorted_swimlanes(&self) -> impl Iterator<Item = &Swimlane> {
        self.swimlanes.values()
    }

    pub fn swimlane_of(&self, b: &EmBox) -> Option<&Swimlane> {
        self.swimlanes.get(&b.swimlane)
    }
}

/// The error `setAst` throws for a reused frame id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbError {
    pub message: String,
    /// 1-based line of the second frame.
    pub line: u32,
}

/// `setAst`: refuses a frame id that an earlier frame already uses.
pub fn check_frames(ast: &Ast) -> Result<(), DbError> {
    let mut seen = std::collections::HashSet::new();
    for frame in &ast.frames {
        if !seen.insert(frame.name.as_str()) {
            let line = frame.line + 1;
            return Err(DbError {
                message: format!(
                    "Duplicate event modeling frame ID \"{}\" on line {line}",
                    frame.name
                ),
                line,
            });
        }
    }
    Ok(())
}

/// `extractNamespace`: the part before the dot of a two-part identifier.
fn extract_namespace(entity_identifier: &str) -> Option<&str> {
    let parts: Vec<&str> = entity_identifier.split('.').collect();
    (parts.len() == 2).then(|| parts[0])
}

/// `extractName`: the part after the dot of a two-part identifier, else
/// the whole identifier.
fn extract_name(entity_identifier: &str) -> &str {
    let parts: Vec<&str> = entity_identifier.split('.').collect();
    if parts.len() == 2 {
        parts[1]
    } else {
        entity_identifier
    }
}

/// `findNextAvailableIndex`.
fn next_available_index(swimlanes: &BTreeMap<i64, Swimlane>, min: i64, max: i64) -> i64 {
    swimlanes
        .keys()
        .copied()
        .filter(|k| *k > min && *k < max)
        .fold(min, i64::max)
        + 1
}

/// `calculateSwimlaneProps`. `findSwimlaneByNamespace` never finds a
/// lane: the lanes `evolveFramePositioned` creates carry no `namespace`,
/// so every namespaced frame opens a lane of its own.
fn swimlane_props(frame: &Frame, swimlanes: &BTreeMap<i64, Swimlane>) -> (i64, String) {
    let namespace = extract_namespace(&frame.entity_identifier);
    let (base, label, prefix) = match EntityKind::from_type(&frame.model_entity_type) {
        EntityKind::Ui | EntityKind::Processor => {
            (0, LABEL_UI_AUTOMATION, LABEL_UI_AUTOMATION_PREFIX)
        }
        EntityKind::ReadModel | EntityKind::Command => (
            100,
            LABEL_COMMAND_READ_MODEL,
            LABEL_COMMAND_READ_MODEL_PREFIX,
        ),
        EntityKind::Event => (200, LABEL_EVENTS, LABEL_EVENTS_PREFIX),
    };
    match namespace {
        Some(ns) if !ns.is_empty() => (
            next_available_index(swimlanes, base, base + 100),
            format!("{prefix}{ns}"),
        ),
        _ => (base, label.to_string()),
    }
}

/// `sanitizeText` under `securityLevel: strict` for plain text: DOMPurify
/// serializes the text back as HTML, so the markup characters come out as
/// entities.
fn sanitize_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// What the browser shows for the entities `sanitizeText` and the
/// `&nbsp;` replacement put in the string.
fn decode_entities(text: &str) -> String {
    const ENTITIES: [(&str, char); 4] = [
        ("&nbsp;", '\u{a0}'),
        ("&amp;", '&'),
        ("&lt;", '<'),
        ("&gt;", '>'),
    ];
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    'outer: while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        for (entity, ch) in ENTITIES {
            if let Some(after) = rest.strip_prefix(entity) {
                out.push(ch);
                rest = after;
                continue 'outer;
            }
        }
        out.push('&');
        rest = &rest[1..];
    }
    out.push_str(rest);
    out
}

/// `<br/>` as `wrapLabel` joins with; the only break the strings carry.
const BR: &str = "<br/>";

/// JS `String.prototype.substring` over characters: negative and `NaN`
/// bounds clamp to 0, bounds past the end clamp to the length, and the
/// bounds swap when the start is larger.
fn js_substring(s: &str, start: i64, end: Option<i64>) -> String {
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len() as i64;
    let clamp = |v: i64| v.clamp(0, len) as usize;
    let a = clamp(start);
    let b = clamp(end.unwrap_or(len));
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    chars[lo..hi].iter().collect()
}

/// JS `indexOf` and `lastIndexOf` over characters, `-1` when absent.
fn js_index_of(s: &str, needle: &str) -> i64 {
    s.find(needle)
        .map_or(-1, |byte| s[..byte].chars().count() as i64)
}

fn js_last_index_of(s: &str, needle: &str) -> i64 {
    s.rfind(needle)
        .map_or(-1, |byte| s[..byte].chars().count() as i64)
}

/// Chrome's SVG text collapses white space: a newline or a tab counts as a
/// space, and a run of spaces as one.
fn svg_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_space = false;
    for c in text.chars() {
        let c = if matches!(c, '\n' | '\r' | '\t') {
            ' '
        } else {
            c
        };
        if c == ' ' {
            if last_space {
                continue;
            }
            last_space = true;
        } else {
            last_space = false;
        }
        out.push(c);
    }
    out
}

/// The `TextDimensionConfig` of `calculateTextProps`.
fn measure_spec() -> FontSpec {
    FontSpec::new(MEASURE_FONT_FAMILY, MEASURE_FONT_SIZE).weight(MEASURE_FONT_WEIGHT)
}

/// `textProps` of `calculateTextProps`.
struct TextProps {
    text: BoxText,
    width: f64,
    height: f64,
}

/// `calculateTextProps`.
fn text_props(frame: &Frame, data_entities: &[DataEntity], metrics: &TextMetrics<'_>) -> TextProps {
    let spec = measure_spec();
    let text_width = |s: &str| metrics.calculate_text_width(&svg_text(s), &spec);
    let wrap = |s: &str| wrap_label(s, TEXT_MAX_WIDTH as f32, BR, &text_width);

    let name = sanitize_text(extract_name(&frame.entity_identifier));
    let wrapped_name = wrap(&name);
    let mut content = format!("<b>{wrapped_name}</b>");

    let mut to_html: Option<String> = None;
    if let Some(inline) = &frame.data_inline_value {
        let after = js_substring(inline, js_index_of(inline, "{") + 1, None);
        let inner = js_substring(&after, 0, Some(js_last_index_of(&after, "}") - 1));
        to_html = Some(wrap(&sanitize_text(&inner)).replace(' ', "&nbsp;"));
    }
    if let Some(reference) = &frame.data_reference
        && let Some(entity) = data_entities.iter().find(|d| &d.name == reference)
    {
        let block = &entity.data_block_value;
        let after = js_substring(block, js_index_of(block, "{\n") + 2, None);
        let inner = js_substring(&after, 0, Some(js_last_index_of(&after, "}") - 1));
        let mut html = wrap(&sanitize_text(&inner)).replace(' ', "&nbsp;");
        html.push_str(BR);
        to_html = Some(html);
    }

    if let Some(html) = &to_html {
        content.push_str(&format!(
            "{BR}{BR}<code style=\"text-align: left; display: block;max-width:{}px\">{html}</code>",
            TEXT_MAX_WIDTH
        ));
    }
    let dims = metrics.calculate_text_dimensions(&svg_text(&content), &spec);
    // "a temporal workaround until a more complex dimension calculation is
    // in place": the code line measures its own markup, so a third of it.
    let width = if to_html.is_some() {
        f64::from(dims.width) / 3.0
    } else {
        f64::from(dims.width)
    };

    let split = |s: &str| -> Vec<String> { s.split(BR).map(decode_entities).collect() };
    let code = to_html.as_deref().map(split);
    TextProps {
        text: BoxText {
            name_lines: split(&wrapped_name),
            code,
            content,
        },
        width,
        height: f64::from(dims.height),
    }
}

/// `calculateX`.
fn calculate_x(swimlane: &Swimlane, previous: Option<&Swimlane>, last_box: Option<&EmBox>) -> f64 {
    let Some(previous) = previous else {
        return CONTENT_START_X;
    };
    if previous.index == swimlane.index && swimlane.r != 0.0 && !swimlane.r.is_nan() {
        return swimlane.r + BOX_PADDING;
    }
    match last_box {
        None => CONTENT_START_X,
        Some(b) => b.r - BOX_OVERLAP + BOX_PADDING,
    }
}

impl State {
    /// `decidePositionFrame` and `evolveFramePositioned`.
    fn position_frame(&mut self, frame: &Frame, props: TextProps, previous_swimlane: Option<i64>) {
        let (index, label) = swimlane_props(frame, &self.swimlanes);
        let mut swimlane = self.swimlanes.get(&index).cloned().unwrap_or(Swimlane {
            index,
            label,
            r: 0.0,
            y: index as f64 * SWIMLANE_MIN_HEIGHT + SWIMLANE_GAP,
            height: SWIMLANE_MIN_HEIGHT,
            max_height: SWIMLANE_MIN_HEIGHT,
        });
        let previous = previous_swimlane.and_then(|n| self.swimlanes.get(&n));
        let width = (props.width + 2.0 * BOX_TEXT_PADDING).clamp_js(BOX_MIN_WIDTH, BOX_MAX_WIDTH)
            + 2.0 * BOX_PADDING;
        let height = (props.height + 2.0 * BOX_TEXT_PADDING)
            .clamp_js(BOX_MIN_HEIGHT, BOX_MAX_HEIGHT)
            + 2.0 * BOX_PADDING;
        let x = calculate_x(&swimlane, previous, self.boxes.last());
        let r = x + width + BOX_PADDING;
        let max_r = self.swimlanes.values().map(|s| s.r).fold(r, js_max);

        swimlane.r = x + width;
        swimlane.max_height = js_max(swimlane.max_height, height);
        swimlane.height = js_max(SWIMLANE_MIN_HEIGHT, swimlane.max_height) + 2.0 * SWIMLANE_PADDING;
        self.boxes.push(EmBox {
            x,
            r,
            width,
            height,
            swimlane: index,
            kind: EntityKind::from_type(&frame.model_entity_type),
            text: props.text,
            frame_name: frame.name.clone(),
        });
        self.swimlanes.insert(index, swimlane);
        self.max_r = max_r;

        // The `swimlane.y` recalculation over the sorted lanes.
        let mut y = 0.0;
        for (i, lane) in self.swimlanes.values_mut().enumerate() {
            if i > 0 {
                lane.y = y;
            } else {
                lane.y = 0.0;
            }
            y = lane.y + lane.height + SWIMLANE_GAP;
        }
    }

    /// `findBoxByFrame`.
    fn box_by_frame(&self, name: &str) -> Option<usize> {
        self.boxes.iter().position(|b| b.frame_name == name)
    }

    /// `findBoxByLineIndex`: the nearest earlier box outside the target's
    /// lane.
    fn box_by_line_index(&self, target_swimlane: i64, line_index: i64) -> Option<usize> {
        if line_index < 0 {
            return None;
        }
        (0..=line_index as usize).rev().find(|&i| {
            self.boxes
                .get(i)
                .is_some_and(|b| b.swimlane != target_swimlane)
        })
    }

    /// `decidePositionRelation` and `evolveRelationPositioned`.
    fn position_relation(&mut self, index: usize, frame: &Frame, source: Option<&Frame>) {
        if index == 0 && frame.source_frames.is_empty() {
            return;
        }
        let Some(target) = self.box_by_frame(&frame.name) else {
            return;
        };
        let source = match source {
            Some(s) => self.box_by_frame(&s.name),
            None if frame.reset => None,
            None => {
                let lane = self.boxes[target].swimlane;
                self.box_by_line_index(lane, index as i64 - 1)
            }
        };
        if let Some(source) = source {
            self.relations.push(Relation { source, target });
        }
    }
}

/// `Math.max` of two numbers, `NaN` when either is.
fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

trait ClampJs {
    /// `Math.max(min, Math.min(max, self))`.
    fn clamp_js(self, min: f64, max: f64) -> f64;
}

impl ClampJs for f64 {
    fn clamp_js(self, min: f64, max: f64) -> f64 {
        js_max(
            min,
            if self.is_nan() {
                f64::NAN
            } else {
                self.min(max)
            },
        )
    }
}

/// `getState`.
pub fn state(ast: &Ast, metrics: &TextMetrics<'_>) -> State {
    let mut state = State::default();
    let mut previous_swimlane: Option<i64> = None;
    for (index, frame) in ast.frames.iter().enumerate() {
        let props = text_props(frame, &ast.data_entities, metrics);
        state.position_frame(frame, props, previous_swimlane);
        previous_swimlane = state.boxes.last().map(|b| b.swimlane);

        if frame.source_frames.is_empty() {
            state.position_relation(index, frame, None);
        } else {
            for source in ast
                .frames
                .iter()
                .filter(|f| frame.source_frames.contains(&f.name))
            {
                state.position_relation(index, frame, Some(source));
            }
        }
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::diagrams::eventmodeling::parser;
    use crate::text;

    fn state_of(src: &str) -> State {
        let ast = parser::parse(src).unwrap_or_else(|e| panic!("{}", e.message));
        let fonts = text::font_system();
        state(&ast, &TextMetrics::new(fonts))
    }

    #[test]
    fn substring_follows_js() {
        assert_eq!(js_substring("{ a }", 1, None), " a }");
        assert_eq!(js_substring(" a }", 0, Some(2)), " a");
        assert_eq!(js_substring("\"q\"", 0, Some(-2)), "");
        assert_eq!(js_substring("abc", 5, None), "");
    }

    #[test]
    fn entities_decode_once() {
        assert_eq!(decode_entities("a&nbsp;&amp;nbsp;&lt;"), "a\u{a0}&nbsp;<");
        assert_eq!(decode_entities("a & b"), "a & b");
    }

    #[test]
    fn duplicate_frames_are_refused() {
        let ast = parser::parse("eventmodeling\ntf 01 ui A\n%% c\nrf 01 evt B\n")
            .unwrap_or_else(|e| panic!("{}", e.message));
        let err = check_frames(&ast).expect_err("duplicate");
        assert_eq!(
            err.message,
            "Duplicate event modeling frame ID \"01\" on line 4"
        );
        assert_eq!(err.line, 4);
    }

    #[test]
    fn lanes_and_inferred_relations() {
        let s = state_of("eventmodeling\ntf 01 ui A\ntf 02 cmd B\ntf 03 evt C\ntf 04 evt Ns.D\n");
        let lanes: Vec<(i64, &str)> = s
            .sorted_swimlanes()
            .map(|l| (l.index, l.label.as_str()))
            .collect();
        assert_eq!(
            lanes,
            vec![
                (0, "UI/Automation"),
                (100, "Command/Read Model"),
                (200, "Events"),
                (201, "Stream: Ns"),
            ]
        );
        assert_eq!(s.boxes[0].x, CONTENT_START_X);
        assert_eq!(s.boxes[1].x, s.boxes[0].r - BOX_OVERLAP + BOX_PADDING);
        let pairs: Vec<(usize, usize)> = s.relations.iter().map(|r| (r.source, r.target)).collect();
        assert_eq!(pairs, vec![(0, 1), (1, 2), (2, 3)]);
    }

    #[test]
    fn reset_frames_keep_only_explicit_sources() {
        let s = state_of(
            "eventmodeling\ntf 01 ui UI\ntf 02 cmd Command\ntf 03 evt Event\nrf 04 rmo ReadModel ->> 03\nrf 05 ui OtherUI\n",
        );
        let names: Vec<(&str, &str)> = s
            .relations
            .iter()
            .map(|r| {
                (
                    s.boxes[r.source].frame_name.as_str(),
                    s.boxes[r.target].frame_name.as_str(),
                )
            })
            .collect();
        assert!(names.contains(&("03", "04")));
        assert!(!names.contains(&("04", "05")));
    }

    #[test]
    fn namespaced_frames_open_a_lane_each() {
        let s = state_of("eventmodeling\ntf 01 pcr Sync.A\ntf 02 ui Sync.B\n");
        let labels: Vec<&str> = s.sorted_swimlanes().map(|l| l.label.as_str()).collect();
        assert_eq!(labels, vec!["UI/A: Sync", "UI/A: Sync"]);
    }

    #[test]
    fn data_blocks_become_code_lines() {
        let s = state_of("eventmodeling\ntf 01 cmd A [[D]]\ndata D {\n  a: 1\n  b: 2\n}\n");
        let text = &s.boxes[0].text;
        assert_eq!(text.name_lines, vec!["A".to_string()]);
        assert_eq!(
            text.code,
            Some(vec![
                "a:\u{a0}1\n\u{a0}b:\u{a0}2".to_string(),
                String::new()
            ])
        );
    }
}
