//! Ports of the d3-shape curves mermaid can select, plus mermaid's own
//! `rounded` edge path from `rendering-elements/edges.js`.
//!
//! Each port follows the d3-shape source statement by statement, in the mode
//! `line()` drives it: `lineStart`, one `point` per datum, `lineEnd`, with
//! the area flag `_line` never set. In that mode `_line` is `undefined`, so
//! the close condition `this._line || (this._line !== 0 && ...)` reduces to
//! the point-count check alone. The numeric state uses `f64::NAN` where d3
//! uses `NaN`, and the comments name the d3 field each variable mirrors.

use super::path::{PathData, Point};

/// A curve mermaid can name. `from_mermaid` accepts both the `curveBasis`
/// spelling of `utils.ts` (`d3CurveTypes`) and the short `basis` spelling of
/// `flowchart.curve` and `edge.curve` in `edges.js`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CurveKind {
    Basis,
    BasisClosed,
    BasisOpen,
    BumpX,
    BumpY,
    Bundle,
    Cardinal,
    CardinalClosed,
    CardinalOpen,
    CatmullRom,
    CatmullRomClosed,
    CatmullRomOpen,
    Linear,
    LinearClosed,
    MonotoneX,
    MonotoneY,
    Natural,
    Step,
    StepAfter,
    StepBefore,
    /// mermaid's `generateRoundedPath` with radius 5; not a d3 curve.
    Rounded,
}

impl CurveKind {
    pub const ALL: [CurveKind; 21] = [
        CurveKind::Basis,
        CurveKind::BasisClosed,
        CurveKind::BasisOpen,
        CurveKind::BumpX,
        CurveKind::BumpY,
        CurveKind::Bundle,
        CurveKind::Cardinal,
        CurveKind::CardinalClosed,
        CurveKind::CardinalOpen,
        CurveKind::CatmullRom,
        CurveKind::CatmullRomClosed,
        CurveKind::CatmullRomOpen,
        CurveKind::Linear,
        CurveKind::LinearClosed,
        CurveKind::MonotoneX,
        CurveKind::MonotoneY,
        CurveKind::Natural,
        CurveKind::Step,
        CurveKind::StepAfter,
        CurveKind::StepBefore,
        CurveKind::Rounded,
    ];

    /// The d3 export name, `curveBasis` and so on; `rounded` for the mermaid
    /// curve.
    pub fn d3_name(self) -> &'static str {
        match self {
            CurveKind::Basis => "curveBasis",
            CurveKind::BasisClosed => "curveBasisClosed",
            CurveKind::BasisOpen => "curveBasisOpen",
            CurveKind::BumpX => "curveBumpX",
            CurveKind::BumpY => "curveBumpY",
            CurveKind::Bundle => "curveBundle",
            CurveKind::Cardinal => "curveCardinal",
            CurveKind::CardinalClosed => "curveCardinalClosed",
            CurveKind::CardinalOpen => "curveCardinalOpen",
            CurveKind::CatmullRom => "curveCatmullRom",
            CurveKind::CatmullRomClosed => "curveCatmullRomClosed",
            CurveKind::CatmullRomOpen => "curveCatmullRomOpen",
            CurveKind::Linear => "curveLinear",
            CurveKind::LinearClosed => "curveLinearClosed",
            CurveKind::MonotoneX => "curveMonotoneX",
            CurveKind::MonotoneY => "curveMonotoneY",
            CurveKind::Natural => "curveNatural",
            CurveKind::Step => "curveStep",
            CurveKind::StepAfter => "curveStepAfter",
            CurveKind::StepBefore => "curveStepBefore",
            CurveKind::Rounded => "rounded",
        }
    }

    /// Resolves a mermaid curve name. Mirrors `interpolateToCurve`: the
    /// first letter is case-insensitive and the `curve` prefix is optional.
    /// Returns `None` for an unknown name so the caller can apply its own
    /// default (`curveBasis` for `interpolateToCurve`, `basis` for the
    /// `edges.js` switch).
    pub fn from_mermaid(name: &str) -> Option<CurveKind> {
        let name = name.trim();
        let short = name.strip_prefix("curve").unwrap_or(name);
        let mut chars = short.chars();
        let head = chars.next()?.to_ascii_lowercase();
        let short: String = std::iter::once(head).chain(chars).collect();
        CurveKind::ALL.iter().copied().find(|kind| {
            let full = kind.d3_name();
            let candidate = full.strip_prefix("curve").unwrap_or(full);
            let mut chars = candidate.chars();
            let head = chars.next().map(|c| c.to_ascii_lowercase());
            head.is_some_and(|h| {
                let mut lowered = String::from(h);
                lowered.extend(chars);
                lowered == short
            })
        })
    }
}

/// mermaid's `curveBundle` beta: d3's default, `0.85`.
pub const BUNDLE_BETA: f64 = 0.85;
/// mermaid's `curveCardinal*` tension: d3's default, `0`.
pub const CARDINAL_TENSION: f64 = 0.0;
/// mermaid's `curveCatmullRom*` alpha: d3's default, `0.5`.
pub const CATMULL_ROM_ALPHA: f64 = 0.5;
/// `generateRoundedPath(points, 5)` in `edges.js`.
pub const ROUNDED_RADIUS: f64 = 5.0;

