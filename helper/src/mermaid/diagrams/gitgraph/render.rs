//! Port of `gitGraphRenderer.ts` (mermaid 12.2.0) for the classic look:
//! `draw` with `setBranchPosition`, `drawCommits` (both passes),
//! `drawBranches`, `drawArrows`, `utils.insertTitle`, and
//! `setupGraphViewbox`.
//!
//! The browser gives mermaid two measurements the helper has to compute:
//! `getBBox()` on a label (`ctx.metrics`), and `getBBox()` on the whole SVG
//! for the viewBox. Blink computes a container's box as the union of each
//! child's own box mapped through the child's transform, so a rotated
//! label contributes the axis-aligned box of its rotated corners, and a
//! group maps its already-unioned box once. [`Node`] carries that box up
//! the tree while the elements are built. Strokes do not count; a line of
//! zero height still does.

use std::collections::HashMap;

use indexmap::IndexMap;

use super::db::{Commit, CommitType, GitGraphDb};
use super::parser::Direction;
use super::{GitGraphConfig, TYPE};
use crate::error::HelperError;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::scene::{
    Circle, Color, Common, Element, FontSpec, Group, Length, Line, LineCap, Paint, Path, PathData,
    Point, Polygon, Rect, Scene, Style, Text, TextAnchor, TextRun, TransformOp, ViewBox,
};
use crate::mermaid::styles::gitgraph::{GitGraphStyles, TITLE_FONT_SIZE};
use crate::text::FontSpec as MeasureFont;
use crate::text::measure::Shaped;

const LAYOUT_OFFSET: f64 = 10.0;
const COMMIT_STEP: f64 = 40.0;
const PX: f64 = 4.0;
const PY: f64 = 2.0;
/// `defaultPos`: where the first TB or BT commit sits.
const DEFAULT_POS: f64 = 30.0;
/// The root `font-size`, which the branch label text inherits.
const BRANCH_LABEL_FONT_SIZE: f64 = 16.0;
/// `.arrow { stroke-width: 8 }`.
const ARROW_STROKE_WIDTH: f64 = 8.0;
/// `.commit-reverse { stroke-width: 3 }`.
const REVERSE_STROKE_WIDTH: f64 = 3.0;
/// `.commit-label-bkg { opacity: 0.5 }`.
const LABEL_BKG_OPACITY: f64 = 0.5;

/// An axis-aligned box in some coordinate space.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BBox {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl BBox {
    fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> BBox {
        BBox {
            x0: x0.min(x1),
            y0: y0.min(y1),
            x1: x0.max(x1),
            y1: y0.max(y1),
        }
    }

    fn of_points(points: &[(f64, f64)]) -> Option<BBox> {
        points.iter().fold(None, |acc, &(x, y)| {
            BBox::union(acc, Some(BBox::new(x, y, x, y)))
        })
    }

    fn union(a: Option<BBox>, b: Option<BBox>) -> Option<BBox> {
        match (a, b) {
            (Some(a), Some(b)) => Some(BBox {
                x0: a.x0.min(b.x0),
                y0: a.y0.min(b.y0),
                x1: a.x1.max(b.x1),
                y1: a.y1.max(b.y1),
            }),
            (a, None) => a,
            (None, b) => b,
        }
    }

    /// `MapRect`: the box of the four mapped corners.
    fn map(self, ops: &[TransformOp]) -> BBox {
        let m = Matrix::of(ops);
        let corners = [
            m.apply(self.x0, self.y0),
            m.apply(self.x1, self.y0),
            m.apply(self.x0, self.y1),
            m.apply(self.x1, self.y1),
        ];
        BBox::of_points(&corners).unwrap_or(self)
    }

    fn width(self) -> f64 {
        self.x1 - self.x0
    }

    fn height(self) -> f64 {
        self.y1 - self.y0
    }
}

/// An SVG transform matrix `[a b c d e f]`.
#[derive(Debug, Clone, Copy)]
struct Matrix {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Matrix {
    const IDENTITY: Matrix = Matrix {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    fn mul(self, o: Matrix) -> Matrix {
        Matrix {
            a: self.a * o.a + self.c * o.b,
            b: self.b * o.a + self.d * o.b,
            c: self.a * o.c + self.c * o.d,
            d: self.b * o.c + self.d * o.d,
            e: self.a * o.e + self.c * o.f + self.e,
            f: self.b * o.e + self.d * o.f + self.f,
        }
    }

    fn translate(x: f64, y: f64) -> Matrix {
        Matrix {
            e: x,
            f: y,
            ..Matrix::IDENTITY
        }
    }

    /// `rotate(angle, cx, cy)` in degrees.
    fn rotate(angle: f64, cx: f64, cy: f64) -> Matrix {
        let (s, c) = angle.to_radians().sin_cos();
        let r = Matrix {
            a: c,
            b: s,
            c: -s,
            d: c,
            e: 0.0,
            f: 0.0,
        };
        Matrix::translate(cx, cy)
            .mul(r)
            .mul(Matrix::translate(-cx, -cy))
    }

    /// The list applies left to right as SVG does.
    fn of(ops: &[TransformOp]) -> Matrix {
        ops.iter().fold(Matrix::IDENTITY, |m, op| {
            m.mul(match *op {
                TransformOp::Translate(x, y) => Matrix::translate(x, y),
                TransformOp::Scale(x, y) => Matrix {
                    a: x,
                    d: y,
                    ..Matrix::IDENTITY
                },
                TransformOp::Rotate(a, cx, cy) => Matrix::rotate(a, cx, cy),
                TransformOp::Matrix([a, b, c, d, e, f]) => Matrix { a, b, c, d, e, f },
            })
        })
    }

    fn apply(self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }
}

/// An element with its `getBBox()` in the parent's coordinates.
struct Node {
    el: Element,
    bbox: Option<BBox>,
}

fn group(class: &str, transform: Vec<TransformOp>, children: Vec<Node>) -> Node {
    let bbox = children
        .iter()
        .fold(None, |acc, n| BBox::union(acc, n.bbox))
        .map(|b| b.map(&transform));
    Node {
        el: Element::Group(Group {
            common: Common {
                class: (!class.is_empty()).then(|| class.to_string()),
                transform,
                ..Common::default()
            },
            children: children.into_iter().map(|n| n.el).collect(),
        }),
        bbox,
    }
}

fn common(class: &str, style: Style, transform: Vec<TransformOp>) -> Common {
    Common {
        id: None,
        class: Some(class.to_string()),
        transform,
        style,
    }
}

fn paint(color: Option<Color>) -> Option<Paint> {
    color.map(Paint::Color)
}

#[derive(Debug, Clone, Copy)]
struct BranchPosition {
    pos: f64,
    index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Pos {
    x: f64,
    y: f64,
}

#[derive(Debug, Clone, Copy)]
struct CommitPositionOffset {
    x: f64,
    y: f64,
    pos_with_offset: f64,
}

struct Renderer<'a, 'f> {
    db: &'a GitGraphDb,
    cfg: &'a GitGraphConfig,
    styles: &'a GitGraphStyles,
    ctx: &'a RenderCtx<'f>,
    dir: Direction,
    /// `DARK_THEMES.has(theme)`: the cherry-pick marks paint black.
    is_dark: bool,
    branch_pos: IndexMap<String, BranchPosition>,
    commit_pos: HashMap<String, Pos>,
    lanes: Vec<f64>,
    max_pos: f64,
}

fn internal(message: String) -> HelperError {
    HelperError::internal(format!("{TYPE}: {message}"))
}

impl Renderer<'_, '_> {
    fn vertical(&self) -> bool {
        matches!(self.dir, Direction::TB | Direction::BT)
    }

