//! One module per diagram type. A renderer turns a resolved diagram into a
//! scene; the pipeline in `mermaid::render` does everything around it.
//! The registry lists the delivered renderers in detector order; a type
//! without one answers `unsupported_diagram_type` (R-113).

pub mod quadrant;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::scene::Scene;
use crate::text::{FontSystem, TextMetrics};

pub mod pie;

/// What a renderer gets besides the resolved diagram.
pub struct RenderCtx<'f> {
    pub fonts: &'f FontSystem,
    /// Browser-equivalent text metrics; memoizes shaped runs per render.
    pub metrics: TextMetrics<'f>,
    /// The width in pixels the plugin asked for. Renderers lay out at the
    /// natural size; the raster step scales down (R-180).
    pub target_width: u32,
}

pub trait Diagram: Send + Sync {
    /// The detector ids this renderer handles, for example
    /// `["flowchart-v2", "flowchart"]`.
    fn types(&self) -> &'static [&'static str];

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError>;
}

/// The delivered renderers. Each diagram package exposes a `DIAGRAM`
/// static and the integration step adds it here.
pub fn registry() -> &'static [&'static dyn Diagram] {
    static REGISTRY: &[&dyn Diagram] = &[&pie::DIAGRAM, &quadrant::DIAGRAM];
    REGISTRY
}

pub fn find(diagram_type: &str) -> Option<&'static dyn Diagram> {
    registry()
        .iter()
        .copied()
        .find(|d| d.types().contains(&diagram_type))
}

/// Every type a delivered renderer handles, for the probe result.
pub fn delivered_types() -> Vec<String> {
    registry()
        .iter()
        .flat_map(|d| d.types().iter().map(|t| t.to_string()))
        .collect()
}
