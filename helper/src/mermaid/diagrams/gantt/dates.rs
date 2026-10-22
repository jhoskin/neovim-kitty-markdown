//! The dayjs 1.11 behavior `ganttDb.js` and `ganttRenderer.js` rely on:
//! strict parsing with `customParseFormat`, `format` with `advancedFormat`,
//! `add`, `startOf`/`endOf('day')`, `isoWeekday`, `diff(_, 'year')`, and
//! `dayjs.duration`. Everything runs in the system time zone like the
//! browser's `Date` (see `layout::d3::time`).
//!
//! Also here: the subset of `new Date(string)` that the db falls back to
//! when strict parsing fails.

use std::sync::LazyLock;

use regex::Regex;

use crate::mermaid::layout::d3::time::{self, Civil, Ms, civil, make};

const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const FORMAT_DEFAULT: &str = "YYYY-MM-DDTHH:mm:ssZ";

/// `dayjs.ManipulateType` shorthands `parseDuration` produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Year,
    Month,
    Week,
    Day,
    Hour,
    Minute,
    Second,
    Millisecond,
}

impl Unit {
    /// The shorthand `parseDuration` matched: `y M w d h m s ms`.
    pub fn from_shorthand(s: &str) -> Option<Unit> {
        Some(match s {
            "y" => Unit::Year,
            "M" => Unit::Month,
            "w" => Unit::Week,
            "d" => Unit::Day,
            "h" => Unit::Hour,
            "m" => Unit::Minute,
            "s" => Unit::Second,
            "ms" => Unit::Millisecond,
            _ => return None,
        })
    }
}

/// `new Date(y, m, d, ...)` maps a year from 0 to 99 to 1900 to 1999.
fn js_year(y: i64) -> i64 {
    if (0..=99).contains(&y) { y + 1900 } else { y }
}

fn make_civil(c: Civil) -> Ms {
    make(
        c.year, c.month, c.day, c.hours, c.minutes, c.seconds, c.millis,
    )
}

/// `dayjs(d).startOf('day')`.
pub fn start_of_day(d: Ms) -> Ms {
    let c = civil(d);
    make(c.year, c.month, c.day, 0, 0, 0, 0)
}

/// `dayjs(d).endOf('day')`.
pub fn end_of_day(d: Ms) -> Ms {
    let c = civil(d);
    make(c.year, c.month, c.day, 23, 59, 59, 999)
}

/// `dayjs(d).isoWeekday()`: Monday 1 to Sunday 7.
pub fn iso_weekday(d: Ms) -> i64 {
    let wd = civil(d).weekday;
    if wd == 0 { 7 } else { wd }
}

/// `today.setHours(0, 0, 0, 0)` on `new Date()`.
pub fn today_midnight() -> Ms {
    start_of_day(time::now())
}

/// `endOf('month').$D` for the month holding `d`.
fn days_in_month(d: Ms) -> i64 {
    let c = civil(d);
    civil(make(c.year, c.month + 1, 0, 0, 0, 0, 0)).day
}

/// `dayjs.$set` for months and years: set the day to 1, move, then clamp
/// the day to the new month's length.
fn set_month_or_year(d: Ms, year: i64, month: i64) -> Ms {
    let c = civil(d);
    let moved = make(year, month, 1, c.hours, c.minutes, c.seconds, c.millis);
    let day = c.day.min(days_in_month(moved));
    let m = civil(moved);
    make(
        m.year, m.month, day, m.hours, m.minutes, m.seconds, m.millis,
    )
}

