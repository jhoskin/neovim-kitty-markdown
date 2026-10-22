//! `%%{init: ...}%%` parsing with mermaid's quote fix-up.
//!
//! Port of `directiveRegex` (`diagram-api/regexes.ts`) and of `detectInit`,
//! `detectDirective`, and `removeDirectives` (`utils.ts`) at mermaid 12.2.0.
//! The regex needs a negative lookahead, which the `regex` crate does not
//! have, so [`find_directives`] is a hand-written matcher with the same
//! result on every input: the same match spans and the same capture groups.
//!
//! ```text
//! %{2}{\s*(?:(\w+)\s*:|(\w+))\s*(?:(\w+)|((?:(?!}%{2}).|\r?\n)*))?\s*(?:}%{2})?
//! ```

use serde_json::{Map, Value};

use super::config::merge::{assign_with_depth, sanitize_directive};
use super::theme::khroma::js_is_whitespace;

/// One match of `directiveRegex`. `kind` is capture group 1 or 2 (the
/// directive name), `word` group 3 (a bare word argument), `body` group 4
/// (everything up to `}%%`). Byte offsets index the text that was matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectiveMatch<'a> {
    pub start: usize,
    pub end: usize,
    pub kind: &'a str,
    pub word: Option<&'a str>,
    pub body: Option<&'a str>,
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn skip_ws(text: &str, mut i: usize) -> usize {
    while let Some(c) = text[i..].chars().next() {
        if !js_is_whitespace(c) {
            break;
        }
        i += c.len_utf8();
    }
    i
}

fn take_word(text: &str, mut i: usize) -> usize {
    while let Some(c) = text[i..].chars().next() {
        if !is_word_char(c) {
            break;
        }
        i += c.len_utf8();
    }
    i
}

/// Tries the regex at `start`, where the text begins with `%%{`.
fn match_at(text: &str, start: usize) -> Option<DirectiveMatch<'_>> {
    let mut i = skip_ws(text, start + 3);
    let word_end = take_word(text, i);
    if word_end == i {
        return None;
    }
    let kind = &text[i..word_end];
    // `(\w+)\s*:` first; a shorter word cannot make the colon appear, so
    // the fallback `(\w+)` always takes the whole word.
    let after_colon = skip_ws(text, word_end);
    i = if text[after_colon..].starts_with(':') {
        after_colon + 1
    } else {
        word_end
    };
    i = skip_ws(text, i);
    let mut word = None;
    let mut body = None;
    let arg_word_end = take_word(text, i);
    if arg_word_end > i {
        word = Some(&text[i..arg_word_end]);
        i = arg_word_end;
    } else {
        // `((?:(?!}%{2}).|\r?\n)*)`: up to the first `}%%`; `.` stops at a
        // line terminator unless it is `\n` or `\r\n`.
        let body_start = i;
        loop {
            let rest = &text[i..];
            if rest.is_empty() || rest.starts_with("}%%") {
                break;
            }
            let c = rest.chars().next().unwrap_or('\0');
            match c {
                '\n' => i += 1,
                '\r' => {
                    if rest.starts_with("\r\n") {
                        i += 2;
                    } else {
                        break;
                    }
                }
                '\u{2028}' | '\u{2029}' => break,
                _ => i += c.len_utf8(),
            }
        }
        body = Some(&text[body_start..i]);
    }
    i = skip_ws(text, i);
    if text[i..].starts_with("}%%") {
        i += 3;
    }
    Some(DirectiveMatch {
        start,
        end: i,
        kind,
        word,
        body,
    })
}

/// Every match of `directiveRegex` with the `g` flag, in text order.
pub fn find_directives(text: &str) -> Vec<DirectiveMatch<'_>> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(rel) = text[from..].find("%%{") {
        let start = from + rel;
        match match_at(text, start) {
            Some(m) => {
                from = m.end;
                out.push(m);
            }
            None => from = start + 1,
        }
    }
    out
}

/// `removeDirectives`: `text.replace(directiveRegex, '')`.
pub fn remove_directives(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in find_directives(text) {
        out.push_str(&text[last..m.start]);
        last = m.end;
    }
    out.push_str(&text[last..]);
    out
}

