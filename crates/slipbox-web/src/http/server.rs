use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::ReadingBridge;
use crate::assets::{Assets, StaticResponse};
use crate::http::response::ApiError;
use crate::http::routes;
use crate::http::wire::{self, HeadError, RequestHead, Verb, WireResponse};

const JSON: &str = "application/json; charset=utf-8";
/// The verbs a reading route answers, as the `Allow` header spells them.
const ALLOWED_METHODS: &str = "GET, HEAD";
/// API bodies are live index state, never revalidatable: a note read after a
/// re-index must reach the daemon rather than a browser or proxy cache.
const NO_STORE: &str = "no-store";
/// How long to wait between the connections that release parked workers during
/// shutdown.
const WAKE_INTERVAL: Duration = Duration::from_millis(2);

/// A running read-only HTTP reading server.
///
/// `GET /api/...` is answered from the daemon and every other path from the
/// embedded reading client, over one port. One connection carries one request: a
/// worker reads the head under a size and time bound, never reads a request body,
/// writes a `Content-Length`-framed reply, and closes.
pub struct ReadingServer {
    /// Cleared to stop the pool; a worker reads it around each accept.
    running: Arc<AtomicBool>,
    /// Workers still inside the serve loop. Shutdown wakes the pool until this
    /// reaches zero, since one wake connection releases whichever worker the OS
    /// chooses rather than a particular one.
    live: Arc<AtomicUsize>,
    /// An option so retiring the pool can take sole ownership of the bridge and
    /// shut it down, from either `shutdown` or `Drop`.
    bridge: Option<Arc<ReadingBridge>>,
    workers: Vec<JoinHandle<()>>,
    local_addr: SocketAddr,
}

impl ReadingServer {
    /// Bind `addr`, spawn `workers` request threads, and start serving.
    ///
    /// The bridge is expected to already front a read-only daemon. An empty
    /// asset set serves the API alone. `workers` is clamped to at least one.
    pub fn start(
        addr: SocketAddr,
        bridge: ReadingBridge,
        assets: Assets,
        workers: usize,
    ) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)
            .map_err(|error| io::Error::other(format!("failed to bind {addr}: {error}")))?;
        // With port 0 the OS assigns a port, so the concrete address has to be
        // resolved before any caller or shutdown wake can reach the server.
        let local_addr = listener.local_addr()?;

        let listener = Arc::new(listener);
        let running = Arc::new(AtomicBool::new(true));
        let bridge = Arc::new(bridge);
        let assets = Arc::new(assets);
        let worker_count = workers.max(1);
        let live = Arc::new(AtomicUsize::new(worker_count));
        let mut handles = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let listener = Arc::clone(&listener);
            let running = Arc::clone(&running);
            let live = Arc::clone(&live);
            let bridge = Arc::clone(&bridge);
            let assets = Arc::clone(&assets);
            handles.push(thread::spawn(move || {
                // A guard, not a decrement after the call, so a worker that
                // unwinds still reports having left the pool; shutdown would
                // otherwise wait forever on a thread that is already gone.
                let _leaving = Leaving(live);
                serve_loop(&listener, &running, &bridge, &assets);
            }));
        }

        Ok(Self {
            running,
            live,
            bridge: Some(bridge),
            workers: handles,
            local_addr,
        })
    }

    /// The address the server is listening on, with the port resolved.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Stop serving: retire the pool and shut the daemon down.
    pub fn shutdown(mut self) -> Result<(), crate::ReadingBridgeError> {
        self.retire()
    }

    /// Retire the pool and the daemon behind it, idempotently.
    ///
    /// Clearing `running` ends a serve loop, but a worker parked in `accept`
    /// reads the flag only once a connection arrives, so the pool is woken by
    /// connecting to it until every worker has left and the bridge is uniquely
    /// owned again.
    fn retire(&mut self) -> Result<(), crate::ReadingBridgeError> {
        self.running.store(false, Ordering::SeqCst);
        // Which worker takes a wake connection is the OS's choice, so waking is
        // driven by the count still in the loop rather than by one connection
        // each. A refused or failed connection costs another turn.
        while self.live.load(Ordering::SeqCst) > 0 {
            let _ = TcpStream::connect(self.local_addr);
            thread::sleep(WAKE_INTERVAL);
        }
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
        match self.bridge.take().and_then(Arc::into_inner) {
            Some(bridge) => bridge.shutdown(),
            None => Ok(()),
        }
    }
}