    fn font(&self, size: f64) -> MeasureFont {
        MeasureFont::new(self.styles.font_family.clone(), size as f32)
    }

    /// `getBBox()` of a `<text>` with this content and font size.
    fn measure(&self, text: &str, size: f64) -> Shaped {
        self.ctx.metrics.shape(text, &self.font(size))
    }

    /// The box of a text whose baseline starts at `(x, y)`.
    fn text_box(&self, shaped: &Shaped, x: f64, y: f64, anchor: Option<TextAnchor>) -> BBox {
        let width = f64::from(shaped.width);
        let x0 = match anchor {
            Some(TextAnchor::Middle) => x - width / 2.0,
            Some(TextAnchor::End) => x - width,
            _ => x,
        };
        let ascent = f64::from(shaped.ascent);
        let descent = f64::from(shaped.descent);
        let (min_x, max_x, top, bottom) = match shaped.ink {
            Some(ink) => (
                f64::from(ink.min_x).min(0.0),
                f64::from(ink.max_x).max(width),
                f64::from(ink.top).max(ascent),
                f64::from(ink.bottom).max(descent),
            ),
            None => (0.0, width, ascent, descent),
        };
        BBox::new(x0 + min_x, y - top, x0 + max_x, y + bottom)
    }

    /// A `<text>` with one run, measured for its box.
    #[allow(clippy::too_many_arguments)]
    fn text(
        &self,
        class: &str,
        x: f64,
        y: f64,
        anchor: Option<TextAnchor>,
        size: f64,
        fill: Option<Color>,
        content: &str,
        transform: Vec<TransformOp>,
    ) -> Node {
        let shaped = self.measure(content, size);
        let bbox = self.text_box(&shaped, x, y, anchor).map(&transform);
        Node {
            el: Element::Text(Text {
                common: common(
                    class,
                    Style {
                        fill: paint(fill),
                        ..Style::default()
                    },
                    transform,
                ),
                x,
                y,
                anchor,
                baseline: None,
                font: FontSpec {
                    size: Some(size),
                    ..FontSpec::default()
                },
                runs: vec![TextRun::new(content)],
            }),
            bbox: Some(bbox),
        }
    }

