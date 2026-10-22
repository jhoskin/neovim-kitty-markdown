//! Protocol glue for `fetch`: `url`, `timeout_ms`, `max_bytes` in; `path`,
//! `bytes`, `cached` out.

use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};

use super::{Error, fetch};

#[derive(Debug, Deserialize)]
struct Params {
    url: String,
    timeout_ms: u64,
    max_bytes: u64,
}

pub fn handle(params: Value, cache_root: &Path) -> Result<Value, Error> {
    let p: Params =
        serde_json::from_value(params).map_err(|e| Error::InvalidParams(e.to_string()))?;
    let fetched = fetch(&p.url, p.timeout_ms, p.max_bytes, cache_root)?;
    Ok(json!({
        "path": fetched.path,
        "bytes": fetched.bytes,
        "cached": fetched.cached,
    }))
}