/// Builds the path d3's `line().curve(kind)(points)` would, with the
/// parameters mermaid uses.
pub fn line(points: &[Point], kind: CurveKind) -> PathData {
    match kind {
        CurveKind::Basis => basis(points),
        CurveKind::BasisClosed => basis_closed(points),
        CurveKind::BasisOpen => basis_open(points),
        CurveKind::BumpX => bump_x(points),
        CurveKind::BumpY => bump_y(points),
        CurveKind::Bundle => bundle(points, BUNDLE_BETA),
        CurveKind::Cardinal => cardinal(points, CARDINAL_TENSION),
        CurveKind::CardinalClosed => cardinal_closed(points, CARDINAL_TENSION),
        CurveKind::CardinalOpen => cardinal_open(points, CARDINAL_TENSION),
        CurveKind::CatmullRom => catmull_rom(points, CATMULL_ROM_ALPHA),
        CurveKind::CatmullRomClosed => catmull_rom_closed(points, CATMULL_ROM_ALPHA),
        CurveKind::CatmullRomOpen => catmull_rom_open(points, CATMULL_ROM_ALPHA),
        CurveKind::Linear => linear(points),
        CurveKind::LinearClosed => linear_closed(points),
        CurveKind::MonotoneX => monotone_x(points),
        CurveKind::MonotoneY => monotone_y(points),
        CurveKind::Natural => natural(points),
        CurveKind::Step => step(points, 0.5),
        CurveKind::StepAfter => step(points, 1.0),
        CurveKind::StepBefore => step(points, 0.0),
        CurveKind::Rounded => rounded(points, ROUNDED_RADIUS),
    }
}

/// The d3-path context, optionally reflected across the diagonal for
/// `monotoneY`, which d3 implements as `monotoneX` through a
/// `ReflectContext`.
struct Ctx<'a> {
    path: &'a mut PathData,
    reflect: bool,
}

impl Ctx<'_> {
    fn move_to(&mut self, x: f64, y: f64) {
        if self.reflect {
            self.path.move_to(y, x);
        } else {
            self.path.move_to(x, y);
        }
    }

    fn line_to(&mut self, x: f64, y: f64) {
        if self.reflect {
            self.path.line_to(y, x);
        } else {
            self.path.line_to(x, y);
        }
    }

    fn bezier_curve_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64) {
        if self.reflect {
            self.path.cubic_to(y1, x1, y2, x2, y, x);
        } else {
            self.path.cubic_to(x1, y1, x2, y2, x, y);
        }
    }

    fn close_path(&mut self) {
        self.path.close();
    }
}

/// The `lineStart`, `point`, `lineEnd` protocol of a d3 curve.
trait Curve {
    fn line_start(&mut self);
    fn point(&mut self, x: f64, y: f64);
    fn line_end(&mut self);
}

fn drive<C: Curve>(mut curve: C, points: &[Point]) {
    curve.line_start();
    for p in points {
        curve.point(p.x, p.y);
    }
    curve.line_end();
}

// ---------------------------------------------------------------- linear

struct Linear<'a> {
    ctx: Ctx<'a>,
    point: u8,
}

impl Curve for Linear<'_> {
    fn line_start(&mut self) {
        self.point = 0;
    }

    fn point(&mut self, x: f64, y: f64) {
        match self.point {
            0 => {
                self.point = 1;
                self.ctx.move_to(x, y);
            }
            1 => {
                self.point = 2;
                self.ctx.line_to(x, y);
            }
            _ => self.ctx.line_to(x, y),
        }
    }

    fn line_end(&mut self) {
        if self.point == 1 {
            self.ctx.close_path();
        }
    }
}

pub fn linear(points: &[Point]) -> PathData {
    let mut path = PathData::new();
    drive(
        Linear {
            ctx: Ctx {
                path: &mut path,
                reflect: false,
            },
            point: 0,
        },
        points,
    );
    path
}

struct LinearClosed<'a> {
    ctx: Ctx<'a>,
    point: u8,
}

impl Curve for LinearClosed<'_> {
    fn line_start(&mut self) {
        self.point = 0;
    }

    fn point(&mut self, x: f64, y: f64) {
        if self.point != 0 {
            self.ctx.line_to(x, y);
        } else {
            self.point = 1;
            self.ctx.move_to(x, y);
        }
    }

    fn line_end(&mut self) {
        if self.point != 0 {
            self.ctx.close_path();
        }
    }
}

pub fn linear_closed(points: &[Point]) -> PathData {
    let mut path = PathData::new();
    drive(
        LinearClosed {
            ctx: Ctx {
                path: &mut path,
                reflect: false,
            },
            point: 0,
        },
        points,
    );
    path
}

// ----------------------------------------------------------------- basis

/// `point(that, x, y)` from `basis.js`.
fn basis_point(ctx: &mut Ctx<'_>, x0: f64, y0: f64, x1: f64, y1: f64, x: f64, y: f64) {
    ctx.bezier_curve_to(
        (2.0 * x0 + x1) / 3.0,
        (2.0 * y0 + y1) / 3.0,
        (x0 + 2.0 * x1) / 3.0,
        (y0 + 2.0 * y1) / 3.0,
        (x0 + 4.0 * x1 + x) / 6.0,
        (y0 + 4.0 * y1 + y) / 6.0,
    );
}

struct Basis<'a> {
    ctx: Ctx<'a>,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    point: u8,
}

impl<'a> Basis<'a> {
    fn new(path: &'a mut PathData) -> Basis<'a> {
        Basis {
            ctx: Ctx {
                path,
                reflect: false,
            },
            x0: f64::NAN,
            y0: f64::NAN,
            x1: f64::NAN,
            y1: f64::NAN,
            point: 0,
        }
    }
}

impl Curve for Basis<'_> {
    fn line_start(&mut self) {
        self.x0 = f64::NAN;
        self.x1 = f64::NAN;
        self.y0 = f64::NAN;
        self.y1 = f64::NAN;
        self.point = 0;
    }

    fn point(&mut self, x: f64, y: f64) {
        match self.point {
            0 => {
                self.point = 1;
                self.ctx.move_to(x, y);
            }
            1 => self.point = 2,
            2 => {
                self.point = 3;
                self.ctx.line_to(
                    (5.0 * self.x0 + self.x1) / 6.0,
                    (5.0 * self.y0 + self.y1) / 6.0,
                );
                basis_point(&mut self.ctx, self.x0, self.y0, self.x1, self.y1, x, y);
            }
            _ => basis_point(&mut self.ctx, self.x0, self.y0, self.x1, self.y1, x, y),
        }
        self.x0 = self.x1;
        self.x1 = x;
        self.y0 = self.y1;
        self.y1 = y;
    }

