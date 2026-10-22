//! Port of d3-time-format 4.1.0 `locale.js` with the en-US
//! `defaultLocale.js`, local-time formatting only (`timeFormat`). Parsing
//! is not needed: mermaid parses dates with dayjs.

use super::time::{self, Ms, civil, day, monday, sunday, thursday, year};

const PERIODS: [&str; 2] = ["AM", "PM"];
const DAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
const SHORT_DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
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
const SHORT_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const LOCALE_DATE_TIME: &str = "%x, %X";
const LOCALE_DATE: &str = "%-m/%-d/%Y";
const LOCALE_TIME: &str = "%-I:%M:%S %p";

/// `timeFormat(specifier)(date)`.
pub fn format(specifier: &str, date: Ms) -> String {
    let chars: Vec<char> = specifier.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c != '%' {
            out.push(c);
            i += 1;
            continue;
        }
        i += 1;
        // A trailing `%` reads an empty `charAt`, which no table names, so
        // d3 pushes the empty string.
        let Some(&c) = chars.get(i) else {
            break;
        };
        let (pad, directive) = match c {
            '-' | '_' | '0' => {
                let pad = match c {
                    '-' => "",
                    '_' => " ",
                    _ => "0",
                };
                i += 1;
                (pad, chars.get(i).copied())
            }
            'e' => (" ", Some('e')),
            other => ("0", Some(other)),
        };
        i += 1;
        if let Some(d) = directive {
            match directive_value(d, date, pad) {
                Some(text) => out.push_str(&text),
                // An unknown directive pushes the directive character itself.
                None => out.push(d),
            }
        }
    }
    out
}

fn pad(value: i64, fill: &str, width: usize) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let digits = value.abs().to_string();
    if digits.len() < width {
        format!("{sign}{}{digits}", fill.repeat(width - digits.len()))
    } else {
        format!("{sign}{digits}")
    }
}

/// `dISO`: the Thursday of the ISO week holding `d`.
fn d_iso(d: Ms) -> Ms {
    let wd = civil(d).weekday;
    if wd >= 4 || wd == 0 {
        thursday().floor(d)
    } else {
        thursday().ceil(d)
    }
}

fn directive_value(directive: char, d: Ms, p: &str) -> Option<String> {
    let c = civil(d);
    let value = match directive {
        'a' => SHORT_DAYS[c.weekday as usize].to_string(),
        'A' => DAYS[c.weekday as usize].to_string(),
        'b' => SHORT_MONTHS[c.month as usize].to_string(),
        'B' => MONTHS[c.month as usize].to_string(),
        'c' => format(LOCALE_DATE_TIME, d),
        'd' | 'e' => pad(c.day, p, 2),
        'f' => format!("{}000", pad(c.millis, p, 3)),
        'g' => pad(civil(d_iso(d)).year.rem_euclid(100), p, 2),
        'G' => pad(civil(d_iso(d)).year % 10000, p, 4),
        'H' => pad(c.hours, p, 2),
        'I' => {
            let h = c.hours % 12;
            pad(if h == 0 { 12 } else { h }, p, 2)
        }
        'j' => pad(1 + day().count(year().floor(d), d).unwrap_or(0), p, 3),
        'L' => pad(c.millis, p, 3),
        'm' => pad(c.month + 1, p, 2),
        'M' => pad(c.minutes, p, 2),
        'p' => PERIODS[usize::from(c.hours >= 12)].to_string(),
        'q' => (1 + c.month / 3).to_string(),
        'Q' => d.to_string(),
        's' => (d as f64 / 1000.0).floor().to_string(),
        'S' => pad(c.seconds, p, 2),
        'u' => (if c.weekday == 0 { 7 } else { c.weekday }).to_string(),
        'U' => pad(sunday().count(year().floor(d) - 1, d).unwrap_or(0), p, 2),
        'V' => {
            let d = d_iso(d);
            let y = year().floor(d);
            let plus = i64::from(civil(y).weekday == 4);
            pad(thursday().count(y, d).unwrap_or(0) + plus, p, 2)
        }
        'w' => c.weekday.to_string(),
        'W' => pad(monday().count(year().floor(d) - 1, d).unwrap_or(0), p, 2),
        'x' => format(LOCALE_DATE, d),
        'X' => format(LOCALE_TIME, d),
        'y' => pad(c.year.rem_euclid(100), p, 2),
        'Y' => pad(c.year % 10000, p, 4),
        'Z' => {
            let z = time::timezone_offset_minutes(d);
            let (sign, z) = if z > 0 { ("-", z) } else { ("+", -z) };
            format!("{sign}{}{}", pad(z / 60, "0", 2), pad(z % 60, "0", 2))
        }
        '%' => "%".to_string(),
        _ => return None,
    };
    Some(value)
}
