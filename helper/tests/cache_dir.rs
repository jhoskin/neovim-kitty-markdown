//! Cache keys, atomic writes, touch, and LRU gc (R-149 to R-152, R-201).

use std::collections::HashSet;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kitty_markdown_helper::cache::{
    CacheDir, Subdir, key_image, key_image_bytes, key_mermaid, key_remote, sha256_hex, touch,
};
use kitty_markdown_helper::protocol::MERMAID_VERSION;
use sha2::{Digest, Sha256};

fn hex(bytes: impl AsRef<[u8]>) -> String {
    Sha256::digest(bytes.as_ref())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn temp_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("kmh-cache-{name}-{}-{nanos}", std::process::id()))
}

fn set_age(path: &Path, age: Duration) {
    File::open(path)
        .unwrap()
        .set_modified(SystemTime::now() - age)
        .unwrap();
}

fn mtime(path: &Path) -> SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

#[test]
fn sha256_hex_matches_the_sha2_crate() {
    assert_eq!(sha256_hex(b"hello"), hex(b"hello"));
}

#[test]
fn image_key_formula() {
    let bytes = b"\x89PNG fake";
    let inner = hex(bytes);
    assert_eq!(
        key_image(&inner, 800, Some(600), Some(1200)),
        hex(format!("{inner}|800|600|1200"))
    );
    assert_eq!(
        key_image(&inner, 800, None, None),
        hex(format!("{inner}|800|0|0"))
    );
    assert_eq!(
        key_image_bytes(bytes, 800, None, Some(5)),
        hex(format!("{inner}|800|0|5"))
    );
}

#[test]
fn remote_key_is_the_url_hash() {
    let url = "https://example.com/a.png";
    assert_eq!(key_remote(url), hex(url));
}

#[test]
fn mermaid_key_formula() {
    let source = "graph TD\n  A-->B\n";
    let inner = hex(source);
    assert_eq!(
        key_mermaid(source, 640, "dark", MERMAID_VERSION),
        hex(format!("{inner}|640|dark|12.2.0"))
    );
    assert_ne!(
        key_mermaid(source, 640, "dark", "12.3.0"),
        key_mermaid(source, 640, "dark", MERMAID_VERSION),
        "a pin bump must invalidate the cache"
    );
}

#[test]
fn new_creates_the_three_subdirectories() {
    let root = temp_root("new");
    let cache = CacheDir::new(&root).unwrap();
    for sub in ["images", "remote", "mermaid"] {
        assert!(root.join(sub).is_dir(), "{sub}");
    }
    assert_eq!(cache.root(), root);
    assert_eq!(cache.path_image("k"), root.join("images/k.png"));
    assert_eq!(cache.path_remote("k"), root.join("remote/k"));
    assert_eq!(cache.path_mermaid("k"), root.join("mermaid/k.png"));
    // Opening an existing cache is fine too.
    CacheDir::new(&root).unwrap();
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn put_publishes_whole_files_and_leaves_no_temp() {
    let root = temp_root("put");
    let cache = CacheDir::new(&root).unwrap();
    let path = cache.put(Subdir::Mermaid, "abc", b"png bytes").unwrap();
    assert_eq!(path, cache.path_mermaid("abc"));
    assert_eq!(fs::read(&path).unwrap(), b"png bytes");
    let names: Vec<String> = fs::read_dir(root.join("mermaid"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["abc.png".to_string()], "no temp file left");

    // A second put replaces the entry in place.
    cache.put(Subdir::Mermaid, "abc", b"new").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"new");
    assert_eq!(cache.total_bytes().unwrap(), 3);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn touch_and_hit_move_the_mtime_forward() {
    let root = temp_root("touch");
    let cache = CacheDir::new(&root).unwrap();
    let path = cache.put(Subdir::Images, "k", &[1, 2, 3]).unwrap();
    set_age(&path, Duration::from_secs(3600));
    let old = mtime(&path);

    touch(&path).unwrap();
    let touched = mtime(&path);
    assert!(touched > old, "touch must set mtime to now");

    set_age(&path, Duration::from_secs(3600));
    assert_eq!(cache.hit(&path).unwrap(), Some(3));
    assert!(mtime(&path) > old, "hit must touch");
    assert_eq!(cache.hit(&cache.path_image("missing")).unwrap(), None);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn total_bytes_sums_every_subdirectory() {
    let root = temp_root("total");
    let cache = CacheDir::new(&root).unwrap();
    assert_eq!(cache.total_bytes().unwrap(), 0);
    cache.put(Subdir::Images, "a", &[0; 10]).unwrap();
    cache.put(Subdir::Remote, "b", &[0; 20]).unwrap();
    cache.put(Subdir::Mermaid, "c", &[0; 30]).unwrap();
    assert_eq!(cache.total_bytes().unwrap(), 60);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn gc_deletes_oldest_first_until_under_the_limit() {
    let root = temp_root("gc");
    let cache = CacheDir::new(&root).unwrap();
    let oldest = cache.put(Subdir::Images, "oldest", &[0; 100]).unwrap();
    let middle = cache.put(Subdir::Remote, "middle", &[0; 100]).unwrap();
    let newest = cache.put(Subdir::Mermaid, "newest", &[0; 100]).unwrap();
    set_age(&oldest, Duration::from_secs(300));
    set_age(&middle, Duration::from_secs(200));
    set_age(&newest, Duration::from_secs(100));

    let result = cache.gc(250, &HashSet::new()).unwrap();
    assert_eq!(result.deleted_bytes, 100);
    assert_eq!(result.cache_bytes, 200);
    assert!(!oldest.exists());
    assert!(middle.exists());
    assert!(newest.exists());

    // Under the limit: nothing happens.
    let result = cache.gc(200, &HashSet::new()).unwrap();
    assert_eq!(result.deleted_bytes, 0);
    assert_eq!(result.cache_bytes, 200);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn gc_skips_keep_paths() {
    let root = temp_root("gc-keep");
    let cache = CacheDir::new(&root).unwrap();
    let oldest = cache.put(Subdir::Images, "oldest", &[0; 100]).unwrap();
    let middle = cache.put(Subdir::Images, "middle", &[0; 100]).unwrap();
    let newest = cache.put(Subdir::Images, "newest", &[0; 100]).unwrap();
    set_age(&oldest, Duration::from_secs(300));
    set_age(&middle, Duration::from_secs(200));
    set_age(&newest, Duration::from_secs(100));

    let keep: HashSet<PathBuf> = [oldest.clone()].into_iter().collect();
    let result = cache.gc(150, &keep).unwrap();
    assert_eq!(result.deleted_bytes, 200);
    assert_eq!(result.cache_bytes, 100);
    assert!(oldest.exists(), "kept even though it is the oldest");
    assert!(!middle.exists());
    assert!(!newest.exists());

    // When only kept entries remain, gc stops without reaching the limit.
    let result = cache.gc(0, &keep).unwrap();
    assert_eq!(result.deleted_bytes, 0);
    assert_eq!(result.cache_bytes, 100);
    let _ = fs::remove_dir_all(&root);
}
