//! Protocol glue for `image.prepare` (R-147, R-172).
//!
//! `handle` takes the raw `params` object and the cache root and returns the
//! `result` object. The server wraps it in its `Handler` trait and converts
//! `Error` with `code()`, `Display`, and `data()`.
//!
//! The cache helpers at the bottom (`sha256_hex`, `key_image`,
//! `write_atomic`, `touch`) duplicate what `crate::cache` will provide; they
//! sit here so the lead can swap them for the cache module in one edit.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{Error, Prepared, prepare};

#[derive(Debug, Deserialize)]
struct Params {
    path: PathBuf,
    width_px: u32,
    #[serde(default)]
    max_width_px: Option<u32>,
    #[serde(default)]
    max_height_px: Option<u32>,
}

/// `image.prepare`: read the file, look the result up in `images/`, and
/// prepare and store it on a miss. The result carries a path, never bytes.
pub fn handle(params: Value, cache_root: &Path) -> Result<Value, Error> {
    let p: Params =
        serde_json::from_value(params).map_err(|e| Error::InvalidParams(e.to_string()))?;
    let bytes = read_input(&p.path)?;
    let key = key_image(
        &sha256_hex(&bytes),
        p.width_px,
        p.max_height_px,
        p.max_width_px,
    );
    let prepared = || {
        prepare(&bytes, p.width_px, p.max_width_px, p.max_height_px).map_err(|e| match e {
            // The sniff saw no magic bytes; the file extension is the only
            // format name left to report (R-089).
            Error::UnsupportedFormat { format } if format == "unknown" => {
                Error::UnsupportedFormat {
                    format: declared_format(&p.path),
                }
            }
            other => other,
        })
    };
    store_png(cache_root, &key, prepared, |path, source| Error::Io {
        path,
        source,
    })
}

/// Shared by `svg.rasterize`: return the cached PNG for `key` or compute,
/// write, and return it. The PNG header gives the size on a hit, so no
/// decode is needed.
pub fn store_png<E>(
    cache_root: &Path,
    key: &str,
    compute: impl FnOnce() -> Result<Prepared, E>,
    io_error: impl FnOnce(PathBuf, std::io::Error) -> E,
) -> Result<Value, E> {
    let dir = cache_root.join("images");
    let path = dir.join(format!("{key}.png"));
    if let Ok(meta) = std::fs::metadata(&path)
        && let Some((width, height)) = png_size(&path)
    {
        touch(&path);
        return Ok(json!({
            "path": path,
            "width_px": width,
            "height_px": height,
            "bytes": meta.len(),
            "cached": true,
        }));
    }
    let prepared = compute()?;
    write_atomic(&path, &prepared.png).map_err(|source| io_error(path.clone(), source))?;
    Ok(json!({
        "path": path,
        "width_px": prepared.width,
        "height_px": prepared.height,
        "bytes": prepared.png.len(),
        "cached": false,
    }))
}

/// Read the input file; any failure is `io` with the path (R-172).
pub fn read_input(path: &Path) -> Result<Vec<u8>, Error> {
    std::fs::read(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn declared_format(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .filter(|e| !e.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Width and height from the IHDR chunk of a PNG file.
fn png_size(path: &Path) -> Option<(u32, u32)> {
    use std::io::Read;
    let mut head = [0u8; 24];
    std::fs::File::open(path).ok()?.read_exact(&mut head).ok()?;
    if &head[..8] != b"\x89PNG\r\n\x1a\n" || &head[12..16] != b"IHDR" {
        return None;
    }
    let be = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    Some((be(&head[16..20]), be(&head[20..24])))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// `sha256(sha256(bytes) | width_px | max_h | max_w)` with `0` for an absent
/// limit, as the Cache keys convention states. `|` is the literal separator.
pub fn key_image(
    content_sha: &str,
    width_px: u32,
    max_h: Option<u32>,
    max_w: Option<u32>,
) -> String {
    let input = format!(
        "{content_sha}|{width_px}|{}|{}",
        max_h.unwrap_or(0),
        max_w.unwrap_or(0)
    );
    sha256_hex(input.as_bytes())
}

/// Write through a sibling temp file and rename, so a reader never sees a
/// partial entry and a crash leaves no half-written key.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent")
    })?;
    std::fs::create_dir_all(dir)?;
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("entry");
    let tmp = dir.join(format!(".{file_name}.{}.{nonce}.tmp", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

/// LRU bookkeeping (R-201): a hit bumps the mtime. A failure to touch is not
/// an error for the caller; the entry only ages faster.
pub fn touch(path: &Path) {
    if let Ok(file) = std::fs::File::open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}
