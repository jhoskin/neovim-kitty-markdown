//! d3-format 3.1.2 `format(specifier)` in the default (en-US) locale, for
//! the treemap `valueFormat`: `formatSpecifier`, `newFormat` with fill,
//! alignment, sign, symbol, zero padding, width, grouping, precision, trim,
//! and every type (`e f g r s % p b o d x X c n` and none).
//!
//! The JavaScript number methods are ported on the exact decimal value of
//! the double: `toFixed`, `toExponential`, and `toPrecision` pick the
//! nearer candidate and, on an exact tie, the larger one, where Rust's
//! formatter would round the tie to even.

use std::fmt;

use crate::mermaid::layout::d3::hierarchy::js_round;
use crate::mermaid::theme::khroma::js_number;

/// The locale's minus sign, U+2212.
const MINUS: &str = "\u{2212}";
/// `formatPrefix`'s SI prefixes from 1e-24 to 1e24.
const PREFIXES: [&str; 17] = [
    "y", "z", "a", "f", "p", "n", "\u{b5}", "m", "", "k", "M", "G", "T", "P", "E", "Z", "Y",
];

/// The error `formatSpecifier` throws: `invalid format: <specifier>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidFormat(pub String);

impl fmt::Display for InvalidFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid format: {}", self.0)
    }
}

impl std::error::Error for InvalidFormat {}

/// `FormatSpecifier` after `newFormat` normalized it.
#[derive(Debug, Clone, PartialEq)]
pub struct Format {
    fill: String,
    align: char,
    sign: char,
    symbol: Option<char>,
    zero: bool,
    width: usize,
    comma: bool,
    precision: usize,
    trim: bool,
    ty: char,
}

/// `formatSpecifier(specifier)`: the regex
/// `^(?:(.)?([<>=^]))?([+\-( ])?([$#])?(0)?(\d+)?(,)?(\.\d+)?(~)?([a-z%])?$`
/// with the `i` flag, matched by hand.
struct Parsed {
    fill: Option<char>,
    align: Option<char>,
    sign: Option<char>,
    symbol: Option<char>,
    zero: bool,
    width: Option<usize>,
    comma: bool,
    precision: Option<usize>,
    trim: bool,
    ty: Option<char>,
}

fn is_align(c: char) -> bool {
    matches!(c, '<' | '>' | '=' | '^')
}

fn parse_specifier(text: &str) -> Option<Parsed> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let (mut fill, mut align) = (None, None);
    // `(?:(.)?([<>=^]))?`: a fill needs an alignment after it; the regex
    // tries the two-character form first.
    if chars.len() >= 2 && is_align(chars[1]) && chars[0] != '\n' {
        fill = Some(chars[0]);
        align = Some(chars[1]);
        i = 2;
    } else if chars.first().is_some_and(|c| is_align(*c)) {
        align = Some(chars[0]);
        i = 1;
    }
    let at = |i: usize| chars.get(i).copied();
    let mut sign = None;
    if let Some(c) = at(i)
        && matches!(c, '+' | '-' | '(' | ' ')
    {
        sign = Some(c);
        i += 1;
    }
    let mut symbol = None;
    if let Some(c) = at(i)
        && matches!(c, '$' | '#')
    {
        symbol = Some(c);
        i += 1;
    }
    let mut zero = false;
    if at(i) == Some('0') {
        zero = true;
        i += 1;
    }
    let start = i;
    while at(i).is_some_and(|c| c.is_ascii_digit()) {
        i += 1;
    }
    let width = (i > start).then(|| digits_value(&chars[start..i]));
    let mut comma = false;
    if at(i) == Some(',') {
        comma = true;
        i += 1;
    }
    let mut precision = None;
    if at(i) == Some('.') {
        let start = i + 1;
        let mut j = start;
        while at(j).is_some_and(|c| c.is_ascii_digit()) {
            j += 1;
        }
        if j == start {
            return None;
        }
        precision = Some(digits_value(&chars[start..j]));
        i = j;
    }
    let mut trim = false;
    if at(i) == Some('~') {
        trim = true;
        i += 1;
    }
    let mut ty = None;
    if let Some(c) = at(i)
        && (c.is_ascii_alphabetic() || c == '%')
    {
        ty = Some(c);
        i += 1;
    }
    (i == chars.len()).then_some(Parsed {
        fill,
        align,
        sign,
        symbol,
        zero,
        width,
        comma,
        precision,
        trim,
        ty,
    })
}

