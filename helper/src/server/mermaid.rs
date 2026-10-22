//! The `mermaid.render` handler. It resolves the diagram (preprocess,
//! detect, config, theme) so parse errors and unsupported features reach
//! the plugin with the oracle's text, then hands the diagram to the
//! renderer of its type. Until a type's package lands, the answer is
//! `unsupported_diagram_type` (R-113).

use serde::Deserialize;
use serde_json::Value;

use crate::error::HelperError;
use crate::mermaid::config::{self, BaseConfig};
use crate::server::{Ctx, Handler};

#[derive(Debug, Deserialize)]
struct Params {
    source: String,
    #[serde(default = "default_theme")]
    theme: String,
}

fn default_theme() -> String {
    "default".to_string()
}

pub struct MermaidRender;

impl Handler for MermaidRender {
    fn method(&self) -> &'static str {
        "mermaid.render"
    }

    fn handle(&self, params: Value, _ctx: &Ctx) -> Result<Value, HelperError> {
        let params: Params = serde_json::from_value(params)?;
        let resolved = config::resolve(&params.source, &BaseConfig::new(&params.theme))?;
        Err(HelperError::UnsupportedDiagramType {
            diagram_type: resolved.diagram_type.to_string(),
        })
    }
}
