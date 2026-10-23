//! A hand-written lexer and parser for the language of `cynefin.langium`
//! plus `common.langium` (`@mermaid-js/parser` 2.0.1, mermaid 12.2.0).
//!
//! Lexer. Langium builds a Chevrotain lexer from the reachable rules only,
//! so this grammar has no `ID`, `NUMBER`, or `BOOLEAN`. The token order is
//! the keywords longest first (`cynefin-beta:`, `cynefin-beta`, `-->`,
//! `:`), then the terminals in grammar order with the cynefin terminal
//! first: `DOMAIN_NAME`, `ACC_DESCR`, `ACC_TITLE`, `TITLE`, `STRING`,
//! `NEWLINE`, and the hidden `WHITESPACE`, `YAML`, `DIRECTIVE`,
//! `SINGLE_LINE_COMMENT`. No keyword has a `LONGER_ALT`, and `DOMAIN_NAME`
//! has no word boundary, so `complexity` is `complex` and then three
//! skipped characters. `CynefinTokenBuilder` adds the
//! `(?:(?=%%)|(?!\S))` suffix to `cynefin-beta`. At each position the
//! first token type that matches wins; a position where none matches is
//! skipped and reported as one lexer error per run of skipped characters.
//!
//! Parser. An LL walk of the `Cynefin` rule with the error recovery of
//! Chevrotain's recovering parser as it plays out on this grammar:
//!
//! - The LL(*) prediction between `DomainBlock` and `Transition` picks a
//!   transition only when the whole `DOMAIN_NAME '-->' DOMAIN_NAME (':'
//!   STRING)? EOL` prefix is there: the domain block alternative can always
//!   end the rule, so it survives every other token.
//! - The top `(...)*` loop exits on a token that cannot start an
//!   iteration. When that token is not the end and the token after it is
//!   not the end either, the in-repetition recovery reports `Expecting
//!   token of type 'EOF' but found ...`, skips to the next token that
//!   starts an iteration, and goes on. Otherwise the loop ends and the
//!   leftover input is `Expecting end of file but found ...`.
//! - The top loop enters on the first token alone. A title or
//!   accessibility token that is not followed by an `EOL` then fails the
//!   first iteration of the fragment's `(...)+` (whose lookahead reads the
//!   `EOL` too) with an `EarlyExitException`. The fragment re-syncs
//!   without consuming anything, so the loop is stuck and exits with no
//!   recovery, and the token is also `Expecting end of file`.
//! - The other repetitions are followed by a repetition or end inside a
//!   rule below the top, so Chevrotain tries no recovery there.
//! - A missing header is a `NoViableAltException`; the top rule then
//!   re-syncs to the end of the input.
//!
//! `MermaidParseError` joins both lists as `Parsing failed: <lexer errors>
//! <parser errors>`.

use std::fmt::Write;

use crate::mermaid::theme::khroma::js_is_whitespace;

/// The five `DOMAIN_NAME` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DomainName {
    Complex,
    Complicated,
    Clear,
    Chaotic,
    Confusion,
}

impl DomainName {
    /// The alternatives of `terminal DOMAIN_NAME`, in grammar order.
    pub const ALL: [DomainName; 5] = [
        DomainName::Complex,
        DomainName::Complicated,
        DomainName::Clear,
        DomainName::Chaotic,
        DomainName::Confusion,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            DomainName::Complex => "complex",
            DomainName::Complicated => "complicated",
            DomainName::Clear => "clear",
            DomainName::Chaotic => "chaotic",
            DomainName::Confusion => "confusion",
        }
    }

    fn from_image(image: &str) -> Option<DomainName> {
        DomainName::ALL.into_iter().find(|d| d.as_str() == image)
    }
}

/// The `Cynefin` AST after Langium's value converters.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ast {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub domains: Vec<DomainBlock>,
    pub transitions: Vec<Transition>,
}

/// `DomainBlock: domain=DOMAIN_NAME NEWLINE* (items+=DomainItem
/// NEWLINE*)*`, with each `DomainItem` reduced to its label.
#[derive(Debug, Clone, PartialEq)]
pub struct DomainBlock {
    pub domain: DomainName,
    pub items: Vec<String>,
}

/// `Transition: from=DOMAIN_NAME '-->' to=DOMAIN_NAME (':' label=STRING)?
/// EOL`.
#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub from: DomainName,
    pub to: DomainName,
    pub label: Option<String>,
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
    CynefinBetaColon,
    CynefinBeta,
    Arrow,
    Colon,
    DomainName,
    AccDescr,
    AccTitle,
    Title,
    Str,
    Newline,
    Eof,
}

