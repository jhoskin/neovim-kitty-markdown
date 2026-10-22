//! Translation of the JavaScript in mermaid's lexer actions to a small
//! program the lexer can run.
//!
//! The grammars use a closed set of idioms: start-condition changes, token
//! returns, `yytext` edits, logger calls, a few `yy` callbacks and one
//! `throw`. Anything outside that set is an `Unsupported` error that names
//! the grammar, the rule and the source, so a grammar bump cannot drop an
//! action without being noticed.

use std::fmt;

use fancy_regex::Regex;

use super::jsregex;

/// What a `return` statement hands the parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// `return 'NAME'`; looked up in `symbols_` by the driver.
    Name(String),
    /// `return 12`; a symbol id as jison-lex emits after token substitution.
    Id(u32),
    /// `return yytext[0]`: the first character of the match as a token name.
    FirstChar,
}

/// A JavaScript value the `yy` hooks exchange.
#[derive(Debug, Clone, PartialEq)]
pub enum Lit {
    Undefined,
    Bool(bool),
    Str(String),
    Num(f64),
}

impl Lit {
    pub fn truthy(&self) -> bool {
        match self {
            Lit::Undefined => false,
            Lit::Bool(b) => *b,
            Lit::Str(s) => !s.is_empty(),
            Lit::Num(n) => *n != 0.0 && !n.is_nan(),
        }
    }
}

/// The diagram db as the lexer actions see it through `yy`. Every method
/// has a no-op default; a grammar whose lexer consults the db (flow's
/// `yy.lex.firstGraph()`, venn's indent mode) needs the db to implement
/// the names its actions use.
pub trait Yy {
    /// `yy.<name>(args)`; `name` is the dotted path after `yy.`.
    fn yy_call(&mut self, name: &str, args: &[Lit]) -> Lit {
        let _ = (name, args);
        Lit::Undefined
    }

    /// `yy.<name>` read as a value, also for `if (yy.setIndentMode)` checks.
    fn yy_get(&mut self, name: &str) -> Lit {
        let _ = name;
        Lit::Undefined
    }

    /// `yy.<name> = value`.
    fn yy_set(&mut self, name: &str, value: Lit) {
        let _ = (name, value);
    }
}

/// A piece of a `throw new Error(...)` message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    Lit(String),
    YyText,
    YyTextTrim,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cond {
    YyCall(String, Vec<Lit>),
    YyGet(String),
    And(Box<Cond>, Box<Cond>),
    Not(Box<Cond>),
    /// stateDiagram's `processId()` from its `%{ %}` block: splits an
    /// inline `%%` comment off the id, returns false when the whole match
    /// is a comment.
    ProcessId,
}

/// `yytext.replace(/pattern/flags, replacement)`.
#[derive(Debug, Clone)]
pub struct Replace {
    pub pattern: String,
    pub replacement: String,
    pub global: bool,
    pub regex: Regex,
}

impl PartialEq for Replace {
    fn eq(&self, other: &Replace) -> bool {
        self.pattern == other.pattern
            && self.replacement == other.replacement
            && self.global == other.global
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// `this.begin(x)` and `this.pushState(x)`.
    Begin(String),
    PopState,
    Return(Token),
    /// A bare `return;`: consume the match, hand the parser nothing.
    ReturnNothing,
    /// `yytext = yytext.trim()`.
    Trim,
    /// `yytext = yytext.slice(start, end)` with JavaScript index rules.
    Slice {
        start: i64,
        end: Option<i64>,
    },
    Replace(Replace),
    SetText(String),
    /// `this.unput(yytext)` and `this.less(0)`: push the whole match back.
    Unput,
    Throw(Vec<Part>),
    YySet(String, Lit),
    YyCall(String, Vec<Lit>),
    If {
        cond: Cond,
        then: Vec<Op>,
        otherwise: Vec<Op>,
    },
}

pub type Program = Vec<Op>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported {
    pub grammar: String,
    pub rule: usize,
    pub source: String,
    pub reason: String,
}

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} lexer rule {}: unsupported action idiom ({}): {}",
            self.grammar,
            self.rule,
            self.reason,
            self.source.trim()
        )
    }
}

