//! Port of d3-scale 4.0.2 `time.js` over `continuous.js`: a two-stop local
//! time scale with `rangeRound`, `ticks`, `tickFormat`, and `nice`.

use super::time::{self, Interval, Ms, day, hour, minute, month, second, week, year};
use super::time_format;

/// `scaleTime()` with one domain segment, which is all mermaid builds.
#[derive(Debug, Clone, PartialEq)]
pub struct ScaleTime {
    d0: f64,
    d1: f64,
    r0: f64,
    r1: f64,
    /// `rangeRound`: `interpolateRound` instead of `interpolateNumber`.
    round: bool,
}

impl ScaleTime {
    /// `scaleTime().domain(domain).range(range)`.
    pub fn new(domain: [Ms; 2], range: [f64; 2]) -> ScaleTime {
        ScaleTime {
            d0: domain[0] as f64,
            d1: domain[1] as f64,
            r0: range[0],
            r1: range[1],
            round: false,
        }
    }

    /// `.rangeRound(range)`.
    pub fn range_round(mut self, range: [f64; 2]) -> ScaleTime {
        self.r0 = range[0];
        self.r1 = range[1];
        self.round = true;
        self
    }

    pub fn domain(&self) -> [Ms; 2] {
        [self.d0 as Ms, self.d1 as Ms]
    }

    pub fn range(&self) -> [f64; 2] {
        [self.r0, self.r1]
    }

    /// `scale(x)` through `bimap`: `normalize` then `interpolate`.
    pub fn apply(&self, x: Ms) -> f64 {
        let x = x as f64;
        // `bimap` reverses a descending domain so the normalizer sees
        // `[lo, hi]` and the interpolator the swapped range.
        let (a, b, ra, rb) = if self.d1 < self.d0 {
            (self.d1, self.d0, self.r1, self.r0)
        } else {
            (self.d0, self.d1, self.r0, self.r1)
        };
        let span = b - a;
        let t = if span != 0.0 {
            (x - a) / span
        } else if span.is_nan() {
            f64::NAN
        } else {
            0.5
        };
        let v = ra * (1.0 - t) + rb * t;
        if self.round { time::js_round(v) } else { v }
    }

    /// `scale.invert(y)`.
    pub fn invert(&self, y: f64) -> Ms {
        let (a, b, ra, rb) = if self.d1 < self.d0 {
            (self.d1, self.d0, self.r1, self.r0)
        } else {
            (self.d0, self.d1, self.r0, self.r1)
        };
        let span = rb - ra;
        let t = if span != 0.0 { (y - ra) / span } else { 0.5 };
        (a * (1.0 - t) + b * t) as Ms
    }

    /// `scale.ticks(count)`; `None` is d3's default of 10.
    pub fn ticks(&self, count: Option<f64>) -> Vec<Ms> {
        let [a, b] = self.domain();
        time::time_ticks(a, b, count.unwrap_or(10.0))
    }

    /// `scale.ticks(interval)`.
    pub fn ticks_with(&self, interval: &Interval) -> Vec<Ms> {
        let [a, b] = self.domain();
        time::time_ticks_with(a, b, interval)
    }

    /// `scale.tickFormat()` without a specifier: the multi-scale format
    /// that names the largest unit a tick starts.
    pub fn tick_format(&self, date: Ms) -> String {
        let specifier = if second().floor(date) < date {
            ".%L"
        } else if minute().floor(date) < date {
            ":%S"
        } else if hour().floor(date) < date {
            "%I:%M"
        } else if day().floor(date) < date {
            "%I %p"
        } else if month().floor(date) < date {
            if week().floor(date) < date {
                "%a %d"
            } else {
                "%b %d"
            }
        } else if year().floor(date) < date {
            "%B"
        } else {
            "%Y"
        };
        time_format::format(specifier, date)
    }

    /// `scale.nice(count)`: extends the domain to the tick interval's
    /// boundaries; a domain d3 cannot pick an interval for stays as it is.
    pub fn nice(&mut self, count: Option<f64>) {
        let [a, b] = self.domain();
        if let Some(interval) = time::time_tick_interval(a, b, count.unwrap_or(10.0)) {
            self.nice_with(&interval);
        }
    }

    /// `scale.nice(interval)`.
    pub fn nice_with(&mut self, interval: &Interval) {
        let (lo, hi, swapped) = if self.d1 < self.d0 {
            (self.d1, self.d0, true)
        } else {
            (self.d0, self.d1, false)
        };
        let lo = interval.floor(lo as Ms) as f64;
        let hi = interval.ceil(hi as Ms) as f64;
        if swapped {
            self.d0 = hi;
            self.d1 = lo;
        } else {
            self.d0 = lo;
            self.d1 = hi;
        }
    }
}
