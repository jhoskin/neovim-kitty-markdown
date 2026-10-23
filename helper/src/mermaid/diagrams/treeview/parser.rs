//! A hand-written lexer and parser for the language of `treeView.langium`
//! (`@mermaid-js/parser` 2.0.1, mermaid 12.2.0). The grammar imports
//! nothing from `common.langium`: it declares its own title and
//! accessibility terminals and its own whitespace.
//!
//! Lexer. `TokenBuilder.buildTokens` gives this order, and at each position
//! the first token type that matches wins: the keyword `treeView-beta`
//! (with the `(?:(?=%%)|(?!\S))` suffix `AbstractMermaidTokenBuilder` adds
//! and `BARE_NAME` as its `LONGER_ALT`), `ACC_DESCR`, `ACC_TITLE`, `TITLE`,
//! `CLASS_ANNOTATION`, `ICON_ANNOTATION`, `DESC_ANNOTATION`, `INDENTATION`,
//! `QUOTED_NAME`, the hidden `WS` (never reached, `INDENTATION` has the
//! same pattern), the hidden `ML_COMMENT`, the hidden `NL`, `BARE_NAME`.
//! So every run of blanks that does not start an annotation is an
//! `INDENTATION`, the blanks after a quoted name included, and the title
//! terminals take the blanks before them. `BARE_NAME` runs to the end of
//! the line or to the first annotation, so a bare name keeps its inner
//! blanks and a `%%` inside it. A position where nothing matches is skipped
//! and reported as one lexer error per run of skipped characters.
//!
//! Parser. An LL walk of `TreeView` with the error recovery of
//! Chevrotain's recovering parser as it plays out on this grammar, so the
//! error list matches the oracle's:
//!
//! - A missing keyword (the first line is longer than `treeView-beta`, so
//!   it lexes as one `BARE_NAME`) is a mismatch error, and single-token
//!   insertion goes on as if the keyword were there.
//! - The `TreeNode*` loop exits on a token that cannot start a node. When
//!   that token is not the end and the token after it is not the end
//!   either, the in-repetition recovery reports `Expecting token of type
//!   'EOF' but found ...`, skips to the next token that starts a node, and
//!   goes on. Otherwise the loop ends and the leftover input is `Expecting
//!   end of file but found ...`.
//! - An `INDENTATION` without a name after it is a no-viable-alternative
//!   error. The rule re-syncs to the next `INDENTATION`, `QUOTED_NAME`, or
//!   `BARE_NAME`, skipping titles and keywords, or abandons the parse at
//!   the end of the input.
//!
//! `MermaidParseError` joins both lists as `Parsing failed: <lexer errors>
//! <parser errors>`.

use std::fmt::Write;

use crate::mermaid::theme::khroma::js_is_whitespace;

/// The `TreeView` AST after the value converters.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ast {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub nodes: Vec<TreeNode>,
}

/// One `TreeNode`. Each annotation keeps the last value the line assigns.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TreeNode {
    /// `INDENTATION` converted to its length in characters.
    pub indent: Option<usize>,
    pub name: String,
    pub class_annotation: Option<String>,
    pub icon_annotation: Option<String>,
    pub desc_annotation: Option<String>,
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
    Keyword,
    AccDescr,
    AccTitle,
    Title,
    ClassAnnotation,
    IconAnnotation,
    DescAnnotation,
    Indentation,
    QuotedName,
    BareName,
    Eof,
}

