//! `shapes/icon.ts`, `iconSquare.ts`, `iconCircle.ts`, `iconRounded.ts`,
//! and `imageSquare.ts`. Icon packs and image nodes are not supported
//! (SPEC.md R-112), so these handlers report the feature instead of
//! drawing.

use super::{Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;

/// `icon`, `iconSquare`, `iconCircle`, `iconRounded`.
pub struct IconShape;

impl Shape for IconShape {
    fn draw(&self, _node: &mut Node, _ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        Err(HelperError::UnsupportedFeature {
            feature: "icon pack".to_string(),
        })
    }
}

/// `imageSquare`.
pub struct ImageSquare;

impl Shape for ImageSquare {
    fn draw(&self, _node: &mut Node, _ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        Err(HelperError::UnsupportedFeature {
            feature: "image node".to_string(),
        })
    }
}