/// `+digits`; a width beyond `usize` saturates, as no padding could reach it.
fn digits_value(digits: &[char]) -> usize {
    digits.iter().fold(0usize, |acc, c| {
        acc.saturating_mul(10)
            .saturating_add(c.to_digit(10).unwrap_or(0) as usize)
    })
}

/// The types `formatTypes` defines.
fn is_format_type(c: char) -> bool {
    matches!(
        c,
        '%' | 'b' | 'c' | 'd' | 'e' | 'f' | 'g' | 'o' | 'p' | 'r' | 's' | 'X' | 'x'
    )
}

impl Format {
    /// `locale.format(specifier)`.
    pub fn new(specifier: &str) -> Result<Format, InvalidFormat> {
        let p = parse_specifier(specifier).ok_or_else(|| InvalidFormat(specifier.to_string()))?;
        let mut fill = p.fill.map_or(" ".to_string(), String::from);
        let mut align = p.align.unwrap_or('>');
        let mut zero = p.zero;
        let mut comma = p.comma;
        let mut precision = p.precision;
        let mut trim = p.trim;
        let mut ty = p.ty.unwrap_or('\0');
        if ty == 'n' {
            // The "n" type is an alias for ",g".
            comma = true;
            ty = 'g';
        } else if !is_format_type(ty) {
            // The "" type, and any invalid type, is an alias for ".12~g".
            precision = precision.or(Some(12));
            trim = true;
            ty = 'g';
        }
        // Zero fill goes after the sign and before the digits.
        if zero || (fill == "0" && align == '=') {
            zero = true;
            fill = "0".to_string();
            align = '=';
        }
        let precision = match precision {
            None => 6,
            Some(v) if matches!(ty, 'g' | 'p' | 'r' | 's') => v.clamp(1, 21),
            Some(v) => v.min(20),
        };
        Ok(Format {
            fill,
            align,
            sign: p.sign.unwrap_or('-'),
            symbol: p.symbol,
            zero,
            width: p.width.unwrap_or(0),
            comma,
            precision,
            trim,
            ty,
        })
    }

    /// `format(',')`, the fallback `draw` uses when a specifier throws.
    pub fn comma() -> Format {
        Format {
            fill: " ".to_string(),
            align: '>',
            sign: '-',
            symbol: None,
            zero: false,
            width: 0,
            comma: true,
            precision: 12,
            trim: true,
            ty: 'g',
        }
    }