    /// `drawText(name)`: a `<text>` holding one `<tspan dy="1em" x="0">`
    /// per row; branch names have one row.
    fn draw_text(&self, content: &str, fill: Option<Color>) -> Node {
        let shaped = self.measure(content, BRANCH_LABEL_FONT_SIZE);
        let bbox = self.text_box(&shaped, 0.0, BRANCH_LABEL_FONT_SIZE, None);
        Node {
            el: Element::Text(Text {
                common: Common::with_style(Style {
                    fill: paint(fill),
                    ..Style::default()
                }),
                x: 0.0,
                y: 0.0,
                anchor: None,
                baseline: None,
                // No rule sets a size, so the text inherits the root
                // `font-size: 16px` of the mermaid SVG; resvg's default is
                // 12 px, so the size is written out.
                font: FontSpec {
                    size: Some(BRANCH_LABEL_FONT_SIZE),
                    ..FontSpec::default()
                },
                runs: vec![TextRun::line(content.trim(), 0.0, Length::Em(1.0))],
            }),
            bbox: Some(bbox),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn rect(
        &self,
        class: &str,
        style: Style,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        r: Option<f64>,
    ) -> Node {
        self.rect_t(class, style, x, y, w, h, r, Vec::new())
    }

    #[allow(clippy::too_many_arguments)]
    fn rect_t(
        &self,
        class: &str,
        style: Style,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        r: Option<f64>,
        transform: Vec<TransformOp>,
    ) -> Node {
        Node {
            bbox: Some(BBox::new(x, y, x + w, y + h).map(&transform)),
            el: Element::Rect(Rect {
                common: common(class, style, transform),
                x,
                y,
                width: w,
                height: h,
                rx: r,
                ry: r,
            }),
        }
    }

    fn circle(&self, class: &str, style: Style, cx: f64, cy: f64, r: f64) -> Node {
        self.circle_t(class, style, cx, cy, r, Vec::new())
    }

    fn circle_t(
        &self,
        class: &str,
        style: Style,
        cx: f64,
        cy: f64,
        r: f64,
        transform: Vec<TransformOp>,
    ) -> Node {
        Node {
            bbox: Some(BBox::new(cx - r, cy - r, cx + r, cy + r).map(&transform)),
            el: Element::Circle(Circle {
                common: common(class, style, transform),
                cx,
                cy,
                r,
            }),
        }
    }

    fn line(&self, class: &str, style: Style, x1: f64, y1: f64, x2: f64, y2: f64) -> Node {
        Node {
            bbox: Some(BBox::new(x1, y1, x2, y2)),
            el: Element::Line(Line {
                common: common(class, style, Vec::new()),
                x1,
                y1,
                x2,
                y2,
            }),
        }
    }

    fn polygon(
        &self,
        class: &str,
        style: Style,
        points: &[(f64, f64)],
        transform: Vec<TransformOp>,
    ) -> Node {
        Node {
            bbox: BBox::of_points(points).map(|b| b.map(&transform)),
            el: Element::Polygon(Polygon {
                common: common(class, style, transform),
                points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            }),
        }
    }

    /// A path whose arcs are quarter circles between axis-aligned ends, so
    /// the box of its points is its box.
    fn path(&self, class: &str, style: Style, d: PathData, points: &[(f64, f64)]) -> Node {
        Node {
            bbox: BBox::of_points(points),
            el: Element::Path(Path {
                common: common(class, style, Vec::new()),
                d,
            }),
        }
    }

    fn git_color(&self, branch_index: usize) -> Option<Color> {
        self.styles.git[GitGraphStyles::index(branch_index)]
    }

    fn branch_index(&self, branch: &str) -> usize {
        self.branch_pos.get(branch).map_or(0, |b| b.index)
    }

    fn branch_pos_of(&self, commit: &Commit) -> Result<BranchPosition, HelperError> {
        self.branch_pos
            .get(&commit.branch)
            .copied()
            .ok_or_else(|| internal(format!("Branch not found for commit {}", commit.id)))
    }

    /// `findClosestParent`.
    fn find_closest_parent<'c>(&self, parents: &'c [String]) -> Option<&'c str> {
        let bt = self.dir == Direction::BT;
        let mut target = if bt { f64::INFINITY } else { 0.0 };
        let mut closest = None;
        for parent in parents {
            let Some(pos) = self.commit_pos.get(parent) else {
                continue;
            };
            let value = if self.vertical() { pos.y } else { pos.x };
            if (bt && value <= target) || (!bt && value >= target) {
                closest = Some(parent.as_str());
                target = value;
            }
        }
        closest
    }

    /// `findClosestParentBT`.
    fn find_closest_parent_bt<'c>(&self, parents: &'c [String]) -> Option<&'c str> {
        let mut max_position = f64::INFINITY;
        let mut closest = None;
        for parent in parents {
            let y = self.commit_pos.get(parent).map_or(0.0, |p| p.y);
            if y <= max_position {
                closest = Some(parent.as_str());
                max_position = y;
            }
        }
        closest
    }

    /// `calculateCommitPosition`: the closest parent's `y` plus a step.
    fn calculate_commit_position(&self, commit: &Commit) -> Result<f64, HelperError> {
        let closest = self.find_closest_parent(&commit.parents).ok_or_else(|| {
            internal(format!("Closest parent not found for commit {}", commit.id))
        })?;
        let y = self.commit_pos.get(closest).map(|p| p.y).ok_or_else(|| {
            internal(format!(
                "Closest parent position not found for commit {}",
                commit.id
            ))
        })?;
        Ok(y + COMMIT_STEP)
    }

    /// `setParallelBTPos`.
    fn set_parallel_bt_pos(
        &mut self,
        sorted: &[&Commit],
        default_pos: f64,
    ) -> Result<(), HelperError> {
        let mut cur_pos = default_pos;
        let mut max_position = default_pos;
        let mut roots: Vec<&Commit> = Vec::new();
        for commit in sorted {
            if !commit.parents.is_empty() {
                cur_pos = self.calculate_commit_position(commit)?;
                max_position = cur_pos.max(max_position);
            } else {
                roots.push(commit);
            }
            let branch = self.branch_pos_of(commit)?;
            self.commit_pos.insert(
                commit.id.clone(),
                Pos {
                    x: branch.pos,
                    y: cur_pos + LAYOUT_OFFSET,
                },
            );
        }
        cur_pos = max_position;
        for commit in roots {
            let branch = self.branch_pos_of(commit)?;
            self.commit_pos.insert(
                commit.id.clone(),
                Pos {
                    x: branch.pos,
                    y: cur_pos + default_pos,
                },
            );
        }
        for commit in sorted {
            if commit.parents.is_empty() {
                continue;
            }
            let closest = self
                .find_closest_parent_bt(&commit.parents)
                .ok_or_else(|| {
                    internal(format!("Closest parent not found for commit {}", commit.id))
                })?;
            cur_pos = self.commit_pos.get(closest).map_or(0.0, |p| p.y) - COMMIT_STEP;
            if cur_pos <= max_position {
                max_position = cur_pos;
            }
            let branch = self.branch_pos_of(commit)?;
            self.commit_pos.insert(
                commit.id.clone(),
                Pos {
                    x: branch.pos,
                    y: cur_pos - LAYOUT_OFFSET,
                },
            );
        }
        Ok(())
    }

    /// `calculatePosition` for `parallelCommits`.
    fn calculate_position(&self, commit: &Commit) -> f64 {
        let current = || {
            self.commit_pos
                .get(&commit.id)
                .copied()
                .unwrap_or(Pos { x: 0.0, y: 0.0 })
        };
        if !commit.parents.is_empty() {
            let Some(closest) = self.find_closest_parent(&commit.parents) else {
                return 0.0;
            };
            let parent = self
                .commit_pos
                .get(closest)
                .copied()
                .unwrap_or(Pos { x: 0.0, y: 0.0 });
            return match self.dir {
                Direction::TB => parent.y + COMMIT_STEP,
                Direction::BT => current().y - COMMIT_STEP,
                Direction::LR => parent.x + COMMIT_STEP,
            };
        }
        match self.dir {
            Direction::TB => DEFAULT_POS,
            Direction::BT => current().y - COMMIT_STEP,
            Direction::LR => 0.0,
        }
    }

