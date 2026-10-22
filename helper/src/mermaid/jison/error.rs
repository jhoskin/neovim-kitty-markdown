//! Parse and lexical errors in the text jison produces, byte for byte.
//!
//! mermaid surfaces the jison `Error.message` unchanged, so the preview's
//! failure box and `:messages` text (SPEC.md R-111, R-177, R-178) come
//! straight from `Display`.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// `table[state][symbol]` had no action.
    Parse,
    /// No lexer rule matched the remaining input.
    Lexical,
    /// A lexer action or a grammar action threw.
    Action,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ErrorKind,
    /// The 1-based line jison prints: the parser's `yylineno + 1` for a
    /// parse error, the lexer's for a lexical error.
    pub line: usize,
    /// `yylloc.first_column` of the offending token, 0-based.
    pub column: usize,
    /// The complete text, what `Display` prints.
    pub message: String,
    /// Terminal names jison would accept here, unquoted, in table order.
    pub expected: Vec<String>,
    /// The terminal name jison reports after `got`.
    pub got: String,
}

impl ParseError {
    pub fn parse(
        line: usize,
        column: usize,
        position: &str,
        expected: Vec<String>,
        got: String,
    ) -> ParseError {
        let quoted: Vec<String> = expected.iter().map(|e| format!("'{e}'")).collect();
        let message = format!(
            "Parse error on line {line}:\n{position}\nExpecting {}, got '{got}'",
            quoted.join(", ")
        );
        ParseError {
            kind: ErrorKind::Parse,
            line,
            column,
            message,
            expected,
            got,
        }
    }

    pub fn lexical(line: usize, column: usize, position: &str) -> ParseError {
        ParseError {
            kind: ErrorKind::Lexical,
            line,
            column,
            message: format!("Lexical error on line {line}. Unrecognized text.\n{position}"),
            expected: Vec::new(),
            got: String::new(),
        }
    }

    /// An error thrown by an action, with the message as the oracle shows it.
    pub fn action(line: usize, column: usize, message: impl Into<String>) -> ParseError {
        ParseError {
            kind: ErrorKind::Action,
            line,
            column,
            message: message.into(),
            expected: Vec::new(),
            got: String::new(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_text_matches_jison_template() {
        let e = ParseError::parse(
            2,
            0,
            "graph TD\n--------^",
            vec!["SPACE".into(), "NEWLINE".into()],
            "EOF".into(),
        );
        assert_eq!(
            e.to_string(),
            "Parse error on line 2:\ngraph TD\n--------^\nExpecting 'SPACE', 'NEWLINE', got 'EOF'"
        );
    }

    #[test]
    fn lexical_error_text_matches_jison_template() {
        let e = ParseError::lexical(1, 3, "abc$\n---^");
        assert_eq!(
            e.to_string(),
            "Lexical error on line 1. Unrecognized text.\nabc$\n---^"
        );
    }
}
