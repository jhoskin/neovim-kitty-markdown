//! Protocol glue for `svg.rasterize` (R-147). Mirrors `image.prepare`: the
//! same `images/` directory and the same key formula over the SVG bytes, so
//! an SVG and a raster file never collide and share one LRU.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use super::{Error, rasterize};
use crate::image::Prepared;
use crate::image::handler::{key_image, sha256_hex, store_png};

#[derive(Debug, Deserialize)]
struct Params {
    path: PathBuf,
    width_px: u32,
}

pub fn handle(params: Value, cache_root: &Path) -> Result<Value, Error> {
    let p: Params =
        serde_json::from_value(params).map_err(|e| Error::InvalidParams(e.to_string()))?;
    let bytes = std::fs::read(&p.path).map_err(|source| Error::Io {
        path: p.path.clone(),
        source,
    })?;
    let key = key_image(&sha256_hex(&bytes), p.width_px, None, None);
    store_png(
        cache_root,
        &key,
        || {
            let r = rasterize(&bytes, p.width_px)?;
            Ok(Prepared {
                png: r.png,
                width: r.width,
                height: r.height,
            })
        },
        |path, source| Error::Io { path, source },
    )
}