impl std::error::Error for Unsupported {}

/// Translates one lexer action. `grammar` and `rule` only label errors.
pub fn translate(grammar: &str, rule: usize, source: &str) -> Result<Program, Unsupported> {
    let fail = |reason: String| Unsupported {
        grammar: grammar.to_string(),
        rule,
        source: source.to_string(),
        reason,
    };
    let tokens = tokenize(source).map_err(fail)?;
    let mut parser = Parser {
        tokens,
        pos: 0,
        regex_names: Vec::new(),
    };
    let mut program = Vec::new();
    while !parser.at_end() {
        program.extend(parser.statement().map_err(fail)?);
    }
    Ok(program)
}

/// The known `%{ %}` prelude of stateDiagram.jison, with whitespace
/// collapsed. Any other prelude with code is unsupported.
const PROCESS_ID_SOURCE: &str = "function processId() { const idx = yytext.indexOf('%%'); if (idx === 0) { return false; } if (idx > 0) { const before = yytext.slice(0, idx); const after = yytext.slice(idx); if (after) { yy.lexer.unput(after); } yytext = before; } return true; }";

/// Checks the lexer prelude: comments only, or the `processId` helper the
/// translator knows.
pub fn check_include(grammar: &str, include: Option<&str>) -> Result<(), Unsupported> {
    let Some(include) = include else {
        return Ok(());
    };
    let tokens = tokenize(include).map_err(|reason| Unsupported {
        grammar: grammar.to_string(),
        rule: usize::MAX,
        source: include.to_string(),
        reason,
    })?;
    if tokens.is_empty() {
        return Ok(());
    }
    let normalized = include.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized == PROCESS_ID_SOURCE {
        return Ok(());
    }
    Err(Unsupported {
        grammar: grammar.to_string(),
        rule: usize::MAX,
        source: include.to_string(),
        reason: "lexer prelude with code other than processId()".to_string(),
    })
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Str(String),
    Num(f64),
    /// `/pattern/flags`.
    Regex(String, String),
    Punct(&'static str),
}

const PUNCTS: [&str; 28] = [
    "===", "!==", "&&", "||", "==", "!=", ">=", "<=", "=", "(", ")", "{", "}", "[", "]", ".", ",",
    ";", "!", "+", "-", ":", "<", ">", "*", "?", "%", "^",
];

fn tokenize(source: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut out: Vec<Tok> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            let mut j = i + 2;
            while j + 1 < chars.len() && !(chars[j] == '*' && chars[j + 1] == '/') {
                j += 1;
            }
            if j + 1 >= chars.len() {
                return Err("unterminated comment".to_string());
            }
            i = j + 2;
            continue;
        }
        if c == '/' {
            // A `/` where an expression starts is a regex literal.
            let expression_start = !matches!(
                out.last(),
                Some(Tok::Ident(_))
                    | Some(Tok::Num(_))
                    | Some(Tok::Str(_))
                    | Some(Tok::Punct(")"))
                    | Some(Tok::Punct("]"))
            );
            if !expression_start {
                return Err("division is not an action idiom".to_string());
            }
            let mut j = i + 1;
            let mut in_class = false;
            let mut body = String::new();
            loop {
                let Some(&d) = chars.get(j) else {
                    return Err("unterminated regex literal".to_string());
                };
                if d == '\\' {
                    body.push(d);
                    if let Some(&e) = chars.get(j + 1) {
                        body.push(e);
                    }
                    j += 2;
                    continue;
                }
                if d == '[' {
                    in_class = true;
                } else if d == ']' {
                    in_class = false;
                } else if d == '/' && !in_class {
                    break;
                }
                body.push(d);
                j += 1;
            }
            j += 1;
            let mut flags = String::new();
            while j < chars.len() && chars[j].is_ascii_alphabetic() {
                flags.push(chars[j]);
                j += 1;
            }
            out.push(Tok::Regex(body, flags));
            i = j;
            continue;
        }
        if c == '\'' || c == '"' {
            let quote = c;
            let mut j = i + 1;
            let mut s = String::new();
            loop {
                let Some(&d) = chars.get(j) else {
                    return Err("unterminated string literal".to_string());
                };
                if d == quote {
                    break;
                }
                if d == '\\' {
                    let Some(&e) = chars.get(j + 1) else {
                        return Err("unterminated string literal".to_string());
                    };
                    s.push(match e {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        other => other,
                    });
                    j += 2;
                    continue;
                }
                s.push(d);
                j += 1;
            }
            out.push(Tok::Str(s));
            i = j + 1;
            continue;
        }
        if c.is_ascii_digit() {
            let mut j = i;
            while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '.') {
                j += 1;
            }
            let text: String = chars[i..j].iter().collect();
            let n: f64 = text.parse().map_err(|_| format!("bad number {text}"))?;
            out.push(Tok::Num(n));
            i = j;
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' || c == '$' {
            let mut j = i;
            while j < chars.len()
                && (chars[j].is_ascii_alphanumeric() || chars[j] == '_' || chars[j] == '$')
            {
                j += 1;
            }
            out.push(Tok::Ident(chars[i..j].iter().collect()));
            i = j;
            continue;
        }
        let rest: String = chars[i..chars.len().min(i + 3)].iter().collect();
        let Some(p) = PUNCTS.iter().find(|p| rest.starts_with(**p)) else {
            return Err(format!("unexpected character {c:?}"));
        };
        out.push(Tok::Punct(p));
        i += p.chars().count();
    }
    Ok(out)
}

