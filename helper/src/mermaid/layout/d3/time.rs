//! Port of d3-time 3.1.0 (`interval.js`, `millisecond.js`, `second.js`,
//! `minute.js`, `hour.js`, `day.js`, `week.js`, `month.js`, `year.js`,
//! `ticks.js`) in local time, plus the JavaScript `Date` arithmetic the
//! intervals are written against.
//!
//! mermaid's gantt renderer runs in the oracle's browser, whose `Date`
//! works in the machine's local time zone. The helper does the same through
//! jiff's system time zone, so a chart renders the way the oracle renders
//! it on the same machine. Goldens and the unit tests therefore assume one
//! zone: `helper/oracle/goldens.mjs` and `tests/d3_time_*.rs` document the
//! `TZ` they were produced under, and a parity run must use that zone.
//!
//! A time value is the JavaScript one: milliseconds since the Unix epoch,
//! here an `i64` because every gantt path produces whole milliseconds.

use std::sync::{Arc, OnceLock};

use jiff::civil::{Date, DateTime, Time};
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};

/// A JavaScript `Date` time value.
pub type Ms = i64;

pub const DURATION_SECOND: i64 = 1000;
pub const DURATION_MINUTE: i64 = DURATION_SECOND * 60;
pub const DURATION_HOUR: i64 = DURATION_MINUTE * 60;
pub const DURATION_DAY: i64 = DURATION_HOUR * 24;
pub const DURATION_WEEK: i64 = DURATION_DAY * 7;
pub const DURATION_MONTH: i64 = DURATION_DAY * 30;
pub const DURATION_YEAR: i64 = DURATION_DAY * 365;

/// The zone every local-time operation uses: the system zone, read once.
pub fn zone() -> &'static TimeZone {
    static TZ: OnceLock<TimeZone> = OnceLock::new();
    TZ.get_or_init(TimeZone::system)
}

/// The broken-down local time of a time value, as the `Date` getters
/// report it: `month` is 0-based, `weekday` has Sunday as 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Civil {
    pub year: i64,
    pub month: i64,
    pub day: i64,
    pub hours: i64,
    pub minutes: i64,
    pub seconds: i64,
    pub millis: i64,
    pub weekday: i64,
}

/// jiff's calendar covers the years -9999 to 9999; JavaScript reaches
/// further, but a gantt chart never does. Values outside are clamped so the
/// arithmetic stays total like the JavaScript it ports.
const MIN_MS: Ms = -377_705_116_800_000;
const MAX_MS: Ms = 253_402_207_199_999;

fn timestamp(ms: Ms) -> Timestamp {
    Timestamp::from_millisecond(ms.clamp(MIN_MS, MAX_MS))
        .unwrap_or_else(|_| Timestamp::from_millisecond(0).unwrap_or_default())
}

fn to_ms(ts: Timestamp) -> Ms {
    ts.as_millisecond()
}

/// The `get*` family of `Date`.
pub fn civil(ms: Ms) -> Civil {
    let dt = zone().to_datetime(timestamp(ms));
    Civil {
        year: i64::from(dt.year()),
        month: i64::from(dt.month()) - 1,
        day: i64::from(dt.day()),
        hours: i64::from(dt.hour()),
        minutes: i64::from(dt.minute()),
        seconds: i64::from(dt.second()),
        millis: i64::from(dt.millisecond()),
        weekday: i64::from(dt.weekday().to_sunday_zero_offset()),
    }
}

/// `new Date(year, month, day, hours, minutes, seconds, ms)` and the `set*`
/// family: ECMAScript `MakeDay` and `MakeTime` carry every overflow into
/// the next larger field, then `LocalTime` resolves the civil time in the
/// system zone (a time inside a DST gap moves forward, as in V8).
pub fn make(
    year: i64,
    month: i64,
    day: i64,
    hours: i64,
    minutes: i64,
    seconds: i64,
    ms: i64,
) -> Ms {
    let year = year + month.div_euclid(12);
    let month0 = month.rem_euclid(12);
    let year = year.clamp(-9999, 9999) as i16;
    let first = Date::new(year, (month0 + 1) as i8, 1).unwrap_or_default();
    let date = first
        .checked_add(Span::new().days(day - 1))
        .unwrap_or(if day > 1 { Date::MAX } else { Date::MIN });
    let dt = DateTime::from_parts(date, Time::midnight());
    let base = zone().to_timestamp(dt).map(to_ms).unwrap_or(
        if date > Date::new(0, 1, 1).unwrap_or_default() {
            MAX_MS
        } else {
            MIN_MS
        },
    );
    // `MakeTime` adds the time fields as plain milliseconds. Doing the same
    // after the zone lookup keeps a day's 00:00 fixed and lets 24:00 roll
    // into the next day exactly like the JavaScript engine.
    base + hours * DURATION_HOUR + minutes * DURATION_MINUTE + seconds * DURATION_SECOND + ms
}

