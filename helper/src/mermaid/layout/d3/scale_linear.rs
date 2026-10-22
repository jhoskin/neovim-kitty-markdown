//! d3-scale 4.0.2 `linear.js` and `continuous.js` for a two-element domain,
//! with `tickFormat.js` and the slice of d3-format 3.1.2 it reaches.
//!
//! mermaid's xychart builds `scaleLinear().domain(d).range(r)` and reads
//! `scale(x)` and `scale.ticks()`; `nice` and `tickFormat` are here for the
//! other d3 users in the helper. The number formatting ports `toFixed`,
//! `toPrecision`, `formatTrim`, and the default locale (`,` grouping and the
//! U+2212 minus sign) for the specifiers `tickFormat` builds.

use std::fmt;

use super::ticks::{tick_step, ticks};

/// `scaleLinear()`: a continuous scale with a linear interpolator, no
/// clamping, and a two-element domain and range.
#[derive(Debug, Clone, PartialEq)]
pub struct ScaleLinear {
    domain: [f64; 2],
    range: [f64; 2],
}

impl Default for ScaleLinear {
    fn default() -> Self {
        ScaleLinear::new()
    }
}

/// `normalize(a, b)`: the position of `x` between `a` and `b`, `0.5` when
/// the domain is a single point, NaN when it is not a number.
fn normalize(a: f64, b: f64, x: f64) -> f64 {
    let span = b - a;
    if span != 0.0 {
        (x - a) / span
    } else if span.is_nan() {
        f64::NAN
    } else {
        0.5
    }
}

/// `interpolateNumber(a, b)(t)`.
fn interpolate(a: f64, b: f64, t: f64) -> f64 {
    a * (1.0 - t) + b * t
}

impl ScaleLinear {
    pub fn new() -> ScaleLinear {
        ScaleLinear {
            domain: [0.0, 1.0],
            range: [0.0, 1.0],
        }
    }

    pub fn domain(mut self, domain: [f64; 2]) -> ScaleLinear {
        self.domain = domain;
        self
    }

    pub fn range(mut self, range: [f64; 2]) -> ScaleLinear {
        self.range = range;
        self
    }

    pub fn domain_values(&self) -> [f64; 2] {
        self.domain
    }

    pub fn range_values(&self) -> [f64; 2] {
        self.range
    }

    /// `scale(x)`: `bimap` over the domain and range.
    pub fn scale(&self, x: f64) -> f64 {
        let [d0, d1] = self.domain;
        let [r0, r1] = self.range;
        // d3 swaps both pairs when the domain is descending, so that the
        // interpolation always runs from the smaller domain value.
        let (d0, d1, r0, r1) = if d1 < d0 {
            (d1, d0, r1, r0)
        } else {
            (d0, d1, r0, r1)
        };
        interpolate(r0, r1, normalize(d0, d1, x))
    }

    /// `scale.invert(y)`: `bimap(range, domain)`, so the swap follows the
    /// range here.
    pub fn invert(&self, y: f64) -> f64 {
        let [d0, d1] = self.domain;
        let [r0, r1] = self.range;
        let (d0, d1, r0, r1) = if r1 < r0 {
            (d1, d0, r1, r0)
        } else {
            (d0, d1, r0, r1)
        };
        interpolate(d0, d1, normalize(r0, r1, y))
    }

    /// `scale.ticks(count)`; `None` is d3's default of 10.
    pub fn ticks(&self, count: Option<f64>) -> Vec<f64> {
        let [d0, d1] = self.domain;
        ticks(d0, d1, count.unwrap_or(10.0))
    }

    /// `scale.nice(count)`: extends the domain to round tick values.
    pub fn nice(mut self, count: Option<f64>) -> ScaleLinear {
        let count = count.unwrap_or(10.0);
        let [mut start, mut stop] = self.domain;
        let reversed = stop < start;
        if reversed {
            std::mem::swap(&mut start, &mut stop);
        }
        let mut prestep: Option<f64> = None;
        let mut max_iter = 10;
        while max_iter > 0 {
            max_iter -= 1;
            let step = super::ticks::tick_increment(start, stop, count);
            if prestep == Some(step) {
                self.domain = if reversed {
                    [stop, start]
                } else {
                    [start, stop]
                };
                return self;
            } else if step > 0.0 {
                start = (start / step).floor() * step;
                stop = (stop / step).ceil() * step;
            } else if step < 0.0 {
                start = (start * step).ceil() / step;
                stop = (stop * step).floor() / step;
            } else {
                break;
            }
            prestep = Some(step);
        }
        self
    }