/// `dayjs(d).add(n, unit)`.
pub fn add(d: Ms, n: f64, unit: Unit) -> Ms {
    let c = civil(d);
    match unit {
        // `setMonth`/`setFullYear` truncate a fractional argument.
        Unit::Month => set_month_or_year(d, c.year, c.month + n.trunc() as i64),
        Unit::Year => set_month_or_year(d, c.year + n.trunc() as i64, c.month),
        Unit::Day | Unit::Week => {
            let step = if unit == Unit::Week { 7.0 } else { 1.0 };
            let days = time::js_round(n * step) as i64;
            make_civil(Civil {
                day: c.day + days,
                ..c
            })
        }
        Unit::Hour | Unit::Minute | Unit::Second | Unit::Millisecond => {
            let step = match unit {
                Unit::Hour => time::DURATION_HOUR,
                Unit::Minute => time::DURATION_MINUTE,
                Unit::Second => time::DURATION_SECOND,
                _ => 1,
            } as f64;
            // `new Date(ms)` clips a fractional time value toward zero.
            (d as f64 + n * step).trunc() as Ms
        }
    }
}

/// `monthDiff` from dayjs `utils.js`, moment's algorithm.
fn month_diff(a: Ms, b: Ms) -> f64 {
    let (ca, cb) = (civil(a), civil(b));
    if ca.day < cb.day {
        return -month_diff(b, a);
    }
    let whole = ((cb.year - ca.year) * 12 + (cb.month - ca.month)) as f64;
    let anchor = add(a, whole, Unit::Month);
    let c = (b - anchor) < 0;
    let anchor2 = add(a, whole + if c { -1.0 } else { 1.0 }, Unit::Month);
    let denominator = if c {
        anchor - anchor2
    } else {
        anchor2 - anchor
    };
    let v = -(whole + (b - anchor) as f64 / denominator as f64);
    if v.is_nan() { 0.0 } else { v }
}

/// `dayjs(a).diff(dayjs(b), 'year')`.
pub fn diff_years(a: Ms, b: Ms) -> i64 {
    let months = month_diff(a, b);
    let years = months / 12.0;
    // `absFloor`
    if years < 0.0 {
        years.ceil() as i64
    } else {
        years.floor() as i64
    }
}

/// `dayjs.duration({ [unit]: n }).asMilliseconds()`; `None` for a unit the
/// plugin does not know.
pub fn duration_ms(unit: &str, n: f64) -> Option<f64> {
    let per = match unit {
        "millisecond" | "milliseconds" => 1.0,
        "second" | "seconds" => 1000.0,
        "minute" | "minutes" => 60_000.0,
        "hour" | "hours" => 3_600_000.0,
        "day" | "days" => 86_400_000.0,
        "week" | "weeks" => 604_800_000.0,
        "month" | "months" => 2_629_746_000.0,
        "year" | "years" => 31_556_952_000.0,
        _ => return None,
    };
    Some(n * per)
}

fn pad_start(value: i64, width: usize) -> String {
    let s = value.to_string();
    if s.len() >= width {
        s
    } else {
        format!("{}{s}", "0".repeat(width - s.len()))
    }
}

/// `padZoneStr`: `+HH:MM`.
fn zone_str(d: Ms, colon: bool) -> String {
    let neg_minutes = time::timezone_offset_minutes(d);
    let minutes = neg_minutes.abs();
    let sign = if neg_minutes <= 0 { "+" } else { "-" };
    let sep = if colon { ":" } else { "" };
    format!(
        "{sign}{}{sep}{}",
        pad_start(minutes / 60, 2),
        pad_start(minutes % 60, 2)
    )
}

/// `isoWeek()`: the ISO 8601 week number.
fn iso_week(d: Ms) -> i64 {
    // The Thursday of this week decides the ISO year.
    let thursday = add(d, (4 - iso_weekday(d)) as f64, Unit::Day);
    let year = civil(thursday).year;
    let jan1 = make(year, 0, 1, 0, 0, 0, 0);
    let mut offset = 4 - iso_weekday(jan1);
    if iso_weekday(jan1) > 4 {
        offset += 7;
    }
    let first_thursday = add(jan1, offset as f64, Unit::Day);
    let weeks = time::day()
        .count(first_thursday, thursday)
        .unwrap_or(0)
        .div_euclid(7);
    weeks + 1
}

fn iso_week_year(d: Ms) -> i64 {
    civil(add(d, (4 - iso_weekday(d)) as f64, Unit::Day)).year
}