    fn line_end(&mut self) {
        match self.point {
            3 => {
                basis_point(
                    &mut self.ctx,
                    self.x0,
                    self.y0,
                    self.x1,
                    self.y1,
                    self.x1,
                    self.y1,
                );
                self.ctx.line_to(self.x1, self.y1);
            }
            2 => self.ctx.line_to(self.x1, self.y1),
            _ => {}
        }
        if self.point == 1 {
            self.ctx.close_path();
        }
    }
}

pub fn basis(points: &[Point]) -> PathData {
    let mut path = PathData::new();
    drive(Basis::new(&mut path), points);
    path
}

struct BasisClosed<'a> {
    ctx: Ctx<'a>,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    x3: f64,
    y3: f64,
    x4: f64,
    y4: f64,
    point: u8,
}

impl BasisClosed<'_> {
    fn point_inner(&mut self, x: f64, y: f64) {
        match self.point {
            0 => {
                self.point = 1;
                self.x2 = x;
                self.y2 = y;
            }
            1 => {
                self.point = 2;
                self.x3 = x;
                self.y3 = y;
            }
            2 => {
                self.point = 3;
                self.x4 = x;
                self.y4 = y;
                self.ctx.move_to(
                    (self.x0 + 4.0 * self.x1 + x) / 6.0,
                    (self.y0 + 4.0 * self.y1 + y) / 6.0,
                );
            }
            _ => basis_point(&mut self.ctx, self.x0, self.y0, self.x1, self.y1, x, y),
        }
        self.x0 = self.x1;
        self.x1 = x;
        self.y0 = self.y1;
        self.y1 = y;
    }
}

impl Curve for BasisClosed<'_> {
    fn line_start(&mut self) {
        self.x0 = f64::NAN;
        self.x1 = f64::NAN;
        self.x2 = f64::NAN;
        self.x3 = f64::NAN;
        self.x4 = f64::NAN;
        self.y0 = f64::NAN;
        self.y1 = f64::NAN;
        self.y2 = f64::NAN;
        self.y3 = f64::NAN;
        self.y4 = f64::NAN;
        self.point = 0;
    }

    fn point(&mut self, x: f64, y: f64) {
        self.point_inner(x, y);
    }

    fn line_end(&mut self) {
        match self.point {
            1 => {
                self.ctx.move_to(self.x2, self.y2);
                self.ctx.close_path();
            }
            2 => {
                self.ctx.move_to(
                    (self.x2 + 2.0 * self.x3) / 3.0,
                    (self.y2 + 2.0 * self.y3) / 3.0,
                );
                self.ctx.line_to(
                    (self.x3 + 2.0 * self.x2) / 3.0,
                    (self.y3 + 2.0 * self.y2) / 3.0,
                );
                self.ctx.close_path();
            }
            3 => {
                let (x2, y2, x3, y3, x4, y4) =
                    (self.x2, self.y2, self.x3, self.y3, self.x4, self.y4);
                self.point_inner(x2, y2);
                self.point_inner(x3, y3);
                self.point_inner(x4, y4);
            }
            _ => {}
        }
    }
}

pub fn basis_closed(points: &[Point]) -> PathData {
    let mut path = PathData::new();
    drive(
        BasisClosed {
            ctx: Ctx {
                path: &mut path,
                reflect: false,
            },
            x0: f64::NAN,
            y0: f64::NAN,
            x1: f64::NAN,
            y1: f64::NAN,
            x2: f64::NAN,
            y2: f64::NAN,
            x3: f64::NAN,
            y3: f64::NAN,
            x4: f64::NAN,
            y4: f64::NAN,
            point: 0,
        },
        points,
    );
    path
}

struct BasisOpen<'a> {
    ctx: Ctx<'a>,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    point: u8,
}

impl Curve for BasisOpen<'_> {
    fn line_start(&mut self) {
        self.x0 = f64::NAN;
        self.x1 = f64::NAN;
        self.y0 = f64::NAN;
        self.y1 = f64::NAN;
        self.point = 0;
    }

    fn point(&mut self, x: f64, y: f64) {
        match self.point {
            0 => self.point = 1,
            1 => self.point = 2,
            2 => {
                self.point = 3;
                let x0 = (self.x0 + 4.0 * self.x1 + x) / 6.0;
                let y0 = (self.y0 + 4.0 * self.y1 + y) / 6.0;
                self.ctx.move_to(x0, y0);
            }
            3 => {
                self.point = 4;
                basis_point(&mut self.ctx, self.x0, self.y0, self.x1, self.y1, x, y);
            }
            _ => basis_point(&mut self.ctx, self.x0, self.y0, self.x1, self.y1, x, y),
        }
        self.x0 = self.x1;
        self.x1 = x;
        self.y0 = self.y1;
        self.y1 = y;
    }

    fn line_end(&mut self) {
        if self.point == 3 {
            self.ctx.close_path();
        }
    }
}

pub fn basis_open(points: &[Point]) -> PathData {
    let mut path = PathData::new();
    drive(
        BasisOpen {
            ctx: Ctx {
                path: &mut path,
                reflect: false,
            },
            x0: f64::NAN,
            y0: f64::NAN,
            x1: f64::NAN,
            y1: f64::NAN,
            point: 0,
        },
        points,
    );
    path
}

/// `curveBundle.beta(beta)`: straightens the points toward the chord, then
/// runs them through `Basis`. d3 returns a plain `Basis` for `beta === 1`.
pub fn bundle(points: &[Point], beta: f64) -> PathData {
    if beta == 1.0 {
        return basis(points);
    }
    let mut path = PathData::new();
    let mut inner = Basis::new(&mut path);
    inner.line_start();
    let j = points.len().saturating_sub(1);
    if j > 0 {
        let x0 = points[0].x;
        let y0 = points[0].y;
        let dx = points[j].x - x0;
        let dy = points[j].y - y0;
        for (i, p) in points.iter().enumerate() {
            let t = i as f64 / j as f64;
            inner.point(
                beta * p.x + (1.0 - beta) * (x0 + t * dx),
                beta * p.y + (1.0 - beta) * (y0 + t * dy),
            );
        }
    }
    inner.line_end();
    path
}