/// `date.getTimezoneOffset()`: minutes to add to local time to get UTC.
pub fn timezone_offset_minutes(ms: Ms) -> i64 {
    -i64::from(zone().to_offset(timestamp(ms)).seconds()) / 60
}

/// `new Date()`.
pub fn now() -> Ms {
    Timestamp::now().as_millisecond()
}

type FloorFn = dyn Fn(Ms) -> Ms + Send + Sync;
type OffsetFn = dyn Fn(Ms, i64) -> Ms + Send + Sync;
type CountFn = dyn Fn(Ms, Ms) -> f64 + Send + Sync;
type FieldFn = dyn Fn(Ms) -> i64 + Send + Sync;
type EveryFn = dyn Fn(f64) -> Option<Interval> + Send + Sync;

/// `timeInterval(floori, offseti, count, field)`.
#[derive(Clone)]
pub struct Interval {
    floori: Arc<FloorFn>,
    offseti: Arc<OffsetFn>,
    count: Option<Arc<CountFn>>,
    field: Option<Arc<FieldFn>>,
    /// `timeYear.every` and `millisecond.every` replace the generic filter
    /// with closed forms.
    every_override: Option<Arc<EveryFn>>,
}

impl std::fmt::Debug for Interval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Interval")
    }
}

impl Interval {
    fn new(
        floori: impl Fn(Ms) -> Ms + Send + Sync + 'static,
        offseti: impl Fn(Ms, i64) -> Ms + Send + Sync + 'static,
    ) -> Interval {
        Interval {
            floori: Arc::new(floori),
            offseti: Arc::new(offseti),
            count: None,
            field: None,
            every_override: None,
        }
    }

    fn with_count(
        mut self,
        count: impl Fn(Ms, Ms) -> f64 + Send + Sync + 'static,
        field: Option<Arc<FieldFn>>,
    ) -> Interval {
        self.count = Some(Arc::new(count));
        self.field = field;
        self
    }

    /// `interval.floor(date)`.
    pub fn floor(&self, date: Ms) -> Ms {
        (self.floori)(date)
    }

    /// `interval.ceil(date)`: floor of `date - 1`, one step on, floored again.
    pub fn ceil(&self, date: Ms) -> Ms {
        let d = (self.floori)(date - 1);
        let d = (self.offseti)(d, 1);
        (self.floori)(d)
    }

    /// `interval.round(date)`.
    pub fn round(&self, date: Ms) -> Ms {
        let d0 = self.floor(date);
        let d1 = self.ceil(date);
        if date - d0 < d1 - date { d0 } else { d1 }
    }

    /// `interval.offset(date, step)`.
    pub fn offset(&self, date: Ms, step: i64) -> Ms {
        (self.offseti)(date, step)
    }

    /// `interval.range(start, stop, step)`: every boundary from the ceiling
    /// of `start` up to but excluding `stop`.
    pub fn range(&self, start: Ms, stop: Ms, step: i64) -> Vec<Ms> {
        let mut range = Vec::new();
        let mut start = self.ceil(start);
        if start >= stop || step <= 0 {
            return range;
        }
        loop {
            let previous = start;
            range.push(previous);
            start = (self.offseti)(start, step);
            start = (self.floori)(start);
            if !(previous < start && start < stop) {
                break;
            }
        }
        range
    }

    /// `interval.filter(test)`: keeps the boundaries `test` accepts.
    pub fn filter(&self, test: impl Fn(Ms) -> bool + Send + Sync + 'static) -> Interval {
        let test = Arc::new(test);
        let offseti = self.offseti.clone();
        let test_floor = test.clone();
        let floor_base = self.floori.clone();
        let floor = move |mut date: Ms| {
            loop {
                date = floor_base(date);
                if test_floor(date) {
                    return date;
                }
                date -= 1;
            }
        };
        let offset = move |mut date: Ms, mut step: i64| {
            if step < 0 {
                while {
                    step += 1;
                    step <= 0
                } {
                    loop {
                        date = offseti(date, -1);
                        if test(date) {
                            break;
                        }
                    }
                }
            } else {
                while {
                    step -= 1;
                    step >= 0
                } {
                    loop {
                        date = offseti(date, 1);
                        if test(date) {
                            break;
                        }
                    }
                }
            }
            date
        };
        Interval::new(floor, offset)
    }