impl Drop for ReadingServer {
    fn drop(&mut self) {
        // A destructor has nowhere to report a shutdown failure, so callers that
        // care use `shutdown` instead.
        let _ = self.retire();
    }
}

impl std::fmt::Debug for ReadingServer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReadingServer")
            .field("local_addr", &self.local_addr)
            .field("workers", &self.workers.len())
            .finish_non_exhaustive()
    }
}

/// Counts one worker out of the pool when its thread ends, however it ends.
struct Leaving(Arc<AtomicUsize>);

impl Drop for Leaving {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// One worker's serve loop: accept connections until the pool is stopped,
/// answering each and closing it. The flag is read again after the accept, since
/// the connection that released the worker may be a shutdown wake whose head
/// would never arrive.
fn serve_loop(
    listener: &TcpListener,
    running: &AtomicBool,
    bridge: &ReadingBridge,
    assets: &Assets,
) {
    while running.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                handle(bridge, assets, running, stream);
            }
            Err(_) => continue,
        }
    }
}

/// Answer one connection: read the head under its bounds, dispatch, reply, close.
/// A head that never arrives, overruns its bound, or declares a body is answered
/// from the failure alone, without running a route.
fn handle(bridge: &ReadingBridge, assets: &Assets, running: &AtomicBool, mut stream: TcpStream) {
    let (verb, reply) = match wire::read_head(&stream, running) {
        Ok(head) => (head.verb, answer(bridge, assets, &head)),
        // The verb is unknown on this path, but the reply still has to carry its
        // body, so it is framed as a `GET` reply rather than a `HEAD` one.
        Err(error) => (Some(Verb::Get), Reply::from(head_error(&error))),
    };
    let _ = wire::write_response(&mut stream, verb, &reply.into_wire());
}

/// The reply a well-formed head earns: either an API route or a static asset.
/// `HEAD` is routed exactly like `GET`, and the wire layer withholds the body.
fn answer(bridge: &ReadingBridge, assets: &Assets, head: &RequestHead) -> Reply {
    if head.verb.is_none() {
        return Reply::from(ApiError::method_not_allowed());
    }
    let (path, raw_query) = split_url(&head.target);
    if is_api_path(&path) {
        match routes::dispatch(bridge, &path, &raw_query) {
            Ok(response) => Reply::json(200, response.body),
            Err(error) => Reply::from(error),
        }
    } else {
        serve_static(assets, &path)
    }
}

/// The error a head this surface will not read maps to.
fn head_error(error: &HeadError) -> ApiError {
    match error {
        // 413 says the request is the problem: a reading route consumes no body
        // at any size.
        HeadError::BodyNotAllowed => ApiError::payload_too_large(),
        HeadError::Malformed | HeadError::Incomplete => {
            ApiError::bad_request("the reading surface could not read a well-formed HTTP request")
        }
    }
}

/// True for a path the JSON API owns: the `/api` root and everything beneath it,
/// which answers in the JSON envelope rather than the app shell.
fn is_api_path(path: &str) -> bool {
    path == "/api" || path.starts_with("/api/")
}

/// Resolve a non-API path against the embedded client.
fn serve_static(assets: &Assets, path: &str) -> Reply {
    match assets.resolve(path) {
        Some(response) => Reply::asset(&response),
        None => Reply::not_found(),
    }
}