    /// `scale.tickFormat(count, specifier)`: a formatter with the precision
    /// the tick step calls for. `None` for `specifier` is d3's `",f"`.
    pub fn tick_format(
        &self,
        count: Option<f64>,
        specifier: Option<&str>,
    ) -> Result<TickFormat, SpecifierError> {
        let [d0, d1] = self.domain;
        tick_format(d0, d1, count.unwrap_or(10.0), specifier)
    }
}

/// `tickFormat(start, stop, count, specifier)` from d3-scale.
pub fn tick_format(
    start: f64,
    stop: f64,
    count: f64,
    specifier: Option<&str>,
) -> Result<TickFormat, SpecifierError> {
    let step = tick_step(start, stop, count);
    let mut spec = Specifier::parse(specifier.unwrap_or(",f"))?;
    let max = start.abs().max(stop.abs());
    match spec.ty {
        None | Some('e') | Some('g') | Some('r') => {
            if spec.precision.is_none()
                && let Some(precision) = precision_round(step, max)
            {
                spec.precision = Some(precision - usize::from(spec.ty == Some('e')));
            }
        }
        Some('f') => {
            if spec.precision.is_none()
                && let Some(precision) = precision_fixed(step)
            {
                spec.precision = Some(precision);
            }
        }
        // `s`, `%`, and `p` need the SI prefix and percent machinery,
        // which no mermaid diagram reaches.
        Some(other) => return Err(SpecifierError::UnsupportedType(other)),
    }
    Ok(TickFormat::new(spec))
}

/// The decimal exponent of `x`: `formatDecimalParts(x)[1]`, `None` for a
/// non-finite or zero value.
fn exponent(x: f64) -> Option<i32> {
    let x = x.abs();
    if !x.is_finite() || x == 0.0 {
        return None;
    }
    // `toExponential()` with no digits argument prints the shortest
    // round-trip mantissa; the exponent is what we need.
    let text = format!("{x:e}");
    let (_, exp) = text.split_once('e')?;
    exp.parse().ok()
}

/// `precisionFixed(step)`.
fn precision_fixed(step: f64) -> Option<usize> {
    exponent(step.abs()).map(|e| (-e).max(0) as usize)
}

/// `precisionRound(step, max)`.
fn precision_round(step: f64, max: f64) -> Option<usize> {
    let step = step.abs();
    let max = max.abs() - step;
    let e_max = exponent(max)?;
    let e_step = exponent(step)?;
    Some(((e_max - e_step).max(0) + 1) as usize)
}

/// A format specifier that is not part of the port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecifierError {
    /// The string did not match d3-format's specifier grammar.
    Invalid(String),
    /// A fill, alignment, sign, symbol, zero flag, or width was given.
    UnsupportedField(String),
    /// A type other than `f`, `e`, `g`, `r`, or none.
    UnsupportedType(char),
}

impl fmt::Display for SpecifierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpecifierError::Invalid(s) => write!(f, "invalid format: {s}"),
            SpecifierError::UnsupportedField(s) => write!(f, "unsupported format field: {s}"),
            SpecifierError::UnsupportedType(c) => write!(f, "unsupported format type: {c}"),
        }
    }
}

impl std::error::Error for SpecifierError {}

/// The subset of `formatSpecifier` the port accepts:
/// `[,][.precision][~][type]`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Specifier {
    comma: bool,
    precision: Option<usize>,
    trim: bool,
    ty: Option<char>,
}

impl Specifier {
    fn parse(text: &str) -> Result<Specifier, SpecifierError> {
        let mut rest = text;
        let mut comma = false;
        if let Some(r) = rest.strip_prefix(',') {
            comma = true;
            rest = r;
        }
        let mut precision = None;
        if let Some(r) = rest.strip_prefix('.') {
            let digits: String = r.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                return Err(SpecifierError::Invalid(text.to_string()));
            }
            precision = Some(
                digits
                    .parse()
                    .map_err(|_| SpecifierError::Invalid(text.to_string()))?,
            );
            rest = &r[digits.len()..];
        }
        let mut trim = false;
        if let Some(r) = rest.strip_prefix('~') {
            trim = true;
            rest = r;
        }
        let mut chars = rest.chars();
        let ty = chars.next();
        if chars.next().is_some() {
            return Err(SpecifierError::Invalid(text.to_string()));
        }
        if let Some(c) = ty
            && !c.is_ascii_lowercase()
            && c != '%'
        {
            return Err(SpecifierError::UnsupportedField(text.to_string()));
        }
        Ok(Specifier {
            comma,
            precision,
            trim,
            ty,
        })
    }
}