    /// `interval.count(start, end)`, `None` for an interval without a count
    /// function (a filtered one).
    pub fn count(&self, start: Ms, end: Ms) -> Option<i64> {
        let count = self.count.as_ref()?;
        let t0 = (self.floori)(start);
        let t1 = (self.floori)(end);
        Some(count(t0, t1).floor() as i64)
    }

    /// `interval.every(step)`: `None` where d3 returns `null`.
    pub fn every(&self, step: f64) -> Option<Interval> {
        if let Some(override_) = &self.every_override {
            return override_(step);
        }
        self.count.as_ref()?;
        let step = step.floor();
        if !step.is_finite() || step <= 0.0 {
            return None;
        }
        if step <= 1.0 {
            return Some(self.clone());
        }
        let step = step as i64;
        match &self.field {
            Some(field) => {
                let field = field.clone();
                Some(self.filter(move |d| field(d) % step == 0))
            }
            None => {
                let base = self.clone();
                Some(self.filter(move |d| base.count(0, d).unwrap_or(0) % step == 0))
            }
        }
    }
}

/// `timeMillisecond`.
pub fn millisecond() -> Interval {
    let mut interval = Interval::new(|date| date, |date, step| date + step)
        .with_count(|start, end| (end - start) as f64, None);
    interval.every_override = Some(Arc::new(|k: f64| {
        let k = k.floor();
        if !k.is_finite() || k <= 0.0 {
            return None;
        }
        if k <= 1.0 {
            return Some(millisecond());
        }
        let k = k as i64;
        Some(
            Interval::new(
                move |date| (date as f64 / k as f64).floor() as i64 * k,
                move |date, step| date + step * k,
            )
            .with_count(move |start, end| (end - start) as f64 / k as f64, None),
        )
    }));
    interval
}

/// `timeSecond`.
pub fn second() -> Interval {
    Interval::new(
        |date| date - civil(date).millis,
        |date, step| date + step * DURATION_SECOND,
    )
    .with_count(
        |start, end| (end - start) as f64 / DURATION_SECOND as f64,
        Some(Arc::new(|date| civil(date).seconds)),
    )
}

/// `timeMinute`.
pub fn minute() -> Interval {
    Interval::new(
        |date| {
            let c = civil(date);
            date - c.millis - c.seconds * DURATION_SECOND
        },
        |date, step| date + step * DURATION_MINUTE,
    )
    .with_count(
        |start, end| (end - start) as f64 / DURATION_MINUTE as f64,
        Some(Arc::new(|date| civil(date).minutes)),
    )
}

/// `timeHour`.
pub fn hour() -> Interval {
    Interval::new(
        |date| {
            let c = civil(date);
            date - c.millis - c.seconds * DURATION_SECOND - c.minutes * DURATION_MINUTE
        },
        |date, step| date + step * DURATION_HOUR,
    )
    .with_count(
        |start, end| (end - start) as f64 / DURATION_HOUR as f64,
        Some(Arc::new(|date| civil(date).hours)),
    )
}

/// `date.setHours(0, 0, 0, 0)` with the date's own calendar fields.
fn start_of_day(date: Ms) -> Ms {
    let c = civil(date);
    make(c.year, c.month, c.day, 0, 0, 0, 0)
}

/// `date.setDate(date.getDate() + step)`.
fn add_days(date: Ms, step: i64) -> Ms {
    let c = civil(date);
    make(
        c.year,
        c.month,
        c.day + step,
        c.hours,
        c.minutes,
        c.seconds,
        c.millis,
    )
}

/// The day and week counts remove the DST shift between the two ends.
fn zone_delta_ms(start: Ms, end: Ms) -> i64 {
    (timezone_offset_minutes(end) - timezone_offset_minutes(start)) * DURATION_MINUTE
}

/// `timeDay`.
pub fn day() -> Interval {
    Interval::new(start_of_day, add_days).with_count(
        |start, end| (end - start - zone_delta_ms(start, end)) as f64 / DURATION_DAY as f64,
        Some(Arc::new(|date| civil(date).day - 1)),
    )
}

