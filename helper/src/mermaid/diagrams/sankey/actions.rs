//! The grammar actions of `sankey.jison`, keyed by production index in
//! `helper/grammars/sankey.tables.json`. The lexer actions only push and
//! pop states, so the default `Yy` is enough.

use super::db::SankeyDb;
use crate::mermaid::jison::lexer::js_trim;
use crate::mermaid::jison::{Actions, Loc, ParseError, Yy};

/// JavaScript `parseFloat`: the longest prefix of the trimmed-start text
/// that is a decimal literal or `Infinity`, else NaN.
pub fn js_parse_float(text: &str) -> f64 {
    let s = text.trim_start_matches(|c: char| js_trim(c.encode_utf8(&mut [0; 4])).is_empty());
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut sign = 1.0;
    if let Some(&c) = bytes.first()
        && (c == b'+' || c == b'-')
    {
        if c == b'-' {
            sign = -1.0;
        }
        i = 1;
    }
    if s[i..].starts_with("Infinity") {
        return sign * f64::INFINITY;
    }
    let digits = |from: usize| {
        bytes[from..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count()
    };
    let int = digits(i);
    let mut end = i + int;
    let mut frac = 0;
    if bytes.get(end) == Some(&b'.') {
        frac = digits(end + 1);
        if int > 0 || frac > 0 {
            end += 1 + frac;
        }
    }
    if int == 0 && frac == 0 {
        return f64::NAN;
    }
    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        let mut j = end + 1;
        if matches!(bytes.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        let exp = digits(j);
        if exp > 0 {
            end = j + exp;
        }
    }
    s[..end].parse::<f64>().unwrap_or(f64::NAN)
}

impl Yy for SankeyDb {}

impl Actions for SankeyDb {
    type Value = String;

    fn terminal(&mut self, yytext: &str) -> String {
        yytext.to_string()
    }

    fn reduce(
        &mut self,
        production: usize,
        rhs: &[String],
        locs: &[Loc],
        _yytext: &str,
    ) -> Result<String, ParseError> {
        let at = |i: usize| rhs.get(i).cloned().unwrap_or_default();
        let value = match production {
            // record: field[source] COMMA field[target] COMMA field[value]
            7 => {
                let source = self.find_or_create_node(&js_trim(&at(0)).replace("\"\"", "\""));
                let target = self.find_or_create_node(&js_trim(&at(2)).replace("\"\"", "\""));
                let value = js_parse_float(js_trim(&at(4)));
                self.add_link(source, target, value);
                String::new()
            }
            // field: escaped | non_escaped; non_escaped: NON_ESCAPED_TEXT
            8 | 9 | 11 => at(0),
            // escaped: DQUOTE ESCAPED_TEXT DQUOTE { $$=$ESCAPED_TEXT; }
            10 => at(1),
            // No action: jison's default `$$ = $1`, nothing for an empty
            // right-hand side.
            1..=6 => at(0),
            other => {
                let loc = locs.first().copied().unwrap_or_default();
                return Err(ParseError::action(
                    loc.first_line,
                    loc.first_column,
                    format!("sankey grammar has no production {other}"),
                ));
            }
        };
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_float_reads_the_longest_number_prefix() {
        assert_eq!(js_parse_float("12.5"), 12.5);
        assert_eq!(js_parse_float("  3e2x"), 300.0);
        assert_eq!(js_parse_float(".5"), 0.5);
        assert_eq!(js_parse_float("5."), 5.0);
        assert_eq!(js_parse_float("-Infinity"), f64::NEG_INFINITY);
        assert_eq!(js_parse_float("1e"), 1.0);
        assert!(js_parse_float("abc").is_nan());
        assert!(js_parse_float("").is_nan());
        assert!(js_parse_float(".").is_nan());
    }
}
