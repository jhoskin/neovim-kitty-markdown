//! Command line surface (R-144 to R-146, R-199, R-200). `serve` is the
//! default and speaks the JSON-lines protocol; the other subcommands exist
//! for direct evaluation and for the parity harness.
//!
//! `main` parses, calls one `run_*` function per subcommand, and turns the
//! [`CliError`] into a stderr line and an exit status (R-146). The render,
//! compare, and measure functions are the integration points for the
//! modules that own those features.

use std::fmt;
use std::path::PathBuf;

use clap::{ArgGroup, Args, Parser, Subcommand};

use crate::error::HelperError;
use crate::server::{self, Ctx};

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
    Compare(CompareArgs),
    /// Measure a string with the helper's font metrics (calibration tool).
    Measure(MeasureArgs),
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

#[derive(Args, Debug)]
pub struct CompareArgs {
    pub a: PathBuf,
    pub b: PathBuf,
}

#[derive(Args, Debug)]
pub struct MeasureArgs {
    #[arg(long)]
    pub text: String,
    /// CSS family chain, for example `"trebuchet ms", verdana, sans-serif`.
    #[arg(long, default_value = "\"trebuchet ms\", verdana, arial, sans-serif")]
    pub font: String,
    #[arg(long, default_value_t = 16.0)]
    pub size: f32,
    /// `svg` for getBBox-style metrics, `html` for div-style metrics.
    #[arg(long, default_value = "svg")]
    pub mode: String,
}

/// Exit status for a feature that is not wired up yet, distinct from a
/// failed render (1) so a harness can tell the two apart.
pub const EXIT_NOT_IMPLEMENTED: u8 = 2;

/// A command failure: the one line `main` prints to stderr and the exit
/// status it returns.
#[derive(Debug)]
pub struct CliError {
    pub message: String,
    pub exit_code: u8,
}

impl CliError {
    pub fn new(message: impl Into<String>) -> Self {
        CliError {
            message: message.into(),
            exit_code: 1,
        }
    }

    pub fn not_implemented(command: &str) -> Self {
        CliError {
            message: format!("`{command}` is not implemented yet"),
            exit_code: EXIT_NOT_IMPLEMENTED,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

impl From<HelperError> for CliError {
    fn from(err: HelperError) -> Self {
        CliError::new(err.to_string())
    }
}

impl From<std::io::Error> for CliError {
    fn from(err: std::io::Error) -> Self {
        CliError::new(err.to_string())
    }
}

pub fn run(command: Command) -> Result<(), CliError> {
    match command {
        Command::Serve { cache_dir } => run_serve(cache_dir),
        Command::Render(args) => run_render(args),
        Command::Compare(args) => run_compare(args),
        Command::Measure(args) => run_measure(args),
    }
}

/// `serve`: the JSON-lines server with the built-in handlers. Pixel method
/// handlers (`image.prepare`, `svg.rasterize`, `fetch`, `mermaid.render`)
/// are appended here when their modules land.
pub fn run_serve(cache_dir: Option<PathBuf>) -> Result<(), CliError> {
    let ctx = Ctx::new(cache_dir)?;
    server::serve(server::builtin_handlers(), ctx)?;
    Ok(())
}

/// `render`: writes the PNG to `--output` without a cache (R-200). Filled by
/// the image, svg, and mermaid modules.
pub fn run_render(_args: RenderArgs) -> Result<(), CliError> {
    Err(CliError::not_implemented("render"))
}

/// `compare`: prints the SSIM of two PNGs (R-199). Filled by `ssim`.
pub fn run_compare(_args: CompareArgs) -> Result<(), CliError> {
    Err(CliError::not_implemented("compare"))
}

/// `measure`: prints text metrics for calibration. Filled by `text`.
pub fn run_measure(_args: MeasureArgs) -> Result<(), CliError> {
    Err(CliError::not_implemented("measure"))
}