/// `timeWeekday(i)`: weeks that start on weekday `i` (Sunday is 0).
pub fn weekday(i: i64) -> Interval {
    Interval::new(
        move |date| {
            let c = civil(date);
            let back = (c.weekday + 7 - i).rem_euclid(7);
            make(c.year, c.month, c.day - back, 0, 0, 0, 0)
        },
        |date, step| add_days(date, step * 7),
    )
    .with_count(
        |start, end| (end - start - zone_delta_ms(start, end)) as f64 / DURATION_WEEK as f64,
        None,
    )
}

pub fn sunday() -> Interval {
    weekday(0)
}

pub fn monday() -> Interval {
    weekday(1)
}

pub fn thursday() -> Interval {
    weekday(4)
}

/// `timeWeek` is `timeSunday`.
pub fn week() -> Interval {
    sunday()
}

/// `timeMonth`.
pub fn month() -> Interval {
    Interval::new(
        |date| {
            let c = civil(date);
            make(c.year, c.month, 1, 0, 0, 0, 0)
        },
        |date, step| {
            let c = civil(date);
            make(
                c.year,
                c.month + step,
                c.day,
                c.hours,
                c.minutes,
                c.seconds,
                c.millis,
            )
        },
    )
    .with_count(
        |start, end| {
            let (s, e) = (civil(start), civil(end));
            (e.month - s.month + (e.year - s.year) * 12) as f64
        },
        Some(Arc::new(|date| civil(date).month)),
    )
}

/// `timeYear`, with its closed-form `every`.
pub fn year() -> Interval {
    let mut interval = Interval::new(
        |date| make(civil(date).year, 0, 1, 0, 0, 0, 0),
        |date, step| {
            let c = civil(date);
            make(
                c.year + step,
                c.month,
                c.day,
                c.hours,
                c.minutes,
                c.seconds,
                c.millis,
            )
        },
    )
    .with_count(
        |start, end| (civil(end).year - civil(start).year) as f64,
        Some(Arc::new(|date| civil(date).year)),
    );
    interval.every_override = Some(Arc::new(|k: f64| {
        let k = k.floor();
        if !k.is_finite() || k <= 0.0 {
            return None;
        }
        let k = k as i64;
        Some(Interval::new(
            move |date| {
                let y = civil(date).year;
                make(y.div_euclid(k) * k, 0, 1, 0, 0, 0, 0)
            },
            move |date, step| {
                let c = civil(date);
                make(
                    c.year + step * k,
                    c.month,
                    c.day,
                    c.hours,
                    c.minutes,
                    c.seconds,
                    c.millis,
                )
            },
        ))
    }));
    interval
}

/// `tickSpec` from d3-array 3.2.4 `ticks.js`: `[i1, i2, inc]`.
fn tick_spec(start: f64, stop: f64, count: f64) -> (f64, f64, f64) {
    let e10 = 50f64.sqrt();
    let e5 = 10f64.sqrt();
    let e2 = 2f64.sqrt();
    let step = (stop - start) / count.max(0.0);
    let power = step.log10().floor();
    let error = step / 10f64.powf(power);
    let factor = if error >= e10 {
        10.0
    } else if error >= e5 {
        5.0
    } else if error >= e2 {
        2.0
    } else {
        1.0
    };
    let (mut i1, mut i2, inc);
    if power < 0.0 {
        let inc0 = 10f64.powf(-power) / factor;
        i1 = js_round(start * inc0);
        i2 = js_round(stop * inc0);
        if i1 / inc0 < start {
            i1 += 1.0;
        }
        if i2 / inc0 > stop {
            i2 -= 1.0;
        }
        inc = -inc0;
    } else {
        let inc0 = 10f64.powf(power) * factor;
        i1 = js_round(start / inc0);
        i2 = js_round(stop / inc0);
        if i1 * inc0 < start {
            i1 += 1.0;
        }
        if i2 * inc0 > stop {
            i2 -= 1.0;
        }
        inc = inc0;
    }
    if i2 < i1 && (0.5..2.0).contains(&count) {
        return tick_spec(start, stop, count * 2.0);
    }
    (i1, i2, inc)
}

/// `Math.round`: half rounds toward positive infinity.
pub fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// `tickIncrement` from d3-array.
pub fn tick_increment(start: f64, stop: f64, count: f64) -> f64 {
    tick_spec(start, stop, count).2
}

/// `tickStep` from d3-array.
pub fn tick_step(start: f64, stop: f64, count: f64) -> f64 {
    let reverse = stop < start;
    let inc = if reverse {
        tick_increment(stop, start, count)
    } else {
        tick_increment(start, stop, count)
    };
    let sign = if reverse { -1.0 } else { 1.0 };
    sign * if inc < 0.0 { 1.0 / -inc } else { inc }
}

