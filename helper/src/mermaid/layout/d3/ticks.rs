//! d3-array 3.2.4 `ticks.js`: `ticks`, `tickIncrement`, and `tickStep`.
//!
//! The port keeps the float behavior of the JavaScript: `Math.round` rounds
//! half up, the increment is negative when the step is a fraction, and the
//! tick values come from integer multiples divided by the inverse step, so
//! `ticks(0, 1, 10)` yields `0.3` and not `0.30000000000000004`.

/// `Math.round`: half rounds toward positive infinity, unlike `f64::round`.
fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// `tickSpec(start, stop, count)`: the first and last tick index and the
/// increment, where a negative increment stands for its reciprocal.
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
        let scale = 10f64.powf(-power) / factor;
        i1 = js_round(start * scale);
        i2 = js_round(stop * scale);
        if i1 / scale < start {
            i1 += 1.0;
        }
        if i2 / scale > stop {
            i2 -= 1.0;
        }
        inc = -scale;
    } else {
        inc = 10f64.powf(power) * factor;
        i1 = js_round(start / inc);
        i2 = js_round(stop / inc);
        if i1 * inc < start {
            i1 += 1.0;
        }
        if i2 * inc > stop {
            i2 -= 1.0;
        }
    }
    if i2 < i1 && (0.5..2.0).contains(&count) {
        return tick_spec(start, stop, count * 2.0);
    }
    (i1, i2, inc)
}

/// `ticks(start, stop, count)`: about `count` nicely rounded values between
/// `start` and `stop`, inclusive, in the direction from `start` to `stop`.
pub fn ticks(start: f64, stop: f64, count: f64) -> Vec<f64> {
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
    if i1.is_nan() || i2.is_nan() || i2 < i1 {
        return Vec::new();
    }
    let n = (i2 - i1 + 1.0) as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let i = i as f64;
        let value = if reverse {
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
        out.push(value);
    }
    out
}

/// `tickIncrement(start, stop, count)`: the increment `ticks` uses, negative
/// for the reciprocal of a fractional step. `start` must not exceed `stop`.
pub fn tick_increment(start: f64, stop: f64, count: f64) -> f64 {
    tick_spec(start, stop, count).2
}

/// `tickStep(start, stop, count)`: the tick increment as a plain step,
/// negative when `stop < start`.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_steps_avoid_float_noise() {
        assert_eq!(
            ticks(0.0, 1.0, 10.0),
            vec![0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0]
        );
    }

    #[test]
    fn degenerate_inputs() {
        assert_eq!(ticks(5.0, 5.0, 10.0), vec![5.0]);
        assert!(ticks(0.0, 1.0, 0.0).is_empty());
        assert!(ticks(0.0, 1.0, f64::NAN).is_empty());
    }

    #[test]
    fn reverse_direction() {
        assert_eq!(ticks(10.0, 0.0, 5.0), vec![10.0, 8.0, 6.0, 4.0, 2.0, 0.0]);
        assert_eq!(tick_step(10.0, 0.0, 5.0), -2.0);
    }
}