/// The function `format(specifier)` returns, for the specifiers
/// [`tick_format`] produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickFormat {
    comma: bool,
    precision: usize,
    trim: bool,
    ty: char,
}

/// The default locale's minus sign.
const MINUS: char = '\u{2212}';

impl TickFormat {
    fn new(spec: Specifier) -> TickFormat {
        let (mut precision, mut trim, ty) = (spec.precision, spec.trim, spec.ty);
        // `newFormat`: the "" type is an alias for `.12~g`.
        let ty = match ty {
            Some(c) if matches!(c, 'f' | 'e' | 'g' | 'r') => c,
            _ => {
                precision = precision.or(Some(12));
                trim = true;
                'g'
            }
        };
        let precision = match precision {
            None => 6,
            Some(p) if matches!(ty, 'g' | 'r') => p.clamp(1, 21),
            Some(p) => p.min(20),
        };
        TickFormat {
            comma: spec.comma,
            precision,
            trim,
            ty,
        }
    }

    pub fn format(&self, value: f64) -> String {
        if value.is_nan() {
            return "NaN".to_string();
        }
        let negative = value < 0.0 || (value == 0.0 && value.is_sign_negative());
        let mut text = match self.ty {
            'f' => to_fixed(value.abs(), self.precision),
            'e' => to_exponential(value.abs(), Some(self.precision)),
            'r' => format_rounded(value.abs(), self.precision),
            _ => to_precision(value.abs(), self.precision),
        };
        if self.trim {
            text = format_trim(&text);
        }
        // `-0` prints as `0` unless the sign is forced.
        let negative = negative && text.parse::<f64>().map(|v| v != 0.0).unwrap_or(true);
        // Split the digits from the decimal suffix or exponent so grouping
        // applies to the integer part only (`maybeSuffix`).
        let split = text.find(|c: char| !c.is_ascii_digit());
        let (digits, suffix) = match split {
            Some(i) => (&text[..i], &text[i..]),
            None => (&text[..], ""),
        };
        let mut out = String::new();
        if negative {
            out.push(MINUS);
        }
        if self.comma {
            out.push_str(&group(digits));
        } else {
            out.push_str(digits);
        }
        out.push_str(suffix);
        out
    }
}

/// The default locale's `group`: `,` every three digits, no width limit.
fn group(digits: &str) -> String {
    let bytes = digits.as_bytes();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}

/// `formatTrim`: drops insignificant trailing zeros after the decimal
/// point, and the point itself when nothing follows it.
fn format_trim(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut i0: isize = -1;
    let mut i1: isize = -1;
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'.' => {
                i0 = i as isize;
                i1 = i as isize;
            }
            b'0' => {
                if i0 == 0 {
                    i0 = i as isize;
                }
                i1 = i as isize;
            }
            c => {
                if !c.is_ascii_digit() {
                    break;
                }
                if i0 > 0 {
                    i0 = 0;
                }
            }
        }
        i += 1;
    }
    if i0 > 0 {
        format!("{}{}", &s[..i0 as usize], &s[(i1 + 1) as usize..])
    } else {
        s.to_string()
    }
}

/// Rounds `x` to `digits` decimals the way `Number.prototype.toFixed` does:
/// on the exact binary value, ties away from zero. Rust's formatter rounds
/// ties to even, so an exact tie is nudged first.
fn to_fixed(x: f64, digits: usize) -> String {
    if x >= 1e21 {
        return crate::mermaid::theme::khroma::js_number(x);
    }
    let scaled = x * 10f64.powi(digits as i32);
    if scaled.fract() == 0.5 && scaled < 9.0e15 {
        let rounded = (scaled + 0.5) / 10f64.powi(digits as i32);
        return format!("{rounded:.digits$}");
    }
    format!("{x:.digits$}")
}

/// `Number.prototype.toExponential(digits)`; `None` prints the shortest
/// round-trip mantissa.
fn to_exponential(x: f64, digits: Option<usize>) -> String {
    let text = match digits {
        Some(d) => format!("{x:.d$e}"),
        None => format!("{x:e}"),
    };
    match text.split_once('e') {
        Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
        _ => text,
    }
}

