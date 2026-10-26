//! `cleanupText`, front matter, directive extraction, comment stripping.
//!
//! Port of `preprocess.ts`, `diagram-api/frontmatter.ts`,
//! `diagram-api/comments.ts`, and the `matchFrontMatter` and
//! `stripAnyComments` scanners of `diagram-api/regexes.ts` at mermaid
//! 12.2.0. `preprocess` returns what `preprocessDiagram` returns: the
//! cleaned code the parser sees, the front matter title, and the config
//! object merged from front matter and directives in mermaid's order
//! (directives win, through lodash `merge`).

use std::fmt;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

use super::detect::{DetectError, detect_type};
use super::directive::{Detected, js_trim_start};
use super::directive::{detect_directive, detect_init, is_wrap_name, remove_directives};
use super::theme::khroma::{JS_WHITESPACE, js_is_whitespace, js_number};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocessError {
    /// js-yaml rejected the front matter; the message is the YAML parser's.
    FrontMatter(String),
    /// `detectInit` ran `detectType` to place a `config` key and no detector
    /// matched.
    Detect(DetectError),
}

impl fmt::Display for PreprocessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PreprocessError::FrontMatter(m) => f.write_str(m),
            PreprocessError::Detect(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for PreprocessError {}

impl From<DetectError> for PreprocessError {
    fn from(err: DetectError) -> Self {
        PreprocessError::Detect(err)
    }
}

/// `preprocessDiagram`'s result.
#[derive(Debug, Clone, PartialEq)]
pub struct Preprocessed {
    /// `code.cleaned`: what the diagram parser receives.
    pub code: String,
    /// `code.withComments`: before `cleanupComments`.
    pub with_comments: String,
    /// Lines the front matter block occupied, for mapping parser line
    /// numbers back to the source.
    pub frontmatter_line_offset: usize,
    pub title: Option<String>,
    /// Always a JSON object.
    pub config: Value,
}

static HTML_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<([A-Za-z0-9_]+)([^>]*)>").expect("valid regex"));
static DOUBLE_QUOTED_ATTR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"="([^"]*)""#).expect("valid regex"));

/// `cleanupText`: CRLF and lone CR become LF, and double-quoted HTML
/// attributes become single-quoted because the jison lexers choke on the
/// double quotes.
pub fn cleanup_text(code: &str) -> String {
    let normalized = code.replace("\r\n", "\n").replace('\r', "\n");
    HTML_TAG
        .replace_all(&normalized, |caps: &regex::Captures| {
            let attrs = DOUBLE_QUOTED_ATTR.replace_all(&caps[2], "='$1'");
            format!("<{}{}>", &caps[1], attrs)
        })
        .into_owned()
}

/// JS `.`: anything but a line terminator.
const JS_DOT: &str = r"[^\n\r\u{2028}\u{2029}]";

/// The `style` and `classDef` rules of `encodeEntities`: a line part that
/// sets a color and ends in `;`.
static STYLE_COLOR: LazyLock<[Regex; 2]> = LazyLock::new(|| {
    let not_ws = format!("[^{}]", &JS_WHITESPACE[1..JS_WHITESPACE.len() - 1]);
    ["style", "classDef"].map(|keyword| {
        Regex::new(&format!("{keyword}{JS_DOT}*:{not_ws}*#{JS_DOT}*;")).expect("valid regex")
    })
});
/// `/#\w+;/g` with JS's ASCII `\w`.
static ENTITY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#[A-Za-z0-9_]+;").expect("valid regex"));

/// `encodeEntities` (`utils.ts`), which `Diagram.fromText` runs on the text
/// before the parser sees it: the last `;` of a `style` or `classDef` color
/// goes, and an entity `#name;` becomes `ﬂ°name¶ß` (`ﬂ°°name¶ß` when the
/// name is a number) so the lexers do not read `#` and `;` as syntax.
pub fn encode_entities(text: &str) -> String {
    let mut txt = text.to_string();
    for re in STYLE_COLOR.iter() {
        txt = re
            .replace_all(&txt, |caps: &regex::Captures| {
                let s = &caps[0];
                s[..s.len() - 1].to_string()
            })
            .into_owned();
    }
    ENTITY
        .replace_all(&txt, |caps: &regex::Captures| {
            let inner = &caps[0][1..caps[0].len() - 1];
            // `/^\+?\d+$/`; `\w` never matches the `+`.
            if inner.bytes().all(|b| b.is_ascii_digit()) {
                format!("\u{fb02}\u{b0}\u{b0}{inner}\u{b6}\u{df}")
            } else {
                format!("\u{fb02}\u{b0}{inner}\u{b6}\u{df}")
            }
        })
        .into_owned()
}

fn is_line_break(c: Option<char>) -> bool {
    matches!(c, Some('\n') | Some('\r'))
}

/// The result of `matchFrontMatter`: the opening fence's indent, the YAML
/// body, and the byte length of the whole block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontMatterMatch<'a> {
    pub indent: &'a str,
    pub body: &'a str,
    pub length: usize,
}

/// Index of the last line break in the whitespace run starting at `from`.
fn last_line_break_in_whitespace_run(chars: &[char], from: usize) -> Option<usize> {
    let mut index = None;
    let mut i = from;
    while i < chars.len() && js_is_whitespace(chars[i]) {
        if is_line_break(Some(chars[i])) {
            index = Some(i);
        }
        i += 1;
    }
    index
}

/// `matchFrontMatter`: the scanner that reproduces
/// `/^([^\S\n\r]*)-{3}\s*[\n\r](.*?)[\n\r]\1-{3}\s*[\n\r]+/s` with its
/// backtracking result (the longest opening that still leaves a closing
/// fence, then the first closing fence after it).
pub fn match_front_matter(text: &str) -> Option<FrontMatterMatch<'_>> {
    let chars: Vec<char> = text.chars().collect();
    let mut cursor = 0;
    while cursor < chars.len()
        && js_is_whitespace(chars[cursor])
        && !is_line_break(Some(chars[cursor]))
    {
        cursor += 1;
    }
    let indent: Vec<char> = chars[..cursor].to_vec();
    if !starts_with_at(&chars, cursor, &['-', '-', '-']) {
        return None;
    }

    let mut opening_ends = Vec::new();
    let mut i = cursor + 3;
    while i < chars.len() && js_is_whitespace(chars[i]) {
        if is_line_break(Some(chars[i])) {
            opening_ends.push(i);
        }
        i += 1;
    }
    if opening_ends.is_empty() {
        return None;
    }

    let mut closing_fence = indent.clone();
    closing_fence.extend(['-', '-', '-']);
    let closing_end = |i: usize| -> Option<usize> {
        if is_line_break(chars.get(i).copied()) && starts_with_at(&chars, i + 1, &closing_fence) {
            last_line_break_in_whitespace_run(&chars, i + 1 + closing_fence.len())
        } else {
            None
        }
    };

    let earliest_body_start = opening_ends[0] + 1;
    let mut last_closing_fence = None;
    for i in earliest_body_start..chars.len() {
        if closing_end(i).is_some() {
            last_closing_fence = Some(i);
        }
    }
    let last_closing_fence = last_closing_fence?;

    let mut opening_end = opening_ends[0];
    for &candidate in &opening_ends {
        if candidate < last_closing_fence {
            opening_end = candidate;
        }
    }

    let body_start = opening_end + 1;
    for i in body_start..=last_closing_fence {
        if let Some(match_end) = closing_end(i) {
            let byte = |ci: usize| chars[..ci].iter().map(|c| c.len_utf8()).sum::<usize>();
            return Some(FrontMatterMatch {
                indent: &text[..byte(cursor)],
                body: &text[byte(body_start)..byte(i)],
                length: byte(match_end + 1),
            });
        }
    }
    None
}

fn starts_with_at(chars: &[char], at: usize, needle: &[char]) -> bool {
    chars.len() >= at + needle.len() && &chars[at..at + needle.len()] == needle
}

/// `extractFrontMatter`'s metadata.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FrontMatterMetadata {
    pub title: Option<String>,
    pub display_mode: Option<String>,
    pub config: Option<Value>,
}

