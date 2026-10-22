//! The `serve` loop: reader thread, worker pool, one shared writer.
//!
//! The thread that calls [`serve`] reads stdin line by line and feeds a
//! bounded queue; `N` workers take requests, dispatch by method, and each
//! writes its own response line through one locked writer. Responses leave
//! in completion order, which R-139 allows, and every request gets exactly
//! one answer with its own `id`, which R-139 demands. `shutdown` is not a
//! handler: the reader stops on it, the workers drain the queue, and the
//! `{}` answer goes out last so the plugin can wait for exit (R-137).

pub mod builtin;
pub mod mermaid;
pub mod pixels;

use std::collections::HashMap;
use std::io::{self, BufRead, BufWriter, Write};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

pub use builtin::builtin_handlers;

use crate::cache::CacheDir;
use crate::error::HelperError;
use crate::protocol::{Request, Response, peek_id};

pub const SHUTDOWN_METHOD: &str = "shutdown";

/// Requests waiting for a worker. Deep enough that the plugin's render
/// batch never stalls the reader, shallow enough that a flood of requests
/// is bounded by the pipe rather than by memory.
const QUEUE_DEPTH: usize = 256;

pub trait Handler: Send + Sync {
    fn method(&self) -> &'static str;
    fn handle(&self, params: Value, ctx: &Ctx) -> Result<Value, HelperError>;
}

/// Shared state every handler sees. The cache may be unset until the first
/// `probe` names a directory (R-200 lets the CLI run without one).
pub struct Ctx {
    cache: Mutex<Option<CacheDir>>,
}

impl Ctx {
    pub fn new(cache_dir: Option<PathBuf>) -> io::Result<Self> {
        let cache = cache_dir.map(CacheDir::new).transpose()?;
        Ok(Ctx {
            cache: Mutex::new(cache),
        })
    }

