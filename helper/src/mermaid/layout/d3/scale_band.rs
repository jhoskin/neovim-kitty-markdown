//! d3-scale 4.0.2 `band.js` over `ordinal.js`.
//!
//! The ordinal domain keeps the first occurrence of a repeated value, so a
//! duplicate category maps to the same position; a value outside the
//! domain maps to `None`, since `band()` deletes the implicit `unknown`.

use indexmap::IndexSet;

#[derive(Debug, Clone, PartialEq)]
pub struct ScaleBand {
    domain: IndexSet<String>,
    r0: f64,
    r1: f64,
    padding_inner: f64,
    padding_outer: f64,
    align: f64,
    round: bool,
    step: f64,
    bandwidth: f64,
    /// `ordinalRange`: the start of each band, in domain order.
    values: Vec<f64>,
}

impl Default for ScaleBand {
    fn default() -> Self {
        ScaleBand::new()
    }
}

/// `Math.round`.
fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

impl ScaleBand {
    pub fn new() -> ScaleBand {
        let mut s = ScaleBand {
            domain: IndexSet::new(),
            r0: 0.0,
            r1: 1.0,
            padding_inner: 0.0,
            padding_outer: 0.0,
            align: 0.5,
            round: false,
            step: 0.0,
            bandwidth: 0.0,
            values: Vec::new(),
        };
        s.rescale();
        s
    }

    pub fn domain<I, S>(mut self, domain: I) -> ScaleBand
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.domain = domain.into_iter().map(Into::into).collect();
        self.rescale();
        self
    }

    pub fn range(mut self, range: [f64; 2]) -> ScaleBand {
        self.r0 = range[0];
        self.r1 = range[1];
        self.rescale();
        self
    }

    /// `rangeRound(range)`: `range` with `round(true)`.
    pub fn range_round(mut self, range: [f64; 2]) -> ScaleBand {
        self.r0 = range[0];
        self.r1 = range[1];
        self.round = true;
        self.rescale();
        self
    }

    pub fn padding_inner(mut self, p: f64) -> ScaleBand {
        self.padding_inner = p.min(1.0);
        self.rescale();
        self
    }

    pub fn padding_outer(mut self, p: f64) -> ScaleBand {
        self.padding_outer = p;
        self.rescale();
        self
    }

    /// `padding(p)`: both paddings at once.
    pub fn padding(mut self, p: f64) -> ScaleBand {
        self.padding_inner = p.min(1.0);
        self.padding_outer = p;
        self.rescale();
        self
    }

    pub fn align(mut self, a: f64) -> ScaleBand {
        self.align = a.clamp(0.0, 1.0);
        self.rescale();
        self
    }

    pub fn round(mut self, round: bool) -> ScaleBand {
        self.round = round;
        self.rescale();
        self
    }

    fn rescale(&mut self) {
        let n = self.domain.len() as f64;
        let reverse = self.r1 < self.r0;
        let (mut start, stop) = if reverse {
            (self.r1, self.r0)
        } else {
            (self.r0, self.r1)
        };
        self.step = (stop - start) / (n - self.padding_inner + self.padding_outer * 2.0).max(1.0);
        if self.round {
            self.step = self.step.floor();
        }
        start += (stop - start - self.step * (n - self.padding_inner)) * self.align;
        self.bandwidth = self.step * (1.0 - self.padding_inner);
        if self.round {
            start = js_round(start);
            self.bandwidth = js_round(self.bandwidth);
        }
        let count = self.domain.len();
        let mut values: Vec<f64> = (0..count).map(|i| start + self.step * i as f64).collect();
        if reverse {
            values.reverse();
        }
        self.values = values;
    }

    /// `scale(value)`: the start of the band for a known domain value.
    pub fn get(&self, value: &str) -> Option<f64> {
        let i = self.domain.get_index_of(value)?;
        self.values.get(i).copied()
    }

    pub fn bandwidth(&self) -> f64 {
        self.bandwidth
    }

    pub fn step(&self) -> f64 {
        self.step
    }

    pub fn domain_values(&self) -> impl Iterator<Item = &str> {
        self.domain.iter().map(String::as_str)
    }

    pub fn range_values(&self) -> [f64; 2] {
        [self.r0, self.r1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mermaid_axis_configuration_puts_bands_on_the_ticks() {
        let s = ScaleBand::new()
            .domain(["a", "b", "c"])
            .range([0.0, 100.0])
            .padding_inner(1.0)
            .padding_outer(0.0)
            .align(0.5);
        assert_eq!(s.get("a"), Some(0.0));
        assert_eq!(s.get("b"), Some(50.0));
        assert_eq!(s.get("c"), Some(100.0));
        assert_eq!(s.bandwidth(), 0.0);
        assert_eq!(s.step(), 50.0);
        assert_eq!(s.get("d"), None);
    }

    #[test]
    fn single_band_sits_in_the_middle_and_duplicates_collapse() {
        let s = ScaleBand::new()
            .domain(["a"])
            .range([0.0, 100.0])
            .padding_inner(1.0);
        assert_eq!(s.get("a"), Some(50.0));
        let d = ScaleBand::new()
            .domain(["x", "y", "x"])
            .range([0.0, 10.0])
            .padding_inner(1.0);
        assert_eq!(d.get("x"), Some(0.0));
        assert_eq!(d.get("y"), Some(10.0));
    }

    #[test]
    fn padding_and_round() {
        let s = ScaleBand::new()
            .domain(["a", "b", "c"])
            .range([0.0, 100.0])
            .padding(0.2);
        assert!((s.step() - 31.25).abs() < 1e-12);
        assert!((s.bandwidth() - 25.0).abs() < 1e-12);
        let r = ScaleBand::new()
            .domain(["a", "b", "c"])
            .range_round([0.0, 100.0])
            .padding(0.2);
        assert_eq!(r.step(), 31.0);
        assert_eq!(r.bandwidth(), 25.0);
        assert_eq!(r.get("a"), Some(7.0));
    }
}
