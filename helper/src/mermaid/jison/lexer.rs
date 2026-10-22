//! A jison-lex compatible lexer over the vendored rule tables.
//!
//! This is a port of `RegExpLexer.prototype` in jison-lex 0.3.4: the same
//! fields (`yytext`, `match`, `matched`, `yylineno`, `yylloc`), the same
//! rule order, first match wins unless `%options flex`, and the same
//! `showPosition` text, so parse errors come out byte for byte as the
//! oracle prints them. Lengths and columns count UTF-16 units, as
//! JavaScript strings do.

use super::Grammar;
use super::actions::{Cond, Op, Part, Token, Yy};
use super::error::ParseError;

/// jison's `yylloc`: 1-based lines, 0-based columns in UTF-16 units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Loc {
    pub first_line: usize,
    pub last_line: usize,
    pub first_column: usize,
    pub last_column: usize,
}

/// What `lex()` hands the parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lexed {
    Token(Token),
    /// jison-lex `EOF` (1): the input is exhausted.
    Eof,
}

pub struct Lexer<'g> {
    grammar: &'g Grammar,
    /// jison-lex `_input`: what is left to lex.
    remaining: String,
    done: bool,
    more: bool,
    pub yytext: String,
    /// jison-lex `match`: the text of the current rule match.
    matched_now: String,
    /// jison-lex `matched`: everything consumed so far.
    matched_all: String,
    pub yylineno: usize,
    pub yyleng: usize,
    pub yylloc: Loc,
    /// Indices into `grammar.tables.lexer.conditions`.
    condition_stack: Vec<usize>,
}

/// Length of `s` in UTF-16 code units.
pub fn units(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// Byte index after the first `n` UTF-16 units of `s`, rounded up to a char
/// boundary. JavaScript can split a surrogate pair here; Rust cannot, so a
/// window edge inside an astral character includes the whole character.
fn byte_at_units(s: &str, n: usize) -> usize {
    let mut count = 0;
    for (i, c) in s.char_indices() {
        if count >= n {
            return i;
        }
        count += c.len_utf16();
    }
    s.len()
}

/// `s.substr(0, s.length - n)` in UTF-16 units.
fn drop_last_units(s: &str, n: usize) -> &str {
    let keep = units(s).saturating_sub(n);
    &s[..byte_at_units(s, keep)]
}

/// `s.substr(-n)` in UTF-16 units.
fn last_units(s: &str, n: usize) -> &str {
    let total = units(s);
    if total <= n {
        s
    } else {
        &s[byte_at_units(s, total - n)..]
    }
}

/// `s.substr(0, n)` in UTF-16 units.
fn first_units(s: &str, n: usize) -> &str {
    &s[..byte_at_units(s, n)]
}

/// Line terminator at the start of `s`, as `(?:\r\n?|\n)` would match it.
fn terminator_len(s: &str) -> usize {
    if s.starts_with("\r\n") {
        2
    } else if s.starts_with('\r') || s.starts_with('\n') {
        1
    } else {
        0
    }
}

fn is_js_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// `text.match(/(?:\r\n?|\n).*/g)`: the number of matches and, for the last
/// one, `(match length, terminator length)` in UTF-16 units.
fn lines_in(text: &str) -> (usize, Option<(usize, usize)>) {
    let mut count = 0;
    let mut last = None;
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let term = terminator_len(rest);
        if term == 0 {
            i += rest.chars().next().map_or(1, char::len_utf8);
            continue;
        }
        // `.` stops at any of the four JavaScript line terminators.
        let after = &rest[term..];
        let line_end = after
            .char_indices()
            .find(|(_, c)| is_js_line_terminator(*c))
            .map_or(after.len(), |(idx, _)| idx);
        let whole = &rest[..term + line_end];
        count += 1;
        last = Some((units(whole), term));
        i += whole.len();
    }
    (count, last)
}

/// `text.split(/(?:\r\n?|\n)/g)`.
fn split_lines(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < text.len() {
        let term = terminator_len(&text[i..]);
        if term == 0 {
            i += text[i..].chars().next().map_or(1, char::len_utf8);
            continue;
        }
        parts.push(&text[start..i]);
        i += term;
        start = i;
    }
    parts.push(&text[start..]);
    parts
}