/// `ordinal` of the en locale: `[1st]`, `[2nd]`, ... The brackets keep the
/// core pass from reading the `h` of `th` as an hour token.
fn ordinal(n: i64) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, r) if r != 11 => "st",
        (2, r) if r != 12 => "nd",
        (3, r) if r != 13 => "rd",
        _ => "th",
    };
    format!("[{n}{suffix}]")
}

static ADVANCED_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[([^\]]+)]|Q|wo|ww|w|WW|W|zzz|z|gggg|GGGG|Do|X|x|k{1,2}|S").expect("valid regex")
});

static CORE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\[([^\]]+)]|YYYY|YY|M{1,4}|D{1,2}|d{1,4}|H{1,2}|h{1,2}|a|A|m{1,2}|s{1,2}|Z{1,2}|SSS",
    )
    .expect("valid regex")
});

/// `advancedFormat`'s pre-pass. Tokens that need the `weekOfYear` plugin
/// (`w`, `ww`, `wo`, `gggg`) make dayjs throw in mermaid; they are left as
/// text here so the render still finishes.
fn advanced_pass(fmt: &str, d: Ms) -> String {
    let c = civil(d);
    ADVANCED_RE
        .replace_all(fmt, |caps: &regex::Captures<'_>| {
            let m = &caps[0];
            match m {
                "Q" => ((c.month + 1 + 2) / 3).to_string(),
                "Do" => ordinal(c.day),
                "GGGG" => iso_week_year(d).to_string(),
                "W" => iso_week(d).to_string(),
                "WW" => pad_start(iso_week(d), 2),
                "k" => (if c.hours == 0 { 24 } else { c.hours }).to_string(),
                "kk" => pad_start(if c.hours == 0 { 24 } else { c.hours }, 2),
                "X" => (d as f64 / 1000.0).floor().to_string(),
                "x" => d.to_string(),
                other => other.to_string(),
            }
        })
        .into_owned()
}

/// `dayjs(d).format(fmt)` with the `advancedFormat` plugin loaded.
pub fn format(d: Ms, fmt: &str) -> String {
    let fmt = if fmt.is_empty() { FORMAT_DEFAULT } else { fmt };
    let fmt = advanced_pass(fmt, d);
    let c = civil(d);
    let h12 = |width: usize| {
        let h = c.hours % 12;
        pad_start(if h == 0 { 12 } else { h }, width)
    };
    let meridiem = |lower: bool| {
        let m = if c.hours < 12 { "AM" } else { "PM" };
        if lower {
            m.to_lowercase()
        } else {
            m.to_string()
        }
    };
    CORE_RE
        .replace_all(&fmt, |caps: &regex::Captures<'_>| {
            if let Some(escaped) = caps.get(1) {
                return escaped.as_str().to_string();
            }
            match &caps[0] {
                "YY" => {
                    let y = c.year.to_string();
                    y[y.len().saturating_sub(2)..].to_string()
                }
                "YYYY" => pad_start(c.year, 4),
                "M" => (c.month + 1).to_string(),
                "MM" => pad_start(c.month + 1, 2),
                "MMM" => MONTHS[c.month as usize][..3].to_string(),
                "MMMM" => MONTHS[c.month as usize].to_string(),
                "D" => c.day.to_string(),
                "DD" => pad_start(c.day, 2),
                "d" => c.weekday.to_string(),
                "dd" => WEEKDAYS[c.weekday as usize][..2].to_string(),
                "ddd" => WEEKDAYS[c.weekday as usize][..3].to_string(),
                "dddd" => WEEKDAYS[c.weekday as usize].to_string(),
                "H" => c.hours.to_string(),
                "HH" => pad_start(c.hours, 2),
                "h" => h12(1),
                "hh" => h12(2),
                "a" => meridiem(true),
                "A" => meridiem(false),
                "m" => c.minutes.to_string(),
                "mm" => pad_start(c.minutes, 2),
                "s" => c.seconds.to_string(),
                "ss" => pad_start(c.seconds, 2),
                "SSS" => pad_start(c.millis, 3),
                "Z" => zone_str(d, true),
                "ZZ" => zone_str(d, false),
                other => other.to_string(),
            }
        })
        .into_owned()
}