impl Kind {
    /// Chevrotain's `tokenType.name`.
    fn name(self) -> &'static str {
        match self {
            Kind::Keyword => "treeView-beta",
            Kind::AccDescr => "ACC_DESCR",
            Kind::AccTitle => "ACC_TITLE",
            Kind::Title => "TITLE",
            Kind::ClassAnnotation => "CLASS_ANNOTATION",
            Kind::IconAnnotation => "ICON_ANNOTATION",
            Kind::DescAnnotation => "DESC_ANNOTATION",
            Kind::Indentation => "INDENTATION",
            Kind::QuotedName => "QUOTED_NAME",
            Kind::BareName => "BARE_NAME",
            Kind::Eof => "EOF",
        }
    }

    fn is_title(self) -> bool {
        matches!(self, Kind::AccDescr | Kind::AccTitle | Kind::Title)
    }

    fn is_name(self) -> bool {
        matches!(self, Kind::QuotedName | Kind::BareName)
    }

    /// The first set of `TreeNode`, which is also its follow set inside
    /// the `TreeNode*` loop.
    fn starts_node(self) -> bool {
        self.is_name() || self == Kind::Indentation
    }

    fn is_annotation(self) -> bool {
        matches!(
            self,
            Kind::ClassAnnotation | Kind::IconAnnotation | Kind::DescAnnotation
        )
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

const KEYWORD: &str = "treeView-beta";

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

    fn is_blank(c: Option<char>) -> bool {
        matches!(c, Some(' ') | Some('\t'))
    }

    /// `\w` in a JS regex without the `u` flag.
    fn is_word(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '_'
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
        while Lexer::is_blank(self.at(j)) {
            j += 1;
        }
        j
    }

    /// `[ \t]+` followed by what `after` accepts there.
    fn blanks_then(&self, i: usize, after: impl Fn(usize) -> Option<usize>) -> Option<usize> {
        let j = self.blanks(i);
        if j == i {
            return None;
        }
        after(j)
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

    /// `treeView-beta(?:(?=%%)|(?!\S))`.
    fn keyword(&self, i: usize) -> Option<usize> {
        if !self.starts_with(i, KEYWORD) {
            return None;
        }
        let end = i + KEYWORD.len();
        let ok = self.starts_with(end, "%%") || self.at(end).is_none_or(js_is_whitespace);
        ok.then_some(end)
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

    /// `[\t ]*title(?:[\t ][^\n\r]*?(?=%%)|[\t ][^\n\r]*|)`: no word
    /// boundary, so `title.md` lexes as a `TITLE` and a `BARE_NAME`.
    fn title(&self, i: usize) -> Option<usize> {
        let i = self.blanks(i);
        if !self.starts_with(i, "title") {
            return None;
        }
        let j = i + "title".len();
        if Lexer::is_blank(self.at(j)) {
            Some(self.rest_of_line_before_comment(j + 1))
        } else {
            Some(j)
        }
    }

    /// `:::[ \t]*[A-Za-z_][\w-]*` from `i`.
    fn class_body(&self, i: usize) -> Option<usize> {
        if !self.starts_with(i, ":::") {
            return None;
        }
        let j = self.blanks(i + 3);
        if !self
            .at(j)
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        {
            return None;
        }
        let mut k = j + 1;
        while self.at(k).is_some_and(|c| Lexer::is_word(c) || c == '-') {
            k += 1;
        }
        Some(k)
    }

    /// `[ \t]+:::[ \t]*[A-Za-z_][\w-]*`.
    fn class_annotation(&self, i: usize) -> Option<usize> {
        self.blanks_then(i, |j| self.class_body(j))
    }

    /// `icon\([\w-]*(?::[\w-]+)?\)` from `i`. `:` is not in `[\w-]`, so the
    /// greedy runs need no backtracking.
    fn icon_body(&self, i: usize) -> Option<usize> {
        if !self.starts_with(i, "icon(") {
            return None;
        }
        let word = |k: usize| self.at(k).is_some_and(|c| Lexer::is_word(c) || c == '-');
        let mut k = i + 5;
        while word(k) {
            k += 1;
        }
        if self.at(k) == Some(':') && word(k + 1) {
            k += 1;
            while word(k) {
                k += 1;
            }
        }
        (self.at(k) == Some(')')).then_some(k + 1)
    }

    /// `[ \t]+icon\([\w-]*(?::[\w-]+)?\)`.
    fn icon_annotation(&self, i: usize) -> Option<usize> {
        self.blanks_then(i, |j| self.icon_body(j))
    }

    /// `[ \t]+##[^\n\r]*`.
    fn desc_annotation(&self, i: usize) -> Option<usize> {
        self.blanks_then(i, |j| {
            self.starts_with(j, "##").then(|| self.line_end(j + 2))
        })
    }

    /// `[ \t]{1,}`.
    fn indentation(&self, i: usize) -> Option<usize> {
        let j = self.blanks(i);
        (j > i).then_some(j)
    }

    /// `"[^"]*"|'[^']*'`: line breaks included.
    fn quoted_name(&self, i: usize) -> Option<usize> {
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

    /// Whether an annotation starts at `j` in the sense of `BARE_NAME`'s
    /// lookahead `(?![ \t]+:::[ \t]*[A-Za-z_]|[ \t]+icon\(|[ \t]+##)`.
    fn annotation_ahead(&self, j: usize) -> bool {
        let k = self.blanks(j);
        if k == j {
            return false;
        }
        self.class_body(k).is_some() || self.starts_with(k, "icon(") || self.starts_with(k, "##")
    }

    /// `(?!:::|icon\(|##)[^ \t\n\r"'](?:(?!<annotation>)[^\n\r])*`.
    fn bare_name(&self, i: usize) -> Option<usize> {
        if self.starts_with(i, ":::") || self.starts_with(i, "icon(") || self.starts_with(i, "##") {
            return None;
        }
        let first = self.at(i)?;
        if matches!(first, ' ' | '\t' | '\n' | '\r' | '"' | '\'') {
            return None;
        }
        let mut j = i + 1;
        while let Some(c) = self.at(j)
            && c != '\n'
            && c != '\r'
            && !self.annotation_ahead(j)
        {
            j += 1;
        }
        Some(j)
    }

    /// The first token type that matches at `i`, in Chevrotain's order.
    fn next(&self, i: usize) -> Option<(Option<Kind>, usize)> {
        if let Some(end) = self.keyword(i) {
            // `LONGER_ALT`: a longer `BARE_NAME` match wins.
            return Some(match self.bare_name(i) {
                Some(longer) if longer > end => (Some(Kind::BareName), longer),
                _ => (Some(Kind::Keyword), end),
            });
        }
        let table: [(Option<Kind>, Matcher<'a>); 12] = [
            (Some(Kind::AccDescr), Lexer::acc_descr),
            (Some(Kind::AccTitle), Lexer::acc_title),
            (Some(Kind::Title), Lexer::title),
            (Some(Kind::ClassAnnotation), Lexer::class_annotation),
            (Some(Kind::IconAnnotation), Lexer::icon_annotation),
            (Some(Kind::DescAnnotation), Lexer::desc_annotation),
            (Some(Kind::Indentation), Lexer::indentation),
            (Some(Kind::QuotedName), Lexer::quoted_name),
            (HIDDEN, Lexer::indentation),
            (HIDDEN, Lexer::comment),
            (HIDDEN, Lexer::newline),
            (Some(Kind::BareName), Lexer::bare_name),
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

/// `buildNoViableAltMessage` for the `QUOTED_NAME | BARE_NAME`
/// alternatives.
const NAME_ALTERNATIVES: &str =
    "Expecting: one of these possible Token sequences:\n  1. [QUOTED_NAME]\n  2. [BARE_NAME]";

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

    /// `TreeView: 'treeView-beta' TitleAndAccessibilities? nodes+=TreeNode*`.
    fn tree_view(&mut self) -> Ast {
        let mut ast = Ast::default();
        if self.kind(1) == Kind::Keyword {
            self.advance();
        } else {
            // Langium's `buildMismatchTokenMessage`. The keyword can only be
            // missing when the first line lexes as a longer `BARE_NAME`,
            // which is in the keyword's follow set, so single-token
            // insertion recovers and the parse goes on.
            let t = self.la(1).clone();
            let message = format!(
                "Expecting token of type '{}' but found `{}`.",
                Kind::Keyword.name(),
                self.image(&t)
            );
            self.error_at(&t, message);
        }
        if self.kind(1).is_title() {
            self.titles(&mut ast);
        }
        // The top rule catches what its rules rethrow and re-syncs to the
        // end of the input.
        if self.repetition(&mut ast).is_err() {
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

    /// The `TreeNode*` loop with Chevrotain's in-repetition recovery at its
    /// exit.
    fn repetition(&mut self, ast: &mut Ast) -> Result<(), Thrown> {
        loop {
            while self.kind(1).starts_node() {
                self.node(ast)?;
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
            while self.kind(1) != Kind::Eof && !self.kind(1).starts_node() {
                self.advance();
            }
        }
    }

    /// `TreeNode` with the rule's re-sync: on an error, skip to the next
    /// token that can start a node, or rethrow when only the end is left.
    fn node(&mut self, ast: &mut Ast) -> Result<(), Thrown> {
        match self.node_inner() {
            Ok(node) => {
                ast.nodes.push(node);
                Ok(())
            }
            Err(Thrown) => {
                while self.kind(1) != Kind::Eof && !self.kind(1).starts_node() {
                    self.advance();
                }
                // The rule's follow set is the loop's first set; the end of
                // the input belongs to the top rule only.
                if self.kind(1).starts_node() {
                    Ok(())
                } else {
                    Err(Thrown)
                }
            }
        }
    }

    /// `TreeNode: indent=INDENTATION? (name=QUOTED_NAME | name=BARE_NAME)
    /// (classAnnotation=CLASS_ANNOTATION | iconAnnotation=ICON_ANNOTATION |
    /// descAnnotation=DESC_ANNOTATION)*`.
    fn node_inner(&mut self) -> Result<TreeNode, Thrown> {
        let mut node = TreeNode::default();
        if self.kind(1) == Kind::Indentation {
            let t = self.advance();
            node.indent = Some(t.end - t.start);
        }
        let name = self.la(1).clone();
        node.name = match name.kind {
            Kind::QuotedName => convert_quoted(&self.image(&name)),
            Kind::BareName => convert_bare(&self.image(&name)),
            _ => {
                let message = format!("{NAME_ALTERNATIVES}\nbut found: '{}'", self.image(&name));
                self.error_at(&name, message);
                return Err(Thrown);
            }
        };
        self.advance();
        while self.kind(1).is_annotation() {
            let t = self.advance();
            let image = self.image(&t);
            let trimmed = js_trim(&image);
            match t.kind {
                Kind::ClassAnnotation => {
                    node.class_annotation = Some(js_trim(after_chars(trimmed, 3)).to_string());
                }
                Kind::IconAnnotation => {
                    let inner = after_chars(trimmed, 5);
                    let inner = inner.strip_suffix(')').unwrap_or(inner);
                    node.icon_annotation = Some(inner.to_string());
                }
                _ => {
                    node.desc_annotation = Some(js_trim(after_chars(trimmed, 2)).to_string());
                }
            }
        }
        Ok(node)
    }
}

/// `s.substring(n)` for an ASCII prefix of `n` characters.
fn after_chars(s: &str, n: usize) -> &str {
    s.char_indices().nth(n).map_or("", |(i, _)| &s[i..])
}

/// `QUOTED_NAME`: `input.substring(1, input.length - 1)`, no escapes.
fn convert_quoted(image: &str) -> String {
    let chars: Vec<char> = image.chars().collect();
    if chars.len() < 2 {
        return String::new();
    }
    chars[1..chars.len() - 1].iter().collect()
}

/// `BARE_NAME`: `input.replace(/[\t ]+$/, '')`.
fn convert_bare(image: &str) -> String {
    image.trim_end_matches([' ', '\t']).to_string()
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

/// `parse('treeView', text)`: the AST, or the `MermaidParseError` text with
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
    let ast = parser.tree_view();
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

    fn node(indent: Option<usize>, name: &str) -> TreeNode {
        TreeNode {
            indent,
            name: name.to_string(),
            ..TreeNode::default()
        }
    }

    const ALTERNATIVES: &str = "Expecting: one of these possible Token sequences:\n  1. [QUOTED_NAME]\n  2. [BARE_NAME]\nbut found:";

    #[test]
    fn the_docs_example_parses() {
        let ast = ok(
            "treeView-beta\n    my-project/\n        src/\n            index.js\n        package.json\n",
        );
        assert_eq!(
            ast.nodes,
            vec![
                node(Some(4), "my-project/"),
                node(Some(8), "src/"),
                node(Some(12), "index.js"),
                node(Some(8), "package.json"),
            ]
        );
    }

    #[test]
    fn a_bare_keyword_is_an_empty_tree() {
        assert!(ok("treeView-beta\n").nodes.is_empty());
        assert!(ok("treeView-beta").nodes.is_empty());
    }

    /// Each case and its expected AST come from `@mermaid-js/parser` 2.0.1.
    #[test]
    fn annotations_and_names_convert_like_the_oracle() {
        let ast = ok("treeView-beta\n  a icon(mdi:x) icon(y) :::k ::: j ## d1 ## d2\n");
        assert_eq!(
            ast.nodes,
            vec![TreeNode {
                indent: Some(2),
                name: "a".to_string(),
                class_annotation: Some("j".to_string()),
                icon_annotation: Some("y".to_string()),
                desc_annotation: Some("d1 ## d2".to_string()),
            }]
        );
        let ast = ok("treeView-beta\n  a\n  ## desc only\n");
        assert_eq!(ast.nodes[0].desc_annotation.as_deref(), Some("desc only"));
        let ast = ok("treeView-beta\n  a   b  \n  c  %% comment\n");
        assert_eq!(
            ast.nodes,
            vec![node(Some(2), "a   b"), node(Some(2), "c  %% comment")]
        );
        let ast = ok("treeView-beta\n  \"a\" x\n  'b'\"c\"\n  \"\"\n");
        assert_eq!(
            ast.nodes,
            vec![
                node(Some(2), "a"),
                node(Some(1), "x"),
                node(Some(2), "b"),
                node(None, "c"),
                node(Some(2), ""),
            ]
        );
        let ast = ok("treeView-beta\n  title.md\n");
        assert_eq!(ast.title.as_deref(), Some(""));
        assert_eq!(ast.nodes, vec![node(None, ".md")]);
        let ast = ok("treeView-beta\n  b ## \n  c icon()\n");
        assert_eq!(ast.nodes[0].desc_annotation.as_deref(), Some(""));
        assert_eq!(ast.nodes[1].icon_annotation.as_deref(), Some(""));
    }

    #[test]
    fn titles_convert_like_the_oracle() {
        let ast = ok("treeView-beta\naccDescr: d\ntitle a  b\naccTitle: t  %% x\n  x\n");
        assert_eq!(ast.title.as_deref(), Some("a b"));
        assert_eq!(ast.acc_title.as_deref(), Some("t"));
        assert_eq!(ast.acc_descr.as_deref(), Some("d"));
        let ast = ok("treeView-beta\naccDescr {\n multi\n line }\n  a\n");
        assert_eq!(ast.acc_descr.as_deref(), Some("multi\nline"));
    }

    /// Each message is the text `@mermaid-js/parser` 2.0.1 throws.
    #[test]
    fn errors_match_the_oracle_text() {
        assert_eq!(
            message("treeView-beta x\n  \"a\"  \n  b\n"),
            format!(
                "Parsing failed:  Parse error on line 1, column 1: Expecting token of type 'treeView-beta' but found `treeView-beta x`.\nParse error on line 3, column 1: {ALTERNATIVES} '  '"
            )
        );
        assert_eq!(
            message("treeView-beta\nfoo\ntitle T\n"),
            "Parsing failed:  Parse error on line 3, column 1: Expecting end of file but found `title T`."
        );
        assert_eq!(
            message("treeView-beta\n  a\n  title x\n  b\n  accTitle: y\n"),
            "Parsing failed:  Parse error on line 3, column 1: Expecting token of type 'EOF' but found `  title x`.\nParse error on line 5, column 1: Expecting end of file but found `  accTitle: y`."
        );
        assert_eq!(
            message("treeView-beta\n  a\n  treeView-beta\n"),
            format!(
                "Parsing failed:  Parse error on line 3, column 3: {ALTERNATIVES} 'treeView-beta'"
            )
        );
        assert_eq!(
            message("treeView-beta\n  a\n  \"b\"\t\n"),
            format!("Parsing failed:  Parse error on line ?, column ?: {ALTERNATIVES} ''")
        );
        assert_eq!(
            message("treeView-beta\n  \"a\" \n  \"b\" \n  \"c\"\n"),
            format!(
                "Parsing failed:  Parse error on line 3, column 1: {ALTERNATIVES} '  '\nParse error on line 4, column 1: {ALTERNATIVES} '  '"
            )
        );
        assert_eq!(
            message("treeView-beta\ntitle T\n  \"a\" \ntitle U\n  b\n"),
            format!("Parsing failed:  Parse error on line 4, column 1: {ALTERNATIVES} 'title U'")
        );
        assert_eq!(
            message("treeView-beta\n  a\ntitle T\n\"q"),
            "Parsing failed: Lexer error on line 4, column 1: unexpected character: ->\"<- at offset: 26, skipped 1 characters. Parse error on line 3, column 1: Expecting token of type 'EOF' but found `title T`."
        );
        assert_eq!(
            message("treeView-beta\n  a icon(bad name)\n"),
            "Parsing failed: Lexer error on line 2, column 5: unexpected character: ->i<- at offset: 18, skipped 1 characters. "
        );
        assert_eq!(
            message("treeView-beta\n  \"a\" \n  \n"),
            format!(
                "Parsing failed:  Parse error on line 3, column 1: {ALTERNATIVES} '  '\nParse error on line ?, column ?: {ALTERNATIVES} ''"
            )
        );
        assert_eq!(
            message("treeView-beta %% c\n  a\n"),
            "Parsing failed:  Parse error on line 1, column 1: Expecting token of type 'treeView-beta' but found `treeView-beta %% c`."
        );
    }

    #[test]
    fn the_first_error_gives_the_position() {
        let failure = parse("treeView-beta\n  a\n  ::x\n:::x\n").unwrap_err();
        assert_eq!((failure.line, failure.column), (4, 1));
        let failure = parse("treeView-beta\n  a\ntitle T\n  b\n").unwrap_err();
        assert_eq!((failure.line, failure.column), (3, 1));
    }
}