    /// `getCommitPosition`.
    fn commit_position(
        &self,
        commit: &Commit,
        pos: f64,
        parallel: bool,
    ) -> Result<CommitPositionOffset, HelperError> {
        let pos_with_offset = if self.dir == Direction::BT && parallel {
            pos
        } else {
            pos + LAYOUT_OFFSET
        };
        let branch = self.branch_pos_of(commit)?;
        if self.vertical() {
            Ok(CommitPositionOffset {
                x: branch.pos,
                y: pos_with_offset,
                pos_with_offset,
            })
        } else {
            Ok(CommitPositionOffset {
                x: pos_with_offset,
                y: branch.pos - 2.0,
                pos_with_offset,
            })
        }
    }

    /// `drawCommitBullet` for the classic geometry.
    fn draw_commit_bullet(
        &self,
        bullets: &mut Vec<Node>,
        commit: &Commit,
        cp: CommitPositionOffset,
        type_class: &str,
        branch_index: usize,
    ) {
        let color = self.git_color(branch_index);
        let index = GitGraphStyles::index(branch_index);
        let id = &commit.id;
        let primary = self.styles.primary_color;
        match commit.symbol_type() {
            CommitType::Highlight => {
                let inv = self.styles.git_inv[index];
                bullets.push(self.rect(
                    &format!("commit {id} commit-highlight{index} {type_class}-outer"),
                    Style {
                        fill: paint(inv),
                        stroke: paint(inv),
                        ..Style::default()
                    },
                    cp.x - 10.0,
                    cp.y - 10.0,
                    20.0,
                    20.0,
                    None,
                ));
                // `.commit-highlight-inner` comes after `.commit<i>` in the
                // stylesheet, so `primaryColor` wins for the inner square.
                bullets.push(self.rect(
                    &format!("commit {id} commit{index} {type_class}-inner"),
                    Style {
                        fill: paint(primary),
                        stroke: paint(primary),
                        ..Style::default()
                    },
                    cp.x - 6.0,
                    cp.y - 6.0,
                    12.0,
                    12.0,
                    None,
                ));
            }
            CommitType::CherryPick => {
                // No `.commit-cherry-pick` rule exists for the classic
                // themes: the circle inherits the root fill and has no
                // stroke.
                let class = format!("commit {id} {type_class}");
                let mark = Some(if self.is_dark {
                    Color::BLACK
                } else {
                    Color::WHITE
                });
                bullets.push(self.circle(&class, Style::default(), cp.x, cp.y, 10.0));
                bullets.push(self.circle(
                    &class,
                    Style {
                        fill: paint(mark),
                        ..Style::default()
                    },
                    cp.x - 3.0,
                    cp.y + 2.0,
                    2.75,
                ));
                bullets.push(self.circle(
                    &class,
                    Style {
                        fill: paint(mark),
                        ..Style::default()
                    },
                    cp.x + 3.0,
                    cp.y + 2.0,
                    2.75,
                ));
                let stem = Style {
                    stroke: paint(mark),
                    ..Style::default()
                };
                bullets.push(self.line(
                    &class,
                    stem.clone(),
                    cp.x + 3.0,
                    cp.y + 1.0,
                    cp.x,
                    cp.y - 5.0,
                ));
                bullets.push(self.line(&class, stem, cp.x - 3.0, cp.y + 1.0, cp.x, cp.y - 5.0));
            }
            symbol => {
                bullets.push(self.circle(
                    &format!("commit {id} commit{index}"),
                    Style {
                        fill: paint(color),
                        stroke: paint(color),
                        ..Style::default()
                    },
                    cp.x,
                    cp.y,
                    10.0,
                ));
                // `.commit-merge` and `.commit-reverse` follow `.commit<i>`
                // in the stylesheet and win at equal specificity.
                if symbol == CommitType::Merge {
                    bullets.push(self.circle(
                        &format!("commit {type_class} {id} commit{index}"),
                        Style {
                            fill: paint(primary),
                            stroke: paint(primary),
                            ..Style::default()
                        },
                        cp.x,
                        cp.y,
                        6.0,
                    ));
                }
                if symbol == CommitType::Reverse {
                    let k = 5.0;
                    let mut d = PathData::new();
                    d.move_to(cp.x - k, cp.y - k)
                        .line_to(cp.x + k, cp.y + k)
                        .move_to(cp.x - k, cp.y + k)
                        .line_to(cp.x + k, cp.y - k);
                    bullets.push(self.path(
                        &format!("commit {type_class} {id} commit{index}"),
                        Style {
                            fill: paint(primary),
                            stroke: paint(primary),
                            stroke_width: Some(REVERSE_STROKE_WIDTH),
                            ..Style::default()
                        },
                        d,
                        &[(cp.x - k, cp.y - k), (cp.x + k, cp.y + k)],
                    ));
                }
            }
        }
    }