    /// The `format(value)` closure.
    pub fn format(&self, value: f64) -> String {
        let ty = self.ty;
        let mut prefix = match self.symbol {
            Some('$') => "$".to_string(),
            Some('#') if matches!(ty, 'b' | 'o' | 'x' | 'X') => {
                format!("0{}", ty.to_ascii_lowercase())
            }
            _ => String::new(),
        };
        let mut suffix = if self.symbol != Some('$') && matches!(ty, '%' | 'p') {
            "%".to_string()
        } else {
            String::new()
        };
        let mut digits;
        if ty == 'c' {
            suffix = format!("{}{suffix}", js_number(value));
            digits = String::new();
        } else {
            let negative = value < 0.0 || (value == 0.0 && value.is_sign_negative());
            let mut prefix_exponent = 0;
            digits = if value.is_nan() {
                "NaN".to_string()
            } else {
                let (text, exp) = format_type(ty, value.abs(), self.precision);
                prefix_exponent = exp;
                text
            };
            if self.trim {
                digits = format_trim(&digits);
            }
            // `+value === 0`: a negative that rounds to zero loses its sign.
            let negative = negative && !(js_to_number(&digits) == Some(0.0) && self.sign != '+');
            let sign_text = if negative {
                if self.sign == '(' {
                    "(".to_string()
                } else {
                    MINUS.to_string()
                }
            } else if self.sign == '-' || self.sign == '(' {
                String::new()
            } else {
                self.sign.to_string()
            };
            prefix = format!("{sign_text}{prefix}");
            let si = if ty == 's' {
                PREFIXES[(8 + prefix_exponent / 3) as usize]
            } else {
                ""
            };
            let close = if negative && self.sign == '(' {
                ")"
            } else {
                ""
            };
            suffix = format!("{si}{suffix}{close}");
            if matches!(ty, 'd' | 'e' | 'f' | 'g' | 'p' | 'r' | 's' | '%')
                && let Some(i) = digits.find(|c: char| !c.is_ascii_digit())
            {
                // The decimal point stays the locale's ".".
                suffix = format!("{}{suffix}", &digits[i..]);
                digits.truncate(i);
            }
        }
        if self.comma && !self.zero {
            digits = group(&digits, usize::MAX);
        }
        let length = prefix.encode_utf16().count()
            + digits.encode_utf16().count()
            + suffix.encode_utf16().count();
        let mut padding = if length < self.width {
            self.fill.repeat(self.width - length)
        } else {
            String::new()
        };
        if self.comma && self.zero {
            let width = if padding.is_empty() {
                usize::MAX
            } else {
                self.width - suffix.encode_utf16().count()
            };
            digits = group(&format!("{padding}{digits}"), width);
            padding.clear();
        }
        match self.align {
            '<' => format!("{prefix}{digits}{suffix}{padding}"),
            '=' => format!("{prefix}{padding}{digits}{suffix}"),
            '^' => {
                let pad: Vec<char> = padding.chars().collect();
                let half = pad.len() >> 1;
                let left: String = pad[..half].iter().collect();
                let right: String = pad[half..].iter().collect();
                format!("{left}{prefix}{digits}{suffix}{right}")
            }
            _ => format!("{padding}{prefix}{digits}{suffix}"),
        }
    }
}

/// `+string` for the digit strings the types produce.
fn js_to_number(s: &str) -> Option<f64> {
    s.parse::<f64>().ok()
}

/// `formatTypes[type](x, p)` for a non-negative `x`, with the SI exponent
/// `formatPrefixAuto` sets for the `s` type.
fn format_type(ty: char, x: f64, p: usize) -> (String, i32) {
    match ty {
        '%' => (to_fixed(x * 100.0, p), 0),
        'b' => (to_radix(js_round(x), 2), 0),
        'd' => (format_decimal(x), 0),
        'e' => (to_exponential(x, Some(p)), 0),
        'f' => (to_fixed(x, p), 0),
        'g' => (to_precision(x, p), 0),
        'o' => (to_radix(js_round(x), 8), 0),
        'p' => (format_rounded(x * 100.0, p), 0),
        'r' => (format_rounded(x, p), 0),
        's' => format_prefix_auto(x, p),
        'X' => (to_radix(js_round(x), 16).to_ascii_uppercase(), 0),
        'x' => (to_radix(js_round(x), 16), 0),
        _ => (to_precision(x, p), 0),
    }
}

/// `formatGroup([3], ",")(value, width)`.
fn group(value: &str, width: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    let mut i = chars.len();
    let mut parts: Vec<String> = Vec::new();
    let mut length = 0usize;
    let mut g = 3usize;
    while i > 0 && g > 0 {
        if length.saturating_add(g + 1) > width {
            g = width.saturating_sub(length).max(1);
        }
        let start = i.saturating_sub(g);
        parts.push(chars[start..i].iter().collect());
        i = start;
        length = length.saturating_add(g + 1);
        if length > width {
            break;
        }
        g = 3;
    }
    parts.reverse();
    parts.join(",")
}