/// JavaScript `String.prototype.trim`: WhiteSpace and LineTerminator.
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// JavaScript `String.prototype.slice` with negative indices, in chars.
fn js_slice(s: &str, start: i64, end: Option<i64>) -> String {
    let len = s.chars().count() as i64;
    let norm = |i: i64| -> usize {
        if i < 0 {
            (len + i).max(0) as usize
        } else {
            i.min(len) as usize
        }
    };
    let from = norm(start);
    let to = norm(end.unwrap_or(len));
    if from >= to {
        return String::new();
    }
    s.chars().skip(from).take(to - from).collect()
}

enum Flow {
    Continue,
    Return(Option<Token>),
}

impl<'g> Lexer<'g> {
    pub fn new(grammar: &'g Grammar, input: &str) -> Lexer<'g> {
        let initial = grammar
            .tables
            .lexer
            .conditions
            .get_index_of("INITIAL")
            .unwrap_or(0);
        Lexer {
            grammar,
            remaining: input.to_string(),
            done: false,
            more: false,
            yytext: String::new(),
            matched_now: String::new(),
            matched_all: String::new(),
            yylineno: 0,
            yyleng: 0,
            yylloc: Loc {
                first_line: 1,
                last_line: 1,
                first_column: 0,
                last_column: 0,
            },
            condition_stack: vec![initial],
        }
    }

    /// jison-lex `match`: the text of the current token.
    pub fn current_match(&self) -> &str {
        &self.matched_now
    }

    /// The name of the active start condition.
    pub fn top_state(&self) -> &str {
        let idx = self.condition_stack.last().copied().unwrap_or(0);
        self.grammar
            .tables
            .lexer
            .conditions
            .get_index(idx)
            .map_or("INITIAL", |(name, _)| name.as_str())
    }

    pub fn begin(&mut self, condition: &str) -> Result<(), ParseError> {
        let Some(idx) = self.grammar.tables.lexer.conditions.get_index_of(condition) else {
            // jison-lex would push the name and fail on the next
            // `_currentRules()` lookup with a TypeError.
            return Err(self.action_error(format!(
                "Cannot read properties of undefined (reading 'rules') in start condition {condition}"
            )));
        };
        self.condition_stack.push(idx);
        Ok(())
    }

    pub fn pop_state(&mut self) {
        if self.condition_stack.len() > 1 {
            self.condition_stack.pop();
        }
    }

    /// `pastInput()`: up to 20 units before the current match, one line.
    pub fn past_input(&self) -> String {
        let past = drop_last_units(&self.matched_all, units(&self.matched_now));
        let prefix = if units(past) > 20 { "..." } else { "" };
        format!("{prefix}{}", last_units(past, 20).replace('\n', ""))
    }

    /// `upcomingInput()`: the current match padded to 20 units from the input.
    pub fn upcoming_input(&self) -> String {
        let mut next = self.matched_now.clone();
        let have = units(&next);
        if have < 20 {
            next.push_str(first_units(&self.remaining, 20 - have));
        }
        let suffix = if units(&next) > 20 { "..." } else { "" };
        format!("{}{suffix}", first_units(&next, 20)).replace('\n', "")
    }

    /// `showPosition()`: the two error lines with the caret.
    pub fn show_position(&self) -> String {
        let pre = self.past_input();
        let dashes = "-".repeat(units(&pre));
        format!("{pre}{}\n{dashes}^", self.upcoming_input())
    }

    fn action_error(&self, message: String) -> ParseError {
        ParseError::action(self.yylineno + 1, self.yylloc.first_column, message)
    }

    /// `unput(ch)`, including jison-lex's one-character trims of `match` and
    /// `matched`, which `showPosition` then reflects.
    fn unput(&mut self, ch: &str) {
        let len = units(ch);
        let lines = split_lines(ch);
        self.remaining.insert_str(0, ch);
        self.yytext = drop_last_units(&self.yytext, len).to_string();
        let old_lines: Vec<String> = split_lines(&self.matched_now)
            .into_iter()
            .map(str::to_string)
            .collect();
        self.matched_now = drop_last_units(&self.matched_now, 1).to_string();
        self.matched_all = drop_last_units(&self.matched_all, 1).to_string();
        if lines.len() > 1 {
            self.yylineno = self.yylineno.saturating_sub(lines.len() - 1);
        }
        let base = if lines.len() == old_lines.len() {
            self.yylloc.first_column
        } else {
            0
        };
        let old_segment = old_lines
            .len()
            .checked_sub(lines.len())
            .and_then(|i| old_lines.get(i))
            .map_or(0, |s| units(s));
        self.yylloc = Loc {
            first_line: self.yylloc.first_line,
            last_line: self.yylineno + 1,
            first_column: self.yylloc.first_column,
            last_column: (base + old_segment).saturating_sub(units(lines[0])),
        };
        self.yyleng = units(&self.yytext);
    }

    fn current_rules(&self) -> &'g [usize] {
        let idx = self.condition_stack.last().copied().unwrap_or(0);
        self.grammar
            .tables
            .lexer
            .conditions
            .get_index(idx)
            .map_or(&[], |(_, c)| c.rules.as_slice())
    }

