//! Path data builder and the shared number format.
//!
//! d3-path (which d3-shape's `line()` writes through) rounds every coordinate
//! to three decimals with `Math.round(v * 1000) / 1000` and then prints the
//! JS number. [`fmt_num`] reproduces that, so a curve port here prints the
//! same `d` string as d3 and an insta snapshot stays byte-stable.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Point {
        Point { x, y }
    }
}

impl From<(f64, f64)> for Point {
    fn from((x, y): (f64, f64)) -> Point {
        Point { x, y }
    }
}

/// Formats a coordinate the way d3-path does with `digits = 3`: rounded half
/// up to three decimals, trailing zeros trimmed, and never `-0`.
pub fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        // JS prints NaN and Infinity verbatim; a scene should never hold
        // one, but a snapshot must still be readable when it does.
        return format!("{v}");
    }
    // `Math.round` rounds half toward positive infinity, unlike `f64::round`.
    let rounded = (v * 1000.0 + 0.5).floor() / 1000.0;
    let mut s = format!("{rounded:.3}");
    if s.contains('.') {
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        s = trimmed.to_string();
    }
    if s == "-0" { "0".to_string() } else { s }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    MoveTo(Point),
    LineTo(Point),
    QuadTo {
        control: Point,
        to: Point,
    },
    CubicTo {
        c1: Point,
        c2: Point,
        to: Point,
    },
    ArcTo {
        rx: f64,
        ry: f64,
        x_axis_rotation: f64,
        large_arc: bool,
        sweep: bool,
        to: Point,
    },
    Close,
}

/// An SVG path in absolute commands.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PathData {
    commands: Vec<Command>,
}

impl PathData {
    pub fn new() -> PathData {
        PathData::default()
    }

    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn move_to(&mut self, x: f64, y: f64) -> &mut Self {
        self.commands.push(Command::MoveTo(Point::new(x, y)));
        self
    }

    pub fn line_to(&mut self, x: f64, y: f64) -> &mut Self {
        self.commands.push(Command::LineTo(Point::new(x, y)));
        self
    }

    pub fn quad_to(&mut self, cx: f64, cy: f64, x: f64, y: f64) -> &mut Self {
        self.commands.push(Command::QuadTo {
            control: Point::new(cx, cy),
            to: Point::new(x, y),
        });
        self
    }

    pub fn cubic_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64) -> &mut Self {
        self.commands.push(Command::CubicTo {
            c1: Point::new(x1, y1),
            c2: Point::new(x2, y2),
            to: Point::new(x, y),
        });
        self
    }

    #[allow(clippy::too_many_arguments)]
    pub fn arc_to(
        &mut self,
        rx: f64,
        ry: f64,
        x_axis_rotation: f64,
        large_arc: bool,
        sweep: bool,
        x: f64,
        y: f64,
    ) -> &mut Self {
        self.commands.push(Command::ArcTo {
            rx,
            ry,
            x_axis_rotation,
            large_arc,
            sweep,
            to: Point::new(x, y),
        });
        self
    }

    pub fn close(&mut self) -> &mut Self {
        self.commands.push(Command::Close);
        self
    }

    /// The `d` attribute in d3-path's compact form: `M1,2L3,4Z`.
    pub fn to_svg(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for PathData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for command in &self.commands {
            match *command {
                Command::MoveTo(p) => write!(f, "M{},{}", fmt_num(p.x), fmt_num(p.y))?,
                Command::LineTo(p) => write!(f, "L{},{}", fmt_num(p.x), fmt_num(p.y))?,
                Command::QuadTo { control, to } => write!(
                    f,
                    "Q{},{},{},{}",
                    fmt_num(control.x),
                    fmt_num(control.y),
                    fmt_num(to.x),
                    fmt_num(to.y)
                )?,
                Command::CubicTo { c1, c2, to } => write!(
                    f,
                    "C{},{},{},{},{},{}",
                    fmt_num(c1.x),
                    fmt_num(c1.y),
                    fmt_num(c2.x),
                    fmt_num(c2.y),
                    fmt_num(to.x),
                    fmt_num(to.y)
                )?,
                Command::ArcTo {
                    rx,
                    ry,
                    x_axis_rotation,
                    large_arc,
                    sweep,
                    to,
                } => write!(
                    f,
                    "A{},{},{},{},{},{},{}",
                    fmt_num(rx),
                    fmt_num(ry),
                    fmt_num(x_axis_rotation),
                    u8::from(large_arc),
                    u8::from(sweep),
                    fmt_num(to.x),
                    fmt_num(to.y)
                )?,
                Command::Close => f.write_str("Z")?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_follow_d3_path_rounding() {
        assert_eq!(fmt_num(1.0), "1");
        assert_eq!(fmt_num(1.5), "1.5");
        assert_eq!(fmt_num(0.12345), "0.123");
        assert_eq!(fmt_num(0.0005), "0.001");
        assert_eq!(fmt_num(-0.0004), "0");
        assert_eq!(fmt_num(-0.0), "0");
        assert_eq!(fmt_num(123456.789), "123456.789");
        assert_eq!(fmt_num(-2.25), "-2.25");
    }

    #[test]
    fn display_is_compact() {
        let mut d = PathData::new();
        d.move_to(0.0, 0.0)
            .line_to(10.0, 0.5)
            .quad_to(1.0, 2.0, 3.0, 4.0)
            .cubic_to(1.0, 2.0, 3.0, 4.0, 5.0, 6.0)
            .arc_to(5.0, 5.0, 0.0, false, true, 10.0, 10.0)
            .close();
        assert_eq!(
            d.to_svg(),
            "M0,0L10,0.5Q1,2,3,4C1,2,3,4,5,6A5,5,0,0,1,10,10Z"
        );
    }
}
