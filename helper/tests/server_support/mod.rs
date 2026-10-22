//! Spawns the built helper binary with piped stdio for the server tests.
//! Lives in its own directory (not `tests/common`) so it does not collide
//! with the shared module another package adds.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

pub fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("kmh-{name}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub struct Helper {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    pub started: Instant,
}

impl Helper {
    pub fn spawn(cache_dir: Option<&Path>, env: &[(&str, &str)]) -> Helper {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_kitty-markdown-helper"));
        cmd.arg("serve");
        if let Some(dir) = cache_dir {
            cmd.arg("--cache-dir").arg(dir);
        }
        // The debug method must be opt-in per test, whatever the shell has.
        cmd.env_remove("KMH_DEBUG_METHODS");
        for (key, value) in env {
            cmd.env(key, value);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let started = Instant::now();
        let mut child = cmd.spawn().expect("spawn helper");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Helper {
            child,
            stdin: Some(stdin),
            stdout,
            started,
        }
    }

    pub fn send_raw(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().expect("stdin open");
        stdin.write_all(line.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin.flush().unwrap();
    }

    pub fn send(&mut self, id: u64, method: &str, params: Value) {
        let line = json!({"id": id, "method": method, "params": params}).to_string();
        self.send_raw(&line);
    }

    /// One response line, parsed. Panics at end of output.
    pub fn recv(&mut self) -> Value {
        let mut line = String::new();
        let n = self.stdout.read_line(&mut line).unwrap();
        assert!(n > 0, "helper closed stdout");
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("bad response line {line:?}: {e}"))
    }

    pub fn call(&mut self, id: u64, method: &str, params: Value) -> Value {
        self.send(id, method, params);
        self.recv()
    }

    /// Sends `shutdown`, returns its answer and the exit status.
    pub fn shutdown(mut self, id: u64) -> (Value, ExitStatus) {
        self.send(id, "shutdown", Value::Null);
        let answer = self.recv();
        (answer, self.wait_exit())
    }

    /// Closes stdin and waits for the process to end on its own.
    pub fn wait_exit(mut self) -> ExitStatus {
        drop(self.stdin.take());
        wait_with_timeout(&mut self.child, Duration::from_secs(5))
    }
}

fn wait_with_timeout(child: &mut Child, timeout: Duration) -> ExitStatus {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "helper did not exit after shutdown"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        // A test that failed mid-way must not leave a helper waiting on stdin.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
