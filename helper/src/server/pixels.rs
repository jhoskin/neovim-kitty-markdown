//! Adapters from the image, svg, and fetch modules to the server's
//! [`Handler`] trait. The modules take a cache root and return their own
//! typed errors; this file is the only place that knows both sides.

use std::path::Path;

use serde_json::Value;

use crate::error::HelperError;
use crate::server::{Ctx, Handler};
use crate::{fetch, image, svg};

fn with_cache<E: Into<HelperError>>(
    ctx: &Ctx,
    f: impl FnOnce(&Path) -> Result<Value, E>,
) -> Result<Value, HelperError> {
    let cache = ctx.cache()?;
    f(cache.root()).map_err(Into::into)
}

pub struct ImagePrepare;

impl Handler for ImagePrepare {
    fn method(&self) -> &'static str {
        "image.prepare"
    }

    fn handle(&self, params: Value, ctx: &Ctx) -> Result<Value, HelperError> {
        with_cache(ctx, |root| image::handler::handle(params, root))
    }
}

pub struct SvgRasterize;

impl Handler for SvgRasterize {
    fn method(&self) -> &'static str {
        "svg.rasterize"
    }

    fn handle(&self, params: Value, ctx: &Ctx) -> Result<Value, HelperError> {
        with_cache(ctx, |root| svg::handler::handle(params, root))
    }
}

pub struct Fetch;

impl Handler for Fetch {
    fn method(&self) -> &'static str {
        "fetch"
    }

    fn handle(&self, params: Value, ctx: &Ctx) -> Result<Value, HelperError> {
        with_cache(ctx, |root| fetch::handler::handle(params, root))
    }
}

/// The pixel method handlers, appended to the built-in ones by `serve`.
pub fn handlers() -> Vec<Box<dyn Handler>> {
    vec![
        Box::new(ImagePrepare),
        Box::new(SvgRasterize),
        Box::new(Fetch),
    ]
}