    /// `drawCommitLabel`.
    fn draw_commit_label(
        &self,
        labels: &mut Vec<Node>,
        commit: &Commit,
        cp: CommitPositionOffset,
        pos: f64,
    ) {
        let show = commit.kind != CommitType::CherryPick
            && (commit.kind != CommitType::Merge || commit.custom_id)
            && self.cfg.show_commit_label;
        if !show {
            return;
        }
        let size = self.styles.commit_label_font_size;
        let shaped = self.measure(&commit.id, size);
        let w = f64::from(shaped.bbox().width);
        let h = f64::from(shaped.bbox().height);
        let (mut bkg_x, mut bkg_y) = (cp.pos_with_offset - w / 2.0 - PY, cp.y + 13.5);
        // The text is first appended at `x = pos` to be measured, then
        // moved; only the final position matters here.
        let (text_x, text_y) = if self.vertical() {
            bkg_x = cp.x - (w + 4.0 * PX + 5.0);
            bkg_y = cp.y - 12.0;
            (cp.x - (w + 4.0 * PX), cp.y + h - 12.0)
        } else {
            (cp.pos_with_offset - w / 2.0, cp.y + 25.0)
        };
        let (mut wrapper_t, mut element_t) = (Vec::new(), Vec::new());
        if self.cfg.rotate_commit_label {
            if self.vertical() {
                element_t.push(TransformOp::Rotate(-45.0, cp.x, cp.y));
            } else {
                let r_x = -7.5 - ((w + 10.0) / 25.0) * 9.5;
                let r_y = 10.0 + (w / 25.0) * 8.5;
                wrapper_t.push(TransformOp::Translate(r_x, r_y));
                wrapper_t.push(TransformOp::Rotate(-45.0, pos, cp.y));
            }
        }
        let bkg = self.rect_t(
            "commit-label-bkg",
            Style {
                fill: paint(self.styles.commit_label_background),
                opacity: Some(LABEL_BKG_OPACITY),
                ..Style::default()
            },
            bkg_x,
            bkg_y,
            w + 2.0 * PY,
            h + 2.0 * PY,
            None,
            element_t.clone(),
        );
        let text = self.text(
            "commit-label",
            text_x,
            text_y,
            None,
            size,
            self.styles.commit_label_color,
            &commit.id,
            element_t,
        );
        labels.push(group("", wrapper_t, vec![bkg, text]));
    }

    /// `drawCommitTags`: the tags in reverse order, each 20 px further out.
    fn draw_commit_tags(
        &self,
        labels: &mut Vec<Node>,
        commit: &Commit,
        cp: CommitPositionOffset,
        pos: f64,
    ) {
        if commit.tags.is_empty() {
            return;
        }
        let size = self.styles.tag_label_font_size;
        let mut max_w = 0.0f64;
        let mut max_h = 0.0f64;
        let mut tags: Vec<(&str, f64, f64)> = Vec::new();
        let mut y_offset = 0.0;
        for tag in commit.tags.iter().rev() {
            let b = self.measure(tag, size).bbox();
            let (w, h) = (f64::from(b.width), f64::from(b.height));
            max_w = max_w.max(w);
            max_h = max_h.max(h);
            tags.push((tag, w, y_offset));
            y_offset += 20.0;
        }
        let bkg_style = Style {
            fill: paint(self.styles.tag_label_background),
            stroke: paint(self.styles.tag_label_border),
            ..Style::default()
        };
        let hole_style = Style {
            fill: paint(self.styles.text_color),
            ..Style::default()
        };
        for (tag, w, y_offset) in tags {
            let h2 = max_h / 2.0;
            let ly = cp.y - 19.2 - y_offset;
            if self.vertical() {
                let y_origin = pos + y_offset;
                let t = |d: f64| {
                    vec![
                        TransformOp::Translate(d, d),
                        TransformOp::Rotate(45.0, cp.x, pos),
                    ]
                };
                labels.push(self.polygon(
                    "tag-label-bkg",
                    bkg_style.clone(),
                    &[
                        (cp.x, y_origin + 2.0),
                        (cp.x, y_origin - 2.0),
                        (cp.x + LAYOUT_OFFSET, y_origin - h2 - 2.0),
                        (cp.x + LAYOUT_OFFSET + max_w + 4.0, y_origin - h2 - 2.0),
                        (cp.x + LAYOUT_OFFSET + max_w + 4.0, y_origin + h2 + 2.0),
                        (cp.x + LAYOUT_OFFSET, y_origin + h2 + 2.0),
                    ],
                    t(12.0),
                ));
                labels.push(self.circle_t(
                    "tag-hole",
                    hole_style.clone(),
                    cp.x + PX / 2.0,
                    y_origin,
                    1.5,
                    t(12.0),
                ));
                labels.push(self.text(
                    "tag-label",
                    cp.x + 5.0,
                    y_origin + 3.0,
                    None,
                    size,
                    self.styles.tag_label_color,
                    tag,
                    t(14.0),
                ));
            } else {
                labels.push(self.polygon(
                    "tag-label-bkg",
                    bkg_style.clone(),
                    &[
                        (pos - max_w / 2.0 - PX / 2.0, ly + PY),
                        (pos - max_w / 2.0 - PX / 2.0, ly - PY),
                        (cp.pos_with_offset - max_w / 2.0 - PX, ly - h2 - PY),
                        (cp.pos_with_offset + max_w / 2.0 + PX, ly - h2 - PY),
                        (cp.pos_with_offset + max_w / 2.0 + PX, ly + h2 + PY),
                        (cp.pos_with_offset - max_w / 2.0 - PX, ly + h2 + PY),
                    ],
                    Vec::new(),
                ));
                labels.push(self.circle(
                    "tag-hole",
                    hole_style.clone(),
                    pos - max_w / 2.0 + PX / 2.0,
                    ly,
                    1.5,
                ));
                labels.push(self.text(
                    "tag-label",
                    cp.pos_with_offset - w / 2.0,
                    cp.y - 16.0 - y_offset,
                    None,
                    size,
                    self.styles.tag_label_color,
                    tag,
                    Vec::new(),
                ));
            }
        }
    }

    /// `getCommitClassType`.
    fn commit_class_type(commit: &Commit) -> &'static str {
        match commit.symbol_type() {
            CommitType::Normal => "commit-normal",
            CommitType::Reverse => "commit-reverse",
            CommitType::Highlight => "commit-highlight",
            CommitType::Merge => "commit-merge",
            CommitType::CherryPick => "commit-cherry-pick",
        }
    }