/// A directive as `detectDirective` returns it: `args` is a string for a
/// bare word, a JSON value for a `{...}` body, or `Null`.
#[derive(Debug, Clone, PartialEq)]
pub struct Directive {
    pub kind: String,
    pub args: Value,
}

/// The shapes `detectDirective` returns.
#[derive(Debug, Clone, PartialEq)]
pub enum Detected {
    /// No directive matched the filter: `{ type: text, args: null }`.
    NoMatch,
    One(Directive),
    Many(Vec<Directive>),
    /// A body was not JSON: mermaid logs the error and returns
    /// `{ type: undefined, args: null }`, dropping every directive.
    Failed,
}

/// JS `String.prototype.trim`.
pub fn js_trim(text: &str) -> &str {
    text.trim_matches(js_is_whitespace)
}

/// JS `String.prototype.trimStart`.
pub fn js_trim_start(text: &str) -> &str {
    text.trim_start_matches(js_is_whitespace)
}

/// `commentWithoutDirectives` in `detectDirective`: the pattern
/// `%%(?!{...})(?=}%%).*\n` removes a `%%}%%` through its newline. Nothing
/// else can satisfy the lookaheads.
fn remove_comment_without_directives(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    let mut from = 0;
    while let Some(rel) = text[from..].find("%%}%%") {
        let start = from + rel;
        // `.*\n`: the rest of the line must end in `\n` with no other line
        // terminator before it.
        let mut i = start + 2;
        let mut end = None;
        for c in text[i..].chars() {
            match c {
                '\n' => {
                    end = Some(i + 1);
                    break;
                }
                '\r' | '\u{2028}' | '\u{2029}' => break,
                _ => i += c.len_utf8(),
            }
        }
        match end {
            Some(e) => {
                out.push_str(&text[last..start]);
                last = e;
                from = e;
            }
            None => from = start + 1,
        }
    }
    out.push_str(&text[last..]);
    out
}

/// `detectDirective(text, type)`. `filter` plays the part of the `type`
/// argument: `None` accepts every directive, `Some(f)` keeps the ones whose
/// name satisfies `f`.
pub fn detect_directive(text: &str, filter: Option<&dyn Fn(&str) -> bool>) -> Detected {
    let fixed = remove_comment_without_directives(js_trim(text)).replace('\'', "\"");
    let mut result = Vec::new();
    for m in find_directives(&fixed) {
        if let Some(f) = filter
            && !f(m.kind)
        {
            continue;
        }
        let args = if let Some(w) = m.word {
            Value::String(js_trim(w).to_string())
        } else if let Some(b) = m.body.filter(|b| !b.is_empty()) {
            match serde_json::from_str::<Value>(js_trim(b)) {
                Ok(v) => v,
                Err(_) => return Detected::Failed,
            }
        } else {
            Value::Null
        };
        result.push(Directive {
            kind: m.kind.to_string(),
            args,
        });
    }
    match result.len() {
        0 => Detected::NoMatch,
        1 => Detected::One(result.remove(0)),
        _ => Detected::Many(result),
    }
}

/// `/(?:init\b)|(?:initialize\b)/` tested against a `\w+` name: the word
/// boundary can only be the end of the name.
pub fn is_init_name(kind: &str) -> bool {
    kind.ends_with("init") || kind.ends_with("initialize")
}

/// `'wrap'` as a `String.match` argument: a substring search.
pub fn is_wrap_name(kind: &str) -> bool {
    kind.contains("wrap")
}