/// JS `String(value)` for the YAML scalars `title` and `displayMode` take.
fn js_to_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => js_number(n.as_f64().unwrap_or(f64::NAN)),
        Value::String(s) => s.clone(),
        Value::Array(items) => items.iter().map(js_to_string).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

fn yaml_to_json(y: serde_yaml_ng::Value) -> Value {
    match y {
        serde_yaml_ng::Value::Null => Value::Null,
        serde_yaml_ng::Value::Bool(b) => Value::Bool(b),
        serde_yaml_ng::Value::Number(n) => n
            .as_i64()
            .map(Value::from)
            .or_else(|| n.as_u64().map(Value::from))
            .or_else(|| n.as_f64().map(Value::from))
            .unwrap_or(Value::Null),
        serde_yaml_ng::Value::String(s) => Value::String(s),
        serde_yaml_ng::Value::Sequence(items) => {
            Value::Array(items.into_iter().map(yaml_to_json).collect())
        }
        serde_yaml_ng::Value::Mapping(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                // js-yaml stringifies non-string keys; a mapping key is a
                // scalar in every config mermaid documents.
                let key = match k {
                    serde_yaml_ng::Value::String(s) => s,
                    other => js_to_string(&yaml_to_json(other)),
                };
                out.insert(key, yaml_to_json(v));
            }
            Value::Object(out)
        }
        serde_yaml_ng::Value::Tagged(t) => yaml_to_json(t.value),
    }
}