struct Parser {
    tokens: Vec<Tok>,
    pos: usize,
    /// `const re = /.../;` bindings, consumed by a later `replace(re, ...)`.
    regex_names: Vec<(String, String, String)>,
}

impl Parser {
    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn peek_at(&self, n: usize) -> Option<&Tok> {
        self.tokens.get(self.pos + n)
    }

    fn next(&mut self) -> Result<Tok, String> {
        let t = self
            .tokens
            .get(self.pos)
            .cloned()
            .ok_or("unexpected end of action")?;
        self.pos += 1;
        Ok(t)
    }

    fn is_punct(&self, p: &str) -> bool {
        matches!(self.peek(), Some(Tok::Punct(q)) if *q == p)
    }

    fn is_ident(&self, name: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(q)) if q == name)
    }

    fn expect_punct(&mut self, p: &str) -> Result<(), String> {
        if self.is_punct(p) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!("expected `{p}`, found {:?}", self.peek()))
        }
    }

    fn expect_ident(&mut self) -> Result<String, String> {
        match self.next()? {
            Tok::Ident(s) => Ok(s),
            other => Err(format!("expected identifier, found {other:?}")),
        }
    }

    fn expect_str(&mut self) -> Result<String, String> {
        match self.next()? {
            Tok::Str(s) => Ok(s),
            other => Err(format!("expected string literal, found {other:?}")),
        }
    }

    fn optional_semicolon(&mut self) {
        while self.is_punct(";") {
            self.pos += 1;
        }
    }

    /// Skips a balanced `( ... )` whose content does not matter.
    fn skip_parens(&mut self) -> Result<(), String> {
        self.expect_punct("(")?;
        let mut depth = 1;
        while depth > 0 {
            match self.next()? {
                Tok::Punct("(") => depth += 1,
                Tok::Punct(")") => depth -= 1,
                _ => {}
            }
        }
        Ok(())
    }

    /// One statement; a `yytext` method chain yields several ops.
    fn statement(&mut self) -> Result<Vec<Op>, String> {
        if self.is_punct(";") {
            self.pos += 1;
            return Ok(Vec::new());
        }
        let ops = match self.next()? {
            Tok::Ident(w) if w == "this" => self.this_call()?,
            Tok::Ident(w) if w == "return" => self.return_stmt()?,
            Tok::Ident(w) if w == "yytext" => {
                self.expect_punct("=")?;
                self.yytext_assignment()?
            }
            Tok::Ident(w) if w == "const" || w == "let" || w == "var" => {
                let name = self.expect_ident()?;
                self.expect_punct("=")?;
                match self.next()? {
                    Tok::Regex(body, flags) => self.regex_names.push((name, body, flags)),
                    other => {
                        return Err(format!(
                            "only regex bindings are supported, found {other:?}"
                        ));
                    }
                }
                Vec::new()
            }
            Tok::Ident(w) if w == "yy" => self.yy_statement()?,
            Tok::Ident(w) if w == "if" => return self.if_stmt().map(|op| vec![op]),
            Tok::Ident(w) if w == "throw" => self.throw_stmt()?,
            Tok::Ident(w) => {
                // A bare identifier as a statement, like the `c` in
                // c4Diagram's dead comment rule, is a ReferenceError at run
                // time in JavaScript; keep that behavior.
                vec![Op::Throw(vec![Part::Lit(format!("{w} is not defined"))])]
            }
            other => return Err(format!("unexpected {other:?}")),
        };
        self.optional_semicolon();
        Ok(ops)
    }

    fn this_call(&mut self) -> Result<Vec<Op>, String> {
        self.expect_punct(".")?;
        let method = self.expect_ident()?;
        self.expect_punct("(")?;
        let op = match method.as_str() {
            "begin" | "pushState" => {
                let cond = self.expect_str()?;
                self.expect_punct(")")?;
                Op::Begin(cond)
            }
            "popState" => {
                // jison-lex ignores any argument to popState.
                if !self.is_punct(")") {
                    self.expect_str()?;
                }
                self.expect_punct(")")?;
                Op::PopState
            }
            "unput" => {
                if !self.is_ident("yytext") {
                    return Err("only unput(yytext) is supported".to_string());
                }
                self.pos += 1;
                self.expect_punct(")")?;
                Op::Unput
            }
            "less" => {
                match self.next()? {
                    Tok::Num(0.0) => {}
                    other => return Err(format!("only less(0) is supported, found {other:?}")),
                }
                self.expect_punct(")")?;
                Op::Unput
            }
            other => return Err(format!("this.{other}() is not supported")),
        };
        Ok(vec![op])
    }

    fn return_stmt(&mut self) -> Result<Vec<Op>, String> {
        if self.at_end() || self.is_punct(";") || self.is_punct("}") {
            return Ok(vec![Op::ReturnNothing]);
        }
        let token = match self.next()? {
            Tok::Str(s) => Token::Name(s),
            Tok::Num(n) if n >= 0.0 && n.fract() == 0.0 => Token::Id(n as u32),
            Tok::Ident(w) if w == "yytext" => {
                self.expect_punct("[")?;
                match self.next()? {
                    Tok::Num(0.0) => {}
                    other => return Err(format!("only yytext[0] is supported, found {other:?}")),
                }
                self.expect_punct("]")?;
                Token::FirstChar
            }
            other => return Err(format!("unsupported return value {other:?}")),
        };
        Ok(vec![Op::Return(token)])
    }

    fn yytext_assignment(&mut self) -> Result<Vec<Op>, String> {
        match self.next()? {
            Tok::Str(s) => Ok(vec![Op::SetText(s)]),
            Tok::Num(n) => Ok(vec![Op::SetText(format_js_number(n))]),
            Tok::Punct("-") => match self.next()? {
                Tok::Num(n) => Ok(vec![Op::SetText(format_js_number(-n))]),
                other => Err(format!("unsupported yytext value -{other:?}")),
            },
            Tok::Ident(w) if w == "yytext" => {
                // A chain of string methods on yytext. Each becomes one op,
                // so `yytext.slice(0,-8).trim()` yields Slice then Trim.
                let mut ops = Vec::new();
                while self.is_punct(".") {
                    self.pos += 1;
                    let method = self.expect_ident()?;
                    self.expect_punct("(")?;
                    ops.push(self.string_method(&method)?);
                }
                Ok(ops)
            }
            other => Err(format!("unsupported yytext value {other:?}")),
        }
    }

    fn string_method(&mut self, method: &str) -> Result<Op, String> {
        let op = match method {
            "trim" => {
                self.expect_punct(")")?;
                Op::Trim
            }
            "slice" | "substr" => {
                let start = self.int_arg()?;
                let mut end = None;
                if self.is_punct(",") {
                    self.pos += 1;
                    let second = self.int_arg()?;
                    end = Some(if method == "substr" {
                        if start < 0 || second < 0 {
                            return Err("substr with negative arguments".to_string());
                        }
                        start + second
                    } else {
                        second
                    });
                }
                self.expect_punct(")")?;
                if method == "substr" && start < 0 {
                    return Err("substr with a negative start".to_string());
                }
                Op::Slice { start, end }
            }
            "replace" => {
                let (body, flags) = match self.next()? {
                    Tok::Regex(body, flags) => (body, flags),
                    Tok::Ident(name) => {
                        let Some((_, body, flags)) =
                            self.regex_names.iter().rev().find(|(n, _, _)| *n == name)
                        else {
                            return Err(format!("replace with unknown regex {name}"));
                        };
                        (body.clone(), flags.clone())
                    }
                    other => return Err(format!("replace with {other:?}")),
                };
                self.expect_punct(",")?;
                let replacement = self.expect_str()?;
                self.expect_punct(")")?;
                if replacement.contains('$') {
                    return Err("replacement patterns with `$` are not supported".to_string());
                }
                let mut global = false;
                let mut case_insensitive = false;
                for f in flags.chars() {
                    match f {
                        'g' => global = true,
                        'i' => case_insensitive = true,
                        other => return Err(format!("regex flag {other} is not supported")),
                    }
                }
                let regex =
                    jsregex::compile_free(&body, case_insensitive).map_err(|e| e.to_string())?;
                Op::Replace(Replace {
                    pattern: body,
                    replacement,
                    global,
                    regex,
                })
            }
            other => return Err(format!("yytext.{other}() is not supported")),
        };
        Ok(op)
    }

    fn int_arg(&mut self) -> Result<i64, String> {
        let negative = if self.is_punct("-") {
            self.pos += 1;
            true
        } else {
            false
        };
        match self.next()? {
            Tok::Num(n) if n.fract() == 0.0 => Ok(if negative { -(n as i64) } else { n as i64 }),
            other => Err(format!("expected integer, found {other:?}")),
        }
    }

    /// `yy.getLogger().x(...)`, `yy.field = value`, `yy.method(args)`.
    fn yy_statement(&mut self) -> Result<Vec<Op>, String> {
        let path = self.yy_path()?;
        if path == "getLogger" {
            self.skip_parens()?;
            self.expect_punct(".")?;
            self.expect_ident()?;
            self.skip_parens()?;
            return Ok(Vec::new());
        }
        if self.is_punct("=") {
            self.pos += 1;
            let value = self.literal()?;
            return Ok(vec![Op::YySet(path, value)]);
        }
        if self.is_punct("(") {
            let args = self.call_args()?;
            return Ok(vec![Op::YyCall(path, args)]);
        }
        Err(format!("unsupported use of yy.{path}"))
    }

    /// The dotted path after `yy.`, such as `lex.firstGraph`.
    fn yy_path(&mut self) -> Result<String, String> {
        self.expect_punct(".")?;
        let mut path = self.expect_ident()?;
        while self.is_punct(".") && matches!(self.peek_at(1), Some(Tok::Ident(_))) {
            self.pos += 1;
            path.push('.');
            path.push_str(&self.expect_ident()?);
        }
        Ok(path)
    }

    fn call_args(&mut self) -> Result<Vec<Lit>, String> {
        self.expect_punct("(")?;
        let mut args = Vec::new();
        while !self.is_punct(")") {
            args.push(self.literal()?);
            if self.is_punct(",") {
                self.pos += 1;
            }
        }
        self.expect_punct(")")?;
        Ok(args)
    }

    fn literal(&mut self) -> Result<Lit, String> {
        match self.next()? {
            Tok::Ident(w) if w == "true" => Ok(Lit::Bool(true)),
            Tok::Ident(w) if w == "false" => Ok(Lit::Bool(false)),
            Tok::Ident(w) if w == "undefined" => Ok(Lit::Undefined),
            Tok::Str(s) => Ok(Lit::Str(s)),
            Tok::Num(n) => Ok(Lit::Num(n)),
            other => Err(format!("expected literal, found {other:?}")),
        }
    }

    fn if_stmt(&mut self) -> Result<Op, String> {
        self.expect_punct("(")?;
        let cond = self.condition()?;
        self.expect_punct(")")?;
        let then = self.block_or_statement()?;
        let otherwise = if self.is_ident("else") {
            self.pos += 1;
            self.block_or_statement()?
        } else {
            Vec::new()
        };
        Ok(Op::If {
            cond,
            then,
            otherwise,
        })
    }

    fn block_or_statement(&mut self) -> Result<Vec<Op>, String> {
        let mut ops = Vec::new();
        if self.is_punct("{") {
            self.pos += 1;
            while !self.is_punct("}") {
                if self.at_end() {
                    return Err("unterminated block".to_string());
                }
                ops.extend(self.statement()?);
            }
            self.pos += 1;
        } else {
            ops.extend(self.statement()?);
        }
        Ok(ops)
    }

    fn condition(&mut self) -> Result<Cond, String> {
        let mut cond = self.unary_condition()?;
        while self.is_punct("&&") {
            self.pos += 1;
            let rhs = self.unary_condition()?;
            cond = Cond::And(Box::new(cond), Box::new(rhs));
        }
        Ok(cond)
    }

    fn unary_condition(&mut self) -> Result<Cond, String> {
        if self.is_punct("!") {
            self.pos += 1;
            return Ok(Cond::Not(Box::new(self.unary_condition()?)));
        }
        match self.next()? {
            Tok::Ident(w) if w == "processId" => {
                self.expect_punct("(")?;
                self.expect_punct(")")?;
                Ok(Cond::ProcessId)
            }
            Tok::Ident(w) if w == "yy" => {
                let path = self.yy_path()?;
                if self.is_punct("(") {
                    let args = self.call_args()?;
                    Ok(Cond::YyCall(path, args))
                } else {
                    Ok(Cond::YyGet(path))
                }
            }
            other => Err(format!("unsupported condition {other:?}")),
        }
    }

    /// `throw new Error(<string> + yytext.trim() + <string> ...)`.
    fn throw_stmt(&mut self) -> Result<Vec<Op>, String> {
        if !self.is_ident("new") {
            return Err("only `throw new Error(...)` is supported".to_string());
        }
        self.pos += 1;
        if self.expect_ident()? != "Error" {
            return Err("only `throw new Error(...)` is supported".to_string());
        }
        self.expect_punct("(")?;
        let mut parts = Vec::new();
        loop {
            match self.next()? {
                Tok::Str(s) => parts.push(Part::Lit(s)),
                Tok::Ident(w) if w == "yytext" => {
                    if self.is_punct(".") {
                        self.pos += 1;
                        if self.expect_ident()? != "trim" {
                            return Err("only yytext.trim() is supported in a message".to_string());
                        }
                        self.expect_punct("(")?;
                        self.expect_punct(")")?;
                        parts.push(Part::YyTextTrim);
                    } else {
                        parts.push(Part::YyText);
                    }
                }
                other => return Err(format!("unsupported message part {other:?}")),
            }
            if self.is_punct("+") {
                self.pos += 1;
                continue;
            }
            break;
        }
        self.expect_punct(")")?;
        Ok(vec![Op::Throw(parts)])
    }
}