// -------------------------------------------------------------- cardinal

/// `point(that, x, y)` from `cardinal.js`.
#[allow(clippy::too_many_arguments)]
fn cardinal_point(
    ctx: &mut Ctx<'_>,
    k: f64,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    x: f64,
    y: f64,
) {
    ctx.bezier_curve_to(
        x1 + k * (x2 - x0),
        y1 + k * (y2 - y0),
        x2 + k * (x1 - x),
        y2 + k * (y1 - y),
        x2,
        y2,
    );
}

struct Cardinal<'a> {
    ctx: Ctx<'a>,
    k: f64,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    point: u8,
}

impl<'a> Cardinal<'a> {
    fn new(path: &'a mut PathData, tension: f64) -> Cardinal<'a> {
        Cardinal {
            ctx: Ctx {
                path,
                reflect: false,
            },
            k: (1.0 - tension) / 6.0,
            x0: f64::NAN,
            y0: f64::NAN,
            x1: f64::NAN,
            y1: f64::NAN,
            x2: f64::NAN,
            y2: f64::NAN,
            point: 0,
        }
    }

    fn shift(&mut self, x: f64, y: f64) {
        self.x0 = self.x1;
        self.x1 = self.x2;
        self.x2 = x;
        self.y0 = self.y1;
        self.y1 = self.y2;
        self.y2 = y;
    }

    fn emit(&mut self, x: f64, y: f64) {
        cardinal_point(
            &mut self.ctx,
            self.k,
            self.x0,
            self.y0,
            self.x1,
            self.y1,
            self.x2,
            self.y2,
            x,
            y,
        );
    }
}

impl Curve for Cardinal<'_> {
    fn line_start(&mut self) {
        self.x0 = f64::NAN;
        self.x1 = f64::NAN;
        self.x2 = f64::NAN;
        self.y0 = f64::NAN;
        self.y1 = f64::NAN;
        self.y2 = f64::NAN;
        self.point = 0;
    }

    fn point(&mut self, x: f64, y: f64) {
        match self.point {
            0 => {
                self.point = 1;
                self.ctx.move_to(x, y);
            }
            1 => {
                self.point = 2;
                self.x1 = x;
                self.y1 = y;
            }
            2 => {
                self.point = 3;
                self.emit(x, y);
            }
            _ => self.emit(x, y),
        }
        self.shift(x, y);
    }

    fn line_end(&mut self) {
        match self.point {
            2 => self.ctx.line_to(self.x2, self.y2),
            3 => {
                let (x1, y1) = (self.x1, self.y1);
                self.emit(x1, y1);
            }
            _ => {}
        }
        if self.point == 1 {
            self.ctx.close_path();
        }
    }
}

pub fn cardinal(points: &[Point], tension: f64) -> PathData {
    let mut path = PathData::new();
    drive(Cardinal::new(&mut path, tension), points);
    path
}

struct CardinalClosed<'a> {
    ctx: Ctx<'a>,
    k: f64,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    x3: f64,
    y3: f64,
    x4: f64,
    y4: f64,
    x5: f64,
    y5: f64,
    point: u8,
}

impl<'a> CardinalClosed<'a> {
    fn new(path: &'a mut PathData, tension: f64) -> CardinalClosed<'a> {
        CardinalClosed {
            ctx: Ctx {
                path,
                reflect: false,
            },
            k: (1.0 - tension) / 6.0,
            x0: f64::NAN,
            y0: f64::NAN,
            x1: f64::NAN,
            y1: f64::NAN,
            x2: f64::NAN,
            y2: f64::NAN,
            x3: f64::NAN,
            y3: f64::NAN,
            x4: f64::NAN,
            y4: f64::NAN,
            x5: f64::NAN,
            y5: f64::NAN,
            point: 0,
        }
    }

    fn point_inner(&mut self, x: f64, y: f64) {
        match self.point {
            0 => {
                self.point = 1;
                self.x3 = x;
                self.y3 = y;
            }
            1 => {
                self.point = 2;
                self.x4 = x;
                self.y4 = y;
                self.ctx.move_to(x, y);
            }
            2 => {
                self.point = 3;
                self.x5 = x;
                self.y5 = y;
            }
            _ => cardinal_point(
                &mut self.ctx,
                self.k,
                self.x0,
                self.y0,
                self.x1,
                self.y1,
                self.x2,
                self.y2,
                x,
                y,
            ),
        }
        self.x0 = self.x1;
        self.x1 = self.x2;
        self.x2 = x;
        self.y0 = self.y1;
        self.y1 = self.y2;
        self.y2 = y;
    }
}

impl Curve for CardinalClosed<'_> {
    fn line_start(&mut self) {
        for v in [
            &mut self.x0,
            &mut self.x1,
            &mut self.x2,
            &mut self.x3,
            &mut self.x4,
            &mut self.x5,
            &mut self.y0,
            &mut self.y1,
            &mut self.y2,
            &mut self.y3,
            &mut self.y4,
            &mut self.y5,
        ] {
            *v = f64::NAN;
        }
        self.point = 0;
    }

    fn point(&mut self, x: f64, y: f64) {
        self.point_inner(x, y);
    }

    fn line_end(&mut self) {
        match self.point {
            1 => {
                self.ctx.move_to(self.x3, self.y3);
                self.ctx.close_path();
            }
            2 => {
                self.ctx.line_to(self.x3, self.y3);
                self.ctx.close_path();
            }
            3 => {
                let (x3, y3, x4, y4, x5, y5) =
                    (self.x3, self.y3, self.x4, self.y4, self.x5, self.y5);
                self.point_inner(x3, y3);
                self.point_inner(x4, y4);
                self.point_inner(x5, y5);
            }
            _ => {}
        }
    }
}

