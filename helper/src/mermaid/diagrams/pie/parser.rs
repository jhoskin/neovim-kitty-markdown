//! A hand-written lexer and parser for the language of `pie.langium` plus
//! `common.langium` (`@mermaid-js/parser` 2.0.1, mermaid 12.2.0).
//!
//! Langium builds a Chevrotain lexer from the grammar: the token types are
//! the terminals a parser rule reaches plus the hidden terminals, and the
//! `pie` and `showData` keywords get the `(?:(?=%%)|(?!\S))` suffix that
//! `AbstractMermaidTokenBuilder` adds. At each position the first token
//! type in Chevrotain's order that matches wins, and a position where none
//! matches is skipped and reported as one lexer error per run of skipped
//! characters. The parser is an LL(1) walk of the `Pie` rule with the
//! error texts of Chevrotain's default error provider; it reports the first
//! parser error, where Langium's recovering parser can report several.
//! `MermaidParseError` joins both lists as
//! `Parsing failed: <lexer errors> <parser errors>`.

use std::fmt::Write;

use crate::mermaid::theme::khroma::js_is_whitespace;

/// The `Pie` AST after Langium's value converters.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ast {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub show_data: bool,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub label: String,
    pub value: f64,
    /// 1-based source line of the label, for the error the db throws on a
    /// negative value.
    pub line: u32,
}

/// What `MermaidParseError` carries: the joined message and the position of
/// the first error (lexer errors first, as mermaid prints them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseFailure {
    pub message: String,
    pub line: u32,
    /// 1-based; 0 when Chevrotain has no position (an inserted EOF token).
    pub column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Newline,
    Pie,
    ShowData,
    Colon,
    AccDescr,
    AccTitle,
    Title,
    Str,
    Number,
    Eof,
}

impl Kind {
    /// Chevrotain's `tokenType.name`: the keyword text for keywords, the
    /// terminal rule name otherwise.
    fn name(self) -> &'static str {
        match self {
            Kind::Newline => "NEWLINE",
            Kind::Pie => "pie",
            Kind::ShowData => "showData",
            Kind::Colon => ":",
            Kind::AccDescr => "ACC_DESCR",
            Kind::AccTitle => "ACC_TITLE",
            Kind::Title => "TITLE",
            Kind::Str => "STRING",
            Kind::Number => "NUMBER_PIE",
            Kind::Eof => "EOF",
        }
    }
}

#[derive(Debug, Clone)]
struct Token {
    kind: Kind,
    /// Character range in the input.
    start: usize,
    end: usize,
    line: u32,
    column: u32,
}

#[derive(Debug, Clone)]
struct LexError {
    line: u32,
    column: u32,
    /// UTF-16 offset, as Chevrotain counts JS string indices.
    offset: usize,
    skipped: usize,
    first: char,
}

/// A hidden terminal: matched and dropped.
const HIDDEN: Option<Kind> = None;

/// A token matcher: the end of the match at a position, if any.
type Matcher<'a> = fn(&Lexer<'a>, usize) -> Option<usize>;

struct Lexer<'a> {
    chars: &'a [char],
    /// UTF-16 offset of each character, plus one past the end.
    utf16: Vec<usize>,
}

