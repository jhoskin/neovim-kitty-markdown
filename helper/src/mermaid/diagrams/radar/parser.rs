//! A hand-written lexer and parser for the language of `radar.langium`
//! plus `common.langium` (`@mermaid-js/parser` 2.0.1, mermaid 12.2.0).
//!
//! Langium builds a Chevrotain lexer from the grammar. For this grammar the
//! token order is the keywords longest first (`radar-beta:`, `radar-beta`,
//! `showLegend`, `graticule`, `curve`, `ticks`, `axis`, `max`, `min`, then
//! the one-character keywords), then the terminals in grammar order with the
//! radar terminal `GRATICULE` before the common ones: `BOOLEAN`,
//! `ACC_DESCR`, `ACC_TITLE`, `TITLE`, `NUMBER`, `STRING`, `ID`, `NEWLINE`,
//! and the hidden `WHITESPACE`, `YAML`, `DIRECTIVE`, `SINGLE_LINE_COMMENT`.
//! Because `WHITESPACE` comes after `TITLE` and the accessibility terminals,
//! their `[\t ]*` prefix is part of the token. Every word keyword carries
//! `ID` as its `LONGER_ALT`, so `axisX` is an `ID`. `RadarTokenBuilder` adds
//! the `(?:(?=%%)|(?!\S))` suffix to `radar-beta`. At each position the
//! first token type that matches wins; a position where none matches is
//! skipped and reported as one lexer error per run of skipped characters.
//!
//! The parser is an LL walk of the `Radar` rule with the error texts of
//! Langium's parser error provider. It reports the first parser error,
//! where Langium's recovering parser can report several. `MermaidParseError`
//! joins both lists as `Parsing failed: <lexer errors> <parser errors>`.
//!
//! The lexer scaffolding mirrors the packet parser, which is private to its
//! module; the two grammars share `common.langium` but nothing else.

use std::fmt::Write;

use crate::mermaid::theme::khroma::js_is_whitespace;

/// The `Radar` AST after Langium's value converters.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ast {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub axes: Vec<Axis>,
    pub curves: Vec<Curve>,
    pub options: Vec<RadarOption>,
}

/// `Axis: name=ID ('[' label=STRING ']')?`.
#[derive(Debug, Clone, PartialEq)]
pub struct Axis {
    pub name: String,
    pub label: Option<String>,
}

/// `Curve: name=ID Label? '{' Entries '}'`.
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    pub name: String,
    pub label: Option<String>,
    pub entries: Vec<Entry>,
    /// 1-based source line of the curve name, for the errors `setCurves`
    /// throws.
    pub line: u32,
}

/// `Entry`: a `NumberEntry` has no axis, a `DetailedEntry` carries the
/// `$refText` of its axis reference.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub axis: Option<String>,
    pub value: f64,
}

/// One `Option` with its converted value.
#[derive(Debug, Clone, PartialEq)]
pub enum RadarOption {
    ShowLegend(bool),
    Ticks(f64),
    Max(f64),
    Min(f64),
    Graticule(String),
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
    RadarBetaColon,
    RadarBeta,
    ShowLegend,
    GraticuleKw,
    CurveKw,
    Ticks,
    AxisKw,
    Max,
    Min,
    Colon,
    Comma,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Graticule,
    Boolean,
    AccDescr,
    AccTitle,
    Title,
    Number,
    Str,
    Id,
    Eof,
}

