//! Command line surface (R-144 to R-146, R-199, R-200). `serve` is the
//! default and speaks the JSON-lines protocol; the other subcommands exist
//! for direct evaluation and for the parity harness.

use std::path::PathBuf;

use clap::{ArgGroup, Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "kitty-markdown-helper", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Serve JSON-lines requests on stdin and stdout (the default).
    Serve {
        /// Cache root; the plugin also passes it in `probe`.
        #[arg(long)]
        cache_dir: Option<PathBuf>,
    },
    /// Render one input file to a PNG.
    Render(RenderArgs),
    /// Print the SSIM of two PNG files of the same size.
    Compare { a: PathBuf, b: PathBuf },
    /// Measure a string with the helper's font metrics (calibration tool).
    Measure {
        #[arg(long)]
        text: String,
        /// CSS family chain, for example `"trebuchet ms", verdana, sans-serif`.
        #[arg(long, default_value = "\"trebuchet ms\", verdana, arial, sans-serif")]
        font: String,
        #[arg(long, default_value_t = 16.0)]
        size: f32,
        /// `svg` for getBBox-style metrics, `html` for div-style metrics.
        #[arg(long, default_value = "svg")]
        mode: String,
    },
}

#[derive(Args, Debug)]
#[command(group(ArgGroup::new("input").required(true)))]
pub struct RenderArgs {
    /// Mermaid source file.
    #[arg(long, group = "input")]
    pub mermaid: Option<PathBuf>,
    /// Raster image file (PNG, JPEG, GIF, WebP, BMP).
    #[arg(long, group = "input")]
    pub image: Option<PathBuf>,
    /// SVG file.
    #[arg(long, group = "input")]
    pub svg: Option<PathBuf>,
    /// Target width in pixels.
    #[arg(long)]
    pub width: u32,
    /// Mermaid theme: default, dark, neutral, forest, base.
    #[arg(long, default_value = "default")]
    pub theme: String,
    /// Background color (CSS color or `transparent`).
    #[arg(long, default_value = "white")]
    pub background: String,
    /// Output PNG path.
    #[arg(short, long)]
    pub output: PathBuf,
}
