//! kitty-markdown-helper: images, SVG, and mermaid for kitty-markdown.nvim.
//! The binary in `main.rs` is a thin wrapper; everything testable lives here.

pub mod cache;
pub mod cli;
pub mod error;
pub mod fetch;
pub mod image;
pub mod mermaid;
pub mod protocol;
pub mod server;
pub mod ssim;
pub mod svg;
pub mod text;
