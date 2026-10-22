//! End-to-end protocol tests against the built binary (R-025, R-137 to
//! R-139, R-149, R-151, R-152).

mod server_support;

use std::fs::{self, File};
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};
use server_support::{Helper, temp_dir};

fn probe_params(cache_dir: Option<&std::path::Path>) -> Value {
    let mut params = json!({"plugin_version": env!("CARGO_PKG_VERSION")});
    if let Some(dir) = cache_dir {
        params["cache_dir"] = json!(dir);
    }
    params
}

#[test]
fn probe_reports_versions_and_creates_cache_dirs() {
    let dir = temp_dir("probe");
    let mut helper = Helper::spawn(Some(&dir), &[]);
    let answer = helper.call(1, "probe", probe_params(None));
    assert_eq!(answer["id"], 1);
    let result = &answer["result"];
    assert_eq!(result["helper_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(result["mermaid_version"], "12.2.0");
    assert_eq!(result["cache_bytes"], 0);
    assert_eq!(result["fonts"], json!({}));
    assert_eq!(result["diagram_types"], json!([]));
    for sub in ["images", "remote", "mermaid"] {
        assert!(dir.join(sub).is_dir(), "{sub} missing");
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn probe_sets_the_cache_dir_when_the_cli_did_not() {
    let dir = temp_dir("probe-late");
    let root = dir.join("cache");
    let mut helper = Helper::spawn(None, &[]);
    let answer = helper.call(1, "probe", probe_params(Some(&root)));
    assert!(answer.get("result").is_some(), "{answer}");
    for sub in ["images", "remote", "mermaid"] {
        assert!(root.join(sub).is_dir(), "{sub} missing");
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cache_gc_without_a_cache_dir_is_an_internal_error() {
    let mut helper = Helper::spawn(None, &[]);
    let answer = helper.call(1, "cache.gc", json!({"max_bytes": 0}));
    assert_eq!(answer["error"]["code"], "internal", "{answer}");
}

#[test]
fn unknown_method_and_malformed_lines_are_answered() {
    let mut helper = Helper::spawn(None, &[]);

    let answer = helper.call(7, "no.such.method", Value::Null);
    assert_eq!(answer["id"], 7);
    assert_eq!(answer["error"]["code"], "unknown_method");
    assert_eq!(answer["error"]["data"], Value::Null);

    helper.send_raw("{this is not json");
    let answer = helper.recv();
    assert_eq!(answer["id"], 0);
    assert_eq!(answer["error"]["code"], "invalid_params");

    helper.send_raw(r#"{"id": 12, "params": {}}"#);
    let answer = helper.recv();
    assert_eq!(answer["id"], 12, "id survives a missing method");
    assert_eq!(answer["error"]["code"], "invalid_params");

    helper.send_raw(r#"{"id": 13, "method": "cache.gc", "params": {"keep": []}}"#);
    let answer = helper.recv();
    assert_eq!(answer["id"], 13);
    assert_eq!(answer["error"]["code"], "invalid_params");
}

#[test]
fn shutdown_answers_empty_and_exits_zero() {
    let helper = Helper::spawn(None, &[]);
    let (answer, status) = helper.shutdown(99);
    assert_eq!(answer, json!({"id": 99, "result": {}}));
    assert!(status.success(), "exit status {status}");
}

#[test]
fn first_answer_arrives_quickly() {
    let dir = temp_dir("startup");
    // R-025 allows 500 ms. The margin covers a loaded machine; a debug build
    // on a laptop answers in a few milliseconds. One sample can still land
    // on a scheduler hiccup while other builds run, so the test measures
    // startup capability as the best of three spawns.
    let mut best = Duration::MAX;
    for id in 1..=3 {
        let mut helper = Helper::spawn(Some(&dir), &[]);
        let answer = helper.call(id, "probe", probe_params(None));
        best = best.min(helper.started.elapsed());
        assert_eq!(answer["id"], id);
        helper.shutdown(id + 10);
    }
    assert!(
        best < Duration::from_millis(400),
        "best first answer of three spawns took {best:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn debug_sleep_is_hidden_without_the_env_var() {
    let mut helper = Helper::spawn(None, &[]);
    let answer = helper.call(1, "debug.sleep", json!({"ms": 1}));
    assert_eq!(answer["error"]["code"], "unknown_method");
}

#[test]
fn answers_leave_in_completion_order_with_their_ids() {
    let mut helper = Helper::spawn(None, &[("KMH_DEBUG_METHODS", "1")]);
    helper.send(1, "debug.sleep", json!({"ms": 300}));
    helper.send(2, "probe", probe_params(None));
    let first = helper.recv();
    let second = helper.recv();
    assert_eq!(first["id"], 2, "probe should overtake the sleeping request");
    assert!(first.get("result").is_some(), "{first}");
    assert_eq!(second, json!({"id": 1, "result": {}}));
}

#[test]
fn shutdown_waits_for_in_flight_work() {
    let mut helper = Helper::spawn(None, &[("KMH_DEBUG_METHODS", "1")]);
    helper.send(1, "debug.sleep", json!({"ms": 200}));
    helper.send(2, "shutdown", Value::Null);
    let first = helper.recv();
    assert_eq!(first, json!({"id": 1, "result": {}}), "sleep answers first");
    let second = helper.recv();
    assert_eq!(
        second,
        json!({"id": 2, "result": {}}),
        "shutdown answers last"
    );
    assert!(helper.wait_exit().success());
}

#[test]
fn cache_gc_deletes_oldest_first_and_keeps_open_paths() {
    let dir = temp_dir("gc");
    let images = dir.join("images");
    fs::create_dir_all(&images).unwrap();
    let now = SystemTime::now();
    let mut paths = Vec::new();
    for (i, name) in ["old.png", "mid.png", "new.png"].iter().enumerate() {
        let path = images.join(name);
        fs::write(&path, vec![0u8; 100]).unwrap();
        // Oldest first: three distinct mtimes, well apart.
        let age = Duration::from_secs(300 - 100 * i as u64);
        File::open(&path).unwrap().set_modified(now - age).unwrap();
        paths.push(path);
    }

    let mut helper = Helper::spawn(Some(&dir), &[]);
    let probe = helper.call(1, "probe", probe_params(None));
    assert_eq!(probe["result"]["cache_bytes"], 300);

    let answer = helper.call(2, "cache.gc", json!({"max_bytes": 150, "keep": [paths[0]]}));
    assert_eq!(
        answer["result"],
        json!({"deleted_bytes": 200, "cache_bytes": 100}),
        "{answer}"
    );
    assert!(paths[0].exists(), "kept path survives");
    assert!(!paths[1].exists(), "oldest unkept goes first");
    assert!(!paths[2].exists(), "newest goes when still over the limit");
    let _ = fs::remove_dir_all(&dir);
}