    /// `test_match`: consume `text` as rule `rule`'s match and run its action.
    fn test_match(
        &mut self,
        text: &str,
        rule: usize,
        yy: &mut dyn Yy,
    ) -> Result<Option<Token>, ParseError> {
        let (line_count, last) = lines_in(text);
        self.yylineno += line_count;
        self.yylloc = Loc {
            first_line: self.yylloc.last_line,
            last_line: self.yylineno + 1,
            first_column: self.yylloc.last_column,
            last_column: match last {
                Some((len, term)) => len - term,
                None => self.yylloc.last_column + units(text),
            },
        };
        self.yytext.push_str(text);
        self.matched_now.push_str(text);
        self.yyleng = units(&self.yytext);
        self.more = false;
        self.remaining.drain(..text.len());
        self.matched_all.push_str(text);
        let program = &self.grammar.rules[rule].program;
        let token = match self.run(program, yy)? {
            Flow::Return(token) => token,
            Flow::Continue => None,
        };
        if self.done && !self.remaining.is_empty() {
            self.done = false;
        }
        Ok(token)
    }

    fn run(&mut self, program: &[Op], yy: &mut dyn Yy) -> Result<Flow, ParseError> {
        for op in program {
            match op {
                Op::Begin(cond) => self.begin(cond)?,
                Op::PopState => self.pop_state(),
                Op::Return(token) => return Ok(Flow::Return(Some(token.clone()))),
                Op::ReturnNothing => return Ok(Flow::Return(None)),
                Op::Trim => self.yytext = js_trim(&self.yytext).to_string(),
                Op::Slice { start, end } => self.yytext = js_slice(&self.yytext, *start, *end),
                Op::Replace(r) => {
                    let replaced = if r.global {
                        r.regex.replace_all(&self.yytext, r.replacement.as_str())
                    } else {
                        r.regex.replace(&self.yytext, r.replacement.as_str())
                    };
                    self.yytext = replaced.into_owned();
                }
                Op::SetText(text) => self.yytext = text.clone(),
                Op::Unput => {
                    let text = self.yytext.clone();
                    self.unput(&text);
                }
                Op::Throw(parts) => {
                    let message: String = parts
                        .iter()
                        .map(|p| match p {
                            Part::Lit(s) => s.clone(),
                            Part::YyText => self.yytext.clone(),
                            Part::YyTextTrim => js_trim(&self.yytext).to_string(),
                        })
                        .collect();
                    return Err(self.action_error(message));
                }
                Op::YySet(name, value) => yy.yy_set(name, value.clone()),
                Op::YyCall(name, args) => {
                    yy.yy_call(name, args);
                }
                Op::If {
                    cond,
                    then,
                    otherwise,
                } => {
                    let branch = if self.eval(cond, yy) { then } else { otherwise };
                    if let Flow::Return(token) = self.run(branch, yy)? {
                        return Ok(Flow::Return(token));
                    }
                }
            }
        }
        Ok(Flow::Continue)
    }

    fn eval(&mut self, cond: &Cond, yy: &mut dyn Yy) -> bool {
        match cond {
            Cond::YyCall(name, args) => yy.yy_call(name, args).truthy(),
            Cond::YyGet(name) => yy.yy_get(name).truthy(),
            Cond::And(a, b) => self.eval(a, yy) && self.eval(b, yy),
            Cond::Not(c) => !self.eval(c, yy),
            Cond::ProcessId => self.process_id(),
        }
    }