/// The token regex of `customParseFormat`.
static PARSE_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(\[[^\[]*\])|([-_:/.,()\s]+)|(A|a|Q|YYYY|YY?|ww?|MM?M?M?|Do|DD?|hh?|HH?|mm?|ss?|S{1,3}|z|ZZ?)",
    )
    .expect("valid regex")
});

#[derive(Default, Debug)]
struct Parsed {
    year: Option<i64>,
    month: Option<i64>,
    day: Option<i64>,
    hours: Option<i64>,
    minutes: Option<i64>,
    seconds: Option<i64>,
    milliseconds: Option<i64>,
    /// Minutes, with the plugin's sign: `+01:00` is -60.
    zone_offset: Option<i64>,
    afternoon: Option<bool>,
}

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("valid regex")
}

/// The `[regex, parser]` pair of one token, as the plugin's table.
fn token_regex(token: &str) -> Option<Regex> {
    let digit = r"\d";
    let two = r"\d\d";
    let one_or_two = r"\d\d?";
    let word = r"\d*[^-_:/,()\s\d]+";
    let pattern = match token {
        "A" | "a" | "Do" | "MMM" | "MMMM" | "z" => word,
        "Q" | "S" => digit,
        "SS" | "DD" | "MM" | "YY" | "ww" => two,
        "SSS" => r"\d{3}",
        "s" | "ss" | "m" | "mm" | "H" | "h" | "HH" | "hh" | "D" | "w" | "M" => one_or_two,
        "Y" => r"[+-]?\d+",
        "YYYY" => r"\d{4}",
        "Z" | "ZZ" => r"[+-]\d\d:?(\d\d)?|Z",
        _ => return None,
    };
    Some(re(pattern))
}

fn apply_token(token: &str, text: &str, p: &mut Parsed) -> Option<()> {
    let num = || text.parse::<i64>().ok();
    match token {
        "A" => p.afternoon = Some(text == "PM"),
        "a" => p.afternoon = Some(text == "pm"),
        "Q" => p.month = Some(3 * (num()? - 1) + 1),
        "S" => p.milliseconds = Some(100 * num()?),
        "SS" => p.milliseconds = Some(10 * num()?),
        "SSS" => p.milliseconds = num(),
        "s" | "ss" => p.seconds = num(),
        "m" | "mm" => p.minutes = num(),
        "H" | "h" | "HH" | "hh" => p.hours = num(),
        "D" | "DD" => p.day = num(),
        "Do" => {
            let digits: String = text
                .chars()
                .skip_while(|c| !c.is_ascii_digit())
                .take_while(char::is_ascii_digit)
                .collect();
            p.day = digits.parse().ok();
        }
        "M" | "MM" => p.month = num(),
        "MMM" => {
            let i = MONTHS.iter().position(|m| m[..3] == *text)? as i64 + 1;
            p.month = Some(if i % 12 == 0 { i } else { i % 12 });
        }
        "MMMM" => {
            let i = MONTHS.iter().position(|m| *m == text)? as i64 + 1;
            p.month = Some(if i % 12 == 0 { i } else { i % 12 });
        }
        "Y" | "YYYY" => p.year = num(),
        "YY" => {
            let y = num()?;
            p.year = Some(y + if y > 68 { 1900 } else { 2000 });
        }
        "Z" | "ZZ" => {
            p.zone_offset = Some(if text == "Z" {
                0
            } else {
                let parts: Vec<&str> = re(r"([+-]|\d\d)")
                    .find_iter(text)
                    .map(|m| m.as_str())
                    .collect();
                let n = 60 * parts.get(1)?.parse::<i64>().ok()?
                    + parts
                        .get(2)
                        .and_then(|s| s.parse::<i64>().ok())
                        .unwrap_or(0);
                if n == 0 {
                    0
                } else if parts[0] == "+" {
                    -n
                } else {
                    n
                }
            });
        }
        // `w`/`ww` need the `weekOfYear` plugin and `z` only records text.
        _ => {}
    }
    Some(())
}

