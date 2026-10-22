//! Translation of JavaScript regex source (as jison-lex compiles it, without
//! the `u` flag) to `fancy_regex` syntax with the same matches.
//!
//! The two dialects agree on most syntax, but JavaScript's shorthand classes
//! are ASCII-only, `.` skips four line terminators, a quantifier may follow a
//! lookahead, and a bare `{` or `[` is literal. Each of those is rewritten
//! here rather than depending on the engine's defaults, so a diagram with
//! non-ASCII text lexes as it does in mermaid.

use std::fmt;

use fancy_regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegexError {
    pub pattern: String,
    pub message: String,
}

impl fmt::Display for RegexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "regex /{}/: {}", self.pattern, self.message)
    }
}

impl std::error::Error for RegexError {}

/// JavaScript `\s` (non-`u` mode): WhiteSpace plus LineTerminator.
const JS_SPACE: &str =
    "\\t\\n\\x0B\\f\\r \\u00A0\\u1680\\u2000-\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000\\uFEFF";
const JS_WORD: &str = "A-Za-z0-9_";
const JS_DIGIT: &str = "0-9";
/// JavaScript `.`: anything but LineTerminator.
const JS_DOT: &str = "[^\\n\\r\\u2028\\u2029]";
const JS_WORD_BOUNDARY: &str =
    "(?:(?<![A-Za-z0-9_])(?=[A-Za-z0-9_])|(?<=[A-Za-z0-9_])(?![A-Za-z0-9_]))";
const JS_NON_WORD_BOUNDARY: &str =
    "(?:(?<=[A-Za-z0-9_])(?=[A-Za-z0-9_])|(?<![A-Za-z0-9_])(?![A-Za-z0-9_]))";

/// Compiles a jison-lex rule pattern anchored at the start of the remaining
/// input, as `new RegExp("^(?:" + pattern + ")", caseless ? "i" : "")`.
pub fn compile_rule(pattern: &str, case_insensitive: bool) -> Result<Regex, RegexError> {
    let body = translate(pattern)?;
    let flags = if case_insensitive { "(?i)" } else { "" };
    compile(pattern, &format!("{flags}^(?:{body})"))
}

/// Compiles an unanchored JavaScript regex literal body, for the `replace`
/// calls in lexer actions.
pub fn compile_free(pattern: &str, case_insensitive: bool) -> Result<Regex, RegexError> {
    let body = translate(pattern)?;
    let flags = if case_insensitive { "(?i)" } else { "" };
    compile(pattern, &format!("{flags}{body}"))
}

