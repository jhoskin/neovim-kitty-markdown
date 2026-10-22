//! `fetch::fetch` and the `fetch` handler against a loopback HTTP server
//! (R-079 to R-084, R-173). Nothing here reaches the network.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use kitty_markdown_helper::fetch::{self, Error, fetch, handler};
use serde_json::json;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_dir(name: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "kitty-markdown-fetch-{}-{n}-{name}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

const IMAGE_BODY: &[u8] = b"\x89PNG\r\n\x1a\nnot really a png but bytes are bytes";

/// A one-thread HTTP/1.0-style responder. Each path has one behavior; the
/// request count is observable so the cache tests can prove no request
/// happened.
struct Server {
    base: String,
    requests: Arc<AtomicUsize>,
}

fn read_request(stream: &mut TcpStream) -> String {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    while !buf.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => buf.push(byte[0]),
            _ => break,
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}

fn respond(stream: &mut TcpStream, status: &str, headers: &[String], body: &[u8]) {
    let mut head = format!("HTTP/1.1 {status}\r\nConnection: close\r\n");
    for h in headers {
        head.push_str(h);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn start_server() -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("addr");
    let requests = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&requests);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let counter = Arc::clone(&counter);
            thread::spawn(move || {
                let request = read_request(&mut stream);
                counter.fetch_add(1, Ordering::SeqCst);
                let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
                match path.as_str() {
                    "/image.png" => respond(
                        &mut stream,
                        "200 OK",
                        &[
                            format!("Content-Length: {}", IMAGE_BODY.len()),
                            "Content-Type: image/png".to_string(),
                        ],
                        IMAGE_BODY,
                    ),
                    "/slow" => {
                        thread::sleep(Duration::from_millis(1500));
                        respond(
                            &mut stream,
                            "200 OK",
                            &[format!("Content-Length: {}", IMAGE_BODY.len())],
                            IMAGE_BODY,
                        )
                    }
                    "/slow-body" => {
                        // Headers arrive at once; the body trickles in past
                        // the client timeout.
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                            IMAGE_BODY.len()
                        );
                        let _ = stream.write_all(head.as_bytes());
                        let _ = stream.write_all(&IMAGE_BODY[..4]);
                        let _ = stream.flush();
                        thread::sleep(Duration::from_millis(1500));
                        let _ = stream.write_all(&IMAGE_BODY[4..]);
                    }
                    "/big-header" => respond(
                        &mut stream,
                        "200 OK",
                        &["Content-Length: 1000000".to_string()],
                        &[0u8; 16],
                    ),
                    "/big-body" => {
                        // Chunked, so there is no Content-Length to reject
                        // early; the cap must trip while streaming.
                        let head = "HTTP/1.1 200 OK\r\nConnection: close\r\nTransfer-Encoding: chunked\r\n\r\n";
                        let _ = stream.write_all(head.as_bytes());
                        let chunk = vec![b'x'; 4096];
                        for _ in 0..64 {
                            let _ = stream.write_all(format!("{:x}\r\n", chunk.len()).as_bytes());
                            let _ = stream.write_all(&chunk);
                            let _ = stream.write_all(b"\r\n");
                        }
                        let _ = stream.write_all(b"0\r\n\r\n");
                    }
                    "/redirect" => respond(
                        &mut stream,
                        "302 Found",
                        &[
                            "Location: /image.png".to_string(),
                            "Content-Length: 0".to_string(),
                        ],
                        &[],
                    ),
                    _ => respond(
                        &mut stream,
                        "404 Not Found",
                        &["Content-Length: 9".to_string()],
                        b"not found",
                    ),
                }
            });
        }
    });
    Server {
        base: format!("http://{addr}"),
        requests,
    }
}

impl Server {
    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    fn count(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }
}

const TIMEOUT: u64 = 5000;
const CAP: u64 = 1 << 20;

