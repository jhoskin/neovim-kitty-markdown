//! SVG filters. mermaid only emits filters for the `handDrawn` look (rough.js
//! textures) and the `neo` look (drop shadows); both are reported as
//! `unsupported_feature` (R-112, R-175), so the classic look needs no filter
//! and the scene has no filter element yet. This module holds the place so
//! the module tree in AGENTS.md matches the code.

/// Filter support status, for callers that want to fail early.
pub const FILTERS_SUPPORTED: bool = false;