/// JavaScript `Number(string)` for the timestamp formats.
fn js_number(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return Some(0.0);
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// `dayjs(input, format, true)`: the plugin's parse followed by the strict
/// round trip `input === parsed.format(format)`. `None` is `Invalid Date`.
pub fn parse_strict(input: &str, fmt: &str) -> Option<Ms> {
    let ms = parse_with_format(input, fmt)?;
    if format(ms, fmt) != input {
        return None;
    }
    Some(ms)
}

fn parse_with_format(input: &str, fmt: &str) -> Option<Ms> {
    if fmt == "x" || fmt == "X" {
        let scale = if fmt == "X" { 1000.0 } else { 1.0 };
        return Some((scale * js_number(input)?).trunc() as Ms);
    }
    let mut parsed = Parsed::default();
    // `e` and `r`: the plugin removes each matched value from the input and
    // only advances `r` past the literal pieces.
    let mut remaining: Vec<char> = input.chars().collect();
    let mut r = 0usize;
    let mut saw_token = false;
    for m in PARSE_TOKEN_RE.find_iter(fmt) {
        saw_token = true;
        let token = m.as_str();
        if let Some(regex) = token_regex(token) {
            let rest: String = remaining.iter().skip(r).collect();
            let found = regex.find(&rest)?;
            apply_token(token, found.as_str(), &mut parsed)?;
            let text: Vec<char> = found.as_str().chars().collect();
            if let Some(pos) = remaining
                .windows(text.len())
                .position(|window| window == text.as_slice())
            {
                remaining.drain(pos..pos + text.len());
            }
        } else {
            let literal = token.trim_start_matches('[').trim_end_matches(']');
            r += literal.chars().count();
        }
    }
    // An empty format has no tokens: `"".match(re)` is `null` and the plugin
    // throws, which dayjs turns into `Invalid Date`.
    if !saw_token {
        return None;
    }
    if let Some(afternoon) = parsed.afternoon {
        let hours = parsed.hours.unwrap_or(0);
        if afternoon {
            if hours < 12 {
                parsed.hours = Some(hours + 12);
            }
        } else if hours == 12 {
            parsed.hours = Some(0);
        }
    }
    let now = civil(time::now());
    let truthy = |v: Option<i64>| v.is_some_and(|n| n != 0);
    let day = if truthy(parsed.day) {
        parsed.day.unwrap_or(1)
    } else if truthy(parsed.year) || truthy(parsed.month) {
        1
    } else {
        now.day
    };
    let year = if truthy(parsed.year) {
        parsed.year.unwrap_or(0)
    } else {
        now.year
    };
    let month = if truthy(parsed.year) && !truthy(parsed.month) {
        0
    } else if parsed.month.is_some_and(|m| m > 0) {
        parsed.month.unwrap_or(1) - 1
    } else {
        now.month
    };
    let hours = parsed.hours.unwrap_or(0);
    let minutes = parsed.minutes.unwrap_or(0);
    let seconds = parsed.seconds.unwrap_or(0);
    let millis = parsed.milliseconds.unwrap_or(0);
    let year = js_year(year);
    Some(match parsed.zone_offset {
        Some(offset) => {
            // `Date.UTC(...)` plus the zone offset in minutes.
            let local = make(year, month, day, hours, minutes, seconds, millis);
            let utc_shift = time::timezone_offset_minutes(local) * time::DURATION_MINUTE;
            local - utc_shift + offset * time::DURATION_MINUTE
        }
        None => make(year, month, day, hours, minutes, seconds, millis),
    })
}

static ISO_RE: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"^(\d{4})(?:-(\d{2})(?:-(\d{2}))?)?(?:[T ](\d{2}):(\d{2})(?::(\d{2})(?:\.(\d{1,3}))?)?)?\s*(Z|[+-]\d{2}:?\d{2})?$",
    )
});
static SLASH_RE: LazyLock<Regex> = LazyLock::new(|| re(r"^(\d{4})/(\d{1,2})/(\d{1,2})$"));
static US_RE: LazyLock<Regex> = LazyLock::new(|| re(r"^(\d{1,2})/(\d{1,2})/(\d{4})$"));

