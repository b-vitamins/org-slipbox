//! The HTTP/1.1 wire layer: read one request head, write one framed response.
//!
//! The reading surface answers a fixed, tiny protocol — `GET` over loopback, one
//! request per connection, every response framed by `Content-Length` — so the
//! transport is owned here rather than delegated. That keeps three properties
//! structural rather than hoped for:
//!
//! - **No request body is ever read.** A reading route has no body to consume,
//!   so a request that declares one is refused on its face and the connection is
//!   closed. Nothing derived from a client-supplied length reaches an allocation.
//! - **Every read is bounded in size and in time.** A head is capped at
//!   [`MAX_HEAD_BYTES`], and it is capped in time twice: a short
//!   [`FIRST_BYTE_TIMEOUT`] grace for a connection that has said nothing, then a
//!   generous [`IO_TIMEOUT`] once it starts speaking. A client that connects and
//!   stalls therefore costs a worker a moment, not the window a real request is
//!   owed, so more silent sockets than workers cannot hold the surface shut.
//! - **A worker is never parked indefinitely.** Every wait is sliced, and the
//!   pool's serving flag is read between slices, so those bounds also let the
//!   pool retire without waiting on a client that has gone quiet.
//!
//! Only the request line and headers are parsed, and only the three header
//! fields the surface acts on are inspected. Everything else in the head is
//! read, bounded, and discarded.

use std::io::{self, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Largest request head (request line plus headers) the surface will read.
/// Anything larger is refused rather than buffered.
const MAX_HEAD_BYTES: usize = 8 * 1024;

/// Longest a connection that has begun its head may take to finish it, and
/// longest a response write may block.
const IO_TIMEOUT: Duration = Duration::from_secs(10);

/// Longest a fresh connection may stay silent before the worker gives it up, so
/// opening more sockets than there are workers cannot hold the surface shut.
const FIRST_BYTE_TIMEOUT: Duration = Duration::from_millis(500);

/// How long one read waits before the deadline and the serving flag are checked
/// again, so a silent socket does not delay shutdown by the head deadline.
const POLL_SLICE: Duration = Duration::from_millis(50);

/// The verb of a request the reading surface can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verb {
    Get,
}

/// One request head, reduced to what the reading surface acts on.
#[derive(Debug)]
pub(crate) struct RequestHead {
    /// The request verb, or `None` for a verb this surface does not serve.
    pub(crate) verb: Option<Verb>,
    /// The request target exactly as sent, path and query together.
    pub(crate) target: String,
}

/// Why a request could not be read far enough to answer it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HeadError {
    /// The head was not valid HTTP/1.x, or exceeded [`MAX_HEAD_BYTES`].
    Malformed,
    /// The request declared a body, which reading one would mean trusting a
    /// client-supplied length for.
    BodyNotAllowed,
    /// The connection failed, ended, or ran past a deadline before a whole head
    /// arrived. Carries no error: every such cause is answered the same way.
    Incomplete,
}

/// Read one request head from `stream`, bounded in both size and time.
///
/// A head that declares a body by `Content-Length`, `Transfer-Encoding`, or
/// `Expect: 100-continue` is [`HeadError::BodyNotAllowed`]. `serving` is the
/// pool's flag; a read waiting on a silent socket gives up when it clears.
pub(crate) fn read_head(
    stream: &TcpStream,
    serving: &AtomicBool,
) -> Result<RequestHead, HeadError> {
    // Both bounds are in force before any byte moves, and a socket that will not
    // take a timeout is refused rather than read unbounded.
    stream
        .set_read_timeout(Some(POLL_SLICE))
        .map_err(|_| HeadError::Incomplete)?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(|_| HeadError::Incomplete)?;

    // `BufReader` is capped at the head bound, and each line read is capped
    // again below, so a client cannot make this buffer grow.
    let mut reader = BufReader::with_capacity(MAX_HEAD_BYTES, stream);
    let mut limit = Limit::new(serving);

    let request_line = read_line(&mut reader, &mut limit)?;
    let (verb, target) = parse_request_line(&request_line).ok_or(HeadError::Malformed)?;

    let mut declares_body = false;
    loop {
        let line = read_line(&mut reader, &mut limit)?;
        // The empty line ends the head.
        if line.is_empty() {
            break;
        }
        declares_body |= header_declares_body(&line);
    }

    if declares_body {
        return Err(HeadError::BodyNotAllowed);
    }
    Ok(RequestHead { verb, target })
}