    pub fn cache_opt(&self) -> Option<CacheDir> {
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The cache, or an `internal` error naming the fix. Handlers that
    /// write pixels need it; `probe` and the CLI do not.
    pub fn cache(&self) -> Result<CacheDir, HelperError> {
        self.cache_opt().ok_or_else(|| {
            HelperError::internal(
                "no cache directory: start with --cache-dir or send cache_dir in probe",
            )
        })
    }

    /// Adopts `root` when no cache directory is set yet. A root given on the
    /// command line wins over a later `probe`, so the current cache stays.
    pub fn set_cache_dir_if_unset(&self, root: &Path) -> io::Result<CacheDir> {
        let mut guard = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(existing) = guard.as_ref() {
            return Ok(existing.clone());
        }
        let cache = CacheDir::new(root)?;
        *guard = Some(cache.clone());
        Ok(cache)
    }
}

/// Decodes `params`, treating an absent (`null`) object as the default so
/// a handler with only optional parameters accepts `{"id", "method"}`.
pub fn params_or_default<T: DeserializeOwned + Default>(params: Value) -> Result<T, HelperError> {
    if params.is_null() {
        return Ok(T::default());
    }
    Ok(serde_json::from_value(params)?)
}

/// Serializes a handler result. Failure here is a programming error in the
/// result type, not bad input, so it is `internal`.
pub fn to_result<T: Serialize>(value: &T) -> Result<Value, HelperError> {
    serde_json::to_value(value).map_err(|e| HelperError::internal(e.to_string()))
}

type SharedWriter = Arc<Mutex<BufWriter<Box<dyn Write + Send>>>>;
type HandlerMap = HashMap<&'static str, Box<dyn Handler>>;

/// Serves stdin to stdout until `shutdown` or end of input. Returns when
/// every accepted request has been answered.
pub fn serve(handlers: Vec<Box<dyn Handler>>, ctx: Ctx) -> io::Result<()> {
    serve_io(handlers, ctx, io::stdin().lock(), io::stdout())
}

/// [`serve`] over arbitrary streams, for tests that do not want a process.
pub fn serve_io<R, W>(
    handlers: Vec<Box<dyn Handler>>,
    ctx: Ctx,
    input: R,
    output: W,
) -> io::Result<()>
where
    R: BufRead,
    W: Write + Send + 'static,
{
    let handlers: Arc<HandlerMap> =
        Arc::new(handlers.into_iter().map(|h| (h.method(), h)).collect());
    let ctx = Arc::new(ctx);
    let writer: SharedWriter = Arc::new(Mutex::new(BufWriter::new(Box::new(output))));
    let (tx, rx) = mpsc::sync_channel::<Request>(QUEUE_DEPTH);
    let rx = Arc::new(Mutex::new(rx));

    let workers: Vec<_> = (0..worker_count())
        .map(|_| {
            let rx = Arc::clone(&rx);
            let handlers = Arc::clone(&handlers);
            let ctx = Arc::clone(&ctx);
            let writer = Arc::clone(&writer);
            thread::spawn(move || worker(&rx, &handlers, &ctx, &writer))
        })
        .collect();

    let shutdown_id = read_loop(input, tx, &writer)?;
    // `tx` is gone, so each worker exits once the queue is empty; joining
    // them is the "wait for in-flight work" step of shutdown.
    for worker in workers {
        // A worker that panicked already lost nothing: dispatch catches
        // handler panics, and the writer lock recovers from poisoning.
        let _ = worker.join();
    }
    if let Some(id) = shutdown_id {
        write_response(&writer, &Response::ok(id, json!({})))?;
    }
    Ok(())
}

/// Available parallelism clamped to 2..=8: two so a slow render never
/// blocks a probe, eight because renders are memory heavy.
fn worker_count() -> usize {
    thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        .clamp(2, 8)
}

/// Returns the id of the `shutdown` request, or `None` at end of input.
fn read_loop<R: BufRead>(
    mut input: R,
    tx: SyncSender<Request>,
    writer: &SharedWriter,
) -> io::Result<Option<u64>> {
    let mut raw = Vec::new();
    loop {
        raw.clear();
        if input.read_until(b'\n', &mut raw)? == 0 {
            return Ok(None);
        }
        // Lossy so a bad byte in one line costs that line, not the session.
        let line = String::from_utf8_lossy(&raw);
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let request = match serde_json::from_str::<Request>(line) {
            Ok(request) => request,
            Err(err) => {
                let error = HelperError::invalid_params(format!("malformed request: {err}"));
                write_response(writer, &Response::error(peek_id(line), error))?;
                continue;
            }
        };
        if request.method == SHUTDOWN_METHOD {
            return Ok(Some(request.id));
        }
        if tx.send(request).is_err() {
            // Every worker is gone, which only happens when stdout closed
            // under them; there is nobody left to answer.
            return Ok(None);
        }
    }
}

fn worker(rx: &Mutex<Receiver<Request>>, handlers: &HandlerMap, ctx: &Ctx, writer: &SharedWriter) {
    loop {
        let received = rx.lock().unwrap_or_else(PoisonError::into_inner).recv();
        let Ok(request) = received else { break };
        let response = dispatch(handlers, ctx, request);
        if write_response(writer, &response).is_err() {
            // stdout is closed: the plugin is gone, so stop taking work.
            break;
        }
    }
}

fn dispatch(handlers: &HandlerMap, ctx: &Ctx, request: Request) -> Response {
    let Request { id, method, params } = request;
    let Some(handler) = handlers.get(method.as_str()) else {
        return Response::error(id, HelperError::UnknownMethod(method));
    };
    // A handler panic must still produce an answer (R-139); the default
    // panic hook has already written the backtrace to stderr.
    match panic::catch_unwind(AssertUnwindSafe(|| handler.handle(params, ctx))) {
        Ok(Ok(result)) => Response::ok(id, result),
        Ok(Err(err)) => Response::error(id, err),
        Err(payload) => Response::error(
            id,
            HelperError::internal(format!(
                "handler `{method}` panicked: {}",
                panic_message(payload.as_ref())
            )),
        ),
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

/// One `write_all` and one flush per response, under the lock, so lines
/// from different workers never interleave.
fn write_response(writer: &SharedWriter, response: &Response) -> io::Result<()> {
    let mut line = serde_json::to_vec(response).map_err(io::Error::other)?;
    line.push(b'\n');
    let mut out = writer.lock().unwrap_or_else(PoisonError::into_inner);
    out.write_all(&line)?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Collects output through a shared buffer, because `serve_io` owns its
    /// writer and the test needs to read it afterwards.
    #[derive(Clone, Default)]
    struct Sink(Arc<Mutex<Vec<u8>>>);

    impl Write for Sink {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct Echo;
    impl Handler for Echo {
        fn method(&self) -> &'static str {
            "echo"
        }
        fn handle(&self, params: Value, _ctx: &Ctx) -> Result<Value, HelperError> {
            Ok(params)
        }
    }

    struct Boom;
    impl Handler for Boom {
        fn method(&self) -> &'static str {
            "boom"
        }
        fn handle(&self, _params: Value, _ctx: &Ctx) -> Result<Value, HelperError> {
            panic!("kaboom")
        }
    }

    fn run(input: &str) -> Vec<Value> {
        let sink = Sink::default();
        let ctx = Ctx::new(None).unwrap();
        serve_io(
            vec![Box::new(Echo), Box::new(Boom)],
            ctx,
            Cursor::new(input.as_bytes().to_vec()),
            sink.clone(),
        )
        .unwrap();
        let bytes = sink.0.lock().unwrap().clone();
        String::from_utf8(bytes)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn by_id(responses: &[Value], id: u64) -> &Value {
        responses
            .iter()
            .find(|r| r["id"] == json!(id))
            .unwrap_or_else(|| panic!("no response with id {id}"))
    }

    #[test]
    fn answers_every_request_with_its_id() {
        let out = run(concat!(
            "{\"id\":1,\"method\":\"echo\",\"params\":{\"x\":1}}\n",
            "{\"id\":2,\"method\":\"nope\"}\n",
            "not json\n",
            "{\"id\":4}\n",
            "\n",
            "{\"id\":5,\"method\":\"boom\"}\n",
            "{\"id\":6,\"method\":\"shutdown\"}\n",
            "{\"id\":7,\"method\":\"echo\"}\n",
        ));
        assert_eq!(out.len(), 6, "{out:?}");
        assert_eq!(by_id(&out, 1)["result"], json!({"x": 1}));
        assert_eq!(by_id(&out, 2)["error"]["code"], "unknown_method");
        assert_eq!(by_id(&out, 0)["error"]["code"], "invalid_params");
        assert_eq!(by_id(&out, 4)["error"]["code"], "invalid_params");
        assert_eq!(by_id(&out, 5)["error"]["code"], "internal");
        assert!(
            by_id(&out, 5)["error"]["message"]
                .as_str()
                .unwrap()
                .contains("kaboom")
        );
        assert_eq!(out.last().unwrap(), &json!({"id": 6, "result": {}}));
    }

    #[test]
    fn end_of_input_without_shutdown_returns() {
        let out = run("{\"id\":1,\"method\":\"echo\",\"params\":[]}\n");
        assert_eq!(out, vec![json!({"id": 1, "result": []})]);
    }

    #[test]
    fn cache_dir_is_set_once() {
        let root = std::env::temp_dir().join(format!("kmh-ctx-{}", std::process::id()));
        let ctx = Ctx::new(None).unwrap();
        assert!(ctx.cache().is_err());
        let first = ctx.set_cache_dir_if_unset(&root.join("a")).unwrap();
        let second = ctx.set_cache_dir_if_unset(&root.join("b")).unwrap();
        assert_eq!(first, second);
        assert_eq!(ctx.cache().unwrap().root(), root.join("a"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