/// `formatTrim`: drops insignificant trailing zeros after the decimal
/// point, and the point when nothing is left after it.
fn format_trim(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let (mut i0, mut i1): (isize, isize) = (-1, 0);
    for (i, &c) in chars.iter().enumerate().skip(1) {
        let i = i as isize;
        match c {
            '.' => {
                i0 = i;
                i1 = i;
            }
            '0' => {
                if i0 == 0 {
                    i0 = i;
                }
                i1 = i;
            }
            // `!+s[i]`: a non-digit ends the scan.
            c if c.is_ascii_digit() => {
                if i0 > 0 {
                    i0 = 0;
                }
            }
            _ => break,
        }
    }
    if i0 > 0 {
        let head: String = chars[..i0 as usize].iter().collect();
        let tail: String = chars[(i1 + 1) as usize..].iter().collect();
        format!("{head}{tail}")
    } else {
        s.to_string()
    }
}

/// `formatDecimal`: `Math.round(x)` printed in full.
fn format_decimal(x: f64) -> String {
    let r = js_round(x);
    if r.abs() >= 1e21 {
        // `toLocaleString("en")` without separators: the integer digits.
        format!("{r:.0}")
    } else {
        js_number(r)
    }
}

/// `Number.prototype.toString(radix)` for an integral value.
fn to_radix(x: f64, radix: u32) -> String {
    if !x.is_finite() {
        return js_number(x);
    }
    let mut n = x.abs();
    if n == 0.0 {
        return "0".to_string();
    }
    let mut digits = Vec::new();
    let r = f64::from(radix);
    while n >= 1.0 {
        let d = (n % r) as u32;
        digits.push(std::char::from_digit(d, radix).unwrap_or('0'));
        n = (n / r).floor();
    }
    digits.reverse();
    let s: String = digits.into_iter().collect();
    if x < 0.0 { format!("-{s}") } else { s }
}

/// The exact decimal expansion of a finite, positive `x`: its significant
/// digits and the exponent of the first one (`x = d.ddd × 10^exp`).
fn exact_digits(x: f64) -> (Vec<u8>, i32) {
    // 800 significant digits cover every double exactly (at most 767).
    let text = format!("{x:.800e}");
    let (mantissa, exp) = text.split_once('e').unwrap_or((&text, "0"));
    let mut digits: Vec<u8> = mantissa
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|b| b - b'0')
        .collect();
    while digits.len() > 1 && digits.last() == Some(&0) {
        digits.pop();
    }
    (digits, exp.parse().unwrap_or(0))
}

/// `x` rounded to `n` significant digits, ties up: the digits and the
/// exponent of the first digit, which moves up when the rounding carries.
fn round_significant(x: f64, n: usize) -> (Vec<u8>, i32) {
    let (mut digits, mut exp) = exact_digits(x);
    if digits.len() <= n {
        digits.resize(n, 0);
        return (digits, exp);
    }
    let round_up = digits[n] >= 5;
    digits.truncate(n);
    if round_up {
        let mut i = n;
        loop {
            if i == 0 {
                digits.insert(0, 1);
                digits.truncate(n);
                exp += 1;
                break;
            }
            i -= 1;
            if digits[i] == 9 {
                digits[i] = 0;
            } else {
                digits[i] += 1;
                break;
            }
        }
    }
    (digits, exp)
}

fn digit_string(digits: &[u8]) -> String {
    digits.iter().map(|d| char::from(b'0' + d)).collect()
}

/// `Number.prototype.toFixed(p)` for a non-negative `x`: the exact
/// decimal expansion rounded at `p` places, ties up.
fn to_fixed(x: f64, p: usize) -> String {
    if !x.is_finite() || x >= 1e21 {
        return js_number(x);
    }
    // 1100 places hold every double's fraction exactly (at most 1074).
    let exact = format!("{x:.1100}");
    let (int, frac) = exact.split_once('.').unwrap_or((&exact, ""));
    let mut digits: Vec<u8> = int
        .bytes()
        .chain(frac.bytes().take(p))
        .map(|b| b - b'0')
        .collect();
    if frac.as_bytes().get(p).is_some_and(|b| *b >= b'5') {
        let mut i = digits.len();
        loop {
            if i == 0 {
                digits.insert(0, 1);
                break;
            }
            i -= 1;
            if digits[i] == 9 {
                digits[i] = 0;
            } else {
                digits[i] += 1;
                break;
            }
        }
    }
    let s = digit_string(&digits);
    if p == 0 {
        return s;
    }
    let split = s.len() - p;
    format!("{}.{}", &s[..split], &s[split..])
}