pub fn cardinal_closed(points: &[Point], tension: f64) -> PathData {
    let mut path = PathData::new();
    drive(CardinalClosed::new(&mut path, tension), points);
    path
}

struct CardinalOpen<'a> {
    ctx: Ctx<'a>,
    k: f64,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    point: u8,
}

impl Curve for CardinalOpen<'_> {
    fn line_start(&mut self) {
        self.x0 = f64::NAN;
        self.x1 = f64::NAN;
        self.x2 = f64::NAN;
        self.y0 = f64::NAN;
        self.y1 = f64::NAN;
        self.y2 = f64::NAN;
        self.point = 0;
    }

    fn point(&mut self, x: f64, y: f64) {
        match self.point {
            0 => self.point = 1,
            1 => self.point = 2,
            2 => {
                self.point = 3;
                self.ctx.move_to(self.x2, self.y2);
            }
            3 => {
                self.point = 4;
                cardinal_point(
                    &mut self.ctx,
                    self.k,
                    self.x0,
                    self.y0,
                    self.x1,
                    self.y1,
                    self.x2,
                    self.y2,
                    x,
                    y,
                );
            }
            _ => cardinal_point(
                &mut self.ctx,
                self.k,
                self.x0,
                self.y0,
                self.x1,
                self.y1,
                self.x2,
                self.y2,
                x,
                y,
            ),
        }
        self.x0 = self.x1;
        self.x1 = self.x2;
        self.x2 = x;
        self.y0 = self.y1;
        self.y1 = self.y2;
        self.y2 = y;
    }

    fn line_end(&mut self) {
        if self.point == 3 {
            self.ctx.close_path();
        }
    }
}

pub fn cardinal_open(points: &[Point], tension: f64) -> PathData {
    let mut path = PathData::new();
    drive(
        CardinalOpen {
            ctx: Ctx {
                path: &mut path,
                reflect: false,
            },
            k: (1.0 - tension) / 6.0,
            x0: f64::NAN,
            y0: f64::NAN,
            x1: f64::NAN,
            y1: f64::NAN,
            x2: f64::NAN,
            y2: f64::NAN,
            point: 0,
        },
        points,
    );
    path
}

// ----------------------------------------------------------- catmull-rom

/// d3-shape's `epsilon` from `math.js`.
const EPSILON: f64 = 1e-12;

/// The `_l01_a`, `_l12_a`, ... state shared by the three Catmull-Rom
/// variants, with `point(that, x, y)` from `catmullRom.js`.
struct CatmullRomState {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    l01_a: f64,
    l12_a: f64,
    l23_a: f64,
    l01_2a: f64,
    l12_2a: f64,
    l23_2a: f64,
    alpha: f64,
}

impl CatmullRomState {
    fn new(alpha: f64) -> CatmullRomState {
        CatmullRomState {
            x0: f64::NAN,
            y0: f64::NAN,
            x1: f64::NAN,
            y1: f64::NAN,
            x2: f64::NAN,
            y2: f64::NAN,
            l01_a: 0.0,
            l12_a: 0.0,
            l23_a: 0.0,
            l01_2a: 0.0,
            l12_2a: 0.0,
            l23_2a: 0.0,
            alpha,
        }
    }

    fn reset(&mut self) {
        *self = CatmullRomState::new(self.alpha);
    }

    /// The length update at the top of `point` (runs when `_point` is set).
    fn measure(&mut self, x: f64, y: f64) {
        let x23 = self.x2 - x;
        let y23 = self.y2 - y;
        self.l23_2a = (x23 * x23 + y23 * y23).powf(self.alpha);
        self.l23_a = self.l23_2a.sqrt();
    }

    fn emit(&self, ctx: &mut Ctx<'_>, x: f64, y: f64) {
        let mut x1 = self.x1;
        let mut y1 = self.y1;
        let mut x2 = self.x2;
        let mut y2 = self.y2;

        if self.l01_a > EPSILON {
            let a = 2.0 * self.l01_2a + 3.0 * self.l01_a * self.l12_a + self.l12_2a;
            let n = 3.0 * self.l01_a * (self.l01_a + self.l12_a);
            x1 = (x1 * a - self.x0 * self.l12_2a + self.x2 * self.l01_2a) / n;
            y1 = (y1 * a - self.y0 * self.l12_2a + self.y2 * self.l01_2a) / n;
        }

        if self.l23_a > EPSILON {
            let b = 2.0 * self.l23_2a + 3.0 * self.l23_a * self.l12_a + self.l12_2a;
            let m = 3.0 * self.l23_a * (self.l23_a + self.l12_a);
            x2 = (x2 * b + self.x1 * self.l23_2a - x * self.l12_2a) / m;
            y2 = (y2 * b + self.y1 * self.l23_2a - y * self.l12_2a) / m;
        }

        ctx.bezier_curve_to(x1, y1, x2, y2, self.x2, self.y2);
    }

    /// The shift at the bottom of `point`.
    fn shift(&mut self, x: f64, y: f64) {
        self.l01_a = self.l12_a;
        self.l12_a = self.l23_a;
        self.l01_2a = self.l12_2a;
        self.l12_2a = self.l23_2a;
        self.x0 = self.x1;
        self.x1 = self.x2;
        self.x2 = x;
        self.y0 = self.y1;
        self.y1 = self.y2;
        self.y2 = y;
    }
}

struct CatmullRom<'a> {
    ctx: Ctx<'a>,
    s: CatmullRomState,
    point: u8,
}

impl CatmullRom<'_> {
    fn point_inner(&mut self, x: f64, y: f64) {
        if self.point != 0 {
            self.s.measure(x, y);
        }
        match self.point {
            0 => {
                self.point = 1;
                self.ctx.move_to(x, y);
            }
            1 => self.point = 2,
            2 => {
                self.point = 3;
                self.s.emit(&mut self.ctx, x, y);
            }
            _ => self.s.emit(&mut self.ctx, x, y),
        }
        self.s.shift(x, y);
    }
}