/// `ticks` from d3-array: the nice values between `start` and `stop`.
pub fn linear_ticks(start: f64, stop: f64, count: f64) -> Vec<f64> {
    if count.is_nan() || count <= 0.0 {
        return Vec::new();
    }
    if start == stop {
        return vec![start];
    }
    let reverse = stop < start;
    let (i1, i2, inc) = if reverse {
        tick_spec(stop, start, count)
    } else {
        tick_spec(start, stop, count)
    };
    if i2 < i1 || i1.is_nan() || i2.is_nan() {
        return Vec::new();
    }
    let n = (i2 - i1 + 1.0) as usize;
    let mut ticks = Vec::with_capacity(n);
    for i in 0..n {
        let i = i as f64;
        let v = if reverse {
            if inc < 0.0 {
                (i2 - i) / -inc
            } else {
                (i2 - i) * inc
            }
        } else if inc < 0.0 {
            (i1 + i) / -inc
        } else {
            (i1 + i) * inc
        };
        ticks.push(v);
    }
    ticks
}

/// The `tickIntervals` table of `ticks.js`.
fn tick_intervals() -> Vec<(Interval, f64, f64)> {
    vec![
        (second(), 1.0, DURATION_SECOND as f64),
        (second(), 5.0, 5.0 * DURATION_SECOND as f64),
        (second(), 15.0, 15.0 * DURATION_SECOND as f64),
        (second(), 30.0, 30.0 * DURATION_SECOND as f64),
        (minute(), 1.0, DURATION_MINUTE as f64),
        (minute(), 5.0, 5.0 * DURATION_MINUTE as f64),
        (minute(), 15.0, 15.0 * DURATION_MINUTE as f64),
        (minute(), 30.0, 30.0 * DURATION_MINUTE as f64),
        (hour(), 1.0, DURATION_HOUR as f64),
        (hour(), 3.0, 3.0 * DURATION_HOUR as f64),
        (hour(), 6.0, 6.0 * DURATION_HOUR as f64),
        (hour(), 12.0, 12.0 * DURATION_HOUR as f64),
        (day(), 1.0, DURATION_DAY as f64),
        (day(), 2.0, 2.0 * DURATION_DAY as f64),
        (week(), 1.0, DURATION_WEEK as f64),
        (month(), 1.0, DURATION_MONTH as f64),
        (month(), 3.0, 3.0 * DURATION_MONTH as f64),
        (year(), 1.0, DURATION_YEAR as f64),
    ]
}

/// `timeTickInterval(start, stop, count)`: the interval whose step is
/// closest to the requested spacing, `None` where d3 returns `null`.
pub fn time_tick_interval(start: Ms, stop: Ms, count: f64) -> Option<Interval> {
    let table = tick_intervals();
    let target = ((stop - start) as f64).abs() / count;
    // `bisector(step).right`: the first entry whose step exceeds the target.
    let i = table.iter().position(|(_, _, step)| *step > target);
    match i {
        None => {
            let yr = DURATION_YEAR as f64;
            year().every(tick_step(start as f64 / yr, stop as f64 / yr, count))
        }
        Some(0) => millisecond().every(tick_step(start as f64, stop as f64, count).max(1.0)),
        Some(i) => {
            let pick = if target / table[i - 1].2 < table[i].2 / target {
                i - 1
            } else {
                i
            };
            let (interval, step, _) = &table[pick];
            interval.every(*step)
        }
    }
}

/// `timeTicks(start, stop, count)`.
pub fn time_ticks(start: Ms, stop: Ms, count: f64) -> Vec<Ms> {
    let reverse = stop < start;
    let (start, stop) = if reverse {
        (stop, start)
    } else {
        (start, stop)
    };
    let ticks = match time_tick_interval(start, stop, count) {
        Some(interval) => interval.range(start, stop + 1, 1),
        None => Vec::new(),
    };
    if reverse {
        ticks.into_iter().rev().collect()
    } else {
        ticks
    }
}

/// `timeTicks(start, stop, interval)` with an explicit interval.
pub fn time_ticks_with(start: Ms, stop: Ms, interval: &Interval) -> Vec<Ms> {
    let reverse = stop < start;
    let (start, stop) = if reverse {
        (stop, start)
    } else {
        (start, stop)
    };
    let ticks = interval.range(start, stop + 1, 1);
    if reverse {
        ticks.into_iter().rev().collect()
    } else {
        ticks
    }
}