/// JavaScript `String(number)` for the integers that appear in actions.
fn format_js_number(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(src: &str) -> Program {
        translate("test", 0, src).expect("translates")
    }

    #[test]
    fn plain_return_and_comments() {
        assert_eq!(
            t("return 'NEWLINE';"),
            vec![Op::Return(Token::Name("NEWLINE".into()))]
        );
        assert_eq!(
            t("return \"STR\""),
            vec![Op::Return(Token::Name("STR".into()))]
        );
        assert_eq!(t("/* skip comments */"), Vec::<Op>::new());
        assert_eq!(t(""), Vec::<Op>::new());
        assert_eq!(t("return 12;"), vec![Op::Return(Token::Id(12))]);
        assert_eq!(
            t("return 'SPACELIST' /* skip all whitespace */ ;"),
            vec![Op::Return(Token::Name("SPACELIST".into()))]
        );
    }

    #[test]
    fn state_changes() {
        assert_eq!(
            t("this.begin(\"dir\"); return 'GRAPH';"),
            vec![
                Op::Begin("dir".into()),
                Op::Return(Token::Name("GRAPH".into()))
            ]
        );
        assert_eq!(
            t("this.pushState('CLASSDEF'); return 'classDef';"),
            vec![
                Op::Begin("CLASSDEF".into()),
                Op::Return(Token::Name("classDef".into()))
            ]
        );
        assert_eq!(
            t("this.popState(); this.popState(); return 'STRUCT_STOP';"),
            vec![
                Op::PopState,
                Op::PopState,
                Op::Return(Token::Name("STRUCT_STOP".into()))
            ]
        );
        assert_eq!(
            t("this.popState('escaped_text'); return 'DQUOTE';"),
            vec![Op::PopState, Op::Return(Token::Name("DQUOTE".into()))]
        );
        assert_eq!(
            t("this.popState(); this.less(0);"),
            vec![Op::PopState, Op::Unput]
        );
    }

    #[test]
    fn yytext_edits() {
        assert_eq!(
            t("yytext = yytext.trim(); return 'unqString';"),
            vec![Op::Trim, Op::Return(Token::Name("unqString".into()))]
        );
        assert_eq!(
            t("yytext=yytext.slice(0,-8).trim();return 'JOIN';"),
            vec![
                Op::Slice {
                    start: 0,
                    end: Some(-8)
                },
                Op::Trim,
                Op::Return(Token::Name("JOIN".into()))
            ]
        );
        assert_eq!(
            t("yytext = yytext.substr(2).trim(); return 'NOTE_TEXT';"),
            vec![
                Op::Slice {
                    start: 2,
                    end: None
                },
                Op::Trim,
                Op::Return(Token::Name("NOTE_TEXT".into()))
            ]
        );
        assert_eq!(
            t("yytext = yytext.slice(2, -2);"),
            vec![Op::Slice {
                start: 2,
                end: Some(-2)
            }]
        );
        assert_eq!(
            t("yytext=yytext.slice(1);"),
            vec![Op::Slice {
                start: 1,
                end: None
            }]
        );
        assert_eq!(t("yytext = '1';"), vec![Op::SetText("1".into())]);
        assert_eq!(
            t("yytext=-1; return 'COLUMNS';")[0],
            Op::SetText("-1".into())
        );
        assert_eq!(t("yytext=\"\";"), vec![Op::SetText(String::new())]);
    }

    #[test]
    fn replace_literal_and_bound_regex() {
        let p = t("yytext = yytext.replace(/^,\\s*/, ''); return \"DIR\";");
        match &p[0] {
            Op::Replace(r) => {
                assert_eq!(r.pattern, "^,\\s*");
                assert_eq!(r.replacement, "");
                assert!(!r.global);
            }
            other => panic!("unexpected {other:?}"),
        }
        let p = t(
            "const re = /\\n\\s*/g;\nyytext = yytext.replace(re,\"<br/>\");\nreturn 'SHAPE_DATA'",
        );
        match &p[0] {
            Op::Replace(r) => {
                assert_eq!(r.pattern, "\\n\\s*");
                assert_eq!(r.replacement, "<br/>");
                assert!(r.global);
            }
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(p[1], Op::Return(Token::Name("SHAPE_DATA".into())));
    }

    #[test]
    fn logger_calls_are_dropped() {
        assert_eq!(
            t("this.popState();yy.getLogger().trace('node end ((');return \"NODE_DEND\";"),
            vec![Op::PopState, Op::Return(Token::Name("NODE_DEND".into()))]
        );
        assert_eq!(
            t("yy.getLogger().debug('Lex: LINK', '#'+yytext+'#'); return 'LINK';"),
            vec![Op::Return(Token::Name("LINK".into()))]
        );
        assert_eq!(
            t("yy.getLogger().debug('.', yytext); /* skip all whitespace */"),
            vec![]
        );
    }

    #[test]
    fn conditionals_and_yy_hooks() {
        assert_eq!(
            t("if(yy.lex.firstGraph()){this.begin(\"dir\");}  return 'GRAPH';"),
            vec![
                Op::If {
                    cond: Cond::YyCall("lex.firstGraph".into(), vec![]),
                    then: vec![Op::Begin("dir".into())],
                    otherwise: vec![],
                },
                Op::Return(Token::Name("GRAPH".into()))
            ]
        );
        assert_eq!(
            t(
                "if (yy.getIndentMode && yy.getIndentMode()) {\n yy.consumeIndentText = true;\n this.begin('INITIAL');\n return 'INDENT_TEXT';\n }"
            ),
            vec![Op::If {
                cond: Cond::And(
                    Box::new(Cond::YyGet("getIndentMode".into())),
                    Box::new(Cond::YyCall("getIndentMode".into(), vec![]))
                ),
                then: vec![
                    Op::YySet("consumeIndentText".into(), Lit::Bool(true)),
                    Op::Begin("INITIAL".into()),
                    Op::Return(Token::Name("INDENT_TEXT".into())),
                ],
                otherwise: vec![],
            }]
        );
        assert_eq!(
            t(
                "if (yy.setIndentMode) { yy.setIndentMode(false); } this.begin('INITIAL'); this.unput(yytext);"
            ),
            vec![
                Op::If {
                    cond: Cond::YyGet("setIndentMode".into()),
                    then: vec![Op::YyCall("setIndentMode".into(), vec![Lit::Bool(false)])],
                    otherwise: vec![],
                },
                Op::Begin("INITIAL".into()),
                Op::Unput,
            ]
        );
        assert_eq!(
            t(
                "if (yy.consumeIndentText) { yy.consumeIndentText = false; } else { return 'TEXT'; }"
            ),
            vec![Op::If {
                cond: Cond::YyGet("consumeIndentText".into()),
                then: vec![Op::YySet("consumeIndentText".into(), Lit::Bool(false))],
                otherwise: vec![Op::Return(Token::Name("TEXT".into()))],
            }]
        );
        assert_eq!(
            t("if (!processId()) return; this.popState(); return \"ID\";"),
            vec![
                Op::If {
                    cond: Cond::Not(Box::new(Cond::ProcessId)),
                    then: vec![Op::ReturnNothing],
                    otherwise: vec![],
                },
                Op::PopState,
                Op::Return(Token::Name("ID".into())),
            ]
        );
    }

    #[test]
    fn throw_and_first_char() {
        assert_eq!(
            t(
                "throw new Error('Error: State name must be a single word. Found: \"' + yytext.trim() + '\"');"
            ),
            vec![Op::Throw(vec![
                Part::Lit("Error: State name must be a single word. Found: \"".into()),
                Part::YyTextTrim,
                Part::Lit("\"".into()),
            ])]
        );
        assert_eq!(t("return yytext[0];"), vec![Op::Return(Token::FirstChar)]);
        assert_eq!(
            t("c /* skip comments */"),
            vec![Op::Throw(vec![Part::Lit("c is not defined".into())])]
        );
    }

    #[test]
    fn unsupported_idiom_names_grammar_rule_and_source() {
        let e = translate("flow", 7, "this.more(); return 'X';").expect_err("unsupported");
        assert_eq!(e.grammar, "flow");
        assert_eq!(e.rule, 7);
        assert_eq!(e.source, "this.more(); return 'X';");
        assert!(e.to_string().contains("flow lexer rule 7"));
        assert!(e.to_string().contains("this.more()"));
        assert!(translate("x", 0, "yytext = yytext.toUpperCase();").is_err());
        assert!(translate("x", 0, "if (foo) return 'A';").is_err());
        assert!(translate("x", 0, "yy.db.add(yytext);").is_err());
        assert!(translate("x", 0, "return a + b;").is_err());
    }

    #[test]
    fn include_check() {
        assert!(check_include("x", None).is_ok());
        assert!(check_include("x", Some("\n\t// Pre-lexer code can go here\n")).is_ok());
        assert!(check_include("x", Some("function f() { return 1; }")).is_err());
    }
}