/// A fully-rendered reply, leaving the wire layer only the framing.
struct Reply {
    status: u16,
    content_type: &'static str,
    headers: Vec<(&'static str, &'static str)>,
    body: Vec<u8>,
}

impl Reply {
    fn json(status: u16, body: String) -> Self {
        Self {
            status,
            content_type: JSON,
            headers: vec![("Cache-Control", NO_STORE)],
            body: body.into_bytes(),
        }
    }

    fn asset(response: &StaticResponse<'_>) -> Self {
        Self {
            status: 200,
            content_type: response.content_type,
            headers: vec![("Cache-Control", response.cache_control)],
            body: response.bytes.to_vec(),
        }
    }

    /// A not-found for a non-API path that resolved to nothing. Text rather than
    /// the JSON API envelope, since it is not an API result.
    fn not_found() -> Self {
        Self {
            status: 404,
            content_type: "text/plain; charset=utf-8",
            headers: vec![("Cache-Control", NO_STORE)],
            body: b"not found".to_vec(),
        }
    }

    fn into_wire(self) -> WireResponse {
        WireResponse {
            status: self.status,
            content_type: self.content_type,
            headers: self.headers,
            body: self.body,
        }
    }
}

impl From<ApiError> for Reply {
    fn from(error: ApiError) -> Self {
        let mut reply = Self::json(error.status, error.body());
        // HTTP requires a 405 to carry `Allow`.
        if error.status == 405 {
            reply.headers.push(("Allow", ALLOWED_METHODS));
        }
        reply
    }
}

/// Split a request target into its path and raw query string.
fn split_url(url: &str) -> (String, String) {
    match url.split_once('?') {
        Some((path, query)) => (path.to_owned(), query.to_owned()),
        None => (url.to_owned(), String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::{ALLOWED_METHODS, Reply, head_error, is_api_path, split_url};
    use crate::http::response::ApiError;
    use crate::http::wire::HeadError;

    #[test]
    fn split_url_separates_path_and_query() {
        assert_eq!(
            split_url("/api/node?key=file:alpha.org"),
            ("/api/node".to_owned(), "key=file:alpha.org".to_owned())
        );
    }

    #[test]
    fn split_url_without_query_yields_empty_query() {
        assert_eq!(
            split_url("/api/random"),
            ("/api/random".to_owned(), String::new())
        );
    }

    #[test]
    fn split_url_keeps_empty_query_after_bare_question_mark() {
        assert_eq!(
            split_url("/api/status?"),
            ("/api/status".to_owned(), String::new())
        );
    }

    #[test]
    fn api_paths_are_the_api_root_and_everything_under_it() {
        assert!(is_api_path("/api/status"));
        assert!(is_api_path("/api/glossary/terms"));
        assert!(is_api_path("/api"));
        assert!(!is_api_path("/"));
        assert!(!is_api_path("/assets/index-abc123.js"));
        assert!(!is_api_path("/some/reading/path"));
    }

    #[test]
    fn a_method_not_allowed_reply_names_the_verbs_it_serves() {
        let reply = Reply::from(ApiError::method_not_allowed());
        assert_eq!(reply.status, 405);
        assert!(reply.headers.contains(&("Allow", ALLOWED_METHODS)));
    }

    #[test]
    fn json_replies_are_never_stored_by_a_cache() {
        let reply = Reply::json(200, "{}".to_owned());
        assert!(reply.headers.contains(&("Cache-Control", "no-store")));
    }

    #[test]
    fn a_declared_body_is_refused_as_a_payload_the_surface_will_not_read() {
        assert_eq!(head_error(&HeadError::BodyNotAllowed).status, 413);
    }

    #[test]
    fn a_malformed_or_stalled_head_is_a_bad_request() {
        assert_eq!(head_error(&HeadError::Malformed).status, 400);
        assert_eq!(head_error(&HeadError::Incomplete).status, 400);
    }
}