/// What one head is allowed to consume: bytes, time, and a still-serving pool.
///
/// The bounds are spent together across the whole head rather than per line, so a
/// client cannot buy more of any of them by splitting its head up.
struct Limit<'a> {
    bytes: usize,
    deadline: Instant,
    /// Cleared by the first byte, at which point the deadline is extended.
    silent: bool,
    serving: &'a AtomicBool,
}

impl<'a> Limit<'a> {
    fn new(serving: &'a AtomicBool) -> Self {
        Self {
            bytes: MAX_HEAD_BYTES,
            deadline: Instant::now() + FIRST_BYTE_TIMEOUT,
            silent: true,
            serving,
        }
    }

    /// Whether another byte may be read. Exhausted bytes are a malformed head,
    /// while a passed deadline or a stopped pool means the head never arrived.
    fn check(&self) -> Result<(), HeadError> {
        if self.bytes == 0 {
            return Err(HeadError::Malformed);
        }
        if !self.serving.load(Ordering::SeqCst) || Instant::now() >= self.deadline {
            return Err(HeadError::Incomplete);
        }
        Ok(())
    }

    /// Charge one byte that arrived, promoting the deadline on the first.
    fn spend(&mut self) {
        self.bytes -= 1;
        if self.silent {
            self.silent = false;
            self.deadline = Instant::now() + IO_TIMEOUT;
        }
    }
}

/// Read one LF-terminated line, spending from the head's limit and trimming the
/// terminator along with a preceding CR. A line that outruns the limit is refused
/// rather than truncated.
fn read_line(
    reader: &mut BufReader<&TcpStream>,
    limit: &mut Limit<'_>,
) -> Result<String, HeadError> {
    let mut line = Vec::new();
    loop {
        limit.check()?;
        let mut byte = [0_u8; 1];
        match reader.read(&mut byte) {
            // The peer closed mid-head, or never finished sending one.
            Ok(0) => return Err(HeadError::Incomplete),
            Ok(_) => {}
            // An expired poll slice or a signal is not the head's deadline: the
            // limit decides whether there is still time.
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) =>
            {
                continue;
            }
            Err(_) => return Err(HeadError::Incomplete),
        }
        limit.spend();
        if byte[0] == b'\n' {
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            // A header line is a byte string on the wire, and the surface acts
            // only on ASCII field names and an ASCII-safe target, so a non-UTF-8
            // line is malformed rather than lossily decoded.
            return String::from_utf8(line).map_err(|_| HeadError::Malformed);
        }
        line.push(byte[0]);
    }
}

/// Split a request line into the verb this surface recognizes and the target. An
/// unrecognized verb yields `None` for the verb but keeps the target, so the
/// caller can answer `405`. Only HTTP/1.0 and HTTP/1.1 are accepted.
fn parse_request_line(line: &str) -> Option<(Option<Verb>, String)> {
    let mut parts = line.split(' ');
    let method = parts.next()?;
    let target = parts.next()?;
    let version = parts.next()?;
    if parts.next().is_some() || target.is_empty() {
        return None;
    }
    if !matches!(version, "HTTP/1.1" | "HTTP/1.0") {
        return None;
    }
    // Verb comparison is case-sensitive: HTTP methods are case-sensitive tokens,
    // so `get` is an unknown method rather than a spelling of `GET`.
    let verb = match method {
        "GET" => Some(Verb::Get),
        _ => None,
    };
    Some((verb, target.to_owned()))
}