    /// `drawCommits`: the first pass (`modifyGraph` false) only places the
    /// commits and finds `maxPos`; the second draws them.
    fn draw_commits(&mut self, modify: bool) -> Result<Vec<Node>, HelperError> {
        let mut bullets = Vec::new();
        let mut labels = Vec::new();
        let mut pos = if self.vertical() { DEFAULT_POS } else { 0.0 };
        let parallel = self.cfg.parallel_commits;
        let mut sorted: Vec<&Commit> = self.db.commits_array();
        if self.dir == Direction::BT {
            if parallel {
                self.set_parallel_bt_pos(&sorted, pos)?;
            }
            sorted.reverse();
        }
        for commit in sorted {
            if parallel {
                pos = self.calculate_position(commit);
            }
            let cp = self.commit_position(commit, pos, parallel)?;
            if modify {
                let type_class = Renderer::commit_class_type(commit);
                let branch_index = self.branch_index(&commit.branch);
                self.draw_commit_bullet(&mut bullets, commit, cp, type_class, branch_index);
                self.draw_commit_label(&mut labels, commit, cp, pos);
                self.draw_commit_tags(&mut labels, commit, cp, pos);
            }
            let stored = if self.vertical() {
                Pos {
                    x: cp.x,
                    y: cp.pos_with_offset,
                }
            } else {
                Pos {
                    x: cp.pos_with_offset,
                    y: cp.y,
                }
            };
            self.commit_pos.insert(commit.id.clone(), stored);
            pos = if self.dir == Direction::BT && parallel {
                pos + COMMIT_STEP
            } else {
                pos + COMMIT_STEP + LAYOUT_OFFSET
            };
            if pos > self.max_pos {
                self.max_pos = pos;
            }
        }
        Ok(vec![
            group("commit-bullets", Vec::new(), bullets),
            group("commit-labels", Vec::new(), labels),
        ])
    }

    /// `shouldRerouteArrow`: a commit on the curving branch sits between
    /// the two in sequence.
    fn should_reroute_arrow(&self, a: &Commit, b: &Commit, p1: Pos, p2: Pos) -> bool {
        let b_is_furthest = if self.vertical() {
            p1.x < p2.x
        } else {
            p1.y < p2.y
        };
        let branch = if b_is_furthest { &b.branch } else { &a.branch };
        self.db
            .commits()
            .values()
            .any(|x| x.seq > a.seq && x.seq < b.seq && x.branch == *branch)
    }

    /// `findLane`: a free coordinate between `y1` and `y2`, at least 10 px
    /// from every lane taken so far.
    fn find_lane(&mut self, y1: f64, y2: f64, depth: u32) -> f64 {
        let candidate = y1 + (y1 - y2).abs() / 2.0;
        if depth > 5 {
            return candidate;
        }
        if self
            .lanes
            .iter()
            .all(|lane| (lane - candidate).abs() >= 10.0)
        {
            self.lanes.push(candidate);
            return candidate;
        }
        let diff = (y1 - y2).abs();
        self.find_lane(y1, y2 - diff / 5.0, depth + 1)
    }