fn compile(pattern: &str, translated: &str) -> Result<Regex, RegexError> {
    Regex::new(translated).map_err(|e| RegexError {
        pattern: pattern.to_string(),
        message: e.to_string(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    Plain,
    Lookaround,
}

/// Rewrites JavaScript regex source into `fancy_regex` source.
pub fn translate(pattern: &str) -> Result<String, RegexError> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len() + 16);
    // Each open group remembers its kind and where its text starts, so a
    // quantified lookahead can be rewritten once its `)` is seen.
    let mut groups: Vec<(Group, usize)> = Vec::new();
    let mut in_class = false;
    let mut i = 0;
    let err = |message: &str| RegexError {
        pattern: pattern.to_string(),
        message: message.to_string(),
    };
    while i < chars.len() {
        let c = chars[i];
        if in_class {
            match c {
                '\\' => {
                    let Some(&e) = chars.get(i + 1) else {
                        return Err(err("trailing backslash"));
                    };
                    i += 2;
                    match e {
                        'd' => out.push_str(JS_DIGIT),
                        'w' => out.push_str(JS_WORD),
                        's' => out.push_str(JS_SPACE),
                        'D' => out.push_str(&format!("[^{JS_DIGIT}]")),
                        'W' => out.push_str(&format!("[^{JS_WORD}]")),
                        'S' => out.push_str(&format!("[^{JS_SPACE}]")),
                        // Inside a class, JavaScript `\b` is a backspace.
                        'b' => out.push_str("\\x08"),
                        _ => {
                            out.push('\\');
                            out.push(e);
                        }
                    }
                }
                ']' => {
                    in_class = false;
                    out.push(']');
                    i += 1;
                }
                // Rust reads a bare `[` as a nested class and `&&`, `~~`,
                // `--` as set operations; JavaScript takes them literally.
                '[' | '&' | '~' => {
                    out.push('\\');
                    out.push(c);
                    i += 1;
                }
                '-' if chars.get(i + 1) == Some(&'-') => {
                    out.push_str("\\-");
                    i += 1;
                }
                _ => {
                    out.push(c);
                    i += 1;
                }
            }
            continue;
        }
        match c {
            '\\' => {
                let Some(&e) = chars.get(i + 1) else {
                    return Err(err("trailing backslash"));
                };
                i += 2;
                match e {
                    'd' => out.push_str(&format!("[{JS_DIGIT}]")),
                    'D' => out.push_str(&format!("[^{JS_DIGIT}]")),
                    'w' => out.push_str(&format!("[{JS_WORD}]")),
                    'W' => out.push_str(&format!("[^{JS_WORD}]")),
                    's' => out.push_str(&format!("[{JS_SPACE}]")),
                    'S' => out.push_str(&format!("[^{JS_SPACE}]")),
                    // ASCII word boundaries, as in JavaScript without `u`.
                    // fancy_regex cannot switch Unicode mode inline, so
                    // the boundary is spelled out with lookaround.
                    'b' => out.push_str(JS_WORD_BOUNDARY),
                    'B' => out.push_str(JS_NON_WORD_BOUNDARY),
                    _ => {
                        out.push('\\');
                        out.push(e);
                    }
                }
            }
            '[' => {
                // JavaScript closes a class at the first `]`, so `[^]` is
                // any character and `[]` is an empty class.
                if chars.get(i + 1) == Some(&'^') && chars.get(i + 2) == Some(&']') {
                    out.push_str("(?s:.)");
                    i += 3;
                    continue;
                }
                if chars.get(i + 1) == Some(&']') {
                    return Err(err("empty character class"));
                }
                in_class = true;
                out.push('[');
                i += 1;
                if chars.get(i) == Some(&'^') {
                    out.push('^');
                    i += 1;
                }
            }
            '(' => {
                let kind = if chars.get(i + 1) == Some(&'?')
                    && matches!(chars.get(i + 2), Some('=') | Some('!'))
                {
                    Group::Lookaround
                } else {
                    Group::Plain
                };
                groups.push((kind, out.len()));
                out.push('(');
                i += 1;
            }
            ')' => {
                let (kind, start) = groups.pop().ok_or_else(|| err("unbalanced `)`"))?;
                out.push(')');
                i += 1;
                if kind == Group::Lookaround {
                    // JavaScript allows `(?!x)*`; it is zero-width, so `*`
                    // and `?` make it a no-op and `+` makes it one check.
                    match chars.get(i) {
                        Some('*') | Some('?') => {
                            out.truncate(start);
                            i += 1;
                        }
                        Some('+') => i += 1,
                        _ => {}
                    }
                }
            }
            '.' => {
                out.push_str(JS_DOT);
                i += 1;
            }
            '{' => {
                // Only `{n}`, `{n,}` and `{n,m}` are quantifiers in JavaScript;
                // any other `{` or `}` is a literal, which Rust wants escaped.
                if let Some(len) = quantifier_len(&chars[i..]) {
                    out.extend(&chars[i..i + len]);
                    i += len;
                } else {
                    out.push_str("\\{");
                    i += 1;
                }
            }
            '}' => {
                out.push_str("\\}");
                i += 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    if in_class {
        return Err(err("unterminated character class"));
    }
    if !groups.is_empty() {
        return Err(err("unbalanced `(`"));
    }
    Ok(out)
}

/// Length of the `{n}`, `{n,}` or `{n,m}` quantifier at the start of
/// `rest`, or `None` when the brace is a literal.
fn quantifier_len(rest: &[char]) -> Option<usize> {
    let mut j = 1;
    while j < rest.len() && rest[j].is_ascii_digit() {
        j += 1;
    }
    if j == 1 {
        return None;
    }
    if rest.get(j) == Some(&',') {
        j += 1;
        while j < rest.len() && rest[j].is_ascii_digit() {
            j += 1;
        }
    }
    (rest.get(j) == Some(&'}')).then_some(j + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(p: &str) -> String {
        translate(p).expect("translates")
    }

    #[test]
    fn shorthand_classes_are_ascii() {
        assert_eq!(t("\\d+"), "[0-9]+");
        assert_eq!(t("[\\w-]"), "[A-Za-z0-9_-]");
        assert!(t("\\s").starts_with("[\\t\\n"));
    }

    #[test]
    fn dot_skips_line_terminators() {
        assert_eq!(t("a.b"), "a[^\\n\\r\\u2028\\u2029]b");
        assert_eq!(t("a\\.b"), "a\\.b");
    }

    #[test]
    fn quantified_lookahead_is_rewritten() {
        assert_eq!(t("(?!\\n|;|#)*[^\\n]*"), "[^\\n]*");
        assert_eq!(t("\\%\\%(?!\\{)*[^\\n]*"), "\\%\\%[^\\n]*");
        assert_eq!(t("(?!a)+b"), "(?!a)b");
        assert_eq!(t("(?!a)b"), "(?!a)b");
    }

    #[test]
    fn literal_braces_are_escaped() {
        assert_eq!(t("a{"), "a\\{");
        assert_eq!(t("a{2}"), "a{2}");
        assert_eq!(t("a{2,}"), "a{2,}");
        assert_eq!(t("a{2,3}b"), "a{2,3}b");
        assert_eq!(t("[^{}\\n]*"), "[^{}\\n]*");
        assert_eq!(t("x}"), "x\\}");
    }

    #[test]
    fn class_quirks() {
        assert_eq!(t("[[]"), "[\\[]");
        assert_eq!(t("[^]"), "(?s:.)");
        assert!(translate("[]").is_err());
        assert_eq!(t("[\\b]"), "[\\x08]");
        assert_eq!(t("[a&&b]"), "[a\\&\\&b]");
    }

    #[test]
    fn word_boundary_is_ascii() {
        let re = compile_rule("\\bgraph\\b", false).expect("compiles");
        assert!(re.is_match("graph TD").expect("matches"));
        assert!(re.is_match("graphé").expect("matches"));
        assert!(!re.is_match("graphs").expect("matches"));
    }

    #[test]
    fn case_insensitive_flag() {
        let re = compile_rule("gantt", true).expect("compiles");
        assert!(re.is_match("GANTT").expect("matches"));
        let re = compile_rule("gantt", false).expect("compiles");
        assert!(!re.is_match("GANTT").expect("matches"));
    }

    #[test]
    fn unicode_escapes_compile() {
        let re = compile_rule("[\\u00C0-\\uFFFF]+", false).expect("compiles");
        assert!(re.is_match("é").expect("matches"));
        let re = compile_rule("((\\u000D\\u000A)|(\\u000A))", false).expect("compiles");
        assert!(re.is_match("\r\n").expect("matches"));
    }

    #[test]
    fn errors_name_the_pattern() {
        let e = translate("(a").expect_err("unbalanced");
        assert_eq!(e.pattern, "(a");
        let e = compile_rule("a**", false).expect_err("invalid");
        assert_eq!(e.pattern, "a**");
    }
}