/// Whether a header line announces a request body, in any of the three ways a
/// client can: a length, a transfer encoding, or asking leave to send one.
///
/// A zero `Content-Length` announces no body and is allowed, so a client library
/// that always sets the field can still read. An unparseable length declares a
/// body, since it is not a head this surface can reason about.
fn header_declares_body(line: &str) -> bool {
    let Some((name, value)) = line.split_once(':') else {
        return false;
    };
    let value = value.trim();
    if name.eq_ignore_ascii_case("Content-Length") {
        return value.parse::<u64>().ok().is_none_or(|length| length > 0);
    }
    if name.eq_ignore_ascii_case("Transfer-Encoding") {
        return !value.is_empty();
    }
    if name.eq_ignore_ascii_case("Expect") {
        return value.eq_ignore_ascii_case("100-continue");
    }
    false
}

pub(crate) struct WireResponse {
    pub(crate) status: u16,
    pub(crate) content_type: &'static str,
    pub(crate) headers: Vec<(&'static str, &'static str)>,
    pub(crate) body: Vec<u8>,
}

/// Write one response and close the connection.
///
/// The body is always fully materialized, so `Content-Length` is always known
/// and no chunked encoding is needed. `Connection: close` is unconditional: one
/// request per connection means a worker never waits on a socket for a request
/// that may never come.
pub(crate) fn write_response(stream: &mut TcpStream, response: &WireResponse) -> io::Result<()> {
    let reason = reason_phrase(response.status);
    let mut head = format!(
        "HTTP/1.1 {} {reason}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n",
        response.status,
        response.content_type,
        response.body.len()
    );
    for (name, value) in &response.headers {
        head.push_str(name);
        head.push_str(": ");
        head.push_str(value);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");

    stream.write_all(head.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}

/// The reason phrase for a status the reading surface sends. The phrase is
/// advisory in HTTP/1.1, so an unlisted status gets a generic one.
fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
    use std::sync::atomic::AtomicBool;
    use std::thread;

    use super::{
        HeadError, RequestHead, Verb, header_declares_body, parse_request_line, read_head,
        reason_phrase,
    };

    /// Send `request` verbatim over a loopback socket and read the head back.
    fn read_over_socket(request: &str) -> Result<RequestHead, HeadError> {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .expect("loopback should bind");
        let addr = listener.local_addr().expect("the port resolves");
        let bytes = request.as_bytes().to_vec();
        let writer = thread::spawn(move || {
            let mut stream = TcpStream::connect(addr).expect("the listener is accepting");
            // A refused head closes the socket early, so a short write is expected.
            let _ = stream.write_all(&bytes);
            let _ = stream.flush();
        });
        let (stream, _) = listener.accept().expect("the writer connects");
        let head = read_head(&stream, &AtomicBool::new(true));
        drop(stream);
        writer.join().expect("the writer should not panic");
        head
    }

    #[test]
    fn a_get_request_line_yields_its_verb_and_target() {
        let (verb, target) =
            parse_request_line("GET /api/node?key=file:a.org HTTP/1.1").expect("well-formed");
        assert_eq!(verb, Some(Verb::Get));
        assert_eq!(target, "/api/node?key=file:a.org");
    }

    #[test]
    fn an_unserved_verb_keeps_its_target_so_the_reply_can_name_what_is_allowed() {
        let (verb, target) = parse_request_line("POST /api/node HTTP/1.1").expect("well-formed");
        assert_eq!(verb, None);
        assert_eq!(target, "/api/node");
    }

    #[test]
    fn a_lowercase_method_is_unknown_rather_than_a_spelling_of_get() {
        let (verb, _) = parse_request_line("get / HTTP/1.1").expect("well-formed");
        assert_eq!(verb, None);
    }

    #[test]
    fn http_1_0_is_served_and_a_later_version_is_not() {
        assert!(parse_request_line("GET / HTTP/1.0").is_some());
        assert!(parse_request_line("GET / HTTP/2.0").is_none());
    }

    #[test]
    fn a_request_line_missing_a_field_or_carrying_an_extra_one_is_malformed() {
        assert!(parse_request_line("GET /").is_none());
        assert!(parse_request_line("GET").is_none());
        assert!(parse_request_line("").is_none());
        assert!(parse_request_line("GET / HTTP/1.1 extra").is_none());
        assert!(parse_request_line("GET  HTTP/1.1").is_none());
    }

    #[test]
    fn every_way_of_declaring_a_body_is_recognized() {
        assert!(header_declares_body("Content-Length: 1"));
        assert!(header_declares_body("content-length: 9223372036854775807"));
        assert!(header_declares_body("Transfer-Encoding: chunked"));
        assert!(header_declares_body("Expect: 100-continue"));
        assert!(header_declares_body("expect: 100-Continue"));
    }

    #[test]
    fn a_length_that_is_not_a_number_declares_a_body() {
        assert!(header_declares_body("Content-Length: not-a-number"));
        assert!(header_declares_body("Content-Length: -1"));
        assert!(header_declares_body("Content-Length: "));
    }

    #[test]
    fn a_zero_length_announces_no_body_and_is_served() {
        assert!(!header_declares_body("Content-Length: 0"));
        assert!(!header_declares_body("Host: localhost"));
        assert!(!header_declares_body("Accept: */*"));
        assert!(!header_declares_body("not a header line"));
    }

    #[test]
    fn the_head_bound_admits_a_real_reading_request_and_refuses_an_endless_one() {
        let browser = [
            "Host: localhost:8080",
            "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/139.0.0.0 Safari/537.36",
            "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,\
             image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
            "Accept-Encoding: gzip, deflate, br, zstd",
            "Accept-Language: en-GB,en-US;q=0.9,en;q=0.8",
            "Sec-Fetch-Dest: document",
            "Sec-Fetch-Mode: navigate",
            "Sec-Fetch-Site: none",
            "Sec-Fetch-User: ?1",
            "Upgrade-Insecure-Requests: 1",
        ]
        .join("\r\n");
        let deep_link = format!("/?stack={}", "x".repeat(1024));
        let head = read_over_socket(&format!("GET {deep_link} HTTP/1.1\r\n{browser}\r\n\r\n"));
        assert_eq!(head.expect("a browser request fits").target, deep_link);

        // The padding must exceed the bound for the bound to stop the read: the
        // helper sends a finite string.
        let endless = format!("GET / HTTP/1.1\r\n{}\r\n", "X-Pad: pad\r\n".repeat(2048));
        assert_eq!(read_over_socket(&endless).err(), Some(HeadError::Malformed));
    }

    #[test]
    fn a_declared_body_is_refused_before_a_byte_of_it_is_read() {
        // The length below is larger than the address space.
        let crafted = "GET /api/status HTTP/1.1\r\nHost: localhost\r\n\
                       Content-Length: 18446744073709551615\r\n\r\n";
        assert_eq!(
            read_over_socket(crafted).err(),
            Some(HeadError::BodyNotAllowed)
        );
    }

    #[test]
    fn a_head_that_ends_before_it_is_whole_is_never_parsed_as_a_request() {
        for truncated in [
            "GET /api/status HTTP/1.1\r\nHost: localhost\r\n",
            "GET /api/status HTTP/1.1\r\n",
            "GET /api/sta",
            "",
        ] {
            assert_eq!(
                read_over_socket(truncated).err(),
                Some(HeadError::Incomplete),
                "{truncated:?}"
            );
        }
    }

    #[test]
    fn a_head_terminated_with_bare_line_feeds_still_reads() {
        let head = read_over_socket("GET /api/random HTTP/1.1\nHost: localhost\n\n")
            .expect("bare line feeds terminate a head");
        assert_eq!(head.target, "/api/random");
        assert_eq!(head.verb, Some(Verb::Get));
    }

    #[test]
    fn every_status_the_surface_sends_has_its_own_reason_phrase() {
        for status in [200, 400, 403, 404, 405, 409, 413, 500, 502, 503] {
            assert_ne!(
                reason_phrase(status),
                "Status",
                "{status} should name itself"
            );
        }
    }
}