/// `Number.prototype.toExponential(p)`; `None` prints the shortest
/// round-trip digits.
fn to_exponential(x: f64, p: Option<usize>) -> String {
    if !x.is_finite() {
        return js_number(x);
    }
    let (digits, exp) = match p {
        Some(p) if x != 0.0 => round_significant(x, p + 1),
        Some(p) => (vec![0; p + 1], 0),
        None => {
            let text = format!("{x:e}");
            let (m, e) = text.split_once('e').unwrap_or((&text, "0"));
            let digits = m
                .bytes()
                .filter(u8::is_ascii_digit)
                .map(|b| b - b'0')
                .collect();
            (digits, e.parse().unwrap_or(0))
        }
    };
    let s = digit_string(&digits);
    let mantissa = if s.len() > 1 {
        format!("{}.{}", &s[..1], &s[1..])
    } else {
        s
    };
    let sign = if exp < 0 { "-" } else { "+" };
    format!("{mantissa}e{sign}{}", exp.abs())
}

/// `Number.prototype.toPrecision(p)` for a non-negative `x`.
fn to_precision(x: f64, p: usize) -> String {
    if !x.is_finite() {
        return js_number(x);
    }
    if x == 0.0 {
        return if p > 1 {
            format!("0.{}", "0".repeat(p - 1))
        } else {
            "0".to_string()
        };
    }
    let (digits, e) = round_significant(x, p);
    let s = digit_string(&digits);
    if e < -6 || e >= p as i32 {
        let mantissa = if p > 1 {
            format!("{}.{}", &s[..1], &s[1..])
        } else {
            s
        };
        let sign = if e < 0 { "-" } else { "+" };
        return format!("{mantissa}e{sign}{}", e.abs());
    }
    if e >= 0 {
        let int_len = (e + 1) as usize;
        if int_len >= s.len() {
            s
        } else {
            format!("{}.{}", &s[..int_len], &s[int_len..])
        }
    } else {
        format!("0.{}{s}", "0".repeat((-e - 1) as usize))
    }
}

/// `formatDecimalParts(x, p)`: the coefficient digits without the point
/// and the exponent of `x.toExponential(p - 1)`, or of `toExponential()`
/// when `p` is 0; `None` for a non-finite `x`.
fn decimal_parts(x: f64, p: usize) -> Option<(String, i32)> {
    if !x.is_finite() {
        return None;
    }
    let text = if p > 0 {
        to_exponential(x, Some(p - 1))
    } else {
        to_exponential(x, None)
    };
    let (coefficient, exp) = text.split_once('e')?;
    let digits: String = coefficient.chars().filter(|c| *c != '.').collect();
    Some((digits, exp.parse().ok()?))
}

/// `formatRounded(x, p)`.
fn format_rounded(x: f64, p: usize) -> String {
    let Some((coefficient, exponent)) = decimal_parts(x, p) else {
        return js_number(x);
    };
    if exponent < 0 {
        format!("0.{}{coefficient}", "0".repeat((-exponent - 1) as usize))
    } else if coefficient.len() > (exponent + 1) as usize {
        let split = (exponent + 1) as usize;
        format!("{}.{}", &coefficient[..split], &coefficient[split..])
    } else {
        let zeros = (exponent + 1) as usize - coefficient.len();
        format!("{coefficient}{}", "0".repeat(zeros))
    }
}