/// `detectInit(text)`: the merged init directives, with a `config` key moved
/// under the detected diagram type. `detect_type` is `detectType` on the
/// same text; its error (no diagram type) propagates, as in mermaid.
pub fn detect_init<E>(
    text: &str,
    detect_type: impl FnOnce(&str) -> Result<&'static str, E>,
) -> Result<Option<Map<String, Value>>, E> {
    let mut results = match detect_directive(text, Some(&is_init_name)) {
        Detected::Many(inits) => {
            let mut args: Vec<Value> = inits.into_iter().map(|d| d.args).collect();
            for a in &mut args {
                sanitize_directive(a);
            }
            let mut acc = Value::Object(Map::new());
            for a in &args {
                acc = assign_with_depth(acc, a, 2);
            }
            acc
        }
        Detected::One(d) => d.args,
        Detected::NoMatch | Detected::Failed => Value::Null,
    };
    if !super::theme::vars::js_truthy(&results) {
        return Ok(None);
    }
    let mut ty = detect_type(text)?;
    if let Value::Object(map) = &mut results
        && let Some(config) = map.remove("config")
    {
        if ty == "flowchart-v2" {
            ty = "flowchart";
        }
        map.insert(ty.to_string(), config);
    }
    // A bare-word argument is a string: `assignWithDepth` and lodash `merge`
    // both ignore it downstream, so it carries no config.
    Ok(match results {
        Value::Object(map) => Some(map),
        _ => Some(Map::new()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_type(_: &str) -> Result<&'static str, ()> {
        Ok("flowchart-v2")
    }

    #[test]
    fn finds_init_with_body() {
        let text = "%%{init: {\"theme\": \"dark\"}}%%\ngraph TD\nA-->B";
        let ms = find_directives(text);
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].kind, "init");
        assert_eq!(ms[0].body, Some("{\"theme\": \"dark\"}"));
        assert_eq!(
            &text[ms[0].start..ms[0].end],
            "%%{init: {\"theme\": \"dark\"}}%%"
        );
        assert_eq!(remove_directives(text), "\ngraph TD\nA-->B");
    }

    #[test]
    fn bare_word_argument_and_no_argument() {
        let ms = find_directives("%%{wrap}%%");
        assert_eq!(ms[0].kind, "wrap");
        assert_eq!(ms[0].word, None);
        assert_eq!(ms[0].body, Some(""));
        let ms = find_directives("%%{init: dark}%%");
        assert_eq!(ms[0].word, Some("dark"));
    }

    #[test]
    fn multiline_body_and_missing_close() {
        let text = "%%{init: {\n  \"theme\": \"forest\"\n}}%%\nx";
        let ms = find_directives(text);
        assert_eq!(ms[0].body, Some("{\n  \"theme\": \"forest\"\n}"));
        assert_eq!(remove_directives(text), "\nx");
        // Without `}%%` the greedy body runs to the end of the text.
        assert_eq!(remove_directives("%%{init: {\"a\":1}\ngraph TD"), "");
    }

    #[test]
    fn detect_directive_fixes_quotes_and_filters() {
        let text = "%%{init: {'theme': 'dark'}}%%\n%%{wrap}%%\ngraph TD";
        match detect_directive(text, Some(&is_init_name)) {
            Detected::One(d) => {
                assert_eq!(d.kind, "init");
                assert_eq!(d.args["theme"], "dark");
            }
            other => panic!("{other:?}"),
        }
        match detect_directive(text, None) {
            Detected::Many(ds) => assert_eq!(ds.len(), 2),
            other => panic!("{other:?}"),
        }
        assert_eq!(
            detect_directive("graph TD", Some(&is_wrap_name)),
            Detected::NoMatch
        );
    }

    #[test]
    fn malformed_json_drops_every_directive() {
        let text = "%%{init: {\"theme\": \"dark\",}}%%\ngraph TD";
        assert_eq!(detect_directive(text, None), Detected::Failed);
        assert_eq!(detect_init(text, ok_type), Ok(None));
    }

    #[test]
    fn detect_init_merges_and_moves_config() {
        let text = "%%{init: {'config': {'nodeSpacing': 77}}}%%\nflowchart LR";
        let init = detect_init(text, ok_type).unwrap().unwrap();
        assert_eq!(init["flowchart"]["nodeSpacing"], 77);
        assert!(init.get("config").is_none());

        let text = "%%{init: {'theme': 'dark'}}%%\n%%{init: {'flowchart': {'padding': 3}}}%%\n";
        let init = detect_init(text, ok_type).unwrap().unwrap();
        assert_eq!(init["theme"], "dark");
        assert_eq!(init["flowchart"]["padding"], 3);
    }

    #[test]
    fn detect_init_without_directive_is_none_and_never_detects() {
        let r: Result<_, ()> = detect_init("graph TD", |_| panic!("not called"));
        assert_eq!(r, Ok(None));
    }

    #[test]
    fn init_name_boundary() {
        assert!(is_init_name("init"));
        assert!(is_init_name("initialize"));
        assert!(is_init_name("reinit"));
        assert!(!is_init_name("initial"));
    }
}
