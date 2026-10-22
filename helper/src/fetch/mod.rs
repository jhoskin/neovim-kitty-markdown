//! Remote image fetch with timeout and byte cap, cached by URL hash (R-079 to R-084).
//!
//! The cache entry is `<cache_root>/remote/<sha256(url)>` with no extension;
//! the image decoders sniff the bytes (R-080). A hit returns without a
//! request (R-081).

pub mod handler;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::image::handler::{sha256_hex, touch, write_atomic};

/// Where a fetched body lives and whether a request was made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    pub path: PathBuf,
    pub bytes: u64,
    pub cached: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("fetch timed out after {timeout_ms} ms")]
    Timeout { timeout_ms: u64 },
    #[error("remote image is {bytes} bytes, more than the cap of {max_bytes}")]
    TooLarge { bytes: u64, max_bytes: u64 },
    /// `status` is 0 when no HTTP status arrived (DNS, refused connection,
    /// TLS, bad scheme) (R-173).
    #[error("fetch failed: {message}")]
    Failed { status: u16, message: String },
    #[error("cannot write cache entry {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid params: {0}")]
    InvalidParams(String),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Error::Timeout { .. } => "timeout",
            Error::TooLarge { .. } => "too_large",
            Error::Failed { .. } => "fetch_failed",
            Error::Io { .. } => "io",
            Error::InvalidParams(_) => "invalid_params",
        }
    }

    pub fn data(&self) -> Option<serde_json::Value> {
        match self {
            Error::TooLarge { bytes, max_bytes } => {
                Some(serde_json::json!({ "bytes": bytes, "max_bytes": max_bytes }))
            }
            Error::Failed { status, .. } => Some(serde_json::json!({ "status": status })),
            Error::Io { path, .. } => Some(serde_json::json!({ "path": path })),
            Error::Timeout { .. } | Error::InvalidParams(_) => None,
        }
    }
}

/// The cache path for `url`.
pub fn cache_path(cache_root: &Path, url: &str) -> PathBuf {
    cache_root.join("remote").join(sha256_hex(url.as_bytes()))
}

/// Return the cached body for `url` or download it. `timeout_ms` bounds the
/// whole exchange, connect included; 0 means no timeout. `max_bytes` caps the
/// body, checked against `Content-Length` first and then while streaming.
pub fn fetch(
    url: &str,
    timeout_ms: u64,
    max_bytes: u64,
    cache_root: &Path,
) -> Result<Fetched, Error> {
    let path = cache_path(cache_root, url);
    if let Ok(meta) = std::fs::metadata(&path)
        && meta.is_file()
    {
        touch(&path);
        return Ok(Fetched {
            path,
            bytes: meta.len(),
            cached: true,
        });
    }

    let body = download(url, timeout_ms, max_bytes)?;
    write_atomic(&path, &body).map_err(|source| Error::Io {
        path: path.clone(),
        source,
    })?;
    Ok(Fetched {
        path,
        bytes: body.len() as u64,
        cached: false,
    })
}

/// One GET with the limits applied. Only `http` and `https` are accepted;
/// `file:` and friends must not reach the network stack.
pub fn download(url: &str, timeout_ms: u64, max_bytes: u64) -> Result<Vec<u8>, Error> {
    let lower = url.trim_start().to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(Error::Failed {
            status: 0,
            message: format!("unsupported URL scheme in {url}; only http and https are fetched"),
        });
    }

    let timeout = (timeout_ms > 0).then(|| Duration::from_millis(timeout_ms));
    let agent = ureq::Agent::config_builder()
        .timeout_global(timeout)
        .http_status_as_error(false)
        .build()
        .new_agent();

    let mut response = agent.get(url).call().map_err(|e| map_ureq(e, timeout_ms))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(Error::Failed {
            status,
            message: format!("HTTP status {status} for {url}"),
        });
    }
    if let Some(len) = response.body().content_length()
        && len > max_bytes
    {
        return Err(Error::TooLarge {
            bytes: len,
            max_bytes,
        });
    }

    // Read in chunks and stop as soon as the cap is passed, so a server
    // that lies about Content-Length or streams a chunked body cannot fill
    // memory (R-083).
    let mut reader = response.body_mut().as_reader();
    let mut body = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        let n = reader.read(&mut chunk).map_err(|e| map_io(e, timeout_ms))?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
        if body.len() as u64 > max_bytes {
            return Err(Error::TooLarge {
                bytes: body.len() as u64,
                max_bytes,
            });
        }
    }
    Ok(body)
}

fn map_ureq(e: ureq::Error, timeout_ms: u64) -> Error {
    match e {
        ureq::Error::Timeout(_) => Error::Timeout { timeout_ms },
        ureq::Error::Io(io) => map_io(io, timeout_ms),
        ureq::Error::StatusCode(status) => Error::Failed {
            status,
            message: format!("HTTP status {status}"),
        },
        other => Error::Failed {
            status: 0,
            message: other.to_string(),
        },
    }
}

/// Body reads surface ureq errors as `io::Error`; unwrap a wrapped timeout
/// so a slow body is still `timeout`, not `fetch_failed`.
fn map_io(e: std::io::Error, timeout_ms: u64) -> Error {
    if e.kind() == std::io::ErrorKind::TimedOut {
        return Error::Timeout { timeout_ms };
    }
    if let Some(inner) = e
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<ureq::Error>())
    {
        if matches!(inner, ureq::Error::Timeout(_)) {
            return Error::Timeout { timeout_ms };
        }
        return Error::Failed {
            status: 0,
            message: inner.to_string(),
        };
    }
    Error::Failed {
        status: 0,
        message: e.to_string(),
    }
}
