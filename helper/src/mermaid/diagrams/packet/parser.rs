//! A hand-written lexer and parser for the language of `packet.langium`
//! plus `common.langium` (`@mermaid-js/parser` 2.0.1, mermaid 12.2.0).
//!
//! Langium builds a Chevrotain lexer from the grammar: the token types are
//! the terminals a parser rule reaches plus the hidden terminals, with the
//! keywords sorted longest first (`packet-beta`, `packet`, `-`, `+`, `:`).
//! `PacketTokenBuilder` adds the `(?:(?=%%)|(?!\S))` suffix to `packet`
//! only. At each position the first token type in Chevrotain's order that
//! matches wins, and a position where none matches is skipped and reported
//! as one lexer error per run of skipped characters. The parser is an LL
//! walk of the `Packet` rule with the error texts of Langium's parser error
//! provider; it reports the first parser error, where Langium's recovering
//! parser can report several. `MermaidParseError` joins both lists as
//! `Parsing failed: <lexer errors> <parser errors>`.
//!
//! The lexer scaffolding mirrors the pie parser, which is private to its
//! module; the two grammars share `common.langium` but nothing else.

use std::fmt::Write;

use crate::mermaid::theme::khroma::js_is_whitespace;

/// The `Packet` AST after Langium's value converters.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ast {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub blocks: Vec<Block>,
}

/// One `PacketBlock`: `start('-' end)?` or `'+' bits`, then the label.
/// `INT` converts with `Number()`, so the fields are JS numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub bits: Option<f64>,
    pub label: String,
    /// 1-based source line of the block, for the errors `populate` throws.
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
    PacketBeta,
    Packet,
    Minus,
    Plus,
    Colon,
    AccDescr,
    AccTitle,
    Title,
    Int,
    Str,
    Eof,
}