    /// `drawArrow`: the path from `a` to `b` with one or two rounded
    /// corners, colored by the branch the arrow belongs to.
    fn draw_arrow(&mut self, a: &Commit, b: &Commit) -> Result<Node, HelperError> {
        let (Some(p1), Some(p2)) = (
            self.commit_pos.get(&a.id).copied(),
            self.commit_pos.get(&b.id).copied(),
        ) else {
            return Err(internal(format!(
                "Commit positions not found for commits {} and {}",
                a.id, b.id
            )));
        };
        let reroute = self.should_reroute_arrow(a, b, p1, p2);
        let merge_from_side = b.kind == CommitType::Merge && b.parents.first() != Some(&a.id);
        let mut color_index = self.branch_index(&b.branch);
        if merge_from_side {
            color_index = self.branch_index(&a.branch);
        }
        // `arc` sweeps 0 and `arc2` sweeps 1; both are quarter circles, so
        // the path's box is the box of its points.
        let mut d = PathData::new();
        let mut pts: Vec<(f64, f64)> = vec![(p1.x, p1.y), (p2.x, p2.y)];
        d.move_to(p1.x, p1.y);
        fn line(d: &mut PathData, pts: &mut Vec<(f64, f64)>, x: f64, y: f64) {
            d.line_to(x, y);
            pts.push((x, y));
        }
        fn arc(d: &mut PathData, r: f64, sweep: bool, x: f64, y: f64) {
            d.arc_to(r, r, 0.0, false, sweep, x, y);
        }
        if reroute {
            let (r, o) = (10.0, 10.0);
            let line_y = if p1.y < p2.y {
                self.find_lane(p1.y, p2.y, 0)
            } else {
                self.find_lane(p2.y, p1.y, 0)
            };
            let line_x = if p1.x < p2.x {
                self.find_lane(p1.x, p2.x, 0)
            } else {
                self.find_lane(p2.x, p1.x, 0)
            };
            match self.dir {
                Direction::TB => {
                    if p1.x < p2.x {
                        line(&mut d, &mut pts, line_x - r, p1.y);
                        arc(&mut d, r, true, line_x, p1.y + o);
                        line(&mut d, &mut pts, line_x, p2.y - r);
                        arc(&mut d, r, false, line_x + o, p2.y);
                    } else {
                        color_index = self.branch_index(&a.branch);
                        line(&mut d, &mut pts, line_x + r, p1.y);
                        arc(&mut d, r, false, line_x, p1.y + o);
                        line(&mut d, &mut pts, line_x, p2.y - r);
                        arc(&mut d, r, true, line_x - o, p2.y);
                    }
                    pts.push((line_x, p1.y + o));
                    pts.push((line_x, p2.y));
                }
                Direction::BT => {
                    if p1.x < p2.x {
                        line(&mut d, &mut pts, line_x - r, p1.y);
                        arc(&mut d, r, false, line_x, p1.y - o);
                        line(&mut d, &mut pts, line_x, p2.y + r);
                        arc(&mut d, r, true, line_x + o, p2.y);
                    } else {
                        color_index = self.branch_index(&a.branch);
                        line(&mut d, &mut pts, line_x + r, p1.y);
                        arc(&mut d, r, true, line_x, p1.y - o);
                        line(&mut d, &mut pts, line_x, p2.y + r);
                        arc(&mut d, r, false, line_x - o, p2.y);
                    }
                    pts.push((line_x, p1.y - o));
                    pts.push((line_x, p2.y));
                }
                Direction::LR => {
                    if p1.y < p2.y {
                        line(&mut d, &mut pts, p1.x, line_y - r);
                        arc(&mut d, r, false, p1.x + o, line_y);
                        line(&mut d, &mut pts, p2.x - r, line_y);
                        arc(&mut d, r, true, p2.x, line_y + o);
                    } else {
                        color_index = self.branch_index(&a.branch);
                        line(&mut d, &mut pts, p1.x, line_y + r);
                        arc(&mut d, r, true, p1.x + o, line_y);
                        line(&mut d, &mut pts, p2.x - r, line_y);
                        arc(&mut d, r, false, p2.x, line_y - o);
                    }
                    pts.push((p1.x + o, line_y));
                    pts.push((p2.x, line_y));
                }
            }
            line(&mut d, &mut pts, p2.x, p2.y);
        } else {
            let (r, o) = (20.0, 20.0);
            match self.dir {
                Direction::TB => {
                    if p1.x < p2.x {
                        if merge_from_side {
                            line(&mut d, &mut pts, p1.x, p2.y - r);
                            arc(&mut d, r, false, p1.x + o, p2.y);
                        } else {
                            line(&mut d, &mut pts, p2.x - r, p1.y);
                            arc(&mut d, r, true, p2.x, p1.y + o);
                        }
                    } else if p1.x > p2.x {
                        if merge_from_side {
                            line(&mut d, &mut pts, p1.x, p2.y - r);
                            arc(&mut d, r, true, p1.x - o, p2.y);
                        } else {
                            line(&mut d, &mut pts, p2.x + r, p1.y);
                            arc(&mut d, r, false, p2.x, p1.y + o);
                        }
                    }
                }
                Direction::BT => {
                    if p1.x < p2.x {
                        if merge_from_side {
                            line(&mut d, &mut pts, p1.x, p2.y + r);
                            arc(&mut d, r, true, p1.x + o, p2.y);
                        } else {
                            line(&mut d, &mut pts, p2.x - r, p1.y);
                            arc(&mut d, r, false, p2.x, p1.y - o);
                        }
                    } else if p1.x > p2.x {
                        if merge_from_side {
                            line(&mut d, &mut pts, p1.x, p2.y + r);
                            arc(&mut d, r, false, p1.x - o, p2.y);
                        } else {
                            line(&mut d, &mut pts, p2.x + r, p1.y);
                            arc(&mut d, r, true, p2.x, p1.y - o);
                        }
                    }
                }
                Direction::LR => {
                    if p1.y < p2.y {
                        if merge_from_side {
                            line(&mut d, &mut pts, p2.x - r, p1.y);
                            arc(&mut d, r, true, p2.x, p1.y + o);
                        } else {
                            line(&mut d, &mut pts, p1.x, p2.y - r);
                            arc(&mut d, r, false, p1.x + o, p2.y);
                        }
                    } else if p1.y > p2.y {
                        if merge_from_side {
                            line(&mut d, &mut pts, p2.x - r, p1.y);
                            arc(&mut d, r, false, p2.x, p1.y - o);
                        } else {
                            line(&mut d, &mut pts, p1.x, p2.y + r);
                            arc(&mut d, r, true, p1.x + o, p2.y);
                        }
                    }
                }
            }
            line(&mut d, &mut pts, p2.x, p2.y);
        }
        let index = GitGraphStyles::index(color_index);
        Ok(self.path(
            &format!("arrow arrow{index}"),
            Style {
                fill: Some(Paint::None),
                stroke: paint(self.styles.git[index]),
                stroke_width: Some(ARROW_STROKE_WIDTH),
                stroke_linecap: Some(LineCap::Round),
                ..Style::default()
            },
            d,
            &pts,
        ))
    }

    /// `drawArrows`: one arrow per parent, commits in insertion order.
    fn draw_arrows(&mut self) -> Result<Node, HelperError> {
        let mut arrows = Vec::new();
        for commit in self.db.commits().values() {
            for parent in &commit.parents {
                let Some(parent_commit) = self.db.commits().get(parent) else {
                    continue;
                };
                arrows.push(self.draw_arrow(parent_commit, commit)?);
            }
        }
        Ok(group("commit-arrows", Vec::new(), arrows))
    }