    /// stateDiagram's `processId()`: an id followed by an inline `%%`
    /// comment keeps the id and pushes the comment back; a match that is
    /// all comment yields no token.
    fn process_id(&mut self) -> bool {
        match self.yytext.find("%%") {
            Some(0) => false,
            Some(idx) => {
                let after = self.yytext[idx..].to_string();
                let before = self.yytext[..idx].to_string();
                self.unput(&after);
                self.yytext = before;
                true
            }
            None => true,
        }
    }

    /// `next()`: one rule match. `Ok(None)` means the rule consumed input
    /// without a token.
    fn next(&mut self, yy: &mut dyn Yy) -> Result<Option<Lexed>, ParseError> {
        if self.done {
            return Ok(Some(Lexed::Eof));
        }
        if self.remaining.is_empty() {
            self.done = true;
        }
        if !self.more {
            self.yytext.clear();
            self.matched_now.clear();
        }
        let flex = self.grammar.tables.lexer.options.flex;
        let mut best: Option<(usize, usize)> = None;
        for &rule in self.current_rules() {
            let regex = &self.grammar.rules[rule].regex;
            let found = regex
                .find(&self.remaining)
                .map_err(|e| self.action_error(format!("regex error in rule {rule}: {e}")))?;
            if let Some(m) = found {
                if best.is_none_or(|(_, len)| m.end() > len) {
                    best = Some((rule, m.end()));
                    if !flex {
                        break;
                    }
                }
            }
        }
        if let Some((rule, len)) = best {
            let text = self.remaining[..len].to_string();
            return Ok(self.test_match(&text, rule, yy)?.map(Lexed::Token));
        }
        if self.remaining.is_empty() {
            return Ok(Some(Lexed::Eof));
        }
        Err(ParseError::lexical(
            self.yylineno + 1,
            self.yylloc.last_column,
            &self.show_position(),
        ))
    }

    /// `lex()`: the next token, skipping rules that produce none.
    pub fn lex(&mut self, yy: &mut dyn Yy) -> Result<Lexed, ParseError> {
        loop {
            let before = (
                self.remaining.len(),
                self.condition_stack.clone(),
                self.done,
            );
            if let Some(lexed) = self.next(yy)? {
                return Ok(lexed);
            }
            // jison-lex would spin forever on a rule that matches nothing
            // and returns nothing; fail instead.
            if before
                == (
                    self.remaining.len(),
                    self.condition_stack.clone(),
                    self.done,
                )
            {
                return Err(self.action_error(
                    "lexer made no progress: a rule matched the empty string without a token"
                        .to_string(),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_helpers() {
        assert_eq!(units("abc"), 3);
        assert_eq!(units("a😀"), 3);
        assert_eq!(last_units("abcdef", 2), "ef");
        assert_eq!(first_units("abcdef", 2), "ab");
        assert_eq!(drop_last_units("abcdef", 2), "abcd");
        assert_eq!(first_units("a😀b", 2), "a😀");
    }

    #[test]
    fn line_counting_matches_the_js_regex() {
        assert_eq!(lines_in("abc"), (0, None));
        assert_eq!(lines_in("a\nbc"), (1, Some((3, 1))));
        assert_eq!(lines_in("a\r\nbc\n"), (2, Some((1, 1))));
        assert_eq!(lines_in("\n\n"), (2, Some((1, 1))));
        assert_eq!(lines_in("a\rb"), (1, Some((2, 1))));
        assert_eq!(split_lines("a\nb\r\nc"), vec!["a", "b", "c"]);
        assert_eq!(split_lines("abc"), vec!["abc"]);
        assert_eq!(split_lines("a\n"), vec!["a", ""]);
    }

    #[test]
    fn js_slice_and_trim() {
        assert_eq!(js_slice("state <<fork>>", 0, Some(-8)), "state ");
        assert_eq!(js_slice("abcdef", 2, None), "cdef");
        assert_eq!(js_slice("[[x]]", 2, Some(-2)), "x");
        assert_eq!(js_slice("ab", 5, None), "");
        assert_eq!(js_trim(" \u{A0}x\n"), "x");
    }
}
