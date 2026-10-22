//! Handlers that belong to the server itself: `probe`, `cache.gc`, and the
//! test-only `debug.sleep`. Pixel methods live with their modules and are
//! registered next to these in `cli::serve`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Ctx, Handler, params_or_default, to_result};
use crate::error::HelperError;
use crate::protocol::MERMAID_VERSION;

/// Environment variable that enables `debug.sleep`. Tests set it to prove
/// out-of-order answers; the plugin never does.
pub const DEBUG_METHODS_VAR: &str = "KMH_DEBUG_METHODS";

/// The handlers every `serve` registers. `debug.sleep` joins them only when
/// `KMH_DEBUG_METHODS=1`.
pub fn builtin_handlers() -> Vec<Box<dyn Handler>> {
    let mut handlers: Vec<Box<dyn Handler>> = vec![Box::new(Probe::default()), Box::new(CacheGc)];
    if std::env::var(DEBUG_METHODS_VAR).is_ok_and(|v| v == "1") {
        handlers.push(Box::new(DebugSleep));
    }
    handlers
}

type FontsFn = dyn Fn() -> Value + Send + Sync;

/// `probe`: versions and cache size. The font list and diagram types are
/// supplied by the modules that own them; the defaults are empty so the
/// server builds before those modules exist.
pub struct Probe {
    pub diagram_types: Vec<String>,
    pub fonts: Arc<FontsFn>,
}

impl Default for Probe {
    fn default() -> Self {
        Probe {
            diagram_types: Vec::new(),
            fonts: Arc::new(|| json!({})),
        }
    }
}

impl Probe {
    pub fn new(
        diagram_types: Vec<String>,
        fonts: impl Fn() -> Value + Send + Sync + 'static,
    ) -> Self {
        Probe {
            diagram_types,
            fonts: Arc::new(fonts),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct ProbeParams {
    /// Checked by the plugin, not the helper (R-142), so unused here.
    #[serde(default, rename = "plugin_version")]
    _plugin_version: Option<String>,
    #[serde(default)]
    cache_dir: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct ProbeResult<'a> {
    helper_version: &'static str,
    mermaid_version: &'static str,
    cache_bytes: u64,
    fonts: Value,
    diagram_types: &'a [String],
}

impl Handler for Probe {
    fn method(&self) -> &'static str {
        "probe"
    }

    fn handle(&self, params: Value, ctx: &Ctx) -> Result<Value, HelperError> {
        let params: ProbeParams = params_or_default(params)?;
        let cache = match params.cache_dir {
            Some(root) => Some(ctx.set_cache_dir_if_unset(&root)?),
            None => ctx.cache_opt(),
        };
        let cache_bytes = match cache {
            Some(cache) => cache.total_bytes()?,
            None => 0,
        };
        to_result(&ProbeResult {
            helper_version: env!("CARGO_PKG_VERSION"),
            mermaid_version: MERMAID_VERSION,
            cache_bytes,
            fonts: (self.fonts)(),
            diagram_types: &self.diagram_types,
        })
    }
}

/// `cache.gc`: LRU deletion down to `max_bytes`, sparing `keep` (R-151,
/// R-152).
pub struct CacheGc;

#[derive(Debug, Deserialize)]
struct GcParams {
    max_bytes: u64,
    #[serde(default)]
    keep: Vec<PathBuf>,
}

impl Handler for CacheGc {
    fn method(&self) -> &'static str {
        "cache.gc"
    }

    fn handle(&self, params: Value, ctx: &Ctx) -> Result<Value, HelperError> {
        let params: GcParams = serde_json::from_value(params)?;
        let cache = ctx.cache()?;
        let keep: HashSet<PathBuf> = params.keep.into_iter().collect();
        to_result(&cache.gc(params.max_bytes, &keep)?)
    }
}

/// `debug.sleep`: blocks one worker for `ms` so a test can watch a later
/// request answer first.
pub struct DebugSleep;

#[derive(Debug, Deserialize)]
struct SleepParams {
    ms: u64,
}

impl Handler for DebugSleep {
    fn method(&self) -> &'static str {
        "debug.sleep"
    }

    fn handle(&self, params: Value, _ctx: &Ctx) -> Result<Value, HelperError> {
        let params: SleepParams = serde_json::from_value(params)?;
        std::thread::sleep(Duration::from_millis(params.ms));
        Ok(json!({}))
    }
}
