//! Typed errors with the stable `code` strings of the protocol.
//!
//! Every variant maps to one code in the AGENTS.md error table. The plugin
//! turns the code into the failure box reason, so codes never change; the
//! `message` is free text for `:messages`.

use std::path::PathBuf;

use serde_json::{Value, json};
use thiserror::Error;

use crate::protocol::ErrorBody;

#[derive(Debug, Error)]
pub enum HelperError {
    #[error("request timed out")]
    Timeout,

    #[error("input is {bytes} bytes, the limit is {max_bytes}")]
    TooLarge { bytes: u64, max_bytes: u64 },

    #[error("unsupported format: {format}")]
    UnsupportedFormat { format: String },

    /// `message` is the full oracle text, for example
    /// `Parse error on line 2: ...`, because the plugin shows it verbatim.
    #[error("{message}")]
    ParseError {
        line: u32,
        column: u32,
        diagram_type: String,
        message: String,
    },

    #[error("unsupported feature: {feature}")]
    UnsupportedFeature { feature: String },

    #[error("unsupported diagram type: {diagram_type}")]
    UnsupportedDiagramType { diagram_type: String },

    /// `status` is `None` when no HTTP response arrived (DNS, TLS, timeout
    /// on connect), so the plugin still sees `fetch_failed`.
    #[error("fetch failed: {message}")]
    FetchFailed {
        status: Option<u16>,
        message: String,
    },

    #[error("cannot read {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{0}")]
    Internal(String),

    #[error("invalid params: {0}")]
    InvalidParams(String),

    #[error("unknown method: {0}")]
    UnknownMethod(String),
}

impl HelperError {
    /// Attaches the path to an I/O error from a user file, so the failure
    /// box says "cannot read file" and `:messages` names the file.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        HelperError::Io {
            path: path.into(),
            source,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        HelperError::Internal(message.into())
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        HelperError::InvalidParams(message.into())
    }

    pub fn code(&self) -> &'static str {
        match self {
            HelperError::Timeout => "timeout",
            HelperError::TooLarge { .. } => "too_large",
            HelperError::UnsupportedFormat { .. } => "unsupported_format",
            HelperError::ParseError { .. } => "parse_error",
            HelperError::UnsupportedFeature { .. } => "unsupported_feature",
            HelperError::UnsupportedDiagramType { .. } => "unsupported_diagram_type",
            HelperError::FetchFailed { .. } => "fetch_failed",
            HelperError::Io { .. } => "io",
            HelperError::Internal(_) => "internal",
            HelperError::InvalidParams(_) => "invalid_params",
            HelperError::UnknownMethod(_) => "unknown_method",
        }
    }

    pub fn data(&self) -> Option<Value> {
        match self {
            HelperError::TooLarge { bytes, max_bytes } => {
                Some(json!({"bytes": bytes, "max_bytes": max_bytes}))
            }
            HelperError::UnsupportedFormat { format } => Some(json!({"format": format})),
            HelperError::ParseError {
                line,
                column,
                diagram_type,
                ..
            } => Some(json!({"line": line, "column": column, "diagram_type": diagram_type})),
            HelperError::UnsupportedFeature { feature } => Some(json!({"feature": feature})),
            HelperError::UnsupportedDiagramType { diagram_type } => {
                Some(json!({"diagram_type": diagram_type}))
            }
            HelperError::FetchFailed { status, .. } => Some(json!({"status": status})),
            HelperError::Io { path, .. } => Some(json!({"path": path})),
            HelperError::Timeout
            | HelperError::Internal(_)
            | HelperError::InvalidParams(_)
            | HelperError::UnknownMethod(_) => None,
        }
    }
}

impl From<HelperError> for ErrorBody {
    fn from(err: HelperError) -> Self {
        ErrorBody {
            code: err.code().to_string(),
            message: err.to_string(),
            data: err.data(),
        }
    }
}

/// A bare I/O error has no user-facing path, so it is `internal`. Use
/// [`HelperError::io`] when the failing path is a file the user named.
impl From<std::io::Error> for HelperError {
    fn from(err: std::io::Error) -> Self {
        HelperError::Internal(err.to_string())
    }
}

/// Handlers decode `params` with `serde_json::from_value(...)?`, so a JSON
/// error is the caller's fault. Serializing a result cannot fail for the
/// types the handlers return.
impl From<serde_json::Error> for HelperError {
    fn from(err: serde_json::Error) -> Self {
        HelperError::InvalidParams(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_match_the_protocol_table() {
        let cases: Vec<(HelperError, &str)> = vec![
            (HelperError::Timeout, "timeout"),
            (
                HelperError::TooLarge {
                    bytes: 2,
                    max_bytes: 1,
                },
                "too_large",
            ),
            (
                HelperError::UnsupportedFormat {
                    format: "tiff".into(),
                },
                "unsupported_format",
            ),
            (
                HelperError::ParseError {
                    line: 1,
                    column: 2,
                    diagram_type: "flowchart".into(),
                    message: "Parse error on line 1".into(),
                },
                "parse_error",
            ),
            (
                HelperError::UnsupportedFeature {
                    feature: "elk".into(),
                },
                "unsupported_feature",
            ),
            (
                HelperError::UnsupportedDiagramType {
                    diagram_type: "zenuml".into(),
                },
                "unsupported_diagram_type",
            ),
            (
                HelperError::FetchFailed {
                    status: Some(404),
                    message: "not found".into(),
                },
                "fetch_failed",
            ),
            (HelperError::io("/x", std::io::Error::other("boom")), "io"),
            (HelperError::internal("x"), "internal"),
            (HelperError::invalid_params("x"), "invalid_params"),
            (HelperError::UnknownMethod("x".into()), "unknown_method"),
        ];
        for (err, code) in cases {
            assert_eq!(err.code(), code);
        }
    }

    #[test]
    fn data_carries_the_documented_fields() {
        let err = HelperError::TooLarge {
            bytes: 5,
            max_bytes: 4,
        };
        assert_eq!(err.data(), Some(json!({"bytes": 5, "max_bytes": 4})));
        let err = HelperError::ParseError {
            line: 3,
            column: 7,
            diagram_type: "sequence".into(),
            message: "Parse error on line 3".into(),
        };
        assert_eq!(
            err.data(),
            Some(json!({"line": 3, "column": 7, "diagram_type": "sequence"}))
        );
        assert_eq!(err.to_string(), "Parse error on line 3");
        assert_eq!(HelperError::Timeout.data(), None);
    }

    #[test]
    fn error_body_from_helper_error() {
        let body: ErrorBody = HelperError::UnknownMethod("x".into()).into();
        assert_eq!(body.code, "unknown_method");
        assert_eq!(body.message, "unknown method: x");
        assert_eq!(body.data, None);
    }

    #[test]
    fn json_errors_are_invalid_params() {
        let err: HelperError = serde_json::from_str::<u32>("\"x\"")
            .map_err(HelperError::from)
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
    }
}
