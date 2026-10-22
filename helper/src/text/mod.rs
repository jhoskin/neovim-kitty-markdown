//! Fonts and text measurement. Mermaid measures labels in a browser; these
//! modules reproduce `getBBox` and `getBoundingClientRect` with rustybuzz.

pub mod fonts;
pub mod markdown;
pub mod measure;
pub mod wrap;
