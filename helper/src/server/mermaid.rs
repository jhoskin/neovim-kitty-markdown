//! The `mermaid.render` handler: cache lookup by source, width, theme, and
//! mermaid version (R-116), then the render pipeline on a miss. The result
//! carries a path and a size, never bytes (R-147).

use serde::Deserialize;
use serde_json::{Value, json};

use crate::cache::{self, Subdir};
use crate::error::HelperError;
use crate::mermaid::{self, RenderRequest};
use crate::protocol::MERMAID_VERSION;
use crate::server::{Ctx, Handler};

#[derive(Debug, Deserialize)]
struct Params {
    source: String,
    width_px: u32,
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_background")]
    background: String,
}

fn default_theme() -> String {
    "default".to_string()
}

fn default_background() -> String {
    "white".to_string()
}

pub struct MermaidRender;

impl Handler for MermaidRender {
    fn method(&self) -> &'static str {
        "mermaid.render"
    }

    fn handle(&self, params: Value, ctx: &Ctx) -> Result<Value, HelperError> {
        let p: Params = serde_json::from_value(params)?;
        let cache = ctx.cache()?;
        let key = cache::key_mermaid(&p.source, p.width_px, &p.theme, MERMAID_VERSION);
        let path = cache.path_mermaid(&key);
        if let Some(bytes) = cache.hit(&path).map_err(|e| HelperError::io(&path, e))? {
            let (width, height) = image::image_dimensions(&path).map_err(|e| {
                HelperError::internal(format!("cached {} is not a PNG: {e}", path.display()))
            })?;
            // The type is not stored next to the PNG; detection is cheap.
            let resolved =
                mermaid::config::resolve(&p.source, &mermaid::config::BaseConfig::new(&p.theme))?;
            return Ok(json!({
                "path": path,
                "width_px": width,
                "height_px": height,
                "bytes": bytes,
                "cached": true,
                "diagram_type": resolved.diagram_type,
            }));
        }
        let out = mermaid::render(&RenderRequest {
            source: &p.source,
            width_px: p.width_px,
            theme: &p.theme,
            background: mermaid::parse_background(&p.background),
        })?;
        let bytes = out.png.len();
        let path = cache
            .put(Subdir::Mermaid, &key, &out.png)
            .map_err(|e| HelperError::io(cache.path_mermaid(&key), e))?;
        Ok(json!({
            "path": path,
            "width_px": out.width,
            "height_px": out.height,
            "bytes": bytes,
            "cached": false,
            "diagram_type": out.diagram_type,
        }))
    }
}