/// `formatPrefixAuto(x, p)` and the `prefixExponent` it leaves behind.
fn format_prefix_auto(x: f64, p: usize) -> (String, i32) {
    let Some((coefficient, exponent)) = decimal_parts(x, p) else {
        return (js_number(x), 0);
    };
    let prefix_exponent = (exponent as f64 / 3.0).floor().clamp(-8.0, 8.0) as i32 * 3;
    let i = exponent - prefix_exponent + 1;
    let n = coefficient.len() as i32;
    let text = if i == n {
        coefficient
    } else if i > n {
        format!("{coefficient}{}", "0".repeat((i - n) as usize))
    } else if i > 0 {
        format!(
            "{}.{}",
            &coefficient[..i as usize],
            &coefficient[i as usize..]
        )
    } else {
        let p2 = (p as i32 + i - 1).max(0) as usize;
        let more = decimal_parts(x, p2).map(|d| d.0).unwrap_or_default();
        format!("0.{}{more}", "0".repeat((-i) as usize))
    };
    (text, prefix_exponent)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(spec: &str, v: f64) -> String {
        Format::new(spec).unwrap().format(v)
    }

    #[test]
    fn js_number_methods_round_ties_up() {
        assert_eq!(to_fixed(2.5, 0), "3");
        assert_eq!(to_fixed(1.005, 2), "1.00");
        assert_eq!(to_fixed(0.125, 2), "0.13");
        assert_eq!(to_fixed(0.004, 2), "0.00");
        assert_eq!(to_fixed(0.005, 2), "0.01");
        assert_eq!(to_fixed(0.0049, 2), "0.00");
        assert_eq!(to_fixed(9.995, 2), "9.99");
        assert_eq!(to_fixed(99.5, 0), "100");
        assert_eq!(to_fixed(1234.5678, 1), "1234.6");
        assert_eq!(to_exponential(123456.0, Some(2)), "1.23e+5");
        assert_eq!(to_exponential(0.00015, None), "1.5e-4");
        assert_eq!(to_precision(700000.0, 2), "7.0e+5");
        assert_eq!(to_precision(0.35, 12), "0.350000000000");
        assert_eq!(to_precision(1e-7, 3), "1.00e-7");
        assert_eq!(to_precision(999.96, 4), "1000");
    }

    #[test]
    fn default_and_grouped_formats_match_d3() {
        assert_eq!(f(",", 1000.0), "1,000");
        assert_eq!(f(",", 1234567.891), "1,234,567.891");
        assert_eq!(f(",", 0.35), "0.35");
        assert_eq!(f("", 12.0), "12");
        assert_eq!(f(",", -5.0), "\u{2212}5");
        assert_eq!(f(",.2f", 700000.0), "700,000.00");
        assert_eq!(f(",.2", 700000.0), "7e+5");
        assert_eq!(f(".1f", 2.71), "2.7");
        assert_eq!(f(".1%", 0.35), "35.0%");
        assert_eq!(f("$,.2f", 1234.5), "$1,234.50");
        assert_eq!(f(".2s", 42e6), "42M");
        assert_eq!(f("s", 1500.0), "1.50000k");
        assert_eq!(f("~s", 1500.0), "1.5k");
        assert_eq!(f("d", 41.6), "42");
        assert_eq!(f("#x", 255.0), "0xff");
        assert_eq!(f("08.2f", -3.5), "\u{2212}0003.50");
        assert_eq!(f("^9", 12.0), "   12    ");
        assert_eq!(f("012,", 1234.0), "0,000,001,234");
        assert_eq!(f("(.1f", -2.0), "(2.0)");
        assert_eq!(f("+", 3.0), "+3");
        assert_eq!(f("r", 0.000_123_456), "0.000123456");
        assert_eq!(f(".3p", 0.123_456), "12.3%");
        assert_eq!(f(",", 1e21), "1e+21");
        assert_eq!(f(",", 0.1 + 0.2), "0.3");
        assert_eq!(Format::comma(), Format::new(",").unwrap());
    }

    #[test]
    fn invalid_specifiers_are_errors() {
        assert!(Format::new("$0,0").is_err());
        assert!(Format::new("foo").is_err());
        assert!(Format::new(".f").is_err());
        assert!(Format::new("0,0").is_err());
    }
}
