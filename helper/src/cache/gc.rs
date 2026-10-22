//! LRU garbage collection: oldest mtime goes first, keep paths never go.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;

use super::{CacheDir, Subdir};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct GcResult {
    pub deleted_bytes: u64,
    pub cache_bytes: u64,
}

pub(super) struct Entry {
    pub path: PathBuf,
    pub bytes: u64,
    pub modified: SystemTime,
}

/// Lists every regular file in the three subdirectories. Entries that vanish
/// between the directory read and the stat (another worker's gc or rename)
/// are skipped rather than failing the whole scan.
pub(super) fn scan(cache: &CacheDir) -> io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for sub in Subdir::ALL {
        let dir = cache.subdir(sub);
        let read_dir = match fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        for item in read_dir {
            let item = item?;
            let Ok(meta) = item.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            entries.push(Entry {
                path: item.path(),
                bytes: meta.len(),
                modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            });
        }
    }
    Ok(entries)
}

/// Resolves symlinks and `..` so a keep path written by the plugin matches
/// the path the directory scan produces. A path that does not exist keeps
/// its spelling.
fn normalize(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub(super) fn run(
    cache: &CacheDir,
    max_bytes: u64,
    keep: &HashSet<PathBuf>,
) -> io::Result<GcResult> {
    let mut entries = scan(cache)?;
    let mut cache_bytes: u64 = entries.iter().map(|e| e.bytes).sum();
    let mut deleted_bytes = 0;
    if cache_bytes <= max_bytes {
        return Ok(GcResult {
            deleted_bytes,
            cache_bytes,
        });
    }
    let keep: HashSet<PathBuf> = keep.iter().map(|p| normalize(p)).collect();
    entries.sort_by_key(|e| e.modified);
    for entry in entries {
        if cache_bytes <= max_bytes {
            break;
        }
        if keep.contains(&normalize(&entry.path)) {
            continue;
        }
        match fs::remove_file(&entry.path) {
            Ok(()) => {
                deleted_bytes += entry.bytes;
                cache_bytes -= entry.bytes;
            }
            // Already gone: someone else freed those bytes.
            Err(err) if err.kind() == io::ErrorKind::NotFound => cache_bytes -= entry.bytes,
            Err(err) => return Err(err),
        }
    }
    Ok(GcResult {
        deleted_bytes,
        cache_bytes,
    })
}