impl<'a> Lexer<'a> {
    fn new(chars: &'a [char]) -> Lexer<'a> {
        let mut utf16 = Vec::with_capacity(chars.len() + 1);
        let mut offset = 0;
        for c in chars {
            utf16.push(offset);
            offset += c.len_utf16();
        }
        utf16.push(offset);
        Lexer { chars, utf16 }
    }

    fn at(&self, i: usize) -> Option<char> {
        self.chars.get(i).copied()
    }

    fn starts_with(&self, i: usize, s: &str) -> bool {
        s.chars()
            .enumerate()
            .all(|(k, c)| self.at(i + k) == Some(c))
    }

    /// The end of `[^\n\r]*` from `i`.
    fn line_end(&self, i: usize) -> usize {
        let mut j = i;
        while let Some(c) = self.at(j)
            && c != '\n'
            && c != '\r'
        {
            j += 1;
        }
        j
    }

    /// `(?:[^\n\r]*?(?=%%)|[^\n\r]*)` from `i`: up to the first `%%` on
    /// the line when there is one, else the rest of the line.
    fn rest_of_line_before_comment(&self, i: usize) -> usize {
        let end = self.line_end(i);
        let mut j = i;
        while j + 1 < end {
            if self.chars[j] == '%' && self.chars[j + 1] == '%' {
                return j;
            }
            j += 1;
        }
        end
    }

    /// The `(?:(?=%%)|(?!\S))` suffix the mermaid token builder appends to
    /// its keywords.
    fn keyword_boundary(&self, i: usize) -> bool {
        match self.at(i) {
            None => true,
            Some('%') => self.at(i + 1) == Some('%'),
            Some(c) => js_is_whitespace(c),
        }
    }

    fn keyword(&self, i: usize, word: &str) -> Option<usize> {
        let end = i + word.chars().count();
        (self.starts_with(i, word) && self.keyword_boundary(end)).then_some(end)
    }

    /// `\r?\n`.
    fn newline(&self, i: usize) -> Option<usize> {
        match self.at(i) {
            Some('\n') => Some(i + 1),
            Some('\r') if self.at(i + 1) == Some('\n') => Some(i + 2),
            _ => None,
        }
    }

    /// `[\t ]+`.
    fn whitespace(&self, i: usize) -> Option<usize> {
        let mut j = i;
        while matches!(self.at(j), Some(' ') | Some('\t')) {
            j += 1;
        }
        (j > i).then_some(j)
    }

    fn blanks(&self, i: usize) -> usize {
        self.whitespace(i).unwrap_or(i)
    }

    /// `\s*` with JS whitespace.
    fn js_spaces(&self, i: usize) -> usize {
        let mut j = i;
        while let Some(c) = self.at(j)
            && js_is_whitespace(c)
        {
            j += 1;
        }
        j
    }

    /// `accDescr(?:[\t ]*:(?:[^\n\r]*?(?=%%)|[^\n\r]*)|\s*{([^}]*)})`.
    fn acc_descr(&self, i: usize) -> Option<usize> {
        if !self.starts_with(i, "accDescr") {
            return None;
        }
        let j = i + "accDescr".len();
        let after_blanks = self.blanks(j);
        if self.at(after_blanks) == Some(':') {
            return Some(self.rest_of_line_before_comment(after_blanks + 1));
        }
        let brace = self.js_spaces(j);
        if self.at(brace) != Some('{') {
            return None;
        }
        let mut k = brace + 1;
        while let Some(c) = self.at(k) {
            if c == '}' {
                return Some(k + 1);
            }
            k += 1;
        }
        None
    }

    /// `accTitle[\t ]*:(?:[^\n\r]*?(?=%%)|[^\n\r]*)`.
    fn acc_title(&self, i: usize) -> Option<usize> {
        if !self.starts_with(i, "accTitle") {
            return None;
        }
        let colon = self.blanks(i + "accTitle".len());
        (self.at(colon) == Some(':')).then(|| self.rest_of_line_before_comment(colon + 1))
    }

    /// `title(?:[\t ][^\n\r]*?(?=%%)|[\t ][^\n\r]*|)`.
    fn title(&self, i: usize) -> Option<usize> {
        if !self.starts_with(i, "title") {
            return None;
        }
        let j = i + "title".len();
        match self.at(j) {
            Some(' ') | Some('\t') => Some(self.rest_of_line_before_comment(j + 1)),
            _ => Some(j),
        }
    }

    /// `"([^"\\]|\\.)*"|'([^'\\]|\\.)*'`; `.` excludes line terminators.
    fn string(&self, i: usize) -> Option<usize> {
        let quote = self.at(i)?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let mut j = i + 1;
        loop {
            match self.at(j)? {
                c if c == quote => return Some(j + 1),
                '\\' => match self.at(j + 1)? {
                    '\n' | '\r' | '\u{2028}' | '\u{2029}' => return None,
                    _ => j += 2,
                },
                _ => j += 1,
            }
        }
    }

    fn digits(&self, i: usize) -> usize {
        let mut j = i;
        while self.at(j).is_some_and(|c| c.is_ascii_digit()) {
            j += 1;
        }
        j
    }

    /// `NUMBER_PIE`: `-?[0-9]+\.[0-9]+(?!\.)` then `-?(0|[1-9][0-9]*)(?!\.)`,
    /// first alternative wins, no backtracking into a shorter integer.
    fn number(&self, i: usize) -> Option<usize> {
        let j = if self.at(i) == Some('-') { i + 1 } else { i };
        let int_end = self.digits(j);
        if int_end == j {
            return None;
        }
        let not_dot = |k: usize| self.at(k) != Some('.');
        if self.at(int_end) == Some('.') {
            let frac_end = self.digits(int_end + 1);
            if frac_end > int_end + 1 && not_dot(frac_end) {
                return Some(frac_end);
            }
        }
        let int_alt_end = if self.at(j) == Some('0') {
            j + 1
        } else {
            int_end
        };
        not_dot(int_alt_end).then_some(int_alt_end)
    }

    /// `(?:\r?\n|(?!\S))` after a block: a newline, the end, or whitespace.
    fn block_end(&self, i: usize) -> Option<usize> {
        if let Some(end) = self.newline(i) {
            return Some(end);
        }
        match self.at(i) {
            None => Some(i),
            Some(c) if js_is_whitespace(c) => Some(i),
            Some(_) => None,
        }
    }

    /// `---[\t ]*\r?\n(?:[\S\s]*?\r?\n)?---(?:\r?\n|(?!\S))`.
    fn yaml(&self, i: usize) -> Option<usize> {
        if !self.starts_with(i, "---") {
            return None;
        }
        let first_newline = self.newline(self.blanks(i + 3))?;
        let mut k = first_newline;
        loop {
            if self.starts_with(k, "---")
                && (k == first_newline || self.at(k - 1) == Some('\n'))
                && let Some(end) = self.block_end(k + 3)
            {
                return Some(end);
            }
            k += 1;
            if k > self.chars.len() {
                return None;
            }
        }
    }

    /// `%%{[\S\s]*?}%%(?:\r?\n|(?!\S))`.
    fn directive(&self, i: usize) -> Option<usize> {
        if !self.starts_with(i, "%%{") {
            return None;
        }
        let mut k = i + 3;
        while k + 3 <= self.chars.len() {
            if self.starts_with(k, "}%%")
                && let Some(end) = self.block_end(k + 3)
            {
                return Some(end);
            }
            k += 1;
        }
        None
    }

    /// `%%[^\n\r]*`.
    fn comment(&self, i: usize) -> Option<usize> {
        self.starts_with(i, "%%").then(|| self.line_end(i + 2))
    }

    /// The first token type that matches at `i`, in Chevrotain's order:
    /// whitespace-like terminals, keywords, the reachable terminals, the
    /// remaining hidden terminals. `None` in the result means hidden.
    fn next(&self, i: usize) -> Option<(Option<Kind>, usize)> {
        let table: [(Option<Kind>, Matcher<'a>); 13] = [
            (Some(Kind::Newline), Lexer::newline),
            (HIDDEN, Lexer::whitespace),
            (Some(Kind::ShowData), |l, i| l.keyword(i, "showData")),
            (Some(Kind::Pie), |l, i| l.keyword(i, "pie")),
            (Some(Kind::Colon), |l, i| {
                (l.at(i) == Some(':')).then_some(i + 1)
            }),
            (Some(Kind::AccDescr), Lexer::acc_descr),
            (Some(Kind::AccTitle), Lexer::acc_title),
            (Some(Kind::Title), Lexer::title),
            (Some(Kind::Str), Lexer::string),
            (Some(Kind::Number), Lexer::number),
            (HIDDEN, Lexer::yaml),
            (HIDDEN, Lexer::directive),
            (HIDDEN, Lexer::comment),
        ];
        table
            .iter()
            .find_map(|(kind, matcher)| matcher(self, i).map(|end| (*kind, end)))
    }

    fn tokenize(&self) -> (Vec<Token>, Vec<LexError>) {
        let mut tokens = Vec::new();
        let mut errors = Vec::new();
        let mut i = 0;
        let mut line = 1;
        let mut line_start = 0;
        let mut pending: Option<LexError> = None;
        while i < self.chars.len() {
            match self.next(i) {
                Some((kind, end)) => {
                    if let Some(err) = pending.take() {
                        errors.push(err);
                    }
                    if let Some(kind) = kind {
                        tokens.push(Token {
                            kind,
                            start: i,
                            end,
                            line,
                            column: (self.utf16[i] - self.utf16[line_start] + 1) as u32,
                        });
                    }
                    for k in i..end {
                        if self.chars[k] == '\n'
                            || (self.chars[k] == '\r' && self.at(k + 1) != Some('\n'))
                        {
                            line += 1;
                            line_start = k + 1;
                        }
                    }
                    i = end;
                }
                None => {
                    match &mut pending {
                        Some(err) => err.skipped += 1,
                        None => {
                            pending = Some(LexError {
                                line,
                                column: (self.utf16[i] - self.utf16[line_start] + 1) as u32,
                                offset: self.utf16[i],
                                skipped: 1,
                                first: self.chars[i],
                            })
                        }
                    }
                    if self.chars[i] == '\n'
                        || (self.chars[i] == '\r' && self.at(i + 1) != Some('\n'))
                    {
                        line += 1;
                        line_start = i + 1;
                    }
                    i += 1;
                }
            }
        }
        if let Some(err) = pending {
            errors.push(err);
        }
        tokens.push(Token {
            kind: Kind::Eof,
            start: self.chars.len(),
            end: self.chars.len(),
            line: 0,
            column: 0,
        });
        (tokens, errors)
    }
}

struct ParseError {
    line: u32,
    column: u32,
    message: String,
}

struct Parser<'a> {
    chars: &'a [char],
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn image(&self, t: &Token) -> String {
        self.chars[t.start..t.end].iter().collect()
    }

    fn advance(&mut self) -> Token {
        let t = self.peek().clone();
        if t.kind != Kind::Eof {
            self.pos += 1;
        }
        t
    }

    /// `MismatchedTokenException` from Chevrotain's default error provider.
    fn mismatched(&self, expected: Kind) -> ParseError {
        let t = self.peek();
        ParseError {
            line: t.line,
            column: t.column,
            message: format!(
                "Expecting token of type '{}' but found `{}`.",
                expected.name(),
                self.image(t)
            ),
        }
    }

    fn expect(&mut self, kind: Kind) -> Result<Token, ParseError> {
        if self.peek().kind == kind {
            Ok(self.advance())
        } else {
            Err(self.mismatched(kind))
        }
    }

    /// `EOL: NEWLINE+ | EOF`. A token that is neither is a
    /// `NoViableAltException` listing both alternatives.
    fn eol(&mut self) -> Result<(), ParseError> {
        match self.peek().kind {
            Kind::Newline => {
                while self.peek().kind == Kind::Newline {
                    self.advance();
                }
                Ok(())
            }
            Kind::Eof => Ok(()),
            _ => {
                let t = self.peek();
                Err(ParseError {
                    line: t.line,
                    column: t.column,
                    message: format!(
                        "Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: '{}'",
                        self.image(t)
                    ),
                })
            }
        }
    }

    /// The `Pie` entry rule.
    fn pie(&mut self) -> Result<Ast, ParseError> {
        let mut ast = Ast::default();
        while self.peek().kind == Kind::Newline {
            self.advance();
        }
        self.expect(Kind::Pie)?;
        if self.peek().kind == Kind::ShowData {
            self.advance();
            ast.show_data = true;
        }
        loop {
            match self.peek().kind {
                Kind::Newline => {
                    self.advance();
                }
                Kind::AccDescr => {
                    let t = self.advance();
                    ast.acc_descr = Some(convert_acc_descr(&self.image(&t)));
                    self.eol()?;
                }
                Kind::AccTitle => {
                    let t = self.advance();
                    ast.acc_title = Some(convert_acc_title(&self.image(&t)));
                    self.eol()?;
                }
                Kind::Title => {
                    let t = self.advance();
                    ast.title = Some(convert_title(&self.image(&t)));
                    self.eol()?;
                }
                Kind::Str => {
                    let label = self.advance();
                    self.expect(Kind::Colon)?;
                    let value = self.expect(Kind::Number)?;
                    self.eol()?;
                    ast.sections.push(Section {
                        label: convert_string(&self.image(&label)),
                        value: convert_number(&self.image(&value)),
                        line: label.line,
                    });
                }
                _ => break,
            }
        }
        self.expect(Kind::Eof)?;
        Ok(ast)
    }
}

/// Langium's `convertString`: the quotes go, a backslash escape becomes
/// the control character for `b f n r t v 0` and the character itself
/// otherwise.
fn convert_string(image: &str) -> String {
    let inner: Vec<char> = image.chars().collect();
    let mut out = String::new();
    let mut i = 1;
    while i + 1 < inner.len() {
        let c = inner[i];
        if c == '\\' && i + 2 < inner.len() {
            i += 1;
            out.push(match inner[i] {
                'b' => '\u{8}',
                'f' => '\u{c}',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                'v' => '\u{b}',
                '0' => '\0',
                other => other,
            });
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

/// `Number(input)` for a `NUMBER_PIE` image.
fn convert_number(image: &str) -> f64 {
    image.parse().unwrap_or(f64::NAN)
}

/// `[\t ]{2,}` to one space, after a JS `trim`.
fn collapse_blanks(s: &str) -> String {
    let mut out = String::new();
    let mut run = 0;
    for c in s.chars() {
        if c == ' ' || c == '\t' {
            run += 1;
            if run == 1 {
                out.push(c);
            } else if run == 2 {
                out.pop();
                out.push(' ');
            }
        } else {
            run = 0;
            out.push(c);
        }
    }
    out
}

fn js_trim(s: &str) -> &str {
    s.trim_matches(js_is_whitespace)
}

/// `AbstractMermaidValueConverter.runCommonConverter` with `titleRegex`
/// `/title([\t ][^\n\r]*|)/`: group 1 trimmed, blank runs collapsed.
fn convert_title(image: &str) -> String {
    let rest = &image["title".len()..];
    collapse_blanks(js_trim(rest))
}

/// `accessibilityTitleRegex` `/accTitle[\t ]*:([^\n\r]*)/`.
fn convert_acc_title(image: &str) -> String {
    let after = image.split_once(':').map(|(_, r)| r).unwrap_or("");
    collapse_blanks(js_trim(after))
}

/// `accessibilityDescrRegex` `/accDescr(?:[\t ]*:([^\n\r]*)|\s*{([^}]*)})/`:
/// the single-line form like a title, the block form with every line
/// trimmed, blank runs collapsed, and blank lines dropped.
fn convert_acc_descr(image: &str) -> String {
    let after_keyword = &image["accDescr".len()..];
    let colon_form = after_keyword.trim_start_matches([' ', '\t']);
    if let Some(rest) = colon_form.strip_prefix(':') {
        return collapse_blanks(js_trim(rest));
    }
    let inner = colon_form
        .trim_start_matches(js_is_whitespace)
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or("");
    let mut lines: Vec<String> = Vec::new();
    for line in inner.split(['\n', '\r']) {
        let trimmed = js_trim(line);
        if trimmed.is_empty() {
            continue;
        }
        lines.push(collapse_blanks(trimmed));
    }
    lines.join("\n")
}

/// `parse('pie', text)`: the AST, or the `MermaidParseError` text with the
/// position of its first error.
pub fn parse(text: &str) -> Result<Ast, ParseFailure> {
    let chars: Vec<char> = text.chars().collect();
    let lexer = Lexer::new(&chars);
    let (tokens, lex_errors) = lexer.tokenize();
    let mut parser = Parser {
        chars: &chars,
        tokens,
        pos: 0,
    };
    let parsed = parser.pie();
    match (&lex_errors[..], parsed) {
        ([], Ok(ast)) => Ok(ast),
        (lex, parsed) => {
            let mut lexer_text = String::new();
            for (i, e) in lex.iter().enumerate() {
                if i > 0 {
                    lexer_text.push('\n');
                }
                let _ = write!(
                    lexer_text,
                    "Lexer error on line {}, column {}: unexpected character: ->{}<- at offset: {}, skipped {} characters.",
                    e.line, e.column, e.first, e.offset, e.skipped
                );
            }
            let parser_text = match &parsed {
                Err(e) => format!(
                    "Parse error on line {}, column {}: {}",
                    location(e.line),
                    location(e.column),
                    e.message
                ),
                Ok(_) => String::new(),
            };
            let (line, column) = match (lex.first(), &parsed) {
                (Some(e), _) => (e.line, e.column),
                (None, Err(e)) => (e.line, e.column),
                (None, Ok(_)) => (0, 0),
            };
            Err(ParseFailure {
                message: format!("Parsing failed: {lexer_text} {parser_text}"),
                line,
                column,
            })
        }
    }
}

/// `formatLocation`: Chevrotain marks a missing position (the EOF token)
/// with `NaN` or `-1`, printed as `?`.
fn location(v: u32) -> String {
    if v == 0 {
        "?".to_string()
    } else {
        v.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(text: &str) -> Ast {
        parse(text).unwrap_or_else(|e| panic!("{text:?}: {}", e.message))
    }

    fn err(text: &str) -> ParseFailure {
        parse(text)
            .err()
            .unwrap_or_else(|| panic!("{text:?} parsed"))
    }

    fn labels(ast: &Ast) -> Vec<(&str, f64)> {
        ast.sections
            .iter()
            .map(|s| (s.label.as_str(), s.value))
            .collect()
    }

    #[test]
    fn docs_example_with_title_and_show_data() {
        let ast = ok(
            "pie showData\n    title Key elements in Product X\n    \"Calcium\" : 42.96\n    \"Potassium\" : 50.05\n    \"Magnesium\" : 10.01\n    \"Iron\" :  5\n\n",
        );
        assert!(ast.show_data);
        assert_eq!(ast.title.as_deref(), Some("Key elements in Product X"));
        assert_eq!(
            labels(&ast),
            vec![
                ("Calcium", 42.96),
                ("Potassium", 50.05),
                ("Magnesium", 10.01),
                ("Iron", 5.0)
            ]
        );
    }

    #[test]
    fn title_and_show_data_forms() {
        assert_eq!(ok("pie title Pets\n\n").title.as_deref(), Some("Pets"));
        assert_eq!(
            ok("pie\n  title\n  \"A\" : 1\n\n").title.as_deref(),
            Some("")
        );
        let ast = ok("pie showData title X\n  \"A\" : 1\n\n");
        assert!(ast.show_data);
        assert_eq!(ast.title.as_deref(), Some("X"));
        assert_eq!(
            ok("pie\n  \"A\" : 1\n  title Later\n\n").title.as_deref(),
            Some("Later")
        );
        assert_eq!(labels(&ok("\n\npie\n\n")), Vec::<(&str, f64)>::new());
        assert_eq!(labels(&ok("pie\r\n  \"A\" : 1\r\n\n")), vec![("A", 1.0)]);
        assert_eq!(labels(&ok("pie \"A\" : 1\n")), vec![("A", 1.0)]);
    }

    #[test]
    fn strings_keep_spaces_and_decode_escapes() {
        let ast = ok(
            "pie\n  \"A\" : 1\n  \"A\" : 2\n  \" B \" : 3\n  'C\\'d' : 4\n  \"E\\\"f\\n\" : 5\n  \"Z\" : -0\n\n",
        );
        assert_eq!(
            labels(&ast),
            vec![
                ("A", 1.0),
                ("A", 2.0),
                (" B ", 3.0),
                ("C'd", 4.0),
                ("E\"f\n", 5.0),
                ("Z", -0.0)
            ]
        );
    }

    #[test]
    fn comments_directives_and_accessibility_are_hidden_or_converted() {
        let ast = ok(
            "pie\n  title  Two   spaces  %% comment\n  accTitle: acc   T\n  accDescr {\n    multi\n    line\n  }\n  \"A\" : 1 %% trailing\n  %%{init: {\"theme\":\"dark\"}}%%\n  \"B\" : 2\n\n",
        );
        assert_eq!(ast.title.as_deref(), Some("Two spaces"));
        assert_eq!(ast.acc_title.as_deref(), Some("acc T"));
        assert_eq!(ast.acc_descr.as_deref(), Some("multi\nline"));
        assert_eq!(labels(&ast), vec![("A", 1.0), ("B", 2.0)]);
    }

    #[test]
    fn errors_match_the_langium_parser() {
        let cases = [
            (
                "pie title Pets\n    Dogs : 386\n    \"Cats\" : 85\n\n",
                "Parsing failed: Lexer error on line 2, column 5: unexpected character: ->D<- at offset: 19, skipped 4 characters. Parse error on line 2, column 10: Expecting token of type 'EOF' but found `:`.",
                (2, 5),
            ),
            (
                "pie\n    \"Dogs\" 386\n\n",
                "Parsing failed:  Parse error on line 2, column 12: Expecting token of type ':' but found `386`.",
                (2, 12),
            ),
            (
                "pie\n    \"Dogs\" :\n    \"Cats\" : 1\n\n",
                "Parsing failed:  Parse error on line 2, column 13: Expecting token of type 'NUMBER_PIE' but found `\n`.",
                (2, 13),
            ),
            (
                "pieChart\n    \"Dogs\" : 5\n\n",
                "Parsing failed: Lexer error on line 1, column 1: unexpected character: ->p<- at offset: 0, skipped 8 characters. Parse error on line 2, column 5: Expecting token of type 'pie' but found `\"Dogs\"`.",
                (1, 1),
            ),
            (
                "pie\n    \"Dogs\" : 5 extra\n\n",
                "Parsing failed: Lexer error on line 2, column 16: unexpected character: ->e<- at offset: 19, skipped 5 characters. ",
                (2, 16),
            ),
            (
                "pie\n    \"Dogs\" : 5.\n\n",
                "Parsing failed: Lexer error on line 2, column 14: unexpected character: ->5<- at offset: 17, skipped 2 characters. Parse error on line 2, column 16: Expecting token of type 'NUMBER_PIE' but found `\n`.",
                (2, 14),
            ),
            (
                "pie\n    \"Dogs\" : 1.2.3\n\n",
                "Parsing failed: Lexer error on line 2, column 14: unexpected character: ->1<- at offset: 17, skipped 2 characters. ",
                (2, 14),
            ),
            (
                "pie\n    \"Dogs : 5\n\n",
                "Parsing failed: Lexer error on line 2, column 5: unexpected character: ->\"<- at offset: 8, skipped 5 characters. Parse error on line 2, column 11: Expecting token of type 'EOF' but found `:`.",
                (2, 5),
            ),
            (
                "pie\n  \"A\" : 1\n  \"B\" : 2 \"C\" : 3\n\n",
                "Parsing failed:  Parse error on line 3, column 11: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: '\"C\"'",
                (3, 11),
            ),
            (
                "pie\n  \"A\" : 01\n\n",
                "Parsing failed:  Parse error on line 2, column 10: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: '1'",
                (2, 10),
            ),
            (
                "pie\n  \"A\" : +1\n\n",
                "Parsing failed: Lexer error on line 2, column 9: unexpected character: ->+<- at offset: 12, skipped 1 characters. ",
                (2, 9),
            ),
            (
                "pie\n  titles\n  \"A\" : 1\n\n",
                "Parsing failed: Lexer error on line 2, column 8: unexpected character: ->s<- at offset: 11, skipped 1 characters. ",
                (2, 8),
            ),
            (
                "pie\n  \"A\" : 1\n  showData\n\n",
                "Parsing failed:  Parse error on line 3, column 3: Expecting token of type 'EOF' but found `showData`.",
                (3, 3),
            ),
            (
                "pie\n  \"A\" : 1 ; \"B\" : 2\n\n",
                "Parsing failed: Lexer error on line 2, column 11: unexpected character: ->;<- at offset: 14, skipped 1 characters. Parse error on line 2, column 13: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: '\"B\"'",
                (2, 11),
            ),
            (
                "pie\n  \"A\" :: 1\n\n",
                "Parsing failed:  Parse error on line 2, column 8: Expecting token of type 'NUMBER_PIE' but found `:`.",
                (2, 8),
            ),
            (
                "pie\n  \"A\" : -0\n  \"B\" : 1e3\n\n",
                "Parsing failed: Lexer error on line 3, column 10: unexpected character: ->e<- at offset: 24, skipped 1 characters. Parse error on line 3, column 11: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: '3'",
                (3, 10),
            ),
        ];
        for (text, message, (line, column)) in cases {
            let failure = err(text);
            assert_eq!(failure.message, message, "{text:?}");
            assert_eq!((failure.line, failure.column), (line, column), "{text:?}");
        }
    }
}