impl Kind {
    /// Chevrotain's `tokenType.name`: the keyword text for keywords, the
    /// terminal rule name otherwise.
    fn name(self) -> &'static str {
        match self {
            Kind::Newline => "NEWLINE",
            Kind::RadarBetaColon => "radar-beta:",
            Kind::RadarBeta => "radar-beta",
            Kind::ShowLegend => "showLegend",
            Kind::GraticuleKw => "graticule",
            Kind::CurveKw => "curve",
            Kind::Ticks => "ticks",
            Kind::AxisKw => "axis",
            Kind::Max => "max",
            Kind::Min => "min",
            Kind::Colon => ":",
            Kind::Comma => ",",
            Kind::LBracket => "[",
            Kind::RBracket => "]",
            Kind::LBrace => "{",
            Kind::RBrace => "}",
            Kind::Graticule => "GRATICULE",
            Kind::Boolean => "BOOLEAN",
            Kind::AccDescr => "ACC_DESCR",
            Kind::AccTitle => "ACC_TITLE",
            Kind::Title => "TITLE",
            Kind::Number => "NUMBER",
            Kind::Str => "STRING",
            Kind::Id => "ID",
            Kind::Eof => "EOF",
        }
    }

    fn is_title_or_acc(self) -> bool {
        matches!(self, Kind::AccDescr | Kind::AccTitle | Kind::Title)
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

/// The word keywords in Chevrotain's order (longest first, ties in grammar
/// order); each has `ID` as its `LONGER_ALT`.
const WORD_KEYWORDS: [(Kind, &str); 7] = [
    (Kind::ShowLegend, "showLegend"),
    (Kind::GraticuleKw, "graticule"),
    (Kind::CurveKw, "curve"),
    (Kind::Ticks, "ticks"),
    (Kind::AxisKw, "axis"),
    (Kind::Max, "max"),
    (Kind::Min, "min"),
];

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
    /// the `radar-beta` keyword.
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

    /// `[\t ]*accDescr(?:[\t ]*:(?:[^\n\r]*?(?=%%)|[^\n\r]*)|\s*{([^}]*)})`.
    fn acc_descr(&self, i: usize) -> Option<usize> {
        let i = self.blanks(i);
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

    /// `[\t ]*accTitle[\t ]*:(?:[^\n\r]*?(?=%%)|[^\n\r]*)`.
    fn acc_title(&self, i: usize) -> Option<usize> {
        let i = self.blanks(i);
        if !self.starts_with(i, "accTitle") {
            return None;
        }
        let colon = self.blanks(i + "accTitle".len());
        (self.at(colon) == Some(':')).then(|| self.rest_of_line_before_comment(colon + 1))
    }

    /// `[\t ]*title(?:[\t ][^\n\r]*?(?=%%)|[\t ][^\n\r]*|)`.
    fn title(&self, i: usize) -> Option<usize> {
        let i = self.blanks(i);
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

    /// `FLOAT`: `[0-9]+\.[0-9]+(?!\.)`. The first run must reach the dot;
    /// the second backtracks by one digit before a dot, as the regex
    /// engine does.
    fn float(&self, i: usize) -> Option<usize> {
        let int_end = self.digits(i);
        if int_end == i || self.at(int_end) != Some('.') {
            return None;
        }
        let frac_start = int_end + 1;
        let frac_end = self.digits(frac_start);
        if frac_end == frac_start {
            None
        } else if self.at(frac_end) != Some('.') {
            Some(frac_end)
        } else if frac_end - frac_start > 1 {
            Some(frac_end - 1)
        } else {
            None
        }
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

    /// `NUMBER: FLOAT | INT`, inlined into one regex in that order.
    fn number(&self, i: usize) -> Option<usize> {
        self.float(i).or_else(|| self.int(i))
    }

    /// `ID`: `[\w]([-\w]*\w)?` with ASCII `\w`: the longest run of word
    /// characters and dashes that starts and ends with a word character.
    fn id(&self, i: usize) -> Option<usize> {
        let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
        if !self.at(i).is_some_and(word) {
            return None;
        }
        let mut j = i + 1;
        let mut last_word = j;
        while let Some(c) = self.at(j)
            && (word(c) || c == '-')
        {
            j += 1;
            if word(c) {
                last_word = j;
            }
        }
        Some(last_word)
    }

    /// A word keyword, unless its `LONGER_ALT` `ID` matches a longer image.
    fn word_keyword(&self, i: usize, word: &str) -> Option<usize> {
        let end = self.literal(i, word)?;
        match self.id(i) {
            Some(id_end) if id_end > end => None,
            _ => Some(end),
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

    /// `[\t ]*%%{[\S\s]*?}%%(?:\r?\n|(?!\S))`; `WHITESPACE` comes first in
    /// the token order, so the prefix never matters.
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

    /// `[\t ]*%%[^\n\r]*`.
    fn comment(&self, i: usize) -> Option<usize> {
        self.starts_with(i, "%%").then(|| self.line_end(i + 2))
    }

    /// The first token type that matches at `i`, in Chevrotain's order.
    fn next(&self, i: usize) -> Option<(Option<Kind>, usize)> {
        if let Some(end) = self.literal(i, "radar-beta:") {
            return Some((Some(Kind::RadarBetaColon), end));
        }
        if let Some(end) = self
            .literal(i, "radar-beta")
            .filter(|end| self.keyword_boundary(*end))
        {
            return Some((Some(Kind::RadarBeta), end));
        }
        for (kind, word) in WORD_KEYWORDS {
            if let Some(end) = self.word_keyword(i, word) {
                return Some((Some(kind), end));
            }
        }
        let table: [(Option<Kind>, Matcher<'a>); 19] = [
            (Some(Kind::Colon), |l, i| l.literal(i, ":")),
            (Some(Kind::Comma), |l, i| l.literal(i, ",")),
            (Some(Kind::LBracket), |l, i| l.literal(i, "[")),
            (Some(Kind::RBracket), |l, i| l.literal(i, "]")),
            (Some(Kind::LBrace), |l, i| l.literal(i, "{")),
            (Some(Kind::RBrace), |l, i| l.literal(i, "}")),
            (Some(Kind::Graticule), |l, i| {
                l.literal(i, "circle").or_else(|| l.literal(i, "polygon"))
            }),
            (Some(Kind::Boolean), |l, i| {
                l.literal(i, "true").or_else(|| l.literal(i, "false"))
            }),
            (Some(Kind::AccDescr), Lexer::acc_descr),
            (Some(Kind::AccTitle), Lexer::acc_title),
            (Some(Kind::Title), Lexer::title),
            (Some(Kind::Number), Lexer::number),
            (Some(Kind::Str), Lexer::string),
            (Some(Kind::Id), Lexer::id),
            (Some(Kind::Newline), Lexer::newline),
            (HIDDEN, Lexer::whitespace),
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

/// The token sequences Chevrotain lists when neither `Entries` alternative
/// can start; the text does not depend on the input.
const ENTRIES_SEQUENCES: [&str; 8] = [
    "NUMBER",
    "NEWLINE, NUMBER",
    "NEWLINE, NEWLINE, NEWLINE",
    "NEWLINE, NEWLINE, NUMBER",
    "ID",
    "NEWLINE, ID",
    "NEWLINE, NEWLINE, NEWLINE",
    "NEWLINE, NEWLINE, ID",
];

impl Parser<'_> {
    fn peek(&self) -> &Token {
        self.peek_at(0)
    }

    fn peek_at(&self, k: usize) -> &Token {
        &self.tokens[(self.pos + k).min(self.tokens.len() - 1)]
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

    fn skip_newlines(&mut self) {
        while self.peek().kind == Kind::Newline {
            self.advance();
        }
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

    /// `NoViableAltException` from Chevrotain's default error provider,
    /// one line per token sequence.
    fn no_viable_alt(&self, sequences: &[&str]) -> ParseError {
        let t = self.peek();
        let mut message = "Expecting: one of these possible Token sequences:".to_string();
        for (i, seq) in sequences.iter().enumerate() {
            let _ = write!(message, "\n  {}. [{seq}]", i + 1);
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

    /// `fragment TitleAndAccessibilities: ((accDescr=ACC_DESCR |
    /// accTitle=ACC_TITLE | title=TITLE) EOL)+`. The iteration lookahead
    /// reads the `EOL` too, so a title that is not followed by a newline
    /// or the end fails the first iteration with Chevrotain's
    /// `EarlyExitException`.
    fn title_and_accessibilities(&mut self, ast: &mut Ast) -> Result<(), ParseError> {
        let iteration_starts = |p: &Parser<'_>| {
            p.peek().kind.is_title_or_acc()
                && matches!(p.peek_at(1).kind, Kind::Newline | Kind::Eof)
        };
        if !iteration_starts(self) {
            let t = self.peek();
            return Err(ParseError {
                line: t.line,
                column: t.column,
                message: format!(
                    "Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <[ACC_DESCR] ,[ACC_TITLE] ,[TITLE]>\nbut found: '{}'",
                    self.image(t)
                ),
            });
        }
        while iteration_starts(self) {
            let t = self.advance();
            let image = self.image(&t);
            match t.kind {
                Kind::AccDescr => ast.acc_descr = Some(convert_acc_descr(&image)),
                Kind::AccTitle => ast.acc_title = Some(convert_acc_title(&image)),
                _ => ast.title = Some(convert_title(&image)),
            }
            // `EOL: NEWLINE+ | EOF`.
            self.skip_newlines();
        }
        Ok(())
    }

    /// `fragment Label: '[' label=STRING ']'`, when the next token is `[`.
    fn label(&mut self) -> Result<Option<String>, ParseError> {
        if self.peek().kind != Kind::LBracket {
            return Ok(None);
        }
        self.advance();
        let s = self.expect(Kind::Str)?;
        self.expect(Kind::RBracket)?;
        Ok(Some(convert_string(&self.image(&s))))
    }

    /// `Axis: name=ID (Label)?`.
    fn axis(&mut self) -> Result<Axis, ParseError> {
        let name = self.expect(Kind::Id)?;
        let name = self.image(&name);
        let label = self.label()?;
        Ok(Axis { name, label })
    }

    fn number(&mut self) -> Result<f64, ParseError> {
        let t = self.expect(Kind::Number)?;
        Ok(convert_number(&self.image(&t)))
    }

    /// `DetailedEntry: axis=[Axis:ID] ':'? value=NUMBER`.
    fn detailed_entry(&mut self) -> Result<Entry, ParseError> {
        let axis = self.expect(Kind::Id)?;
        let axis = self.image(&axis);
        if self.peek().kind == Kind::Colon {
            self.advance();
        }
        let value = self.number()?;
        Ok(Entry {
            axis: Some(axis),
            value,
        })
    }

    /// `fragment Entries`: a list of `NumberEntry` or a list of
    /// `DetailedEntry`, each with optional newlines around the items. The
    /// alternative is picked by the first token after the newlines.
    fn entries(&mut self) -> Result<Vec<Entry>, ParseError> {
        let mut k = 0;
        while self.peek_at(k).kind == Kind::Newline {
            k += 1;
        }
        let detailed = match self.peek_at(k).kind {
            Kind::Number => false,
            Kind::Id => true,
            _ => return Err(self.no_viable_alt(&ENTRIES_SEQUENCES)),
        };
        let mut entries = Vec::new();
        loop {
            self.skip_newlines();
            if detailed {
                entries.push(self.detailed_entry()?);
            } else {
                entries.push(Entry {
                    axis: None,
                    value: self.number()?,
                });
            }
            if self.peek().kind != Kind::Comma {
                break;
            }
            self.advance();
        }
        self.skip_newlines();
        Ok(entries)
    }

    /// `Curve: name=ID (Label)? '{' Entries '}'`.
    fn curve(&mut self) -> Result<Curve, ParseError> {
        let name = self.expect(Kind::Id)?;
        let line = name.line;
        let name = self.image(&name);
        let label = self.label()?;
        self.expect(Kind::LBrace)?;
        let entries = self.entries()?;
        self.expect(Kind::RBrace)?;
        Ok(Curve {
            name,
            label,
            entries,
            line,
        })
    }

    /// `Option`: a name keyword and the value terminal it takes.
    fn option(&mut self) -> Result<RadarOption, ParseError> {
        let name = self.advance();
        Ok(match name.kind {
            Kind::ShowLegend => {
                let t = self.expect(Kind::Boolean)?;
                // `convertBoolean`: `input.toLowerCase() === 'true'`.
                RadarOption::ShowLegend(self.image(&t) == "true")
            }
            Kind::Ticks => RadarOption::Ticks(self.number()?),
            Kind::Max => RadarOption::Max(self.number()?),
            Kind::Min => RadarOption::Min(self.number()?),
            _ => {
                let t = self.expect(Kind::Graticule)?;
                RadarOption::Graticule(self.image(&t))
            }
        })
    }

    /// The `Radar` entry rule.
    fn radar(&mut self) -> Result<Ast, ParseError> {
        let mut ast = Ast::default();
        self.skip_newlines();
        match self.peek().kind {
            Kind::RadarBeta => {
                self.advance();
                if self.peek().kind == Kind::Colon {
                    self.advance();
                }
            }
            Kind::RadarBetaColon => {
                self.advance();
            }
            _ => {
                return Err(self.no_viable_alt(&["radar-beta", "radar-beta:", "radar-beta, :"]));
            }
        }
        self.skip_newlines();
        loop {
            match self.peek().kind {
                Kind::Newline => {
                    self.advance();
                }
                k if k.is_title_or_acc() => self.title_and_accessibilities(&mut ast)?,
                Kind::AxisKw => {
                    self.advance();
                    ast.axes.push(self.axis()?);
                    while self.peek().kind == Kind::Comma {
                        self.advance();
                        ast.axes.push(self.axis()?);
                    }
                }
                Kind::CurveKw => {
                    self.advance();
                    ast.curves.push(self.curve()?);
                    while self.peek().kind == Kind::Comma {
                        self.advance();
                        ast.curves.push(self.curve()?);
                    }
                }
                Kind::ShowLegend | Kind::Ticks | Kind::Max | Kind::Min | Kind::GraticuleKw => {
                    ast.options.push(self.option()?);
                    while self.peek().kind == Kind::Comma {
                        self.advance();
                        if !matches!(
                            self.peek().kind,
                            Kind::ShowLegend
                                | Kind::Ticks
                                | Kind::Max
                                | Kind::Min
                                | Kind::GraticuleKw
                        ) {
                            return Err(self.no_viable_alt(&[
                                "showLegend",
                                "ticks",
                                "max",
                                "min",
                                "graticule",
                            ]));
                        }
                        ast.options.push(self.option()?);
                    }
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

/// `convertNumber`, `Number(input)` for a `NUMBER` image.
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

/// The image from the first occurrence of `word`, as `RegExp.exec` finds
/// it; the token's `[\t ]*` prefix goes.
fn from_word<'s>(image: &'s str, word: &str) -> &'s str {
    image.find(word).map_or("", |i| &image[i + word.len()..])
}

/// `AbstractMermaidValueConverter.runCommonConverter` with `titleRegex`
/// `/title([\t ][^\n\r]*|)/`: group 1 trimmed, blank runs collapsed.
fn convert_title(image: &str) -> String {
    collapse_blanks(js_trim(from_word(image, "title")))
}

/// `accessibilityTitleRegex` `/accTitle[\t ]*:([^\n\r]*)/`.
fn convert_acc_title(image: &str) -> String {
    let after = from_word(image, "accTitle")
        .split_once(':')
        .map(|(_, r)| r)
        .unwrap_or("");
    collapse_blanks(js_trim(after))
}

/// `accessibilityDescrRegex` `/accDescr(?:[\t ]*:([^\n\r]*)|\s*{([^}]*)})/`:
/// the single-line form like a title, the block form with every line
/// trimmed, blank runs collapsed, and blank lines dropped.
fn convert_acc_descr(image: &str) -> String {
    let after_keyword = from_word(image, "accDescr");
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

/// `parse('radar', text)`: the AST, or the `MermaidParseError` text with
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
    let parsed = parser.radar();
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

    fn axes(ast: &Ast) -> Vec<(&str, Option<&str>)> {
        ast.axes
            .iter()
            .map(|a| (a.name.as_str(), a.label.as_deref()))
            .collect()
    }

    fn entries(curve: &Curve) -> Vec<(Option<&str>, f64)> {
        curve
            .entries
            .iter()
            .map(|e| (e.axis.as_deref(), e.value))
            .collect()
    }

    #[test]
    fn headers_axes_curves_and_options() {
        for text in [
            "radar-beta\n",
            "radar-beta:\n",
            "radar-beta :\n",
            "\n\n  radar-beta\n",
            "%% hi\nradar-beta\n",
            "  radar-beta  %% x\n",
            "radar-beta%%x\n",
        ] {
            assert!(ok(text).axes.is_empty(), "{text:?}");
        }
        let ast = ok("radar-beta\naxis A[\"a\\\"b\"], B['c'], maxim, a---b\n");
        assert_eq!(
            axes(&ast),
            vec![
                ("A", Some("a\"b")),
                ("B", Some("c")),
                ("maxim", None),
                ("a---b", None)
            ]
        );
        let ast = ok("radar-beta\naxis a, b\ncurve c{a 1, b: 2.5}, d[\"D\"]{\n 1,\n\n 2\n}\n");
        assert_eq!(
            entries(&ast.curves[0]),
            vec![(Some("a"), 1.0), (Some("b"), 2.5)]
        );
        assert_eq!(ast.curves[1].label.as_deref(), Some("D"));
        assert_eq!(entries(&ast.curves[1]), vec![(None, 1.0), (None, 2.0)]);
        assert_eq!(ast.curves[1].line, 3);
        let ast = ok("radar-beta\nticks 3, max 10\nshowLegend false\ngraticule polygon\nmin 1.5\n");
        assert_eq!(
            ast.options,
            vec![
                RadarOption::Ticks(3.0),
                RadarOption::Max(10.0),
                RadarOption::ShowLegend(false),
                RadarOption::Graticule("polygon".to_string()),
                RadarOption::Min(1.5),
            ]
        );
    }

    #[test]
    fn titles_take_their_leading_blanks_and_end_a_list() {
        let ast =
            ok("radar-beta\ntitle Hello  World %% c\naccTitle : x  y\naccDescr {\n a\n  b\n}\n");
        assert_eq!(ast.title.as_deref(), Some("Hello World"));
        assert_eq!(ast.acc_title.as_deref(), Some("x y"));
        assert_eq!(ast.acc_descr.as_deref(), Some("a\nb"));
        let ast = ok("radar-beta\naxis A title x\n");
        assert_eq!(ast.title.as_deref(), Some("x"));
        assert_eq!(axes(&ast), vec![("A", None)]);
        assert_eq!(ok("radar-beta title x\n").title.as_deref(), Some("x"));
        assert_eq!(ok("radar-beta\n  title\n").title.as_deref(), Some(""));
    }

    #[test]
    fn number_and_id_follow_the_regex_engine() {
        // `1.5.2`: `NUMBER` fails, `ID` takes the `1`, `.` is skipped, and
        // `5.2` is a `FLOAT`, so the curve has one detailed entry.
        let chars: Vec<char> = "12.34.5".chars().collect();
        let lexer = Lexer::new(&chars);
        assert_eq!(lexer.number(0), Some(4));
        assert_eq!(lexer.id(0), Some(2));
        let chars: Vec<char> = "a-".chars().collect();
        assert_eq!(Lexer::new(&chars).id(0), Some(1));
    }

    /// Every case is the message of the oracle parser (`@mermaid-js/parser`
    /// 2.0.1) cut before its second parser error.
    #[test]
    fn errors_match_the_langium_parser() {
        let cases = [
            (
                "radar-betax\n",
                "Parsing failed:  Parse error on line 1, column 1: Expecting: one of these possible Token sequences:\n  1. [radar-beta]\n  2. [radar-beta:]\n  3. [radar-beta, :]\nbut found: 'radar-betax'",
                (1, 1),
            ),
            (
                "radar-beta\naxis\n",
                "Parsing failed:  Parse error on line 2, column 5: Expecting token of type 'ID' but found `\n`.",
                (2, 5),
            ),
            (
                "radar-beta\naxis A B\n",
                "Parsing failed:  Parse error on line 2, column 8: Expecting token of type 'EOF' but found `B`.",
                (2, 8),
            ),
            (
                "radar-beta\ncurve c1{1, a: 2}\n",
                "Parsing failed:  Parse error on line 2, column 13: Expecting token of type 'NUMBER' but found `a`.",
                (2, 13),
            ),
            (
                "radar-beta\ncurve c1{1 2}\n",
                "Parsing failed:  Parse error on line 2, column 12: Expecting token of type '}' but found `2`.",
                (2, 12),
            ),
            (
                "radar-beta\ncurve {1}\n",
                "Parsing failed:  Parse error on line 2, column 7: Expecting token of type 'ID' but found `{`.",
                (2, 7),
            ),
            (
                "radar-beta\ncurve c1{}\n",
                "Parsing failed:  Parse error on line 2, column 10: Expecting: one of these possible Token sequences:\n  1. [NUMBER]\n  2. [NEWLINE, NUMBER]\n  3. [NEWLINE, NEWLINE, NEWLINE]\n  4. [NEWLINE, NEWLINE, NUMBER]\n  5. [ID]\n  6. [NEWLINE, ID]\n  7. [NEWLINE, NEWLINE, NEWLINE]\n  8. [NEWLINE, NEWLINE, ID]\nbut found: '}'",
                (2, 10),
            ),
            (
                "radar-beta\ncurve c1\n",
                "Parsing failed:  Parse error on line 2, column 9: Expecting token of type '{' but found `\n`.",
                (2, 9),
            ),
            (
                "radar-beta\nticks x\n",
                "Parsing failed:  Parse error on line 2, column 7: Expecting token of type 'NUMBER' but found `x`.",
                (2, 7),
            ),
            (
                "radar-beta\ngraticule square\n",
                "Parsing failed:  Parse error on line 2, column 11: Expecting token of type 'GRATICULE' but found `square`.",
                (2, 11),
            ),
            (
                "radar-beta\nshowLegend maybe\n",
                "Parsing failed:  Parse error on line 2, column 12: Expecting token of type 'BOOLEAN' but found `maybe`.",
                (2, 12),
            ),
            (
                "radar-beta\naxis title\n",
                "Parsing failed:  Parse error on line 2, column 5: Expecting token of type 'ID' but found ` title`.",
                (2, 5),
            ),
            (
                "radar-beta\naxis truex, circlez\n",
                "Parsing failed:  Parse error on line 2, column 6: Expecting token of type 'ID' but found `true`.",
                (2, 6),
            ),
            (
                "radar-beta\naxis a-\n",
                "Parsing failed: Lexer error on line 2, column 7: unexpected character: ->-<- at offset: 17, skipped 1 characters. ",
                (2, 7),
            ),
            (
                "radar-beta\naxis a\"x\"\n",
                "Parsing failed:  Parse error on line 2, column 7: Expecting token of type 'EOF' but found `\"x\"`.",
                (2, 7),
            ),
            (
                "radar-beta\nfoo\n",
                "Parsing failed:  Parse error on line 2, column 1: Expecting token of type 'EOF' but found `foo`.",
                (2, 1),
            ),
            (
                "radar-beta\n  curve c{1.5.2}\n",
                "Parsing failed: Lexer error on line 2, column 12: unexpected character: ->.<- at offset: 22, skipped 1 characters. ",
                (2, 12),
            ),
            (
                "radar-beta\n  curve c{-1}\n",
                "Parsing failed: Lexer error on line 2, column 11: unexpected character: ->-<- at offset: 21, skipped 1 characters. ",
                (2, 11),
            ),
            (
                "radar-beta\n  curve c{01}\n",
                "Parsing failed:  Parse error on line 2, column 12: Expecting token of type '}' but found `1`.",
                (2, 12),
            ),
            (
                "radar-beta\naxis A[\"x\"\n",
                "Parsing failed:  Parse error on line 2, column 11: Expecting token of type ']' but found `\n`.",
                (2, 11),
            ),
            (
                "radar-beta\naxis A[x]\n",
                "Parsing failed:  Parse error on line 2, column 8: Expecting token of type 'STRING' but found `x`.",
                (2, 8),
            ),
            (
                "radar-beta\ncurve c{1,}\n",
                "Parsing failed:  Parse error on line 2, column 11: Expecting token of type 'NUMBER' but found `}`.",
                (2, 11),
            ),
            (
                "radar-beta\ncurve c{a:}\n",
                "Parsing failed:  Parse error on line 2, column 11: Expecting token of type 'NUMBER' but found `}`.",
                (2, 11),
            ),
            (
                "radar-beta\ncurve c{1} d{2}\n",
                "Parsing failed:  Parse error on line 2, column 12: Expecting token of type 'EOF' but found `d`.",
                (2, 12),
            ),
            (
                "radar-beta\nmax\n",
                "Parsing failed:  Parse error on line 2, column 4: Expecting token of type 'NUMBER' but found `\n`.",
                (2, 4),
            ),
            (
                "radar-beta\ncurve c1{\n}\n",
                "Parsing failed:  Parse error on line 2, column 10: Expecting: one of these possible Token sequences:\n  1. [NUMBER]\n  2. [NEWLINE, NUMBER]\n  3. [NEWLINE, NEWLINE, NEWLINE]\n  4. [NEWLINE, NEWLINE, NUMBER]\n  5. [ID]\n  6. [NEWLINE, ID]\n  7. [NEWLINE, NEWLINE, NEWLINE]\n  8. [NEWLINE, NEWLINE, ID]\nbut found: '\n'",
                (2, 10),
            ),
            (
                "radar-beta\ncurve c1{\n\n}\n",
                "Parsing failed:  Parse error on line 2, column 10: Expecting: one of these possible Token sequences:\n  1. [NUMBER]\n  2. [NEWLINE, NUMBER]\n  3. [NEWLINE, NEWLINE, NEWLINE]\n  4. [NEWLINE, NEWLINE, NUMBER]\n  5. [ID]\n  6. [NEWLINE, ID]\n  7. [NEWLINE, NEWLINE, NEWLINE]\n  8. [NEWLINE, NEWLINE, ID]\nbut found: '\n'",
                (2, 10),
            ),
            (
                "radar-beta\ncurve c1{\n\n\n}\n",
                "Parsing failed:  Parse error on line 2, column 10: Expecting: one of these possible Token sequences:\n  1. [NUMBER]\n  2. [NEWLINE, NUMBER]\n  3. [NEWLINE, NEWLINE, NEWLINE]\n  4. [NEWLINE, NEWLINE, NUMBER]\n  5. [ID]\n  6. [NEWLINE, ID]\n  7. [NEWLINE, NEWLINE, NEWLINE]\n  8. [NEWLINE, NEWLINE, ID]\nbut found: '\n'",
                (2, 10),
            ),
            (
                "radar-beta\ncurve c1{,}\n",
                "Parsing failed:  Parse error on line 2, column 10: Expecting: one of these possible Token sequences:\n  1. [NUMBER]\n  2. [NEWLINE, NUMBER]\n  3. [NEWLINE, NEWLINE, NEWLINE]\n  4. [NEWLINE, NEWLINE, NUMBER]\n  5. [ID]\n  6. [NEWLINE, ID]\n  7. [NEWLINE, NEWLINE, NEWLINE]\n  8. [NEWLINE, NEWLINE, ID]\nbut found: ','",
                (2, 10),
            ),
            (
                "radar-beta\naccDescr {a} axis A\n",
                "Parsing failed:  Parse error on line 2, column 1: Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <[ACC_DESCR] ,[ACC_TITLE] ,[TITLE]>\nbut found: 'accDescr {a}'",
                (2, 1),
            ),
            (
                "radar-beta\naxis A, \n",
                "Parsing failed:  Parse error on line 2, column 9: Expecting token of type 'ID' but found `\n`.",
                (2, 9),
            ),
            (
                "radar-beta\naxis A,\nB\n",
                "Parsing failed:  Parse error on line 2, column 8: Expecting token of type 'ID' but found `\n`.",
                (2, 8),
            ),
            (
                "radar-beta\naxis é\n",
                "Parsing failed: Lexer error on line 2, column 6: unexpected character: ->é<- at offset: 16, skipped 1 characters. Parse error on line 2, column 7: Expecting token of type 'ID' but found `\n`.",
                (2, 6),
            ),
            (
                "axis A\n",
                "Parsing failed:  Parse error on line 1, column 1: Expecting: one of these possible Token sequences:\n  1. [radar-beta]\n  2. [radar-beta:]\n  3. [radar-beta, :]\nbut found: 'axis'",
                (1, 1),
            ),
            (
                "radar-beta\nmax 1e3\n",
                "Parsing failed:  Parse error on line 2, column 6: Expecting token of type 'EOF' but found `e3`.",
                (2, 6),
            ),
            (
                "radar-beta\nticks 3, foo\n",
                "Parsing failed:  Parse error on line 2, column 10: Expecting: one of these possible Token sequences:\n  1. [showLegend]\n  2. [ticks]\n  3. [max]\n  4. [min]\n  5. [graticule]\nbut found: 'foo'",
                (2, 10),
            ),
            (
                "radar-beta\nticks 3,\n",
                "Parsing failed:  Parse error on line 2, column 9: Expecting: one of these possible Token sequences:\n  1. [showLegend]\n  2. [ticks]\n  3. [max]\n  4. [min]\n  5. [graticule]\nbut found: '\n'",
                (2, 9),
            ),
            (
                "radar-beta\naxis A[\"x\"] B\n",
                "Parsing failed:  Parse error on line 2, column 13: Expecting token of type 'EOF' but found `B`.",
                (2, 13),
            ),
            (
                "radar-beta\ncurve c{1}, \n",
                "Parsing failed:  Parse error on line 2, column 13: Expecting token of type 'ID' but found `\n`.",
                (2, 13),
            ),
            (
                "radar-beta\ncurve c[\"x\"\n",
                "Parsing failed:  Parse error on line 2, column 12: Expecting token of type ']' but found `\n`.",
                (2, 12),
            ),
            (
                "radar-beta\ncurve c{1}}\n",
                "Parsing failed:  Parse error on line 2, column 11: Expecting token of type 'EOF' but found `}`.",
                (2, 11),
            ),
            (
                "radar-beta\naxis A\ncurve c{A: 1, 2}\n",
                "Parsing failed:  Parse error on line 3, column 15: Expecting token of type 'ID' but found `2`.",
                (3, 15),
            ),
            (
                "radar-beta\ngraticule circles\n",
                "Parsing failed:  Parse error on line 2, column 17: Expecting token of type 'EOF' but found `s`.",
                (2, 17),
            ),
            (
                "radar-beta\n12.34.5\n",
                "Parsing failed:  Parse error on line 2, column 1: Expecting token of type 'EOF' but found `12.3`.",
                (2, 1),
            ),
        ];
        for (text, message, (line, column)) in cases {
            let failure = parse(text)
                .err()
                .unwrap_or_else(|| panic!("{text:?} parsed"));
            assert_eq!(failure.message, message, "{text:?}");
            assert_eq!((failure.line, failure.column), (line, column), "{text:?}");
        }
    }
}
