//! A hand-written lexer and parser for the language of `treemap.langium`
//! (`@mermaid-js/parser` 2.0.1, mermaid 12.2.0). The treemap grammar
//! imports nothing from `common.langium`: it declares its own title and
//! accessibility terminals and its own whitespace.
//!
//! Lexer. Chevrotain tries the token types in the order Langium builds
//! them and the first match wins: `ACC_DESCR`, `ACC_TITLE`, `TITLE`,
//! `TREEMAP_KEYWORD`, `CLASS_DEF`, `STYLE_SEPARATOR`, `SEPARATOR`,
//! `COMMA`, `INDENTATION`, the hidden `WS` (never reached, `INDENTATION`
//! has the same pattern), the hidden `ML_COMMENT`, the hidden `NL`, `ID2`,
//! `NUMBER2`, `STRING2`. So every run of blanks is an `INDENTATION` token,
//! trailing blanks included, and the title terminals swallow the blanks
//! before them. A position where nothing matches is skipped and reported as
//! one lexer error per run of skipped characters.
//!
//! Parser. An LL walk of `Treemap` with the error recovery of Chevrotain's
//! recovering parser as it plays out on this grammar, so the error list
//! matches the oracle's:
//!
//! - The `(TitleAndAccessibilities | TreemapRow)*` loop exits on a token
//!   that cannot start an iteration. When that token is not the end and
//!   the token after it is not the end either, the in-repetition recovery
//!   reports `Expecting token of type 'EOF' but found ...`, skips to the
//!   next token that starts an iteration, and goes on. Otherwise the loop
//!   ends and the leftover input is `Expecting end of file but found ...`.
//! - An error inside a row (no `STRING2` or `CLASS_DEF` where an item must
//!   start, no `ID2` after `:::`) re-syncs to the next token that can start
//!   a row, or abandons the parse at the end of the input.
//! - `Leaf` versus `Section` is decided by the LL(*) prediction: a leaf
//!   when the whole `STRING2 INDENTATION? (':' | ',') INDENTATION? NUMBER2`
//!   prefix is there, a section otherwise.
//!
//! `MermaidParseError` joins both lists as `Parsing failed: <lexer errors>
//! <parser errors>`.

use std::fmt::Write;

use crate::mermaid::theme::khroma::js_is_whitespace;

/// The `Treemap` AST after the value converters.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ast {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub rows: Vec<Row>,
}

/// One `TreemapRow`. A `ClassDef` row is replaced by the
/// `ClassDefStatement` object the value converter returns.
#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    Item {
        /// `INDENTATION` converted to its length in characters.
        indent: Option<usize>,
        item: Item,
    },
    ClassDef {
        class_name: String,
        style_text: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub kind: ItemKind,
    pub name: String,
    /// `NUMBER2` through `parseFloat` with the commas removed; `NaN` when
    /// no digits lead.
    pub value: Option<f64>,
    pub class_selector: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Leaf,
    Section,
}

/// What `MermaidParseError` carries: the joined message and the position of
/// the first error (lexer errors first, as mermaid prints them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseFailure {
    pub message: String,
    pub line: u32,
    /// 1-based; 0 when Chevrotain has no position (the EOF token).
    pub column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    AccDescr,
    AccTitle,
    Title,
    Keyword,
    ClassDef,
    StyleSeparator,
    Separator,
    Comma,
    Indentation,
    Id,
    Number,
    Str,
    Eof,
}

