//! Cache keys, subdirectories, and LRU garbage collection with a keep set.
//!
//! The cache root is `stdpath('cache')/kitty-markdown/` on the plugin side
//! (R-148); the helper only ever receives it. Entries live in `images`,
//! `remote`, and `mermaid` (R-149). Recency is the file mtime (R-201).

mod gc;
mod keys;

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

pub use gc::GcResult;
pub use keys::{key_image, key_image_bytes, key_mermaid, key_remote, sha256_hex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Subdir {
    Images,
    Remote,
    Mermaid,
}

impl Subdir {
    pub const ALL: [Subdir; 3] = [Subdir::Images, Subdir::Remote, Subdir::Mermaid];

    pub fn name(self) -> &'static str {
        match self {
            Subdir::Images => "images",
            Subdir::Remote => "remote",
            Subdir::Mermaid => "mermaid",
        }
    }
}

/// A handle on the cache root. Cloning is cheap; the server hands one to
/// every handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheDir {
    root: PathBuf,
}

/// Distinguishes concurrent temp files for the same key within one process;
/// the pid in the name separates processes.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

impl CacheDir {
    /// Creates the root and the three subdirectories when they are absent.
    pub fn new(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        for sub in Subdir::ALL {
            fs::create_dir_all(root.join(sub.name()))?;
        }
        Ok(CacheDir { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn subdir(&self, sub: Subdir) -> PathBuf {
        self.root.join(sub.name())
    }

    pub fn path_image(&self, key: &str) -> PathBuf {
        self.subdir(Subdir::Images).join(format!("{key}.png"))
    }

    pub fn path_remote(&self, key: &str) -> PathBuf {
        self.subdir(Subdir::Remote).join(key)
    }

    pub fn path_mermaid(&self, key: &str) -> PathBuf {
        self.subdir(Subdir::Mermaid).join(format!("{key}.png"))
    }

    /// Path of an entry written with [`CacheDir::put`].
    pub fn path_for(&self, sub: Subdir, key: &str) -> PathBuf {
        match sub {
            Subdir::Images => self.path_image(key),
            Subdir::Remote => self.path_remote(key),
            Subdir::Mermaid => self.path_mermaid(key),
        }
    }

    /// Reports a hit: the entry's size after touching it, or `None` when the
    /// entry does not exist. One call gives R-201 to every method.
    pub fn hit(&self, path: &Path) -> io::Result<Option<u64>> {
        match fs::metadata(path) {
            Ok(meta) if meta.is_file() => {
                touch(path)?;
                Ok(Some(meta.len()))
            }
            Ok(_) => Ok(None),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// Writes an entry so readers never see a partial file: the bytes go to
    /// a sibling temp file and a rename publishes them. Returns the entry
    /// path. The fresh write sets the mtime, so no separate touch is needed.
    pub fn put(&self, sub: Subdir, key: &str, bytes: &[u8]) -> io::Result<PathBuf> {
        let final_path = self.path_for(sub, key);
        let temp_name = format!(
            ".{key}.{}.{}.tmp",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let temp_path = self.subdir(sub).join(temp_name);
        let write = || -> io::Result<()> {
            fs::write(&temp_path, bytes)?;
            fs::rename(&temp_path, &final_path)
        };
        if let Err(err) = write() {
            // Best effort: a leftover temp file is only wasted space and the
            // next gc removes it.
            let _ = fs::remove_file(&temp_path);
            return Err(err);
        }
        Ok(final_path)
    }

    /// Sum of file sizes under the three subdirectories.
    pub fn total_bytes(&self) -> io::Result<u64> {
        Ok(gc::scan(self)?.iter().map(|e| e.bytes).sum())
    }

    /// Deletes the least recently used entries until the cache is under
    /// `max_bytes`, never touching a path in `keep` (R-151, R-152).
    pub fn gc(
        &self,
        max_bytes: u64,
        keep: &std::collections::HashSet<PathBuf>,
    ) -> io::Result<GcResult> {
        gc::run(self, max_bytes, keep)
    }
}

/// Sets the mtime to now (R-201). Opening read-only is enough: on Unix
/// `futimens` needs ownership or write permission on the file, not a
/// writable descriptor.
pub fn touch(path: &Path) -> io::Result<()> {
    File::open(path)?.set_modified(SystemTime::now())
}