/// `extractFrontMatter`: the text after the block and the `title`,
/// `displayMode`, and `config` keys of its YAML.
pub fn extract_front_matter(text: &str) -> Result<(String, FrontMatterMetadata), PreprocessError> {
    let Some(m) = match_front_matter(text) else {
        return Ok((text.to_string(), FrontMatterMetadata::default()));
    };
    // Dedent by the fence indent, because js-yaml rejects tab-indented
    // documents.
    let yaml_body = if m.indent.is_empty() {
        m.body.to_string()
    } else {
        m.body
            .split('\n')
            .map(|line| line.strip_prefix(m.indent).unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let parsed = if yaml_body.trim().is_empty() {
        Value::Null
    } else {
        serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&yaml_body)
            .map(yaml_to_json)
            .map_err(|e| PreprocessError::FrontMatter(e.to_string()))?
    };
    let parsed = match parsed {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    let mut metadata = FrontMatterMetadata::default();
    if let Some(v) = parsed
        .get("displayMode")
        .filter(|v| super::theme::vars::js_truthy(v))
    {
        metadata.display_mode = Some(js_to_string(v));
    }
    if let Some(v) = parsed
        .get("title")
        .filter(|v| super::theme::vars::js_truthy(v))
    {
        metadata.title = Some(js_to_string(v));
    }
    if let Some(v) = parsed
        .get("config")
        .filter(|v| super::theme::vars::js_truthy(v))
    {
        metadata.config = Some(v.clone());
    }
    Ok((text[m.length..].to_string(), metadata))
}

/// `cleanupComments`: `text.replace(/^\s*%%(?!{)[^\n]+\n?/gm, '').trimStart()`.
/// A `%%` line keeps its directive form `%%{`, and an empty comment (`%%`
/// alone) stays because `[^\n]+` needs one character.
pub fn cleanup_comments(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    let mut p = 0;
    while p <= text.len() {
        // `^` in multiline mode: the start of the text or after `\n`.
        let mut q = p;
        while let Some(c) = text[q..].chars().next() {
            if !js_is_whitespace(c) {
                break;
            }
            q += c.len_utf8();
        }
        let rest = &text[q..];
        if rest.starts_with("%%")
            && !rest.starts_with("%%{")
            && rest.len() > 2
            && !rest[2..].starts_with('\n')
        {
            let mut end = q + 2;
            while end < text.len() && bytes[end] != b'\n' {
                end += 1;
            }
            if end < text.len() {
                end += 1;
            }
            out.push_str(&text[last..p]);
            last = end;
            p = end;
            continue;
        }
        match text[p..].find('\n') {
            Some(rel) => p += rel + 1,
            None => break,
        }
    }
    out.push_str(&text[last..]);
    js_trim_start(&out).to_string()
}

/// `stripAnyComments`: `text.replace(/\s*%%.*\n/gm, '\n')` in linear time,
/// as mermaid's scanner does it.
pub fn strip_any_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut consumed = 0;
    let mut search_from = 0;
    while search_from < text.len() {
        let Some(rel) = text[search_from..].find("%%") else {
            break;
        };
        let marker = search_from + rel;
        let Some(nl_rel) = text[marker..].find('\n') else {
            break;
        };
        let line_end = marker + nl_rel;
        let mut start = marker;
        while start > consumed {
            let prev = text[..start].chars().next_back();
            match prev {
                Some(c) if js_is_whitespace(c) => start -= c.len_utf8(),
                _ => break,
            }
        }
        out.push_str(&text[consumed..start]);
        out.push('\n');
        consumed = line_end + 1;
        search_from = consumed;
    }
    out.push_str(&text[consumed..]);
    out
}

/// lodash `merge(dst, src)`: recursive for plain objects, index-wise for
/// arrays, and a source value replaces anything of another shape.
fn lodash_merge_into(dst: &mut Value, src: &Value) {
    match (dst, src) {
        (Value::Object(d), Value::Object(s)) => {
            for (k, sv) in s {
                match d.get_mut(k) {
                    Some(dv) if mergeable(dv, sv) => lodash_merge_into(dv, sv),
                    _ => {
                        d.insert(k.clone(), sv.clone());
                    }
                }
            }
        }
        (Value::Array(d), Value::Array(s)) => {
            for (i, sv) in s.iter().enumerate() {
                match d.get_mut(i) {
                    Some(dv) if mergeable(dv, sv) => lodash_merge_into(dv, sv),
                    Some(dv) => *dv = sv.clone(),
                    None => d.push(sv.clone()),
                }
            }
        }
        (dst, src) => *dst = src.clone(),
    }
}

fn mergeable(a: &Value, b: &Value) -> bool {
    matches!(
        (a, b),
        (Value::Object(_), Value::Object(_)) | (Value::Array(_), Value::Array(_))
    )
}

/// `cleanAndMerge(defaultData, data)`: `merge({}, defaultData, data)`.
pub fn clean_and_merge(default_data: &Value, data: &Value) -> Value {
    let mut out = Value::Object(Map::new());
    lodash_merge_into(&mut out, default_data);
    lodash_merge_into(&mut out, data);
    out
}

/// `processFrontmatter`.
fn process_front_matter(code: &str) -> Result<(String, Option<String>, Value), PreprocessError> {
    let (text, metadata) = extract_front_matter(code)?;
    // `const { displayMode, title, config = {} } = metadata`; a config that
    // is not an object would make `config.gantt = {}` throw in mermaid.
    let mut config = match metadata.config {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    };
    if let Some(mode) = metadata.display_mode {
        let gantt = config
            .entry("gantt")
            .or_insert_with(|| Value::Object(Map::new()));
        if !gantt.is_object() {
            *gantt = Value::Object(Map::new());
        }
        if let Value::Object(g) = gantt {
            g.insert("displayMode".to_string(), Value::String(mode));
        }
    }
    Ok((text, metadata.title, Value::Object(config)))
}

/// `processDirectives`.
fn process_directives(code: &str) -> Result<(String, Value), PreprocessError> {
    let mut init = detect_init(code, detect_type)?.unwrap_or_default();
    let wrap = match detect_directive(code, Some(&is_wrap_name)) {
        Detected::Many(ds) => Some(ds.iter().any(|d| d.kind == "wrap")),
        Detected::One(d) if d.kind == "wrap" => Some(true),
        _ => None,
    };
    if let Some(w) = wrap {
        init.insert("wrap".to_string(), Value::Bool(w));
    }
    Ok((remove_directives(code), Value::Object(init)))
}

/// `preprocessDiagram(code)`.
pub fn preprocess(code: &str) -> Result<Preprocessed, PreprocessError> {
    let normalized = cleanup_text(code);
    let (text, title, front_config) = process_front_matter(&normalized)?;
    let (with_comments, directive) = process_directives(&text)?;
    let config = clean_and_merge(&front_config, &directive);
    let cleaned = cleanup_comments(&with_comments);
    let frontmatter_line_offset = if normalized.len() > text.len() {
        normalized[..normalized.len() - text.len()]
            .matches('\n')
            .count()
    } else {
        0
    };
    Ok(Preprocessed {
        code: cleaned,
        with_comments,
        frontmatter_line_offset,
        title,
        config,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Expected values from `encodeEntities` in mermaid 12.2.0's bundle.
    #[test]
    fn encode_entities_matches_mermaid() {
        assert_eq!(
            encode_entities("A[#35; x]"),
            "A[\u{fb02}\u{b0}\u{b0}35\u{b6}\u{df} x]"
        );
        assert_eq!(
            encode_entities("A[#quot;]"),
            "A[\u{fb02}\u{b0}quot\u{b6}\u{df}]"
        );
        assert_eq!(encode_entities("A[#;]"), "A[#;]");
        assert_eq!(
            encode_entities("style A fill:#f9f;\nclassDef c fill:#fff;\nB;"),
            "style A fill:#f9f\nclassDef c fill:#fff\nB;"
        );
        // Greedy to the last `;` of the line; the entity rule then takes
        // the color that still ends in `;`.
        assert_eq!(
            encode_entities("style A fill:#f9f;stroke:#333;"),
            "style A fill:\u{fb02}\u{b0}f9f\u{b6}\u{df}stroke:#333"
        );
        assert_eq!(
            encode_entities("graph TD\n  A-->B\n"),
            "graph TD\n  A-->B\n"
        );
    }

    #[test]
    fn cleanup_text_normalizes_line_endings_and_attribute_quotes() {
        assert_eq!(cleanup_text("a\r\nb\rc"), "a\nb\nc");
        assert_eq!(
            cleanup_text("A[<span style=\"color: red\" class=\"x\">hi</span>]"),
            "A[<span style='color: red' class='x'>hi</span>]"
        );
        assert_eq!(cleanup_text("A --> B"), "A --> B");
    }

    #[test]
    fn front_matter_match_and_extract() {
        let text = "---\ntitle: Hello\nconfig:\n  theme: dark\n---\ngraph TD\nA-->B\n";
        let m = match_front_matter(text).unwrap();
        assert_eq!(m.indent, "");
        assert_eq!(m.body, "title: Hello\nconfig:\n  theme: dark");
        let (rest, meta) = extract_front_matter(text).unwrap();
        assert_eq!(rest, "graph TD\nA-->B\n");
        assert_eq!(meta.title.as_deref(), Some("Hello"));
        assert_eq!(meta.config, Some(json!({"theme": "dark"})));
    }

    #[test]
    fn front_matter_needs_a_closing_fence_and_no_indent_mismatch() {
        assert!(match_front_matter("---\ntitle: x\n\ngraph TD").is_none());
        assert!(match_front_matter("graph TD\n---\n").is_none());
        // The closing fence takes the opening fence's indent.
        let m = match_front_matter("  ---\n  title: x\n  ---\ngraph").unwrap();
        assert_eq!(m.indent, "  ");
        assert_eq!(m.body, "  title: x");
        assert!(match_front_matter("  ---\n  title: x\n---\ngraph").is_none());
    }

    #[test]
    fn front_matter_with_blank_lines_is_stripped_whole() {
        // From mermaid's regexes.spec.ts: the opening swallows both leading
        // line breaks, so only the third `---` closes.
        let text = "---\n\n---\n\nMORE\n---\ngraph TD";
        let m = match_front_matter(text).unwrap();
        assert_eq!(&text[m.length..], "graph TD");
    }

    #[test]
    fn front_matter_title_and_display_mode_are_stringified() {
        let text = "---\ntitle: 42\ndisplayMode: compact\nconfig: 3\n---\ngantt\n";
        let (_, meta) = extract_front_matter(text).unwrap();
        assert_eq!(meta.title.as_deref(), Some("42"));
        assert_eq!(meta.display_mode.as_deref(), Some("compact"));
        let p = preprocess(text).unwrap();
        assert_eq!(p.config, json!({"gantt": {"displayMode": "compact"}}));
        assert_eq!(p.code, "gantt\n");
        assert_eq!(p.frontmatter_line_offset, 5);
    }

    #[test]
    fn invalid_yaml_is_an_error() {
        let err = preprocess("---\ntitle: [\n---\ngraph TD").unwrap_err();
        assert!(matches!(err, PreprocessError::FrontMatter(_)));
    }

    #[test]
    fn cleanup_comments_keeps_directives_and_empty_comments() {
        assert_eq!(
            cleanup_comments("%% a\ngraph TD\n  %% b\n  A-->B"),
            "graph TD\n  A-->B"
        );
        assert_eq!(
            cleanup_comments("\n\n%% x\n%%{init}%%\n%%\nA"),
            "%%{init}%%\n%%\nA"
        );
        assert_eq!(cleanup_comments("A %% not a comment"), "A %% not a comment");
    }

    #[test]
    fn strip_any_comments_matches_the_regex() {
        assert_eq!(strip_any_comments("a\n  %% c\nb"), "a\nb");
        assert_eq!(strip_any_comments("%% c"), "%% c");
        assert_eq!(strip_any_comments("x %% c\ny"), "x\ny");
    }

    #[test]
    fn directives_win_over_front_matter() {
        let text = "---\nconfig:\n  theme: dark\n  flowchart:\n    rankSpacing: 123\n---\n%%{init: {\"theme\": \"neutral\", \"flowchart\": {\"nodeSpacing\": 45}}}%%\ngraph TD\n  A-->B\n";
        let p = preprocess(text).unwrap();
        assert_eq!(
            p.config,
            json!({"theme": "neutral", "flowchart": {"rankSpacing": 123, "nodeSpacing": 45}})
        );
        assert_eq!(p.code, "graph TD\n  A-->B\n");
        assert_eq!(p.frontmatter_line_offset, 6);
    }

    #[test]
    fn wrap_directive_sets_the_flag() {
        let p = preprocess("%%{wrap}%%\nsequenceDiagram\nA->>B: hi").unwrap();
        assert_eq!(p.config, json!({"wrap": true}));
    }

    #[test]
    fn init_directive_with_unknown_type_is_a_detect_error() {
        let err = preprocess("%%{init: {\"theme\": \"dark\"}}%%\nnothing here").unwrap_err();
        assert!(matches!(err, PreprocessError::Detect(_)));
        // Without a directive, preprocess does not detect at all.
        assert!(preprocess("nothing here").is_ok());
    }

    #[test]
    fn lodash_merge_semantics() {
        let a = json!({"a": {"x": 1, "arr": [1, 2, 3]}, "s": "keep"});
        let b = json!({"a": {"y": 2, "arr": [9]}, "n": null});
        assert_eq!(
            clean_and_merge(&a, &b),
            json!({"a": {"x": 1, "y": 2, "arr": [9, 2, 3]}, "s": "keep", "n": null})
        );
        assert_eq!(
            clean_and_merge(&json!("str"), &json!({"a": 1})),
            json!({"a": 1})
        );
    }
}
