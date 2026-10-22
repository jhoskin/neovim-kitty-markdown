//! JSON-lines request and response types (R-138, R-139).
//!
//! One request or response per line. A response carries the `id` of its
//! request and either `result` or `error`; never both.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The mermaid release the helper reproduces. The probe reports it and the
/// mermaid cache key includes it, so a pin bump invalidates old renders.
pub const MERMAID_VERSION: &str = "12.2.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: String,
    /// Absent `params` means `null`, so handlers without parameters accept
    /// a bare `{"id", "method"}` line.
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    /// Always present on the wire (`null` when the code carries no data),
    /// so the plugin can read `error.data` without a shape check.
    pub data: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Response {
    Ok { id: u64, result: Value },
    Err { id: u64, error: ErrorBody },
}

impl Response {
    pub fn ok(id: u64, result: Value) -> Self {
        Response::Ok { id, result }
    }

    pub fn error(id: u64, error: impl Into<ErrorBody>) -> Self {
        Response::Err {
            id,
            error: error.into(),
        }
    }

    pub fn id(&self) -> u64 {
        match self {
            Response::Ok { id, .. } | Response::Err { id, .. } => *id,
        }
    }
}

/// Reads the `id` of a line that may not be a valid request, so a malformed
/// request can still be answered with the id the plugin is waiting on.
/// Returns 0 when there is no readable id.
pub fn peek_id(line: &str) -> u64 {
    serde_json::from_str::<Value>(line)
        .ok()
        .and_then(|v| v.get("id").and_then(Value::as_u64))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ok_response_has_id_and_result_only() {
        let text = serde_json::to_string(&Response::ok(3, json!({"a": 1}))).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v, json!({"id": 3, "result": {"a": 1}}));
    }

    #[test]
    fn error_response_has_code_message_data() {
        let body = ErrorBody {
            code: "timeout".into(),
            message: "slow".into(),
            data: None,
        };
        let v = serde_json::to_value(Response::error(9, body)).unwrap();
        assert_eq!(
            v,
            json!({"id": 9, "error": {"code": "timeout", "message": "slow", "data": null}})
        );
    }

    #[test]
    fn request_without_params_parses() {
        let r: Request = serde_json::from_str(r#"{"id":1,"method":"shutdown"}"#).unwrap();
        assert_eq!(r.params, Value::Null);
    }

    #[test]
    fn peek_id_tolerates_garbage() {
        assert_eq!(peek_id("{not json"), 0);
        assert_eq!(peek_id(r#"{"id":"x"}"#), 0);
        assert_eq!(peek_id(r#"{"id":42}"#), 42);
    }
}
