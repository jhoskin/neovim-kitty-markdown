//! Port of d3-axis 3.0.0 `axis.js` for the top and bottom orientations: the
//! numbers d3 writes into the DOM. The caller turns them into scene
//! elements, because mermaid restyles the axis after `call(axis)` and the
//! styles package decides what the classes mean.

use crate::mermaid::scene::PathData;

use super::scale_time::ScaleTime;
use super::time::Ms;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orient {
    Top,
    Bottom,
}

/// `axisTop(scale)` or `axisBottom(scale)` with its tick settings.
#[derive(Debug, Clone, PartialEq)]
pub struct Axis {
    pub orient: Orient,
    pub tick_size_inner: f64,
    pub tick_size_outer: f64,
    pub tick_padding: f64,
    /// `window.devicePixelRatio > 1 ? 0 : 0.5`; the oracle screenshots at a
    /// device scale of 1, so d3 shifts the axis by half a pixel.
    pub offset: f64,
}

/// One `g.tick`: `transform="translate(<position>,0)"` plus its label.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisTick {
    pub value: Ms,
    /// The translate already includes the half-pixel offset.
    pub position: f64,
    pub label: String,
}

/// What `axis(selection)` writes for a top or bottom axis.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisLayout {
    pub orient: Orient,
    /// `path.domain` `d`.
    pub domain_path: PathData,
    pub ticks: Vec<AxisTick>,
    /// `line` `y2`: `k * tickSizeInner`.
    pub line_y2: f64,
    /// `text` `y`: `k * spacing`.
    pub text_y: f64,
    /// `text` `dy` in em: `0em` for top, `0.71em` for bottom.
    pub text_dy_em: f64,
    /// The `font-size` d3 sets on the axis group.
    pub font_size: f64,
}

impl Axis {
    pub fn new(orient: Orient) -> Axis {
        Axis {
            orient,
            tick_size_inner: 6.0,
            tick_size_outer: 6.0,
            tick_padding: 3.0,
            offset: 0.5,
        }
    }

    /// `axis.tickSize(size)`: inner and outer at once.
    pub fn tick_size(mut self, size: f64) -> Axis {
        self.tick_size_inner = size;
        self.tick_size_outer = size;
        self
    }

    /// `axis(selection)` for `values` already chosen (`scale.ticks(...)`)
    /// and formatted by `format`.
    pub fn layout(
        &self,
        scale: &ScaleTime,
        values: &[Ms],
        format: impl Fn(Ms) -> String,
    ) -> AxisLayout {
        let k = if self.orient == Orient::Top {
            -1.0
        } else {
            1.0
        };
        let spacing = self.tick_size_inner.max(0.0) + self.tick_padding;
        let [r0, r1] = scale.range();
        let range0 = r0 + self.offset;
        let range1 = r1 + self.offset;

        let mut domain_path = PathData::new();
        if self.tick_size_outer != 0.0 {
            // `M<range0>,<k*outer>V<offset>H<range1>V<k*outer>`
            domain_path
                .move_to(range0, k * self.tick_size_outer)
                .line_to(range0, self.offset)
                .line_to(range1, self.offset)
                .line_to(range1, k * self.tick_size_outer);
        } else {
            domain_path
                .move_to(range0, self.offset)
                .line_to(range1, self.offset);
        }

        let ticks = values
            .iter()
            .map(|&value| AxisTick {
                value,
                position: scale.apply(value) + self.offset,
                label: format(value),
            })
            .collect();

        AxisLayout {
            orient: self.orient,
            domain_path,
            ticks,
            line_y2: k * self.tick_size_inner,
            text_y: k * spacing,
            text_dy_em: if self.orient == Orient::Top {
                0.0
            } else {
                0.71
            },
            font_size: 10.0,
        }
    }
}