impl Kind {
    /// Chevrotain's `tokenType.name`.
    fn name(self) -> &'static str {
        match self {
            Kind::AccDescr => "ACC_DESCR",
            Kind::AccTitle => "ACC_TITLE",
            Kind::Title => "TITLE",
            Kind::Keyword => "TREEMAP_KEYWORD",
            Kind::ClassDef => "CLASS_DEF",
            Kind::StyleSeparator => "STYLE_SEPARATOR",
            Kind::Separator => "SEPARATOR",
            Kind::Comma => "COMMA",
            Kind::Indentation => "INDENTATION",
            Kind::Id => "ID2",
            Kind::Number => "NUMBER2",
            Kind::Str => "STRING2",
            Kind::Eof => "EOF",
        }
    }

    fn is_title(self) -> bool {
        matches!(self, Kind::AccDescr | Kind::AccTitle | Kind::Title)
    }

    /// The tokens that start an iteration of the `Treemap` loop: a title
    /// terminal or the first token of a `TreemapRow`.
    fn starts_iteration(self) -> bool {
        self.is_title() || matches!(self, Kind::Indentation | Kind::Str | Kind::ClassDef)
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

    fn literal(&self, i: usize, word: &str) -> Option<usize> {
        self.starts_with(i, word).then(|| i + word.chars().count())
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

    /// `[\t ]*`.
    fn blanks(&self, i: usize) -> usize {
        let mut j = i;
        while matches!(self.at(j), Some(' ') | Some('\t')) {
            j += 1;
        }
        j
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

    /// `[\t ]*accDescr(?:[\t ]*:([^\n\r]*?(?=%%)|[^\n\r]*)|\s*{([^}]*)})`.
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

    /// `treemap-beta|treemap`.
    fn keyword(&self, i: usize) -> Option<usize> {
        self.literal(i, "treemap-beta")
            .or_else(|| self.literal(i, "treemap"))
    }

    fn is_word(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '_'
    }

    /// `classDef\s+([a-zA-Z_][a-zA-Z0-9_]+)(?:\s+([^;\r\n]*))?(?:;)?`. Every
    /// quantifier is greedy and nothing after it can fail, so the first
    /// try is the match: `\s` spans line breaks, and a style text can start
    /// on the next line.
    fn class_def(&self, i: usize) -> Option<usize> {
        let j = self.literal(i, "classDef")?;
        let name = self.js_spaces(j);
        if name == j {
            return None;
        }
        if !self
            .at(name)
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        {
            return None;
        }
        let mut end = name + 1;
        while self.at(end).is_some_and(Lexer::is_word) {
            end += 1;
        }
        if end - name < 2 {
            return None;
        }
        let spaces = self.js_spaces(end);
        if spaces > end {
            end = spaces;
            while let Some(c) = self.at(end)
                && !matches!(c, ';' | '\r' | '\n')
            {
                end += 1;
            }
        }
        if self.at(end) == Some(';') {
            end += 1;
        }
        Some(end)
    }

    /// `[ \t]{1,}`.
    fn indentation(&self, i: usize) -> Option<usize> {
        let j = self.blanks(i);
        (j > i).then_some(j)
    }

    /// `\%\%[^\n]*`: a lone `\r` does not end the comment.
    fn comment(&self, i: usize) -> Option<usize> {
        if !self.starts_with(i, "%%") {
            return None;
        }
        let mut j = i + 2;
        while self.at(j).is_some_and(|c| c != '\n') {
            j += 1;
        }
        Some(j)
    }

    /// `\r?\n`.
    fn newline(&self, i: usize) -> Option<usize> {
        match self.at(i) {
            Some('\n') => Some(i + 1),
            Some('\r') if self.at(i + 1) == Some('\n') => Some(i + 2),
            _ => None,
        }
    }

    /// `[a-zA-Z_][a-zA-Z0-9_]*`.
    fn id(&self, i: usize) -> Option<usize> {
        if !self
            .at(i)
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        {
            return None;
        }
        let mut j = i + 1;
        while self.at(j).is_some_and(Lexer::is_word) {
            j += 1;
        }
        Some(j)
    }

    /// `[0-9_\.\,]+`.
    fn number(&self, i: usize) -> Option<usize> {
        let mut j = i;
        while self
            .at(j)
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, '_' | '.' | ','))
        {
            j += 1;
        }
        (j > i).then_some(j)
    }

    /// `"[^"]*"|'[^']*'`: line breaks included.
    fn string(&self, i: usize) -> Option<usize> {
        let quote = self.at(i)?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let mut j = i + 1;
        loop {
            if self.at(j)? == quote {
                return Some(j + 1);
            }
            j += 1;
        }
    }

    /// The first token type that matches at `i`, in Chevrotain's order.
    fn next(&self, i: usize) -> Option<(Option<Kind>, usize)> {
        let table: [(Option<Kind>, Matcher<'a>); 15] = [
            (Some(Kind::AccDescr), Lexer::acc_descr),
            (Some(Kind::AccTitle), Lexer::acc_title),
            (Some(Kind::Title), Lexer::title),
            (Some(Kind::Keyword), Lexer::keyword),
            (Some(Kind::ClassDef), Lexer::class_def),
            (Some(Kind::StyleSeparator), |l, i| l.literal(i, ":::")),
            (Some(Kind::Separator), |l, i| l.literal(i, ":")),
            (Some(Kind::Comma), |l, i| l.literal(i, ",")),
            (Some(Kind::Indentation), Lexer::indentation),
            (HIDDEN, Lexer::indentation),
            (HIDDEN, Lexer::comment),
            (HIDDEN, Lexer::newline),
            (Some(Kind::Id), Lexer::id),
            (Some(Kind::Number), Lexer::number),
            (Some(Kind::Str), Lexer::string),
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

/// A recognition exception on its way up the rule stack; its error is
/// already recorded.
struct Thrown;

struct Parser<'a> {
    chars: &'a [char],
    tokens: Vec<Token>,
    pos: usize,
    errors: Vec<ParseError>,
}

/// `buildNoViableAltMessage` for the `Item | ClassDef` alternatives.
const ROW_ALTERNATIVES: &str =
    "Expecting: one of these possible Token sequences:\n  1. [STRING2]\n  2. [CLASS_DEF]";

impl Parser<'_> {
    fn la(&self, k: usize) -> &Token {
        let i = (self.pos + k - 1).min(self.tokens.len() - 1);
        &self.tokens[i]
    }

    fn kind(&self, k: usize) -> Kind {
        self.la(k).kind
    }

    fn image(&self, t: &Token) -> String {
        self.chars[t.start..t.end].iter().collect()
    }

    fn advance(&mut self) -> Token {
        let t = self.la(1).clone();
        if t.kind != Kind::Eof {
            self.pos += 1;
        }
        t
    }

    fn error_at(&mut self, token: &Token, message: String) {
        self.errors.push(ParseError {
            line: token.line,
            column: token.column,
            message,
        });
    }

    /// Langium's `buildMismatchTokenMessage`.
    fn mismatch(&mut self, expected: Kind) {
        let t = self.la(1).clone();
        let message = format!(
            "Expecting token of type '{}' but found `{}`.",
            expected.name(),
            self.image(&t)
        );
        self.error_at(&t, message);
    }

    fn consume(&mut self, kind: Kind) -> Result<Token, Thrown> {
        if self.kind(1) == kind {
            Ok(self.advance())
        } else {
            self.mismatch(kind);
            Err(Thrown)
        }
    }

    /// `Treemap: TREEMAP_KEYWORD (TitleAndAccessibilities | TreemapRow)*`.
    fn treemap(&mut self) -> Ast {
        let mut ast = Ast::default();
        // The top rule catches what its rules rethrow and re-syncs to the
        // end of the input.
        if self.consume(Kind::Keyword).is_err() || self.repetition(&mut ast).is_err() {
            while self.kind(1) != Kind::Eof {
                self.advance();
            }
        }
        if self.kind(1) != Kind::Eof {
            let t = self.la(1).clone();
            let message = format!("Expecting end of file but found `{}`.", self.image(&t));
            self.error_at(&t, message);
        }
        ast
    }

    /// The `*` loop with Chevrotain's in-repetition recovery at its exit.
    fn repetition(&mut self, ast: &mut Ast) -> Result<(), Thrown> {
        loop {
            while self.kind(1).starts_iteration() {
                if self.kind(1).is_title() {
                    self.titles(ast);
                } else {
                    self.row(ast)?;
                }
            }
            // `shouldInRepetitionRecoveryBeTried`: not at the end, and the
            // offending token cannot simply be deleted before the end.
            if self.kind(1) == Kind::Eof || self.kind(2) == Kind::Eof {
                return Ok(());
            }
            let offending = self.la(1).clone();
            let message = format!(
                "Expecting token of type 'EOF' but found `{}`.",
                self.image(&offending)
            );
            self.error_at(&offending, message);
            while self.kind(1) != Kind::Eof && !self.kind(1).starts_iteration() {
                self.advance();
            }
        }
    }

    /// `TitleAndAccessibilities`: `(accDescr=ACC_DESCR | accTitle=ACC_TITLE
    /// | title=TITLE)+`, the last of each kind winning.
    fn titles(&mut self, ast: &mut Ast) {
        while self.kind(1).is_title() {
            let t = self.advance();
            let image = self.image(&t);
            match t.kind {
                Kind::AccDescr => ast.acc_descr = Some(convert_acc_descr(&image)),
                Kind::AccTitle => ast.acc_title = Some(convert_acc_title(&image)),
                _ => ast.title = Some(convert_title(&image)),
            }
        }
    }

    /// `TreemapRow: indent=INDENTATION? (item=Item | ClassDef)` with the
    /// rule's re-sync: on an error, skip to the next token that can start
    /// a row, or rethrow when only the end is left.
    fn row(&mut self, ast: &mut Ast) -> Result<(), Thrown> {
        match self.row_inner() {
            Ok(row) => {
                ast.rows.push(row);
                Ok(())
            }
            Err(Thrown) => {
                while self.kind(1) != Kind::Eof && !self.kind(1).starts_iteration() {
                    self.advance();
                }
                // The rule's follow set is the loop's first set; the end
                // of the input belongs to the top rule only.
                if self.kind(1).starts_iteration() {
                    Ok(())
                } else {
                    Err(Thrown)
                }
            }
        }
    }

    fn row_inner(&mut self) -> Result<Row, Thrown> {
        let indent = if self.kind(1) == Kind::Indentation {
            let t = self.advance();
            Some(t.end - t.start)
        } else {
            None
        };
        match self.kind(1) {
            Kind::Str => Ok(Row::Item {
                indent,
                item: self.item()?,
            }),
            Kind::ClassDef => {
                let t = self.advance();
                let (class_name, style_text) = convert_class_def(&self.image(&t));
                Ok(Row::ClassDef {
                    class_name,
                    style_text,
                })
            }
            _ => {
                let t = self.la(1).clone();
                let message = format!("{ROW_ALTERNATIVES}\nbut found: '{}'", self.image(&t));
                self.error_at(&t, message);
                Err(Thrown)
            }
        }
    }

    /// Whether the tokens from `LA(1)` (a `STRING2`) spell a whole leaf
    /// prefix: `STRING2 INDENTATION? (SEPARATOR | COMMA) INDENTATION?
    /// NUMBER2`.
    fn predicts_leaf(&self) -> bool {
        let mut k = 2;
        if self.kind(k) == Kind::Indentation {
            k += 1;
        }
        if !matches!(self.kind(k), Kind::Separator | Kind::Comma) {
            return false;
        }
        k += 1;
        if self.kind(k) == Kind::Indentation {
            k += 1;
        }
        self.kind(k) == Kind::Number
    }

    /// `Item: Leaf | Section`.
    fn item(&mut self) -> Result<Item, Thrown> {
        let leaf = self.predicts_leaf();
        let name_token = self.advance();
        let name = convert_quoted(&self.image(&name_token));
        let mut value = None;
        if leaf {
            if self.kind(1) == Kind::Indentation {
                self.advance();
            }
            self.advance();
            if self.kind(1) == Kind::Indentation {
                self.advance();
            }
            let number = self.advance();
            value = Some(convert_number(&self.image(&number)));
        }
        let mut class_selector = None;
        if self.kind(1) == Kind::StyleSeparator {
            self.advance();
            let id = self.consume(Kind::Id)?;
            class_selector = Some(self.image(&id));
        }
        Ok(Item {
            kind: if leaf {
                ItemKind::Leaf
            } else {
                ItemKind::Section
            },
            name,
            value,
            class_selector,
        })
    }
}

/// `STRING2`: `input.substring(1, input.length - 1)`, no escapes.
fn convert_quoted(image: &str) -> String {
    let chars: Vec<char> = image.chars().collect();
    if chars.len() < 2 {
        return String::new();
    }
    chars[1..chars.len() - 1].iter().collect()
}

/// `NUMBER2`: `parseFloat(input.replace(/,/g, ''))`. The image holds only
/// digits, `_`, `.`, and `,`, so the parsed prefix is `\d*(\.\d*)?` with at
/// least one digit.
fn convert_number(image: &str) -> f64 {
    let s: String = image.chars().filter(|c| *c != ',').collect();
    let int: String = s.chars().take_while(char::is_ascii_digit).collect();
    let rest = &s[int.len()..];
    let frac: String = rest
        .strip_prefix('.')
        .map(|r| r.chars().take_while(char::is_ascii_digit).collect())
        .unwrap_or_default();
    if int.is_empty() && frac.is_empty() {
        return f64::NAN;
    }
    format!("{}.{}", if int.is_empty() { "0" } else { &int }, frac)
        .trim_end_matches('.')
        .parse()
        .unwrap_or(f64::NAN)
}

/// `classDefRegex` `/classDef\s+([A-Z_a-z]\w+)(?:\s+([^\n\r;]*))?;?/` on
/// the token image: the class name and the style text when it is not
/// empty (`match[2] || undefined`).
fn convert_class_def(image: &str) -> (String, Option<String>) {
    let rest = image["classDef".len()..].trim_start_matches(js_is_whitespace);
    let name_len = rest
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_'))
        .map_or(rest.len(), |(i, _)| i);
    let name = rest[..name_len].to_string();
    let after = &rest[name_len..];
    let spaced = after.trim_start_matches(js_is_whitespace);
    let style = if spaced.len() < after.len() {
        let end = spaced.find([';', '\r', '\n']).unwrap_or(spaced.len());
        Some(spaced[..end].to_string()).filter(|s| !s.is_empty())
    } else {
        None
    };
    (name, style)
}

/// `[\t ]{2,}` to one space.
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

/// `AbstractMermaidValueConverter` with `/title([\t ][^\n\r]*|)/`: group 1
/// trimmed, blank runs collapsed. The image may start with blanks.
fn convert_title(image: &str) -> String {
    let start = image.find("title").map_or(0, |i| i + "title".len());
    collapse_blanks(js_trim(&image[start..]))
}

/// `/accTitle[\t ]*:([^\n\r]*)/`.
fn convert_acc_title(image: &str) -> String {
    let after = image.split_once(':').map(|(_, r)| r).unwrap_or("");
    collapse_blanks(js_trim(after))
}

/// `/accDescr(?:[\t ]*:([^\n\r]*)|\s*{([^}]*)})/`: the single-line form
/// like a title, the block form with every line trimmed, blank runs
/// collapsed, and blank lines dropped.
fn convert_acc_descr(image: &str) -> String {
    let start = image.find("accDescr").map_or(0, |i| i + "accDescr".len());
    let after_keyword = &image[start..];
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

/// `parse('treemap', text)`: the AST, or the `MermaidParseError` text with
/// the position of its first error.
pub fn parse(text: &str) -> Result<Ast, ParseFailure> {
    let chars: Vec<char> = text.chars().collect();
    let lexer = Lexer::new(&chars);
    let (tokens, lex_errors) = lexer.tokenize();
    let mut parser = Parser {
        chars: &chars,
        tokens,
        pos: 0,
        errors: Vec::new(),
    };
    let ast = parser.treemap();
    if lex_errors.is_empty() && parser.errors.is_empty() {
        return Ok(ast);
    }
    let mut lexer_text = String::new();
    for (i, e) in lex_errors.iter().enumerate() {
        if i > 0 {
            lexer_text.push('\n');
        }
        let _ = write!(
            lexer_text,
            "Lexer error on line {}, column {}: unexpected character: ->{}<- at offset: {}, skipped {} characters.",
            e.line, e.column, e.first, e.offset, e.skipped
        );
    }
    let parser_text = parser
        .errors
        .iter()
        .map(|e| {
            format!(
                "Parse error on line {}, column {}: {}",
                location(e.line),
                location(e.column),
                e.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let (line, column) = match (lex_errors.first(), parser.errors.first()) {
        (Some(e), _) => (e.line, e.column),
        (None, Some(e)) => (e.line, e.column),
        (None, None) => (0, 0),
    };
    Err(ParseFailure {
        message: format!("Parsing failed: {lexer_text} {parser_text}"),
        line,
        column,
    })
}

/// Chevrotain marks a missing position (the EOF token) with `NaN`, which
/// `MermaidParseError` prints as `?`.
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

    fn message(text: &str) -> String {
        parse(text)
            .err()
            .unwrap_or_else(|| panic!("{text:?} parsed"))
            .message
    }

    fn leaf(name: &str, value: f64, indent: usize) -> Row {
        Row::Item {
            indent: Some(indent),
            item: Item {
                kind: ItemKind::Leaf,
                name: name.to_string(),
                value: Some(value),
                class_selector: None,
            },
        }
    }

    fn section(name: &str, indent: Option<usize>, class: Option<&str>) -> Row {
        Row::Item {
            indent,
            item: Item {
                kind: ItemKind::Section,
                name: name.to_string(),
                value: None,
                class_selector: class.map(str::to_string),
            },
        }
    }

    #[test]
    fn docs_styling_example() {
        let ast = ok(
            "treemap-beta\n\"Section 1\"\n    \"Leaf 1.1\": 12\n    \"Section 1.2\":::class1\n      \"Leaf 1.2.1\": 12\n\nclassDef class1 fill:red,color:blue,stroke:#FFD600;\n",
        );
        assert_eq!(ast.rows.len(), 5);
        assert_eq!(ast.rows[0], section("Section 1", None, None));
        assert_eq!(ast.rows[1], leaf("Leaf 1.1", 12.0, 4));
        assert_eq!(ast.rows[2], section("Section 1.2", Some(4), Some("class1")));
        assert_eq!(ast.rows[3], leaf("Leaf 1.2.1", 12.0, 6));
        assert_eq!(
            ast.rows[4],
            Row::ClassDef {
                class_name: "class1".to_string(),
                style_text: Some("fill:red,color:blue,stroke:#FFD600".to_string()),
            }
        );
    }

    #[test]
    fn titles_numbers_and_separators() {
        let ast = ok(
            "treemap\ntitle My  title\naccTitle: at\naccDescr: ad\n\"A\"\n  \"B\": 1,000.5\n  'C' , 2\n  \"D\": 1_000\n  \"E\": ..\n",
        );
        assert_eq!(ast.title.as_deref(), Some("My title"));
        assert_eq!(ast.acc_title.as_deref(), Some("at"));
        assert_eq!(ast.acc_descr.as_deref(), Some("ad"));
        assert_eq!(ast.rows[1], leaf("B", 1000.5, 2));
        assert_eq!(ast.rows[2], leaf("C", 2.0, 2));
        assert_eq!(ast.rows[3], leaf("D", 1.0, 2));
        let Row::Item { item, .. } = &ast.rows[4] else {
            panic!("item");
        };
        assert!(item.value.is_some_and(f64::is_nan));
    }

    #[test]
    fn a_title_terminal_takes_the_blanks_before_it() {
        let ast = ok("treemap\n\"A\" title x\n");
        assert_eq!(ast.title.as_deref(), Some("x"));
        assert_eq!(ast.rows, vec![section("A", None, None)]);
        let ast = ok("treemap \"A\"\n  \"B\": 1\n");
        assert_eq!(ast.rows[0], section("A", Some(1), None));
    }

    #[test]
    fn a_class_def_without_style_takes_the_next_line() {
        let ast = ok("treemap\nclassDef foo\n\"A\"\n  \"B\": 1\n");
        assert_eq!(
            ast.rows[0],
            Row::ClassDef {
                class_name: "foo".to_string(),
                style_text: Some("\"A\"".to_string()),
            }
        );
    }

    #[test]
    fn errors_match_the_oracle_text() {
        let alternatives = "Expecting: one of these possible Token sequences:\n  1. [STRING2]\n  2. [CLASS_DEF]\nbut found:";
        assert_eq!(
            message("treemap\n\"A\"  \n  \"B\": 1\n"),
            format!("Parsing failed:  Parse error on line 3, column 1: {alternatives} '  '")
        );
        assert_eq!(
            message("treemap\n\"A\"\n  \"B\": x\n"),
            format!(
                "Parsing failed:  Parse error on line 3, column 6: Expecting token of type 'EOF' but found `:`.\nParse error on line 3, column 8: {alternatives} 'x'"
            )
        );
        assert_eq!(
            message("treemap\nfoo\n"),
            "Parsing failed:  Parse error on line 2, column 1: Expecting end of file but found `foo`."
        );
        assert_eq!(
            message("treemap\n\"A\":::\n"),
            "Parsing failed:  Parse error on line ?, column ?: Expecting token of type 'ID2' but found ``."
        );
        assert_eq!(
            message("treemap\n\"A\"\n  \"B\": 1\nclassDef x fill:red\n"),
            format!(
                "Parsing failed:  Parse error on line 4, column 1: Expecting token of type 'EOF' but found `classDef`.\nParse error on line 4, column 10: {alternatives} 'x'\nParse error on line 4, column 12: {alternatives} 'fill'"
            )
        );
        assert_eq!(
            message("treemap\n\"A\"\n  \"B\": 1:::\"x\"y\n"),
            "Parsing failed:  Parse error on line 3, column 12: Expecting token of type 'ID2' but found `\"x\"`.\nParse error on line 3, column 15: Expecting end of file but found `y`."
        );
        assert_eq!(
            message("treemap\n\"A\n"),
            "Parsing failed: Lexer error on line 2, column 1: unexpected character: ->\"<- at offset: 8, skipped 1 characters. Parse error on line 2, column 2: Expecting end of file but found `A`."
        );
        assert_eq!(
            message("treemap\ntitle T\nfoo title X\n\"A\"\n"),
            "Parsing failed:  Parse error on line 3, column 1: Expecting token of type 'EOF' but found `foo`."
        );
        assert_eq!(
            message("treemap\n\"A\"\n\t\"B\": 3 %% comment\n  %% c2\n  \"C\": 4\n"),
            format!(
                "Parsing failed:  Parse error on line 4, column 1: {alternatives} '  '\nParse error on line 5, column 1: {alternatives} '  '"
            )
        );
    }

    #[test]
    fn a_bare_keyword_is_an_empty_treemap() {
        assert!(ok("treemap\n").rows.is_empty());
        assert!(ok("treemap").rows.is_empty());
    }
}