/// The part of `new Date(string)` a gantt source plausibly reaches: the
/// ISO forms (a date alone is UTC, a date with a time is local unless it
/// carries a zone) and the `Y/M/D` and `M/D/Y` slash forms, which V8's
/// legacy parser reads as local time. Anything else is `Invalid Date`.
pub fn js_date_parse(s: &str) -> Option<Ms> {
    let s = s.trim();
    let num = |m: Option<regex::Match<'_>>| m.and_then(|m| m.as_str().parse::<i64>().ok());
    if let Some(c) = ISO_RE.captures(s) {
        let year = num(c.get(1))?;
        let month = num(c.get(2)).unwrap_or(1) - 1;
        let day = num(c.get(3)).unwrap_or(1);
        let has_time = c.get(4).is_some();
        let hours = num(c.get(4)).unwrap_or(0);
        let minutes = num(c.get(5)).unwrap_or(0);
        let seconds = num(c.get(6)).unwrap_or(0);
        let millis = c
            .get(7)
            .map(|m| {
                let digits = m.as_str();
                let v: i64 = digits.parse().unwrap_or(0);
                v * 10i64.pow(3 - digits.len() as u32)
            })
            .unwrap_or(0);
        let local = make(year, month, day, hours, minutes, seconds, millis);
        let zone = c.get(8).map(|m| m.as_str());
        return Some(match zone {
            Some("Z") => local - time::timezone_offset_minutes(local) * time::DURATION_MINUTE,
            Some(z) => {
                let sign = if z.starts_with('-') { -1 } else { 1 };
                let digits: String = z.chars().filter(char::is_ascii_digit).collect();
                let hh: i64 = digits[..2].parse().unwrap_or(0);
                let mm: i64 = digits[2..].parse().unwrap_or(0);
                let utc = local - time::timezone_offset_minutes(local) * time::DURATION_MINUTE;
                utc - sign * (hh * 60 + mm) * time::DURATION_MINUTE
            }
            None if has_time => local,
            None => local - time::timezone_offset_minutes(local) * time::DURATION_MINUTE,
        });
    }
    if let Some(c) = SLASH_RE.captures(s) {
        return Some(make(
            num(c.get(1))?,
            num(c.get(2))? - 1,
            num(c.get(3))?,
            0,
            0,
            0,
            0,
        ));
    }
    if let Some(c) = US_RE.captures(s) {
        return Some(make(
            num(c.get(3))?,
            num(c.get(1))? - 1,
            num(c.get(2))?,
            0,
            0,
            0,
            0,
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i64, m: i64, day: i64) -> Ms {
        make(y, m, day, 0, 0, 0, 0)
    }

    #[test]
    fn strict_parse_round_trips_and_rejects() {
        assert_eq!(
            parse_strict("2014-01-06", "YYYY-MM-DD"),
            Some(d(2014, 0, 6))
        );
        assert_eq!(parse_strict("2014-1-6", "YYYY-MM-DD"), None);
        assert_eq!(parse_strict("after a1", "YYYY-MM-DD"), None);
        assert_eq!(parse_strict("3d", "YYYY-MM-DD"), None);
        assert_eq!(parse_strict("1900", "YYYY"), Some(d(1900, 0, 1)));
        assert_eq!(
            parse_strict("06/01/2014", "DD/MM/YYYY"),
            Some(d(2014, 0, 6))
        );
        assert_eq!(
            parse_strict("2014-01-06 13:45", "YYYY-MM-DD HH:mm"),
            Some(make(2014, 0, 6, 13, 45, 0, 0))
        );
        assert_eq!(parse_strict("71", "X"), Some(71_000));
        assert_eq!(parse_strict("71", "x"), Some(71));
        assert_eq!(parse_strict("2014-01-06", ""), None);
        // `HH:mm` lands on today's date.
        let t = parse_strict("17:49", "HH:mm").expect("time parses");
        let c = civil(t);
        assert_eq!((c.hours, c.minutes), (17, 49));
        assert_eq!(start_of_day(t), today_midnight());
    }

    #[test]
    fn format_tokens() {
        let t = make(2014, 0, 5, 9, 7, 3, 42);
        assert_eq!(format(t, "YYYY-MM-DD"), "2014-01-05");
        assert_eq!(format(t, "dddd"), "Sunday");
        assert_eq!(format(t, "ddd D MMM"), "Sun 5 Jan");
        assert_eq!(format(t, "HH:mm:ss.SSS"), "09:07:03.042");
        assert_eq!(format(t, "h:mm A"), "9:07 AM");
        assert_eq!(format(t, "[at] hh a"), "at 09 am");
        assert_eq!(format(t, "YY Q Do"), "14 1 5th");
        assert_eq!(format(make(2014, 0, 22, 0, 0, 0, 0), "Do"), "22nd");
        assert_eq!(format(t, "X x"), format!("{} {}", t / 1000, t));
        assert_eq!(format(t, "W WW k kk"), "1 01 9 09");
        assert_eq!(
            format(make(2020, 11, 31, 0, 0, 0, 0), "GGGG-[W]WW"),
            "2020-W53"
        );
        assert!(format(t, "").starts_with("2014-01-05T09:07:03"));
    }

    #[test]
    fn add_clamps_months_and_rounds_days() {
        assert_eq!(add(d(2014, 0, 31), 1.0, Unit::Month), d(2014, 1, 28));
        assert_eq!(add(d(2014, 0, 31), 1.0, Unit::Year), d(2015, 0, 31));
        assert_eq!(add(d(2014, 0, 1), 1.0, Unit::Week), d(2014, 0, 8));
        assert_eq!(add(d(2014, 0, 1), 1.5, Unit::Day), d(2014, 0, 3));
        assert_eq!(add(d(2014, 0, 1), 1.4, Unit::Day), d(2014, 0, 2));
        assert_eq!(
            add(d(2014, 0, 1), 24.0, Unit::Hour),
            d(2014, 0, 1) + 24 * 3_600_000
        );
        assert_eq!(
            add(d(2014, 0, 1), 2.5, Unit::Minute),
            d(2014, 0, 1) + 150_000
        );
        assert_eq!(
            add(d(2014, 0, 1), 30.0, Unit::Millisecond),
            d(2014, 0, 1) + 30
        );
    }

    #[test]
    fn iso_weekday_diff_and_duration() {
        assert_eq!(iso_weekday(d(2014, 0, 5)), 7);
        assert_eq!(iso_weekday(d(2014, 0, 6)), 1);
        assert_eq!(diff_years(d(2020, 5, 1), d(2014, 0, 1)), 6);
        assert_eq!(diff_years(d(2019, 11, 31), d(2014, 0, 1)), 5);
        assert_eq!(duration_ms("week", 1.0), Some(604_800_000.0));
        assert_eq!(duration_ms("decade", 1.0), None);
        assert_eq!(end_of_day(d(2014, 0, 1)), d(2014, 0, 2) - 1);
    }

    #[test]
    fn js_date_fallback() {
        let utc_midnight = d(2014, 0, 6) - time::timezone_offset_minutes(d(2014, 0, 6)) * 60_000;
        assert_eq!(js_date_parse("2014-01-06"), Some(utc_midnight));
        assert_eq!(
            js_date_parse("2014-01-06T10:00"),
            Some(make(2014, 0, 6, 10, 0, 0, 0))
        );
        assert_eq!(js_date_parse("2014/01/06"), Some(d(2014, 0, 6)));
        assert_eq!(js_date_parse("01/06/2014"), Some(d(2014, 0, 6)));
        assert_eq!(js_date_parse("nope"), None);
    }
}