/// `Number.prototype.toPrecision(p)`.
fn to_precision(x: f64, p: usize) -> String {
    if x == 0.0 {
        return if p > 1 {
            format!("0.{}", "0".repeat(p - 1))
        } else {
            "0".to_string()
        };
    }
    let text = format!("{x:.prec$e}", prec = p - 1);
    let Some((mantissa, exp)) = text.split_once('e') else {
        return text;
    };
    let e: i32 = exp.parse().unwrap_or(0);
    if e < -6 || e >= p as i32 {
        let sign = if e < 0 { "" } else { "+" };
        return format!("{mantissa}e{sign}{e}");
    }
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    if e >= 0 {
        let int_len = (e + 1) as usize;
        if int_len >= digits.len() {
            digits
        } else {
            format!("{}.{}", &digits[..int_len], &digits[int_len..])
        }
    } else {
        format!("0.{}{}", "0".repeat((-e - 1) as usize), digits)
    }
}

/// `formatRounded(x, p)`: `p` significant digits in fixed notation.
fn format_rounded(x: f64, p: usize) -> String {
    if x == 0.0 {
        return "0".to_string();
    }
    let text = format!("{x:.prec$e}", prec = p - 1);
    let Some((mantissa, exp)) = text.split_once('e') else {
        return text;
    };
    let e: i32 = exp.parse().unwrap_or(0);
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let n = digits.len() as i32;
    if e < 0 {
        format!("0.{}{}", "0".repeat((-e - 1) as usize), digits)
    } else if e + 1 >= n {
        format!("{digits}{}", "0".repeat((e + 1 - n) as usize))
    } else {
        format!(
            "{}.{}",
            &digits[..(e + 1) as usize],
            &digits[(e + 1) as usize..]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_and_invert() {
        let s = ScaleLinear::new().domain([0.0, 10.0]).range([100.0, 200.0]);
        assert_eq!(s.scale(5.0), 150.0);
        assert_eq!(s.invert(150.0), 5.0);
        let r = ScaleLinear::new().domain([10.0, 0.0]).range([0.0, 100.0]);
        assert_eq!(r.scale(10.0), 0.0);
        assert_eq!(r.scale(0.0), 100.0);
        let point = ScaleLinear::new().domain([3.0, 3.0]).range([0.0, 10.0]);
        assert_eq!(point.scale(3.0), 5.0);
    }

    #[test]
    fn nice_extends_to_round_values() {
        let s = ScaleLinear::new().domain([0.201479, 0.996679]).nice(None);
        assert_eq!(s.domain_values(), [0.2, 1.0]);
        let s = ScaleLinear::new().domain([0.5, 0.25]).nice(Some(5.0));
        assert_eq!(s.domain_values(), [0.5, 0.25]);
    }

    #[test]
    fn default_tick_format_groups_and_fixes_precision() {
        let s = ScaleLinear::new().domain([0.0, 11000.0]);
        let f = s.tick_format(None, None).unwrap();
        assert_eq!(f.format(1000.0), "1,000");
        assert_eq!(f.format(11000.0), "11,000");
        let s = ScaleLinear::new().domain([0.0, 1.0]);
        let f = s.tick_format(None, None).unwrap();
        assert_eq!(f.format(0.3), "0.3");
        assert_eq!(f.format(-0.5), "\u{2212}0.5");
    }

    #[test]
    fn trim_and_empty_type() {
        assert_eq!(format_trim("1.500"), "1.5");
        assert_eq!(format_trim("1.000"), "1");
        assert_eq!(format_trim("100"), "100");
        assert_eq!(format_trim("1.0e+21"), "1e+21");
        let f = tick_format(0.0, 1.0, 10.0, Some("")).unwrap();
        assert_eq!(f.format(0.3), "0.3");
        assert_eq!(f.format(1.0), "1");
        let f = tick_format(0.0, 100.0, 10.0, Some("~f")).unwrap();
        assert_eq!(f.format(10.0), "10");
    }

    #[test]
    fn js_number_methods() {
        assert_eq!(to_fixed(2.5, 0), "3");
        assert_eq!(to_fixed(1.005, 2), "1.00");
        assert_eq!(to_precision(123.456, 2), "1.2e+2");
        assert_eq!(to_precision(0.000001234, 2), "0.0000012");
        assert_eq!(to_precision(1234.5, 6), "1234.50");
        assert_eq!(to_exponential(12345.0, Some(2)), "1.23e+4");
        assert_eq!(format_rounded(123.456, 2), "120");
        assert_eq!(format_rounded(0.0123, 2), "0.012");
    }
}