impl Kind {
    /// Chevrotain's `tokenType.name`: the keyword text for keywords, the
    /// terminal rule name otherwise.
    fn name(self) -> &'static str {
        match self {
            Kind::Newline => "NEWLINE",
            Kind::PacketBeta => "packet-beta",
            Kind::Packet => "packet",
            Kind::Minus => "-",
            Kind::Plus => "+",
            Kind::Colon => ":",
            Kind::AccDescr => "ACC_DESCR",
            Kind::AccTitle => "ACC_TITLE",
            Kind::Title => "TITLE",
            Kind::Int => "INT",
            Kind::Str => "STRING",
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
    /// the `packet` keyword.
    fn keyword_boundary(&self, i: usize) -> bool {
        match self.at(i) {
            None => true,
            Some('%') => self.at(i + 1) == Some('%'),
            Some(c) => js_is_whitespace(c),
        }
    }

    fn literal(&self, i: usize, word: &str) -> Option<usize> {
        self.starts_with(i, word).then(|| i + word.chars().count())
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

    /// `accDescr(?:[\t ]*:(?:[^\n\r]*?(?=%%)|[^\n\r]*)|\s*{([^}]*)})`. The
    /// grammar's `[\t ]*` prefix never matters: the hidden `WHITESPACE`
    /// terminal comes first in the token order and eats the blanks.
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

    /// `INT`: `0|[1-9][0-9]*(?!\.)`. The lookahead belongs to the second
    /// alternative only, so `0` matches before a dot, and a longer run
    /// before a dot backtracks by one digit as the regex engine does.
    fn int(&self, i: usize) -> Option<usize> {
        match self.at(i)? {
            '0' => Some(i + 1),
            '1'..='9' => {
                let end = self.digits(i);
                if self.at(end) != Some('.') {
                    Some(end)
                } else if end - i > 1 {
                    Some(end - 1)
                } else {
                    None
                }
            }
            _ => None,
        }
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
    /// whitespace-like terminals, keywords longest first, the reachable
    /// terminals in grammar order, the remaining hidden terminals.
    fn next(&self, i: usize) -> Option<(Option<Kind>, usize)> {
        let table: [(Option<Kind>, Matcher<'a>); 15] = [
            (Some(Kind::Newline), Lexer::newline),
            (HIDDEN, Lexer::whitespace),
            (Some(Kind::PacketBeta), |l, i| l.literal(i, "packet-beta")),
            (Some(Kind::Packet), |l, i| {
                l.literal(i, "packet")
                    .filter(|end| l.keyword_boundary(*end))
            }),
            (Some(Kind::Minus), |l, i| l.literal(i, "-")),
            (Some(Kind::Plus), |l, i| l.literal(i, "+")),
            (Some(Kind::Colon), |l, i| l.literal(i, ":")),
            (Some(Kind::AccDescr), Lexer::acc_descr),
            (Some(Kind::AccTitle), Lexer::acc_title),
            (Some(Kind::Title), Lexer::title),
            (Some(Kind::Int), Lexer::int),
            (Some(Kind::Str), Lexer::string),
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
        let is_line_break = |k: usize| {
            self.chars[k] == '\n' || (self.chars[k] == '\r' && self.at(k + 1) != Some('\n'))
        };
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
                        if is_line_break(k) {
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
                    if is_line_break(i) {
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

    /// `MismatchedTokenException` from Langium's parser error provider.
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

    /// `NoViableAltException` from Chevrotain's default error provider, for
    /// an alternation whose every alternative starts with one token.
    fn no_viable_alt(&self, alternatives: &[Kind]) -> ParseError {
        let t = self.peek();
        let mut message = "Expecting: one of these possible Token sequences:".to_string();
        for (i, alt) in alternatives.iter().enumerate() {
            let _ = write!(message, "\n  {}. [{}]", i + 1, alt.name());
        }
        let _ = write!(message, "\nbut found: '{}'", self.image(t));
        ParseError {
            line: t.line,
            column: t.column,
            message,
        }
    }

    fn expect(&mut self, kind: Kind) -> Result<Token, ParseError> {
        if self.peek().kind == kind {
            Ok(self.advance())
        } else {
            Err(self.mismatched(kind))
        }
    }

    /// `EOL: NEWLINE+ | EOF`.
    fn eol(&mut self) -> Result<(), ParseError> {
        match self.peek().kind {
            Kind::Newline => {
                while self.peek().kind == Kind::Newline {
                    self.advance();
                }
                Ok(())
            }
            Kind::Eof => Ok(()),
            _ => Err(self.no_viable_alt(&[Kind::Newline, Kind::Eof])),
        }
    }

    /// `PacketBlock: (start=INT ('-' end=INT)? | '+' bits=INT) ':'
    /// label=STRING EOL`. The caller has seen `INT` or `+`.
    fn block(&mut self) -> Result<Block, ParseError> {
        let first = self.advance();
        let mut block = Block {
            start: None,
            end: None,
            bits: None,
            label: String::new(),
            line: first.line,
        };
        if first.kind == Kind::Plus {
            let bits = self.expect(Kind::Int)?;
            block.bits = Some(convert_int(&self.image(&bits)));
        } else {
            block.start = Some(convert_int(&self.image(&first)));
            if self.peek().kind == Kind::Minus {
                self.advance();
                let end = self.expect(Kind::Int)?;
                block.end = Some(convert_int(&self.image(&end)));
            }
        }
        self.expect(Kind::Colon)?;
        let label = self.expect(Kind::Str)?;
        block.label = convert_string(&self.image(&label));
        self.eol()?;
        Ok(block)
    }

    /// The `Packet` entry rule.
    fn packet(&mut self) -> Result<Ast, ParseError> {
        let mut ast = Ast::default();
        while self.peek().kind == Kind::Newline {
            self.advance();
        }
        match self.peek().kind {
            Kind::Packet | Kind::PacketBeta => {
                self.advance();
            }
            _ => return Err(self.no_viable_alt(&[Kind::Packet, Kind::PacketBeta])),
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
                Kind::Int | Kind::Plus => {
                    let block = self.block()?;
                    ast.blocks.push(block);
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

/// `convertNumber`, `Number(input)` for an `INT` image.
fn convert_int(image: &str) -> f64 {
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

/// `parse('packet', text)`: the AST, or the `MermaidParseError` text with
/// the position of its first error.
pub fn parse(text: &str) -> Result<Ast, ParseFailure> {
    let chars: Vec<char> = text.chars().collect();
    let lexer = Lexer::new(&chars);
    let (tokens, lex_errors) = lexer.tokenize();
    let mut parser = Parser {
        chars: &chars,
        tokens,
        pos: 0,
    };
    let parsed = parser.packet();
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

    type Range<'a> = (Option<f64>, Option<f64>, Option<f64>, &'a str);

    fn ranges(ast: &Ast) -> Vec<Range<'_>> {
        ast.blocks
            .iter()
            .map(|b| (b.start, b.end, b.bits, b.label.as_str()))
            .collect()
    }

    #[test]
    fn docs_tcp_example() {
        let ast = ok(
            "packet-beta\n0-15: \"Source Port\"\n16-31: \"Destination Port\"\n106: \"URG\"\n192-255: \"Data (variable length)\"\n",
        );
        assert_eq!(
            ranges(&ast),
            vec![
                (Some(0.0), Some(15.0), None, "Source Port"),
                (Some(16.0), Some(31.0), None, "Destination Port"),
                (Some(106.0), None, None, "URG"),
                (Some(192.0), Some(255.0), None, "Data (variable length)"),
            ]
        );
        assert_eq!(ast.blocks[2].line, 4);
    }

    #[test]
    fn keywords_bits_title_and_blank_lines() {
        let ast = ok(
            "packet\ntitle UDP Packet\n+16: \"Source Port\"\n\n+16: \"Destination Port\"\n32-47: \"Length\"\n",
        );
        assert_eq!(ast.title.as_deref(), Some("UDP Packet"));
        assert_eq!(
            ranges(&ast),
            vec![
                (None, None, Some(16.0), "Source Port"),
                (None, None, Some(16.0), "Destination Port"),
                (Some(32.0), Some(47.0), None, "Length"),
            ]
        );
        for text in [
            "packet-beta",
            "  packet-beta  ",
            "\tpacket-beta\t",
            "\n    \tpacket-beta\n    ",
            "packet",
            "  packet  ",
            "\tpacket\t",
            "\n    \tpacket\n    ",
        ] {
            assert!(ok(text).blocks.is_empty(), "{text:?}");
        }
        assert_eq!(ranges(&ok("packet 0 - 1 : 'a'\n")).len(), 1);
        assert_eq!(ok("packet\n  title\n").title.as_deref(), Some(""));
        assert_eq!(ok("packet\r\n0: \"a\"\r\n").blocks[0].label, "a");
    }

    #[test]
    fn comments_directives_and_accessibility_are_hidden_or_converted() {
        let ast = ok(
            "packet\n  title  Two   spaces  %% comment\n  accTitle: acc   T\n  accDescr {\n    multi\n    line\n  }\n  0-1: \"A\" %% trailing\n  %%{init: {\"theme\":\"dark\"}}%%\n  2: \"B\\\"c\"\n\n",
        );
        assert_eq!(ast.title.as_deref(), Some("Two spaces"));
        assert_eq!(ast.acc_title.as_deref(), Some("acc T"));
        assert_eq!(ast.acc_descr.as_deref(), Some("multi\nline"));
        assert_eq!(
            ranges(&ast),
            vec![
                (Some(0.0), Some(1.0), None, "A"),
                (Some(2.0), None, None, "B\"c"),
            ]
        );
    }

    #[test]
    fn errors_match_the_langium_parser() {
        let cases = [
            (
                "packet\n0-15 \"Source Port\"\n",
                "Parsing failed:  Parse error on line 2, column 6: Expecting token of type ':' but found `\"Source Port\"`.",
                (2, 6),
            ),
            (
                "packet\n0-: \"x\"\n",
                "Parsing failed:  Parse error on line 2, column 3: Expecting token of type 'INT' but found `:`.",
                (2, 3),
            ),
            (
                "packet\n+: \"x\"\n",
                "Parsing failed:  Parse error on line 2, column 2: Expecting token of type 'INT' but found `:`.",
                (2, 2),
            ),
            (
                "packet\n0-15: Source\n",
                "Parsing failed: Lexer error on line 2, column 7: unexpected character: ->S<- at offset: 13, skipped 6 characters. Parse error on line 2, column 13: Expecting token of type 'STRING' but found `\n`.",
                (2, 7),
            ),
            (
                "packet\n\"x\": 0\n",
                "Parsing failed:  Parse error on line 2, column 1: Expecting token of type 'EOF' but found `\"x\"`.",
                (2, 1),
            ),
            (
                "packets\n0: \"x\"\n",
                "Parsing failed: Lexer error on line 1, column 1: unexpected character: ->p<- at offset: 0, skipped 7 characters. Parse error on line 2, column 1: Expecting: one of these possible Token sequences:\n  1. [packet]\n  2. [packet-beta]\nbut found: '0'",
                (1, 1),
            ),
            (
                "packet\n0: \"a\" 1: \"b\"\n",
                "Parsing failed:  Parse error on line 2, column 8: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: '1'",
                (2, 8),
            ),
            (
                "packet\n1.5: \"x\"\n",
                "Parsing failed: Lexer error on line 2, column 1: unexpected character: ->1<- at offset: 7, skipped 2 characters. ",
                (2, 1),
            ),
            (
                "packet\n05: \"x\"\n",
                "Parsing failed:  Parse error on line 2, column 2: Expecting token of type ':' but found `5`.",
                (2, 2),
            ),
            (
                "packet\n0-7: \"x\" 8-15 \"y\"\n",
                "Parsing failed:  Parse error on line 2, column 10: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: '8'",
                (2, 10),
            ),
        ];
        for (text, message, (line, column)) in cases {
            let failure = err(text);
            assert_eq!(failure.message, message, "{text:?}");
            assert_eq!((failure.line, failure.column), (line, column), "{text:?}");
        }
    }

    #[test]
    fn int_backtracks_before_a_dot() {
        let chars: Vec<char> = "123.".chars().collect();
        let lexer = Lexer::new(&chars);
        assert_eq!(lexer.int(0), Some(2));
        let chars: Vec<char> = "1.".chars().collect();
        assert_eq!(Lexer::new(&chars).int(0), None);
        let chars: Vec<char> = "0.".chars().collect();
        assert_eq!(Lexer::new(&chars).int(0), Some(1));
    }
}