impl Kind {
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
    /// 1-based; 0 for the EOF token, which Chevrotain gives no position.
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

    /// The `(?:(?=%%)|(?!\S))` suffix the mermaid token builder appends to
    /// the `cynefin-beta` keyword.
    fn keyword_boundary(&self, i: usize) -> bool {
        match self.at(i) {
            None => true,
            Some('%') => self.at(i + 1) == Some('%'),
            Some(c) => js_is_whitespace(c),
        }
    }

    /// `DOMAIN_NAME`: `/complex|complicated|clear|chaotic|confusion/`, the
    /// first alternative that matches.
    fn domain_name(&self, i: usize) -> Option<usize> {
        DomainName::ALL
            .iter()
            .find_map(|d| self.literal(i, d.as_str()))
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
        let table: [(Option<Kind>, Matcher<'a>); 14] = [
            (Some(Kind::CynefinBetaColon), |l, i| {
                l.literal(i, "cynefin-beta:")
            }),
            (Some(Kind::CynefinBeta), |l, i| {
                l.literal(i, "cynefin-beta")
                    .filter(|end| l.keyword_boundary(*end))
            }),
            (Some(Kind::Arrow), |l, i| l.literal(i, "-->")),
            (Some(Kind::Colon), |l, i| l.literal(i, ":")),
            (Some(Kind::DomainName), Lexer::domain_name),
            (Some(Kind::AccDescr), Lexer::acc_descr),
            (Some(Kind::AccTitle), Lexer::acc_title),
            (Some(Kind::Title), Lexer::title),
            (Some(Kind::Str), Lexer::string),
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
    errors: Vec<ParseError>,
}

impl Parser<'_> {
    fn la(&self, k: usize) -> &Token {
        &self.tokens[(self.pos + k - 1).min(self.tokens.len() - 1)]
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

    fn skip_newlines(&mut self) {
        while self.kind(1) == Kind::Newline {
            self.advance();
        }
    }

    fn error_at(&mut self, k: usize, message: String) {
        let t = self.la(k);
        self.errors.push(ParseError {
            line: t.line,
            column: t.column,
            message,
        });
    }

    /// Whether the token at `k` is an `EOL` (`NEWLINE+ | EOF`) start.
    fn is_eol(&self, k: usize) -> bool {
        matches!(self.kind(k), Kind::Newline | Kind::Eof)
    }

    /// The LL(*) prediction for `transitions+=Transition`: the whole rule
    /// up to the first token of its `EOL`.
    fn transition_ahead(&self) -> bool {
        if !(self.kind(1) == Kind::DomainName
            && self.kind(2) == Kind::Arrow
            && self.kind(3) == Kind::DomainName)
        {
            return false;
        }
        if self.kind(4) == Kind::Colon {
            self.kind(5) == Kind::Str && self.is_eol(6)
        } else {
            self.is_eol(4)
        }
    }

    /// The lookahead of the top loop, on the first token only.
    fn starts_iteration(&self) -> bool {
        matches!(self.kind(1), Kind::DomainName | Kind::Newline) || self.kind(1).is_title_or_acc()
    }

    /// `fragment TitleAndAccessibilities: ((accDescr=ACC_DESCR |
    /// accTitle=ACC_TITLE | title=TITLE) EOL)+`, the last of each kind
    /// winning. Returns false when the first iteration cannot start: the
    /// `EarlyExitException` is recorded and nothing is consumed.
    fn titles(&mut self, ast: &mut Ast) -> bool {
        if !self.is_eol(2) {
            let found = self.image(&self.la(1).clone());
            self.error_at(
                1,
                format!(
                    "Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <[ACC_DESCR] ,[ACC_TITLE] ,[TITLE]>\nbut found: '{found}'"
                ),
            );
            return false;
        }
        while self.kind(1).is_title_or_acc() && self.is_eol(2) {
            let t = self.advance();
            let image = self.image(&t);
            match t.kind {
                Kind::AccDescr => ast.acc_descr = Some(convert_acc_descr(&image)),
                Kind::AccTitle => ast.acc_title = Some(convert_acc_title(&image)),
                _ => ast.title = Some(convert_title(&image)),
            }
            self.skip_newlines();
        }
        true
    }

    /// `DomainBlock`.
    fn domain_block(&mut self, ast: &mut Ast) {
        let t = self.advance();
        let Some(domain) = DomainName::from_image(&self.image(&t)) else {
            return;
        };
        self.skip_newlines();
        let mut items = Vec::new();
        while self.kind(1) == Kind::Str {
            let s = self.advance();
            items.push(convert_string(&self.image(&s)));
            self.skip_newlines();
        }
        ast.domains.push(DomainBlock { domain, items });
    }

    /// `Transition`, once `transition_ahead` has seen all of it.
    fn transition(&mut self, ast: &mut Ast) {
        let from = self.advance();
        self.advance();
        let to = self.advance();
        let label = if self.kind(1) == Kind::Colon {
            self.advance();
            let s = self.advance();
            Some(convert_string(&self.image(&s)))
        } else {
            None
        };
        // `EOL: NEWLINE+ | EOF`.
        self.skip_newlines();
        if let (Some(from), Some(to)) = (
            DomainName::from_image(&self.image(&from)),
            DomainName::from_image(&self.image(&to)),
        ) {
            ast.transitions.push(Transition { from, to, label });
        }
    }

    /// The top `(...)*` loop with Chevrotain's in-repetition recovery at its
    /// exit.
    fn repetition(&mut self, ast: &mut Ast) {
        loop {
            while self.starts_iteration() {
                match self.kind(1) {
                    Kind::Newline => {
                        self.advance();
                    }
                    Kind::DomainName if self.transition_ahead() => self.transition(ast),
                    Kind::DomainName => self.domain_block(ast),
                    _ => {
                        if !self.titles(ast) {
                            // `notStuck` is false: no recovery.
                            return;
                        }
                    }
                }
            }
            // `shouldInRepetitionRecoveryBeTried`: not at the end, and the
            // offending token cannot simply be deleted before the end.
            if self.kind(1) == Kind::Eof || self.kind(2) == Kind::Eof {
                return;
            }
            let offending = self.la(1).clone();
            let message = format!(
                "Expecting token of type 'EOF' but found `{}`.",
                self.image(&offending)
            );
            self.error_at(1, message);
            while self.kind(1) != Kind::Eof && !self.starts_iteration() {
                self.advance();
            }
        }
    }

    /// The `Cynefin` entry rule.
    fn cynefin(&mut self) -> Ast {
        let mut ast = Ast::default();
        self.skip_newlines();
        match self.kind(1) {
            Kind::CynefinBeta | Kind::CynefinBetaColon => {
                self.advance();
            }
            _ => {
                let found = self.image(&self.la(1).clone());
                self.error_at(
                    1,
                    format!(
                        "Expecting: one of these possible Token sequences:\n  1. [cynefin-beta]\n  2. [cynefin-beta:]\nbut found: '{found}'"
                    ),
                );
                // The top rule re-syncs to the end of the input.
                return ast;
            }
        }
        self.skip_newlines();
        self.repetition(&mut ast);
        if self.kind(1) != Kind::Eof {
            let t = self.la(1).clone();
            let message = format!("Expecting end of file but found `{}`.", self.image(&t));
            self.error_at(1, message);
        }
        ast
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

/// `formatLocation`: Chevrotain marks a missing position (the EOF token)
/// with `NaN`, printed as `?`.
fn location(v: u32) -> String {
    if v == 0 {
        "?".to_string()
    } else {
        v.to_string()
    }
}

/// `parse('cynefin', text)`: the AST, or the `MermaidParseError` text with
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
    let ast = parser.cynefin();
    if lex_errors.is_empty() && parser.errors.is_empty() {
        return Ok(ast);
    }
    let lexer_text = lex_errors
        .iter()
        .map(|e| {
            format!(
                "Lexer error on line {}, column {}: unexpected character: ->{}<- at offset: {}, skipped {} characters.",
                e.line, e.column, e.first, e.offset, e.skipped
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut parser_text = String::new();
    for (i, e) in parser.errors.iter().enumerate() {
        if i > 0 {
            parser_text.push('\n');
        }
        let _ = write!(
            parser_text,
            "Parse error on line {}, column {}: {}",
            location(e.line),
            location(e.column),
            e.message
        );
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    /// The AST as the oracle probe prints it: `JSON.stringify` of the
    /// title fields, the domains, and the transitions, with an absent
    /// field left out and an absent label as `null`.
    fn strip(ast: &Ast) -> Value {
        let mut out = serde_json::Map::new();
        if let Some(t) = &ast.title {
            out.insert("title".into(), json!(t));
        }
        if let Some(t) = &ast.acc_title {
            out.insert("accTitle".into(), json!(t));
        }
        if let Some(t) = &ast.acc_descr {
            out.insert("accDescr".into(), json!(t));
        }
        let domains: Vec<Value> = ast
            .domains
            .iter()
            .map(|d| json!([d.domain.as_str(), d.items]))
            .collect();
        let transitions: Vec<Value> = ast
            .transitions
            .iter()
            .map(|t| json!([t.from.as_str(), t.to.as_str(), t.label]))
            .collect();
        out.insert("domains".into(), Value::Array(domains));
        out.insert("transitions".into(), Value::Array(transitions));
        Value::Object(out)
    }

    /// Inputs and results from the real `@mermaid-js/parser` 2.0.1: the
    /// stripped AST as JSON, or the full `MermaidParseError` message with
    /// the line and column of its first error (0 for `?`).
    #[allow(clippy::type_complexity)]
    const ORACLE: &[(&str, Result<&str, (&str, (u32, u32))>)] = &[
        ("cynefin-beta\n", Ok("{\"domains\":[],\"transitions\":[]}")),
        ("cynefin-beta:\n", Ok("{\"domains\":[],\"transitions\":[]}")),
        (
            "cynefin-beta :\n",
            Err((
                "Parsing failed:  Parse error on line 1, column 14: Expecting token of type 'EOF' but found `:`.",
                (1, 14),
            )),
        ),
        (
            "cynefin-betax\n",
            Err((
                "Parsing failed: Lexer error on line 1, column 1: unexpected character: ->c<- at offset: 0, skipped 13 characters. Parse error on line ?, column ?: Expecting: one of these possible Token sequences:\n  1. [cynefin-beta]\n  2. [cynefin-beta:]\nbut found: ''",
                (1, 1),
            )),
        ),
        (
            "  cynefin-beta%% c\n",
            Ok("{\"domains\":[],\"transitions\":[]}"),
        ),
        (
            "\n\ncynefin-beta\n  title Hello   World  \n",
            Ok("{\"title\":\"Hello World\",\"domains\":[],\"transitions\":[]}"),
        ),
        (
            "cynefin-beta\ntitle\n",
            Ok("{\"title\":\"\",\"domains\":[],\"transitions\":[]}"),
        ),
        (
            "cynefin-beta\ncomplex\n",
            Ok("{\"domains\":[[\"complex\",[]]],\"transitions\":[]}"),
        ),
        (
            "cynefin-beta\ncomplex \"a\" \"b\"\n",
            Ok("{\"domains\":[[\"complex\",[\"a\",\"b\"]]],\"transitions\":[]}"),
        ),
        (
            "cynefin-beta\ncomplex\n\n\n  \"a\"\n\n  'b\\'c'\ncomplex\n\"z\"\n",
            Ok(
                "{\"domains\":[[\"complex\",[\"a\",\"b'c\"]],[\"complex\",[\"z\"]]],\"transitions\":[]}",
            ),
        ),
        (
            "cynefin-beta\ncomplex --> clear\n",
            Ok("{\"domains\":[],\"transitions\":[[\"complex\",\"clear\",null]]}"),
        ),
        (
            "cynefin-beta\ncomplex --> clear : \"Go\"\n",
            Ok("{\"domains\":[],\"transitions\":[[\"complex\",\"clear\",\"Go\"]]}"),
        ),
        (
            "cynefin-beta\ncomplex --> clear : \"\"\n",
            Ok("{\"domains\":[],\"transitions\":[[\"complex\",\"clear\",\"\"]]}"),
        ),
        (
            "cynefin-beta\ncomplex --> complex : \"self\"\n",
            Ok("{\"domains\":[],\"transitions\":[[\"complex\",\"complex\",\"self\"]]}"),
        ),
        (
            "cynefin-beta\ncomplex --> clear",
            Ok("{\"domains\":[],\"transitions\":[[\"complex\",\"clear\",null]]}"),
        ),
        (
            "cynefin-beta\ncomplex --> clear complex\n",
            Err((
                "Parsing failed:  Parse error on line 2, column 9: Expecting token of type 'EOF' but found `-->`.",
                (2, 9),
            )),
        ),
        (
            "cynefin-beta\ncomplex -->\n",
            Err((
                "Parsing failed:  Parse error on line 2, column 9: Expecting token of type 'EOF' but found `-->`.",
                (2, 9),
            )),
        ),
        (
            "cynefin-beta\ncomplex --> clear :\n",
            Err((
                "Parsing failed:  Parse error on line 2, column 9: Expecting token of type 'EOF' but found `-->`.\nParse error on line 2, column 19: Expecting token of type 'EOF' but found `:`.",
                (2, 9),
            )),
        ),
        (
            "cynefin-beta\ncomplex --> clear : Go\n",
            Err((
                "Parsing failed: Lexer error on line 2, column 21: unexpected character: ->G<- at offset: 33, skipped 2 characters. Parse error on line 2, column 9: Expecting token of type 'EOF' but found `-->`.\nParse error on line 2, column 19: Expecting token of type 'EOF' but found `:`.",
                (2, 21),
            )),
        ),
        (
            "cynefin-beta\ncomplex\n  Investigate root cause\n",
            Err((
                "Parsing failed: Lexer error on line 3, column 3: unexpected character: ->I<- at offset: 23, skipped 11 characters.\nLexer error on line 3, column 15: unexpected character: ->r<- at offset: 35, skipped 4 characters.\nLexer error on line 3, column 20: unexpected character: ->c<- at offset: 40, skipped 5 characters. ",
                (3, 3),
            )),
        ),
        (
            "cynefin-beta\nsimple\n  \"a\"\n",
            Err((
                "Parsing failed: Lexer error on line 2, column 1: unexpected character: ->s<- at offset: 13, skipped 6 characters. Parse error on line 3, column 3: Expecting token of type 'EOF' but found `\"a\"`.",
                (2, 1),
            )),
        ),
        (
            "complex\n",
            Err((
                "Parsing failed:  Parse error on line 1, column 1: Expecting: one of these possible Token sequences:\n  1. [cynefin-beta]\n  2. [cynefin-beta:]\nbut found: 'complex'",
                (1, 1),
            )),
        ),
        (
            "cynefin-beta\n\"a\"\n",
            Err((
                "Parsing failed:  Parse error on line 2, column 1: Expecting token of type 'EOF' but found `\"a\"`.",
                (2, 1),
            )),
        ),
        (
            "cynefin-beta\ncomplex\n\"a\"\n-->\n",
            Err((
                "Parsing failed:  Parse error on line 4, column 1: Expecting token of type 'EOF' but found `-->`.",
                (4, 1),
            )),
        ),
        (
            "cynefin-beta\n:\n",
            Err((
                "Parsing failed:  Parse error on line 2, column 1: Expecting token of type 'EOF' but found `:`.",
                (2, 1),
            )),
        ),
        (
            "cynefin-beta\ncomplex \"a\" --> clear\n",
            Err((
                "Parsing failed:  Parse error on line 2, column 13: Expecting token of type 'EOF' but found `-->`.",
                (2, 13),
            )),
        ),
        (
            "cynefin-beta\ncomplexity\n",
            Err((
                "Parsing failed: Lexer error on line 2, column 8: unexpected character: ->i<- at offset: 20, skipped 3 characters. ",
                (2, 8),
            )),
        ),
        (
            "cynefin-beta\naccTitle: T\naccDescr: D\ntitle X\ncomplex\n",
            Ok(
                "{\"title\":\"X\",\"accTitle\":\"T\",\"accDescr\":\"D\",\"domains\":[[\"complex\",[]]],\"transitions\":[]}",
            ),
        ),
        (
            "cynefin-beta\naccDescr {\n  multi\n  line\n}\ncomplex\n",
            Ok("{\"accDescr\":\"multi\\nline\",\"domains\":[[\"complex\",[]]],\"transitions\":[]}"),
        ),
        (
            "cynefin-beta\ntitle A\ncomplex\ntitle B\n",
            Ok("{\"title\":\"B\",\"domains\":[[\"complex\",[]]],\"transitions\":[]}"),
        ),
        (
            "cynefin-beta\n%%{init: {}}%%\ncomplex\n  \"a\" %% note\n",
            Ok("{\"domains\":[[\"complex\",[\"a\"]]],\"transitions\":[]}"),
        ),
        (
            "cynefin-beta\ncomplex\n  \"a\"\nclear\n  \"b\"\ncomplex --> clear : \"x\"\nchaotic\n",
            Ok(
                "{\"domains\":[[\"complex\",[\"a\"]],[\"clear\",[\"b\"]],[\"chaotic\",[]]],\"transitions\":[[\"complex\",\"clear\",\"x\"]]}",
            ),
        ),
        (
            "cynefin-beta\ncomplex --> clear : \"x\" : \"y\"\n",
            Err((
                "Parsing failed:  Parse error on line 2, column 9: Expecting token of type 'EOF' but found `-->`.\nParse error on line 2, column 19: Expecting token of type 'EOF' but found `:`.",
                (2, 9),
            )),
        ),
        (
            "cynefin-beta\ncomplex --> \"x\"\n",
            Err((
                "Parsing failed:  Parse error on line 2, column 9: Expecting token of type 'EOF' but found `-->`.",
                (2, 9),
            )),
        ),
        (
            "cynefin-beta\ncomplex\n\"a\"\n\"b\" :\n",
            Err((
                "Parsing failed:  Parse error on line 4, column 5: Expecting token of type 'EOF' but found `:`.",
                (4, 5),
            )),
        ),
        (
            "cynefin-beta\ntitle A complex\n",
            Ok("{\"title\":\"A complex\",\"domains\":[],\"transitions\":[]}"),
        ),
        (
            "cynefin-beta\n  title\tTabbed\n",
            Ok("{\"title\":\"Tabbed\",\"domains\":[],\"transitions\":[]}"),
        ),
        (
            "cynefin-beta\n\"unterminated\n",
            Err((
                "Parsing failed: Lexer error on line 2, column 1: unexpected character: ->\"<- at offset: 13, skipped 13 characters. ",
                (2, 1),
            )),
        ),
        (
            "cynefin-beta\ncomplex\n:\n\"a\"\n",
            Err((
                "Parsing failed:  Parse error on line 3, column 1: Expecting token of type 'EOF' but found `:`.\nParse error on line 4, column 1: Expecting token of type 'EOF' but found `\"a\"`.",
                (3, 1),
            )),
        ),
        (
            "cynefin-beta\ncomplex\n\"a\" \"b\" : \"c\"\nclear\n",
            Err((
                "Parsing failed:  Parse error on line 3, column 9: Expecting token of type 'EOF' but found `:`.",
                (3, 9),
            )),
        ),
        (
            "cynefin-beta\ntitle X\n:\ncomplex\n\"a\"\n",
            Err((
                "Parsing failed:  Parse error on line 3, column 1: Expecting token of type 'EOF' but found `:`.",
                (3, 1),
            )),
        ),
        (
            "cynefin-beta\naccDescr {x} complex\n",
            Err((
                "Parsing failed:  Parse error on line 2, column 1: Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <[ACC_DESCR] ,[ACC_TITLE] ,[TITLE]>\nbut found: 'accDescr {x}'\nParse error on line 2, column 1: Expecting end of file but found `accDescr {x}`.",
                (2, 1),
            )),
        ),
        (
            "cynefin-beta\nunclear\n",
            Err((
                "Parsing failed: Lexer error on line 2, column 1: unexpected character: ->u<- at offset: 13, skipped 2 characters. ",
                (2, 1),
            )),
        ),
        (
            "cynefin-beta\ncomplex --> clear : 'single'\n",
            Ok("{\"domains\":[],\"transitions\":[[\"complex\",\"clear\",\"single\"]]}"),
        ),
        (
            "cynefin-beta\ncomplex-->clear:\"tight\"\n",
            Ok("{\"domains\":[],\"transitions\":[[\"complex\",\"clear\",\"tight\"]]}"),
        ),
        (
            "cynefin-beta\ncomplex \"a\"\ncomplicated \"b\"\nclear \"c\"\nchaotic \"d\"\nconfusion \"e\"\n",
            Ok(
                "{\"domains\":[[\"complex\",[\"a\"]],[\"complicated\",[\"b\"]],[\"clear\",[\"c\"]],[\"chaotic\",[\"d\"]],[\"confusion\",[\"e\"]]],\"transitions\":[]}",
            ),
        ),
        (
            "\ncynefin-beta:\n\ncomplex\n",
            Ok("{\"domains\":[[\"complex\",[]]],\"transitions\":[]}"),
        ),
        (
            "cynefin-beta\n\"a\" \"b\"\n",
            Err((
                "Parsing failed:  Parse error on line 2, column 1: Expecting token of type 'EOF' but found `\"a\"`.",
                (2, 1),
            )),
        ),
        (
            "cynefin-beta\ncomplex\n\"a\"\n:",
            Err((
                "Parsing failed:  Parse error on line 4, column 1: Expecting end of file but found `:`.",
                (4, 1),
            )),
        ),
        (
            "cynefin-beta\ncomplex\n\"a\"\n: ",
            Err((
                "Parsing failed:  Parse error on line 4, column 1: Expecting end of file but found `:`.",
                (4, 1),
            )),
        ),
        (
            "cynefin-beta\nclear --> chaotic : \"x\"\n\"stray\"\n",
            Err((
                "Parsing failed:  Parse error on line 3, column 1: Expecting token of type 'EOF' but found `\"stray\"`.",
                (3, 1),
            )),
        ),
        (
            "cynefin-beta\naccTitle : spaced\n",
            Ok("{\"accTitle\":\"spaced\",\"domains\":[],\"transitions\":[]}"),
        ),
        (
            "cynefin-beta\n\"a\\nb\"\n",
            Err((
                "Parsing failed:  Parse error on line 2, column 1: Expecting token of type 'EOF' but found `\"a\\nb\"`.",
                (2, 1),
            )),
        ),
        (
            "cynefin-beta\ncomplex\n  \"esc \\\"q\\\" \\t x\"\n",
            Ok("{\"domains\":[[\"complex\",[\"esc \\\"q\\\" \\t x\"]]],\"transitions\":[]}"),
        ),
        (
            "cynefin-beta\r\ncomplex\r\n  \"a\"\r\n",
            Ok("{\"domains\":[[\"complex\",[\"a\"]]],\"transitions\":[]}"),
        ),
        (
            "cynefin-beta\ncomplex --> clear : \"x\" %% c\n",
            Ok("{\"domains\":[],\"transitions\":[[\"complex\",\"clear\",\"x\"]]}"),
        ),
        (
            "cynefin-beta\ntitle Title %% c\n",
            Ok("{\"title\":\"Title\",\"domains\":[],\"transitions\":[]}"),
        ),
        (
            "cynefin-beta\n---\n",
            Err((
                "Parsing failed: Lexer error on line 2, column 1: unexpected character: ->-<- at offset: 13, skipped 3 characters. ",
                (2, 1),
            )),
        ),
        (
            "cynefin-beta\ncomplex ; clear\n",
            Err((
                "Parsing failed: Lexer error on line 2, column 9: unexpected character: ->;<- at offset: 21, skipped 1 characters. ",
                (2, 9),
            )),
        ),
        (
            "cynefin-beta\nconfusion\n\"é ü\"\n",
            Ok("{\"domains\":[[\"confusion\",[\"é ü\"]]],\"transitions\":[]}"),
        ),
        (
            "cynefin-beta\n日本\ncomplex\n",
            Err((
                "Parsing failed: Lexer error on line 2, column 1: unexpected character: ->日<- at offset: 13, skipped 2 characters. ",
                (2, 1),
            )),
        ),
        (
            "cynefin-beta\ncomplex --> clear --> chaotic\n",
            Err((
                "Parsing failed:  Parse error on line 2, column 9: Expecting token of type 'EOF' but found `-->`.",
                (2, 9),
            )),
        ),
        (
            "cynefin-beta\n-->\n:\n",
            Err((
                "Parsing failed:  Parse error on line 2, column 1: Expecting token of type 'EOF' but found `-->`.\nParse error on line 3, column 1: Expecting token of type 'EOF' but found `:`.",
                (2, 1),
            )),
        ),
        (
            "cynefin-beta\ntitle\ncomplex\naccTitle: a\n",
            Ok(
                "{\"title\":\"\",\"accTitle\":\"a\",\"domains\":[[\"complex\",[]]],\"transitions\":[]}",
            ),
        ),
    ];

    #[test]
    fn matches_the_oracle_parser() {
        for (input, expected) in ORACLE {
            match (parse(input), expected) {
                (Ok(ast), Ok(json)) => {
                    let want: Value = serde_json::from_str(json).unwrap();
                    assert_eq!(strip(&ast), want, "{input:?}");
                }
                (Err(e), Err((message, (line, column)))) => {
                    assert_eq!(e.message, *message, "{input:?}");
                    assert_eq!((e.line, e.column), (*line, *column), "{input:?}");
                }
                (got, _) => panic!("{input:?}: {got:?}"),
            }
        }
    }
}