impl Curve for CatmullRom<'_> {
    fn line_start(&mut self) {
        self.s.reset();
        self.point = 0;
    }

    fn point(&mut self, x: f64, y: f64) {
        self.point_inner(x, y);
    }

    fn line_end(&mut self) {
        match self.point {
            2 => self.ctx.line_to(self.s.x2, self.s.y2),
            3 => {
                let (x2, y2) = (self.s.x2, self.s.y2);
                self.point_inner(x2, y2);
            }
            _ => {}
        }
        if self.point == 1 {
            self.ctx.close_path();
        }
    }
}

/// `curveCatmullRom.alpha(alpha)`. d3 substitutes `Cardinal` with tension 0
/// when `alpha` is 0.
pub fn catmull_rom(points: &[Point], alpha: f64) -> PathData {
    if alpha == 0.0 {
        return cardinal(points, 0.0);
    }
    let mut path = PathData::new();
    drive(
        CatmullRom {
            ctx: Ctx {
                path: &mut path,
                reflect: false,
            },
            s: CatmullRomState::new(alpha),
            point: 0,
        },
        points,
    );
    path
}

struct CatmullRomClosed<'a> {
    ctx: Ctx<'a>,
    s: CatmullRomState,
    x3: f64,
    y3: f64,
    x4: f64,
    y4: f64,
    x5: f64,
    y5: f64,
    point: u8,
}

impl CatmullRomClosed<'_> {
    fn point_inner(&mut self, x: f64, y: f64) {
        if self.point != 0 {
            self.s.measure(x, y);
        }
        match self.point {
            0 => {
                self.point = 1;
                self.x3 = x;
                self.y3 = y;
            }
            1 => {
                self.point = 2;
                self.x4 = x;
                self.y4 = y;
                self.ctx.move_to(x, y);
            }
            2 => {
                self.point = 3;
                self.x5 = x;
                self.y5 = y;
            }
            _ => self.s.emit(&mut self.ctx, x, y),
        }
        self.s.shift(x, y);
    }
}

impl Curve for CatmullRomClosed<'_> {
    fn line_start(&mut self) {
        self.s.reset();
        self.x3 = f64::NAN;
        self.y3 = f64::NAN;
        self.x4 = f64::NAN;
        self.y4 = f64::NAN;
        self.x5 = f64::NAN;
        self.y5 = f64::NAN;
        self.point = 0;
    }

    fn point(&mut self, x: f64, y: f64) {
        self.point_inner(x, y);
    }

    fn line_end(&mut self) {
        match self.point {
            1 => {
                self.ctx.move_to(self.x3, self.y3);
                self.ctx.close_path();
            }
            2 => {
                self.ctx.line_to(self.x3, self.y3);
                self.ctx.close_path();
            }
            3 => {
                let (x3, y3, x4, y4, x5, y5) =
                    (self.x3, self.y3, self.x4, self.y4, self.x5, self.y5);
                self.point_inner(x3, y3);
                self.point_inner(x4, y4);
                self.point_inner(x5, y5);
            }
            _ => {}
        }
    }
}

pub fn catmull_rom_closed(points: &[Point], alpha: f64) -> PathData {
    if alpha == 0.0 {
        return cardinal_closed(points, 0.0);
    }
    let mut path = PathData::new();
    drive(
        CatmullRomClosed {
            ctx: Ctx {
                path: &mut path,
                reflect: false,
            },
            s: CatmullRomState::new(alpha),
            x3: f64::NAN,
            y3: f64::NAN,
            x4: f64::NAN,
            y4: f64::NAN,
            x5: f64::NAN,
            y5: f64::NAN,
            point: 0,
        },
        points,
    );
    path
}

struct CatmullRomOpen<'a> {
    ctx: Ctx<'a>,
    s: CatmullRomState,
    point: u8,
}

impl Curve for CatmullRomOpen<'_> {
    fn line_start(&mut self) {
        self.s.reset();
        self.point = 0;
    }

    fn point(&mut self, x: f64, y: f64) {
        if self.point != 0 {
            self.s.measure(x, y);
        }
        match self.point {
            0 => self.point = 1,
            1 => self.point = 2,
            2 => {
                self.point = 3;
                self.ctx.move_to(self.s.x2, self.s.y2);
            }
            3 => {
                self.point = 4;
                self.s.emit(&mut self.ctx, x, y);
            }
            _ => self.s.emit(&mut self.ctx, x, y),
        }
        self.s.shift(x, y);
    }

    fn line_end(&mut self) {
        if self.point == 3 {
            self.ctx.close_path();
        }
    }
}

pub fn catmull_rom_open(points: &[Point], alpha: f64) -> PathData {
    if alpha == 0.0 {
        return cardinal_open(points, 0.0);
    }
    let mut path = PathData::new();
    drive(
        CatmullRomOpen {
            ctx: Ctx {
                path: &mut path,
                reflect: false,
            },
            s: CatmullRomState::new(alpha),
            point: 0,
        },
        points,
    );
    path
}

// -------------------------------------------------------------- monotone

/// JS `Math.sign`-like helper from `monotone.js`: `x < 0 ? -1 : 1`.
fn sign(x: f64) -> f64 {
    if x < 0.0 { -1.0 } else { 1.0 }
}

/// `Math.min` propagates NaN; `f64::min` drops it.
fn js_min(values: &[f64]) -> f64 {
    let mut m = f64::INFINITY;
    for &v in values {
        if v.is_nan() {
            return f64::NAN;
        }
        if v < m {
            m = v;
        }
    }
    m
}

struct MonotoneX<'a> {
    ctx: Ctx<'a>,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    t0: f64,
    point: u8,
}