#[test]
fn fetch_writes_the_body_under_remote_by_url_hash() {
    let server = start_server();
    let cache = temp_dir("ok");
    let url = server.url("/image.png");
    let got = fetch(&url, TIMEOUT, CAP, &cache).expect("fetch");
    assert!(!got.cached);
    assert_eq!(got.bytes, IMAGE_BODY.len() as u64);
    assert_eq!(got.path, fetch::cache_path(&cache, &url));
    assert_eq!(got.path.parent(), Some(cache.join("remote").as_path()));
    assert!(
        got.path.extension().is_none(),
        "no extension: {:?}",
        got.path
    );
    let name = got.path.file_name().and_then(|n| n.to_str()).expect("name");
    assert_eq!(name.len(), 64, "sha256 hex");
    assert!(name.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(std::fs::read(&got.path).expect("entry"), IMAGE_BODY);
    assert_eq!(server.count(), 1);
}

#[test]
fn second_fetch_is_served_from_cache_without_a_request() {
    let server = start_server();
    let cache = temp_dir("cached");
    let url = server.url("/image.png");
    let first = fetch(&url, TIMEOUT, CAP, &cache).expect("first");
    assert_eq!(server.count(), 1);

    let old = std::time::SystemTime::now() - Duration::from_secs(3600);
    std::fs::File::open(&first.path)
        .expect("open")
        .set_modified(old)
        .expect("mtime");

    let second = fetch(&url, TIMEOUT, CAP, &cache).expect("second");
    assert!(second.cached);
    assert_eq!(second.path, first.path);
    assert_eq!(second.bytes, first.bytes);
    assert_eq!(server.count(), 1, "no second request");
    let mtime = std::fs::metadata(&second.path)
        .expect("meta")
        .modified()
        .expect("mtime");
    assert!(
        mtime > old + Duration::from_secs(60),
        "hit touched the entry"
    );
}

#[test]
fn slow_response_is_timeout() {
    let server = start_server();
    let cache = temp_dir("timeout");
    let err = fetch(&server.url("/slow"), 300, CAP, &cache).unwrap_err();
    assert_eq!(err.code(), "timeout", "{err}");
    assert!(matches!(err, Error::Timeout { timeout_ms: 300 }));
    assert_eq!(err.data(), None);
    assert!(!cache.join("remote").exists() || dir_is_empty(&cache.join("remote")));
}

#[test]
fn slow_body_is_timeout() {
    let server = start_server();
    let cache = temp_dir("timeout-body");
    let err = fetch(&server.url("/slow-body"), 300, CAP, &cache).unwrap_err();
    assert_eq!(err.code(), "timeout", "{err}");
}

#[test]
fn content_length_over_cap_is_too_large() {
    let server = start_server();
    let cache = temp_dir("big-header");
    let err = fetch(&server.url("/big-header"), TIMEOUT, 1000, &cache).unwrap_err();
    assert_eq!(err.code(), "too_large");
    assert_eq!(
        err.data(),
        Some(json!({ "bytes": 1_000_000, "max_bytes": 1000 }))
    );
}

#[test]
fn streamed_body_over_cap_is_too_large() {
    let server = start_server();
    let cache = temp_dir("big-body");
    let err = fetch(&server.url("/big-body"), TIMEOUT, 10_000, &cache).unwrap_err();
    assert_eq!(err.code(), "too_large", "{err}");
    match err {
        Error::TooLarge { bytes, max_bytes } => {
            assert_eq!(max_bytes, 10_000);
            assert!(bytes > 10_000, "bytes counted so far: {bytes}");
            assert!(bytes < 64 * 4096, "stopped before the whole body: {bytes}");
        }
        other => panic!("{other:?}"),
    }
    assert!(!cache.join("remote").exists() || dir_is_empty(&cache.join("remote")));
}

#[test]
fn not_found_is_fetch_failed_with_status() {
    let server = start_server();
    let cache = temp_dir("404");
    let err = fetch(&server.url("/missing.png"), TIMEOUT, CAP, &cache).unwrap_err();
    assert_eq!(err.code(), "fetch_failed");
    assert_eq!(err.data(), Some(json!({ "status": 404 })));
    assert!(err.to_string().contains("404"));
}

#[test]
fn redirect_is_followed() {
    let server = start_server();
    let cache = temp_dir("redirect");
    let got = fetch(&server.url("/redirect"), TIMEOUT, CAP, &cache).expect("redirect");
    assert_eq!(std::fs::read(&got.path).expect("entry"), IMAGE_BODY);
}

#[test]
fn connection_refused_is_fetch_failed_with_status_zero() {
    // Bind and drop to find a port nothing listens on.
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").expect("bind");
        l.local_addr().expect("addr").port()
    };
    let cache = temp_dir("refused");
    let err = fetch(
        &format!("http://127.0.0.1:{port}/x.png"),
        TIMEOUT,
        CAP,
        &cache,
    )
    .unwrap_err();
    assert_eq!(err.code(), "fetch_failed", "{err}");
    assert_eq!(err.data(), Some(json!({ "status": 0 })));
}

#[test]
fn unsupported_scheme_is_fetch_failed() {
    let cache = temp_dir("scheme");
    for url in [
        "file:///etc/passwd",
        "ftp://example.invalid/a.png",
        "data:image/png;base64,AAAA",
    ] {
        let err = fetch(url, TIMEOUT, CAP, &cache).unwrap_err();
        assert_eq!(err.code(), "fetch_failed", "{url}");
        assert_eq!(err.data(), Some(json!({ "status": 0 })));
        assert!(err.to_string().contains("scheme"), "{url}: {err}");
    }
}

#[test]
fn handler_returns_path_bytes_cached() {
    let server = start_server();
    let cache = temp_dir("handler");
    let params =
        json!({ "url": server.url("/image.png"), "timeout_ms": TIMEOUT, "max_bytes": CAP });
    let first = handler::handle(params.clone(), &cache).expect("first");
    assert_eq!(first["cached"], json!(false));
    assert_eq!(first["bytes"], json!(IMAGE_BODY.len()));
    let path = PathBuf::from(first["path"].as_str().expect("path"));
    assert!(path.starts_with(cache.join("remote")));

    let second = handler::handle(params, &cache).expect("second");
    assert_eq!(second["cached"], json!(true));
    assert_eq!(second["path"], first["path"]);
    assert_eq!(server.count(), 1);

    let err = handler::handle(json!({ "url": "http://x" }), &cache).unwrap_err();
    assert_eq!(err.code(), "invalid_params");
}

fn dir_is_empty(dir: &std::path::Path) -> bool {
    std::fs::read_dir(dir)
        .map(|mut d| d.next().is_none())
        .unwrap_or(true)
}