    /// `drawBranches`: the dashed line and the label of every branch.
    fn draw_branches(&mut self, branches: &[String]) -> Result<Node, HelperError> {
        let mut children = Vec::new();
        let rotate_extra = if self.cfg.rotate_commit_label {
            30.0
        } else {
            0.0
        };
        for (index, name) in branches.iter().enumerate() {
            let color_index = GitGraphStyles::index(index);
            let pos = self
                .branch_pos
                .get(name)
                .map(|b| b.pos)
                .ok_or_else(|| internal(format!("Position not found for branch {name}")))?;
            let spine_y = if self.vertical() { pos } else { pos - 2.0 };
            let line_style = Style {
                stroke: paint(self.styles.branch_line),
                stroke_width: Some(self.styles.branch_stroke_width),
                stroke_dasharray: Some(vec![2.0]),
                ..Style::default()
            };
            let class = format!("branch branch{color_index}");
            children.push(match self.dir {
                Direction::LR => self.line(&class, line_style, 0.0, spine_y, self.max_pos, spine_y),
                Direction::TB => self.line(&class, line_style, pos, DEFAULT_POS, pos, self.max_pos),
                Direction::BT => self.line(&class, line_style, pos, self.max_pos, pos, DEFAULT_POS),
            });
            self.lanes.push(spine_y);

            let label_text = self.draw_text(name, self.styles.branch_label[color_index]);
            let bbox = label_text.bbox.unwrap_or(BBox::new(0.0, 0.0, 0.0, 0.0));
            let (w, h) = (bbox.width(), bbox.height());
            let bkg_style = Style {
                fill: paint(self.styles.git[color_index]),
                ..Style::default()
            };
            let (bkg_x, bkg_y, bkg_t, label_t) = match self.dir {
                Direction::TB => (
                    pos - w / 2.0 - 10.0,
                    0.0,
                    Vec::new(),
                    TransformOp::Translate(pos - w / 2.0 - 5.0, 0.0),
                ),
                Direction::BT => (
                    pos - w / 2.0 - 10.0,
                    self.max_pos,
                    Vec::new(),
                    TransformOp::Translate(pos - w / 2.0 - 5.0, self.max_pos),
                ),
                Direction::LR => (
                    -w - 4.0 - rotate_extra,
                    -h / 2.0 + 10.0,
                    vec![TransformOp::Translate(-19.0, spine_y - 12.0)],
                    TransformOp::Translate(-w - 14.0 - rotate_extra, spine_y - h / 2.0 - 2.0),
                ),
            };
            children.push(self.rect_t(
                &format!("branchLabelBkg label{color_index}"),
                bkg_style,
                bkg_x,
                bkg_y,
                w + 18.0,
                h + 4.0,
                Some(4.0),
                bkg_t,
            ));
            let label = group(
                &format!("label branch-label{color_index}"),
                vec![label_t],
                vec![label_text],
            );
            children.push(group("branchLabel", Vec::new(), vec![label]));
        }
        Ok(group("", Vec::new(), children))
    }

    /// `setBranchPosition` for every branch, in `draw`.
    fn set_branch_positions(&mut self, branches: &[String]) {
        let mut pos = 0.0;
        for (index, name) in branches.iter().enumerate() {
            self.branch_pos
                .insert(name.clone(), BranchPosition { pos, index });
            let w = f64::from(self.measure(name, BRANCH_LABEL_FONT_SIZE).bbox().width);
            pos +=
                50.0 + if self.cfg.rotate_commit_label {
                    40.0
                } else {
                    0.0
                } + if self.vertical() { w / 2.0 } else { 0.0 };
        }
    }
}

/// `gitGraphRenderer.draw`.
pub fn draw(
    db: &GitGraphDb,
    cfg: &GitGraphConfig,
    styles: &GitGraphStyles,
    theme: &str,
    ctx: &RenderCtx<'_>,
) -> Result<Scene, HelperError> {
    let mut r = Renderer {
        db,
        cfg,
        styles,
        ctx,
        dir: db.direction(),
        is_dark: theme == "dark",
        branch_pos: IndexMap::new(),
        commit_pos: HashMap::new(),
        lanes: Vec::new(),
        max_pos: 0.0,
    };
    let branches = db.branches_as_obj_array();
    r.set_branch_positions(&branches);

    let mut nodes: Vec<Node> = Vec::new();
    r.draw_commits(false)?;
    if cfg.show_branches {
        nodes.push(r.draw_branches(&branches)?);
    }
    nodes.push(r.draw_arrows()?);
    nodes.extend(r.draw_commits(true)?);

    // `utils.insertTitle`: centered over the drawn box, above the origin.
    let drawn = nodes.iter().fold(None, |acc, n| BBox::union(acc, n.bbox));
    let title = db.diagram_title();
    if !title.is_empty()
        && let Some(b) = drawn
    {
        nodes.push(r.text(
            "gitTitleText",
            b.x0 + b.width() / 2.0,
            -cfg.title_top_margin,
            Some(TextAnchor::Middle),
            TITLE_FONT_SIZE,
            styles.text_color,
            title,
            Vec::new(),
        ));
    }

    // `setupGraphViewbox`: the SVG box plus `diagramPadding` on each side.
    let bounds = nodes
        .iter()
        .fold(None, |acc, n| BBox::union(acc, n.bbox))
        .unwrap_or(BBox::new(0.0, 0.0, 0.0, 0.0));
    let pad = cfg.diagram_padding;
    let width = bounds.width() + 2.0 * pad;
    let height = bounds.height() + 2.0 * pad;
    let mut scene = Scene::new(width, height);
    scene.view_box = ViewBox::new(bounds.x0 - pad, bounds.y0 - pad, width, height);
    scene.font_family = styles.font_family.clone();
    // The root rule of mermaid's stylesheet: `fill: <textColor>`, inherited
    // by every shape without a fill of its own.
    scene.root.common.style.fill = paint(styles.text_color);
    scene.root.children = nodes.into_iter().map(|n| n.el).collect();
    Ok(scene)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotated_box_maps_to_its_axis_aligned_bounds() {
        let b = BBox::new(0.0, 0.0, 10.0, 0.0);
        let m = b.map(&[TransformOp::Rotate(-45.0, 0.0, 0.0)]);
        let s = (2.0f64).sqrt() / 2.0 * 10.0;
        assert!((m.x1 - s).abs() < 1e-9);
        assert!((m.y0 + s).abs() < 1e-9);
        assert!(m.x0.abs() < 1e-9 && m.y1.abs() < 1e-9);
        let t = b.map(&[
            TransformOp::Translate(5.0, 7.0),
            TransformOp::Rotate(90.0, 0.0, 0.0),
        ]);
        // translate then rotate: the segment turns vertical at x = 5.
        assert!((t.x0 - 5.0).abs() < 1e-9 && (t.x1 - 5.0).abs() < 1e-9);
        assert!((t.y0 - 7.0).abs() < 1e-9 && (t.y1 - 17.0).abs() < 1e-9);
    }
}