impl MonotoneX<'_> {
    /// `slope3`: Steffen's monotone slope with the JS `||` quirks kept.
    fn slope3(&self, x2: f64, y2: f64) -> f64 {
        let h0 = self.x1 - self.x0;
        let h1 = x2 - self.x1;
        // `(h0 || h1 < 0 && -0)`: a zero `h0` becomes `-0` when `h1 < 0`,
        // else `false` (coerced to `+0`). Only the sign of the division
        // result depends on it.
        let d0 = if h0 != 0.0 {
            h0
        } else if h1 < 0.0 {
            -0.0
        } else {
            0.0
        };
        let d1 = if h1 != 0.0 {
            h1
        } else if h0 < 0.0 {
            -0.0
        } else {
            0.0
        };
        let s0 = (self.y1 - self.y0) / d0;
        let s1 = (y2 - self.y1) / d1;
        let p = (s0 * h1 + s1 * h0) / (h0 + h1);
        let r = (sign(s0) + sign(s1)) * js_min(&[s0.abs(), s1.abs(), 0.5 * p.abs()]);
        // `|| 0`: NaN and -0 become 0.
        if r.is_nan() || r == 0.0 { 0.0 } else { r }
    }

    fn slope2(&self, t: f64) -> f64 {
        let h = self.x1 - self.x0;
        if h != 0.0 {
            (3.0 * (self.y1 - self.y0) / h - t) / 2.0
        } else {
            t
        }
    }

    fn emit(&mut self, t0: f64, t1: f64) {
        let dx = (self.x1 - self.x0) / 3.0;
        self.ctx.bezier_curve_to(
            self.x0 + dx,
            self.y0 + dx * t0,
            self.x1 - dx,
            self.y1 - dx * t1,
            self.x1,
            self.y1,
        );
    }
}

impl Curve for MonotoneX<'_> {
    fn line_start(&mut self) {
        self.x0 = f64::NAN;
        self.x1 = f64::NAN;
        self.y0 = f64::NAN;
        self.y1 = f64::NAN;
        self.t0 = f64::NAN;
        self.point = 0;
    }

    fn point(&mut self, x: f64, y: f64) {
        let mut t1 = f64::NAN;
        if x == self.x1 && y == self.y1 {
            // d3 ignores coincident points.
            return;
        }
        match self.point {
            0 => {
                self.point = 1;
                self.ctx.move_to(x, y);
            }
            1 => self.point = 2,
            2 => {
                self.point = 3;
                t1 = self.slope3(x, y);
                let t0 = self.slope2(t1);
                self.emit(t0, t1);
            }
            _ => {
                t1 = self.slope3(x, y);
                let t0 = self.t0;
                self.emit(t0, t1);
            }
        }
        self.x0 = self.x1;
        self.x1 = x;
        self.y0 = self.y1;
        self.y1 = y;
        self.t0 = t1;
    }

    fn line_end(&mut self) {
        match self.point {
            2 => self.ctx.line_to(self.x1, self.y1),
            3 => {
                let t0 = self.t0;
                let t1 = self.slope2(t0);
                self.emit(t0, t1);
            }
            _ => {}
        }
        if self.point == 1 {
            self.ctx.close_path();
        }
    }
}

fn monotone(points: &[Point], reflect: bool) -> PathData {
    let mut path = PathData::new();
    let mut curve = MonotoneX {
        ctx: Ctx {
            path: &mut path,
            reflect,
        },
        x0: f64::NAN,
        y0: f64::NAN,
        x1: f64::NAN,
        y1: f64::NAN,
        t0: f64::NAN,
        point: 0,
    };
    curve.line_start();
    for p in points {
        if reflect {
            curve.point(p.y, p.x);
        } else {
            curve.point(p.x, p.y);
        }
    }
    curve.line_end();
    path
}

pub fn monotone_x(points: &[Point]) -> PathData {
    monotone(points, false)
}

/// `monotoneY` is `monotoneX` with x and y swapped on the way in and out.
pub fn monotone_y(points: &[Point]) -> PathData {
    monotone(points, true)
}

// --------------------------------------------------------------- natural

/// `controlPoints(x)` from `natural.js`: the tridiagonal solve for a natural
/// cubic spline through `x`.
fn control_points(x: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let n = x.len() - 1;
    let mut a = vec![0.0; n];
    let mut b = vec![0.0; n];
    let mut r = vec![0.0; n];
    a[0] = 0.0;
    b[0] = 2.0;
    r[0] = x[0] + 2.0 * x[1];
    for i in 1..n.saturating_sub(1) {
        a[i] = 1.0;
        b[i] = 4.0;
        r[i] = 4.0 * x[i] + 2.0 * x[i + 1];
    }
    a[n - 1] = 2.0;
    b[n - 1] = 7.0;
    r[n - 1] = 8.0 * x[n - 1] + x[n];
    for i in 1..n {
        let m = a[i] / b[i - 1];
        b[i] -= m;
        r[i] -= m * r[i - 1];
    }
    a[n - 1] = r[n - 1] / b[n - 1];
    for i in (0..n.saturating_sub(1)).rev() {
        a[i] = (r[i] - a[i + 1]) / b[i];
    }
    b[n - 1] = (x[n] + a[n - 1]) / 2.0;
    for i in 0..n.saturating_sub(1) {
        b[i] = 2.0 * x[i + 1] - a[i + 1];
    }
    (a, b)
}

pub fn natural(points: &[Point]) -> PathData {
    let mut path = PathData::new();
    let n = points.len();
    if n > 0 {
        path.move_to(points[0].x, points[0].y);
        if n == 2 {
            path.line_to(points[1].x, points[1].y);
        } else if n > 2 {
            let xs: Vec<f64> = points.iter().map(|p| p.x).collect();
            let ys: Vec<f64> = points.iter().map(|p| p.y).collect();
            let (px0, px1) = control_points(&xs);
            let (py0, py1) = control_points(&ys);
            for i1 in 1..n {
                let i0 = i1 - 1;
                path.cubic_to(px0[i0], py0[i0], px1[i0], py1[i0], xs[i1], ys[i1]);
            }
        }
    }
    if n == 1 {
        path.close();
    }
    path
}

// ------------------------------------------------------------------ step

/// `curveStep` (`t = 0.5`), `curveStepBefore` (`t = 0`), `curveStepAfter`
/// (`t = 1`). In line mode `lineEnd` never flips `t`.
pub fn step(points: &[Point], t: f64) -> PathData {
    let mut path = PathData::new();
    let mut x = f64::NAN;
    let mut y = f64::NAN;
    let mut point = 0u8;
    for p in points {
        match point {
            0 => {
                point = 1;
                path.move_to(p.x, p.y);
            }
            _ => {
                point = 2;
                if t <= 0.0 {
                    path.line_to(x, p.y);
                    path.line_to(p.x, p.y);
                } else {
                    let x1 = x * (1.0 - t) + p.x * t;
                    path.line_to(x1, y);
                    path.line_to(x1, p.y);
                }
            }
        }
        x = p.x;
        y = p.y;
    }
    if 0.0 < t && t < 1.0 && point == 2 {
        path.line_to(x, y);
    }
    if point == 1 {
        path.close();
    }
    path
}

// ------------------------------------------------------------------ bump

fn bump(points: &[Point], horizontal: bool) -> PathData {
    let mut path = PathData::new();
    let mut x0 = f64::NAN;
    let mut y0 = f64::NAN;
    let mut point = 0u8;
    for p in points {
        match point {
            0 => {
                point = 1;
                path.move_to(p.x, p.y);
            }
            _ => {
                point = 2;
                if horizontal {
                    // `bezierCurveTo(this._x0 = (this._x0 + x) / 2, this._y0,
                    // this._x0, y, x, y)`: the first argument reassigns `_x0`
                    // before the third reads it.
                    let mid = (x0 + p.x) / 2.0;
                    path.cubic_to(mid, y0, mid, p.y, p.x, p.y);
                } else {
                    let mid = (y0 + p.y) / 2.0;
                    path.cubic_to(x0, mid, p.x, mid, p.x, p.y);
                }
            }
        }
        x0 = p.x;
        y0 = p.y;
    }
    if point == 1 {
        path.close();
    }
    path
}

pub fn bump_x(points: &[Point]) -> PathData {
    bump(points, true)
}

pub fn bump_y(points: &[Point]) -> PathData {
    bump(points, false)
}

// --------------------------------------------------------------- rounded

/// `generateRoundedPath(points, radius)` from `edges.js`: straight segments
/// with each interior corner cut by a quadratic curve. The caller applies
/// `applyMarkerOffsetsToPoints` first, as `insertEdge` does.
pub fn rounded(points: &[Point], radius: f64) -> PathData {
    let mut path = PathData::new();
    let size = points.len();
    if size < 2 {
        return path;
    }
    let epsilon = 1e-5;
    for i in 0..size {
        let curr = points[i];
        if i == 0 {
            path.move_to(curr.x, curr.y);
            continue;
        }
        if i == size - 1 {
            path.line_to(curr.x, curr.y);
            continue;
        }
        let prev = points[i - 1];
        let next = points[i + 1];
        let dx1 = curr.x - prev.x;
        let dy1 = curr.y - prev.y;
        let dx2 = next.x - curr.x;
        let dy2 = next.y - curr.y;
        let len1 = dx1.hypot(dy1);
        let len2 = dx2.hypot(dy2);
        if len1 < epsilon || len2 < epsilon {
            path.line_to(curr.x, curr.y);
            continue;
        }
        let nx1 = dx1 / len1;
        let ny1 = dy1 / len1;
        let nx2 = dx2 / len2;
        let ny2 = dy2 / len2;
        let dot = (nx1 * nx2 + ny1 * ny2).clamp(-1.0, 1.0);
        let angle = dot.acos();
        if angle < epsilon || (std::f64::consts::PI - angle).abs() < epsilon {
            path.line_to(curr.x, curr.y);
            continue;
        }
        let cut_len = js_min(&[radius / (angle / 2.0).sin(), len1 / 2.0, len2 / 2.0]);
        let start_x = curr.x - nx1 * cut_len;
        let start_y = curr.y - ny1 * cut_len;
        let end_x = curr.x + nx2 * cut_len;
        let end_y = curr.y + ny2 * cut_len;
        path.line_to(start_x, start_y);
        path.quad_to(curr.x, curr.y, end_x, end_y);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_resolve_like_interpolate_to_curve() {
        assert_eq!(CurveKind::from_mermaid("basis"), Some(CurveKind::Basis));
        assert_eq!(
            CurveKind::from_mermaid("curveBasis"),
            Some(CurveKind::Basis)
        );
        assert_eq!(CurveKind::from_mermaid("Basis"), Some(CurveKind::Basis));
        assert_eq!(
            CurveKind::from_mermaid("stepBefore"),
            Some(CurveKind::StepBefore)
        );
        assert_eq!(
            CurveKind::from_mermaid("catmullRomClosed"),
            Some(CurveKind::CatmullRomClosed)
        );
        assert_eq!(CurveKind::from_mermaid("rounded"), Some(CurveKind::Rounded));
        assert_eq!(CurveKind::from_mermaid("wavy"), None);
        assert_eq!(CurveKind::from_mermaid(""), None);
    }

    #[test]
    fn single_point_closes_like_d3() {
        let p = [Point::new(1.0, 2.0)];
        assert_eq!(linear(&p).to_svg(), "M1,2Z");
        assert_eq!(basis(&p).to_svg(), "M1,2Z");
        assert_eq!(natural(&p).to_svg(), "M1,2Z");
        assert_eq!(step(&p, 0.5).to_svg(), "M1,2Z");
        assert_eq!(bump_x(&p).to_svg(), "M1,2Z");
        assert_eq!(basis_open(&p).to_svg(), "");
        assert_eq!(rounded(&p, 5.0).to_svg(), "");
    }
}
