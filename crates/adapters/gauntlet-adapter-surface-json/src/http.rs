//! The minimal HTTP/1.1 server over `std::net::TcpListener` (charter
//! D2): loopback-only binds, bounded request size, per-request deadline,
//! no external dependency. The §20.2 routes, the §20.2 error body shape,
//! `405` (+ `Allow`) for unsupported methods, `404` for unknown routes,
//! `400` for malformed requests. The surface is additional
//! observability, never load-bearing (§21.3): a per-connection failure
//! is reported and the serve loop continues; a LISTENER-level accept
//! failure is classified — the transient classes (interrupted call,
//! client abort/reset, fd-table exhaustion) are reported and the loop
//! continues (rate-bounded by a short constant backoff, COR-2's R2-F-3),
//! while a listener-fatal class (the listener itself gone or
//! unusable) is reported and ends the loop, honestly, with no
//! "continuing" claim. Nothing here writes run state.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gauntlet_domain::{EpochSeconds, RunId, UnitId};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::json::{JsonError, JsonValue};
use gauntlet_ports::operator_surface::OperatorSurface;

use crate::action::{
    ActionCommand, ActionKind, ActionOutcome, OperatorActionHandler, SnapshotDisclosure,
};
use crate::views::{RunScopedError, RunScopedReader};
use crate::{error_json, snapshot_json};

/// The request-target prefix every served route shares (§20.2).
pub const API_PREFIX: &str = "/api/v1/";

/// The identity header (charter D7's builder choice): every action
/// carries the operator's identity; missing/blank is a typed 4xx.
pub const OPERATOR_HEADER: &str = "X-Gauntlet-Operator";

/// The bound request-line + header block cap: a larger "request" is not
/// a request this surface serves (bounded request size, charter D2).
const HEADER_BLOCK_CAP: usize = 16 * 1024;

/// The request body cap (actions carry a note, not a payload; large
/// payloads travel by evidence ref, §20.3).
const BODY_CAP: usize = 1024 * 1024;

/// The per-request deadline span (socket timeouts AND the port deadline
/// the reads run under): one bound, on its own line (numeric literals
/// never share a line with a duration-shaped identifier).
const REQUEST_SPAN_SECONDS: u64 = 10;

/// Why the surface could not start — typed (spec §21.2, fail-closed
/// configuration).
#[derive(Debug, Clone, PartialEq)]
pub enum SurfaceHttpError {
    /// The bind text was absent, empty, or unparsable as `host:port`.
    BindUnparsable { detail: String },
    /// The bind resolved to a NON-loopback address: the surface refuses
    /// to expose the runtime beyond the operator's own machine (D9).
    BindRefused { detail: String },
    /// The listener could not be opened.
    BindFailed { detail: String },
}

impl core::fmt::Display for SurfaceHttpError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BindUnparsable { detail } => {
                write!(f, "the bind text is not usable as host:port: {detail}")
            }
            Self::BindRefused { detail } => write!(f, "the bind is refused: {detail}"),
            Self::BindFailed { detail } => {
                write!(f, "the listener could not be opened: {detail}")
            }
        }
    }
}

impl std::error::Error for SurfaceHttpError {}

/// The JSON API operator surface server. Holds the read surface (the
/// `OperatorSurface` port), the run-scoped reader, the ONE action
/// handler, and the D6 disclosure. Serving is SERIAL by design: the
/// port seams carry no thread-safety bounds, the surface is loopback
/// operator observability (never load-bearing, §21.3), and the
/// per-request deadline bounds every connection — the listener backlog
/// is the only queue.
pub struct SurfaceHttp {
    surface: Box<dyn OperatorSurface>,
    reader: Box<dyn RunScopedReader>,
    actions: Box<dyn OperatorActionHandler>,
    disclosure: Box<dyn SnapshotDisclosure>,
}

impl SurfaceHttp {
    /// Assembles the server over the composition-root-provided seams.
    pub fn new(
        surface: Box<dyn OperatorSurface>,
        reader: Box<dyn RunScopedReader>,
        actions: Box<dyn OperatorActionHandler>,
        disclosure: Box<dyn SnapshotDisclosure>,
    ) -> Self {
        Self {
            surface,
            reader,
            actions,
            disclosure,
        }
    }

    /// Parses the bind text and opens the listener — LOOPBACK ONLY
    /// (charter D9): a bind text whose addresses are ALL non-loopback is
    /// a typed refusal before a socket is opened. Port 0 keeps its
    /// meaning (the OS picks; the caller reads the listener's local
    /// address).
    pub fn bind(bind_text: &str) -> Result<TcpListener, SurfaceHttpError> {
        let text = bind_text.trim();
        if text.is_empty() {
            return Err(SurfaceHttpError::BindUnparsable {
                detail: "the bind text is empty".to_owned(),
            });
        }
        let addrs = text
            .to_socket_addrs()
            .map_err(|error| SurfaceHttpError::BindUnparsable {
                detail: format!("{text}: {error}"),
            })?
            .collect::<Vec<_>>();
        let Some(addr) = addrs.first() else {
            return Err(SurfaceHttpError::BindUnparsable {
                detail: format!("{text} resolves to no address"),
            });
        };
        // EVERY resolved address must be loopback (COR-1 C6): a name
        // resolving to a non-loopback address plus a loopback one would
        // otherwise open the surface on whichever address the resolver
        // lists first — the loopback rule holds for the whole set.
        if let Some(offender) = addrs.iter().find(|candidate| !candidate.ip().is_loopback()) {
            return Err(SurfaceHttpError::BindRefused {
                detail: format!(
                    "{offender} is not a loopback address — the operator surface serves the local \
                     operator only (spec 20.2); bind a 127.0.0.0/8 or ::1 address"
                ),
            });
        }
        TcpListener::bind(addr).map_err(|error| SurfaceHttpError::BindFailed {
            detail: format!("{addr}: {error}"),
        })
    }

    /// The listener's bound local address (the `serve` command's
    /// `listening on` line; with port 0 this is the OS-chosen port).
    pub fn local_addr(listener: &TcpListener) -> Result<SocketAddr, SurfaceHttpError> {
        listener
            .local_addr()
            .map_err(|error| SurfaceHttpError::BindFailed {
                detail: error.to_string(),
            })
    }

    /// Serves until the listener fails (interrupt kills the process —
    /// D9's "serves until interrupted"). Per §21.3 a per-connection
    /// failure never takes the surface down: it is reported and the loop
    /// continues.
    pub fn serve(&self, listener: TcpListener) {
        let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        self.serve_while(listener, running);
    }

    /// Serves while `running` holds. The loop checks the flag between
    /// accepts; the flag falls while an accept is blocked, the NEXT
    /// connection unblocks it and the loop then returns — the one
    /// ordered way to stop a serial surface without a thread bound on
    /// the seams (the port traits carry none).
    pub fn serve_while(
        &self,
        listener: TcpListener,
        running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) {
        self.serve_accept_loop(&listener, running);
    }

    /// The accept loop over any accept source (the `TcpListener` in
    /// production, an injected script in the tests). Per §21.3 the loop
    /// reports and continues through every accept failure the listener
    /// itself survives — the transient classes — and ends only when the
    /// listener is no longer usable (reported honestly; no "continuing"
    /// is printed on the break).
    fn serve_accept_loop(
        &self,
        source: &dyn AcceptSource,
        running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) {
        use std::net::Shutdown;
        while running.load(std::sync::atomic::Ordering::SeqCst) {
            match source.accept() {
                Ok((mut stream, _)) => {
                    if running.load(std::sync::atomic::Ordering::SeqCst) {
                        self.handle(&mut stream);
                    } else {
                        let _ = stream.shutdown(Shutdown::Both);
                    }
                }
                Err(error) => {
                    if accept_error_is_transient(&error) {
                        eprintln!("surface: the accept loop observed {error}; continuing");
                        // COR-2 (falsifier round 2, R2-F-3): bound the
                        // continue's rate — a persistent transient class
                        // (EMFILE under a stuck fd ceiling) must not
                        // hot-spin this loop. See TRANSIENT_ACCEPT_BACKOFF.
                        std::thread::sleep(TRANSIENT_ACCEPT_BACKOFF);
                        continue;
                    }
                    eprintln!(
                        "surface: the accept loop failed: {error} — the listener is no longer \
                         usable; the surface stops"
                    );
                    break;
                }
            }
        }
    }

    /// The per-connection worker: one request, one typed response.
    fn handle(&self, stream: &mut TcpStream) {
        let timeout = request_deadline();
        let _ = stream.set_read_timeout(Some(timeout));
        let _ = stream.set_write_timeout(Some(timeout));
        let _ = stream.set_nodelay(true);
        let request = match read_request(stream) {
            Ok(request) => request,
            Err(error) => {
                if let Some((status, reason, body)) = error.bad_request_response() {
                    let _ = write_response(stream, status, reason, &body, None);
                }
                if !error.is_client_fault() {
                    eprintln!("surface: reading the request failed: {error}");
                }
                return;
            }
        };
        let response = self.route(&request);
        let _ = write_response(
            stream,
            response.status,
            response.reason,
            &response.body,
            response.allow.as_deref(),
        );
        eprintln!(
            "{}",
            access_log_line(&request.method, &request.target, response.status)
        );
    }

    /// The §20.2 routing table. Method mismatches answer 405 with the
    /// method that IS supported; unknown routes answer 404.
    fn route(&self, request: &Request) -> Response {
        let path = request.path();
        if !path.starts_with(API_PREFIX) {
            return Response::not_found(&format!(
                "no route serves {path} — the surface serves the /api/v1 tree"
            ));
        }
        let route = &path[API_PREFIX.len()..];
        let segments: Vec<&str> = route
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect();
        match (request.method.as_str(), segments.as_slice()) {
            ("GET", ["state"]) => self.snapshot_response(),
            ("POST", ["refresh"]) => self.snapshot_response(),
            ("GET", ["escalated"]) => self.escalated_response(),
            ("GET", ["runs", run_id]) => self.run_response(run_id, request),
            ("GET", ["units", unit_id]) => self.unit_response(unit_id, request),
            ("POST", ["units", unit_id, "resolve"]) => {
                self.action_response(request, ActionKind::Resolve, unit_id)
            }
            ("POST", ["units", unit_id, "retry"]) => {
                self.action_response(request, ActionKind::Retry, unit_id)
            }
            // Supported path, wrong method: 405 with the supported one.
            (_, ["state"]) => Response::method_not_allowed("GET"),
            (_, ["refresh"]) => Response::method_not_allowed("POST"),
            (_, ["escalated"]) => Response::method_not_allowed("GET"),
            (_, ["units", _, last]) if matches!(*last, "resolve" | "retry") => {
                Response::method_not_allowed("POST")
            }
            (_, ["units", _]) => Response::method_not_allowed("GET"),
            (_, ["runs", _]) => Response::method_not_allowed("GET"),
            _ => Response::not_found(&format!(
                "no route serves {path} — the surface serves the §20.2 endpoints"
            )),
        }
    }

    /// The snapshot (`GET /state`, `POST /refresh`): assembled fresh per
    /// request — refresh is a read-side action (D8), it writes nothing.
    fn snapshot_response(&self) -> Response {
        let deadline = port_deadline();
        match self.surface.snapshot(deadline) {
            Ok(snapshot) => {
                Response::ok(snapshot_json(&snapshot, &self.disclosure.run_derivation()))
            }
            Err(error) => Response::read_failed(error.to_string()),
        }
    }

    /// The escalated section alone (§20.2's actionable inbox).
    fn escalated_response(&self) -> Response {
        let deadline = port_deadline();
        match self.surface.snapshot(deadline) {
            Ok(snapshot) => Response::ok(JsonValue::Object(vec![(
                "escalated".to_owned(),
                JsonValue::Array(
                    snapshot
                        .escalated
                        .iter()
                        .map(|ticket| JsonValue::Str(ticket.to_string()))
                        .collect(),
                ),
            )])),
            Err(error) => Response::read_failed(error.to_string()),
        }
    }

    /// The run-scoped view; the run id IS the route segment (typed 400
    /// when unparsable, 404 when not durable).
    fn run_response(&self, run_id: &str, _request: &Request) -> Response {
        let Ok(run) = RunId::new(run_id) else {
            return Response::bad_request(
                "run_unparsable",
                &format!("{run_id} is not a valid run id"),
            );
        };
        match self.reader.run_view(&run, port_deadline()) {
            Ok(view) => Response::ok(view.to_json(&run)),
            Err(RunScopedError::NotFound { detail }) => Response::not_found(&detail),
            Err(RunScopedError::ReadFailed { detail }) => Response::read_failed(detail),
        }
    }

    /// The unit-scoped view across the anchored run: `?run=` when given,
    /// else the snapshot's derived active run (D6: disclosed in the
    /// snapshot, never a silent guess here either).
    fn unit_response(&self, unit_id: &str, request: &Request) -> Response {
        let Ok(unit) = UnitId::new(unit_id) else {
            return Response::bad_request(
                "unit_unparsable",
                &format!("{unit_id} is not a valid unit id"),
            );
        };
        let deadline = port_deadline();
        let run = match self.anchor_run(request, deadline) {
            Ok(run) => run,
            Err(response) => return *response,
        };
        match self.reader.unit_view(&run, &unit, deadline) {
            Ok(view) => Response::ok(view.to_json(&unit)),
            Err(RunScopedError::NotFound { detail }) => Response::not_found(&detail),
            Err(RunScopedError::ReadFailed { detail }) => Response::read_failed(detail),
        }
    }

    /// The action endpoints (D3): parse + validate, then the ONE shared
    /// handler. `409` carries a typed refusal; `500` an infrastructure
    /// failure; `400` a malformed request (identity, body, ids).
    fn action_response(&self, request: &Request, kind: ActionKind, unit_id: &str) -> Response {
        let Ok(unit) = UnitId::new(unit_id) else {
            return Response::bad_request(
                "unit_unparsable",
                &format!("{unit_id} is not a valid unit id"),
            );
        };
        let operator = match request.header(OPERATOR_HEADER) {
            Some(value) if !value.trim().is_empty() => value.trim().to_owned(),
            _ => {
                return Response::bad_request(
                    "operator_identity_required",
                    "every action carries the operator's identity in the X-Gauntlet-Operator \
                     header (spec 16.1 condition 3); missing or blank is refused",
                );
            }
        };
        let note = match request.body_text() {
            Ok(body) => {
                let trimmed = body.trim();
                if trimmed.is_empty() {
                    String::new()
                } else {
                    // Untrusted input is parsed, validated, and narrowed
                    // — never silently coerced (COR-1 C8): the `note`
                    // value must be a JSON string (a non-string is a
                    // typed 400). Unknown fields stay ignored — the
                    // documented posture: the body contributes ONLY an
                    // optional note, so no other field can reach the
                    // action path.
                    match gauntlet_ports::json::parse(trimmed) {
                        Ok(JsonValue::Object(entries)) => {
                            let mut note = String::new();
                            for (key, value) in entries {
                                if key == "note" {
                                    match value {
                                        JsonValue::Str(text) => note = text,
                                        _ => {
                                            return Response::bad_request(
                                                "body_unparsable",
                                                "the action body's 'note' must be a JSON string",
                                            );
                                        }
                                    }
                                }
                            }
                            note
                        }
                        Ok(_) => {
                            return Response::bad_request(
                                "body_unparsable",
                                "the action body must be a JSON object",
                            );
                        }
                        Err(error) => {
                            return Response::bad_request(
                                "body_unparsable",
                                &json_error_text(&error),
                            );
                        }
                    }
                }
            }
            Err(error) => return Response::bad_request("body_unreadable", &error),
        };
        let run = match request.query("run") {
            Some(requested) => match RunId::new(requested) {
                Ok(run) => Some(run.to_string()),
                Err(error) => {
                    return Response::bad_request(
                        "run_unparsable",
                        &format!("the ?run value is not a valid run id: {error}"),
                    );
                }
            },
            None => None,
        };
        let outcome = self.actions.perform(
            ActionCommand {
                kind,
                unit: unit.to_string(),
                operator,
                note,
                run,
            },
            port_deadline(),
        );
        match outcome {
            ActionOutcome::Accepted(accepted) => Response::ok(JsonValue::Object(vec![
                ("action".to_owned(), JsonValue::Str(kind.token().to_owned())),
                ("run_id".to_owned(), JsonValue::Str(accepted.run_id)),
                ("unit_id".to_owned(), JsonValue::Str(accepted.unit_id)),
                (
                    "committed_now".to_owned(),
                    JsonValue::Bool(accepted.committed_now),
                ),
                (
                    "tickets_resolved".to_owned(),
                    JsonValue::Array(
                        accepted
                            .tickets_resolved
                            .iter()
                            .map(|ticket| JsonValue::Str(ticket.clone()))
                            .collect(),
                    ),
                ),
            ])),
            ActionOutcome::Refused(refusal) => Response::refused(&refusal.code, &refusal.message),
            ActionOutcome::Failed(failure) => {
                Response::action_failed(&failure.code, &failure.message)
            }
        }
    }

    /// The run anchor for unit reads: `?run=` when given (typed 400 when
    /// unparsable), else the snapshot's derived active run (typed 404
    /// when none was derived — honest absence, never a guess).
    fn anchor_run(&self, request: &Request, deadline: Deadline) -> Result<RunId, Box<Response>> {
        if let Some(requested) = request.query("run") {
            return RunId::new(requested).map_err(|error| {
                Box::new(Response::bad_request(
                    "run_unparsable",
                    &format!("the ?run value is not a valid run id: {error}"),
                ))
            });
        }
        match self.surface.snapshot(deadline) {
            Ok(snapshot) => snapshot.run.ok_or_else(|| {
                Box::new(Response::not_found(
                    "no active run is derivable from the durable artifacts; pass ?run=<run-id>",
                ))
            }),
            Err(error) => Err(Box::new(Response::read_failed(error.to_string()))),
        }
    }
}

/// The accept source (COR-1 C3's seam): one accepted connection, or one
/// accept failure. `TcpListener` is the production source; the tests
/// inject a scripted one.
trait AcceptSource {
    fn accept(&self) -> std::io::Result<(TcpStream, SocketAddr)>;
}

impl AcceptSource for TcpListener {
    fn accept(&self) -> std::io::Result<(TcpStream, SocketAddr)> {
        TcpListener::accept(self)
    }
}

/// The transient-accept backoff (COR-2, falsifier round 2 R2-F-3): a
/// PERSISTENT transient condition — the executed repro held
/// RLIMIT_NOFILE so every accept returned EMFILE — would otherwise spin
/// the accept loop at full CPU, flooding stderr with "continuing" lines
/// (~37 MB/s measured). The transient branch sleeps this long before it
/// continues; the classification itself is unchanged (transient
/// continues, fatal breaks).
const TRANSIENT_ACCEPT_BACKOFF: Duration = Duration::from_millis(50);

/// Whether the listener itself survives this accept failure (§21.3's
/// report-and-continue posture, COR-1 C3). Transient — continue: the
/// interrupted call, a client that aborted or reset before the accept
/// completed, the pipe a peer already closed, momentary resource
/// exhaustion (EAGAIN, ENOMEM-class, and the fd/table exhaustion
/// EMFILE/ENFILE, whose dedicated `ErrorKind` variant is unstable to
/// name in this toolchain — the raw numbers are matched under the
/// catch-all). Fatal — break: every other class, because it names the
/// LISTENER as gone or unusable (EBADF-class, EINVAL-class, a NotFound
/// after a deliberate close).
fn accept_error_is_transient(error: &std::io::Error) -> bool {
    use std::io::ErrorKind;
    match error.kind() {
        ErrorKind::Interrupted
        | ErrorKind::ConnectionAborted
        | ErrorKind::ConnectionReset
        | ErrorKind::BrokenPipe
        | ErrorKind::WouldBlock
        | ErrorKind::TimedOut
        | ErrorKind::OutOfMemory
        | ErrorKind::StorageFull => true,
        _ => matches!(error.raw_os_error(), Some(23 | 24)), // ENFILE | EMFILE
    }
}

/// A parsed request (the tiny subset of HTTP/1.1 this surface serves).
struct Request {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    /// The path half of the target (before any `?`).
    fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or(&self.target)
    }

    /// A header value (case-insensitive name).
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// A query parameter value (the first match, raw — ids are validated
    /// typed downstream).
    fn query(&self, name: &str) -> Option<&str> {
        let query = self.target.split_once('?')?.1;
        query.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            key.eq_ignore_ascii_case(name).then_some(value)
        })
    }

    /// The body as UTF-8 text.
    fn body_text(&self) -> Result<String, String> {
        String::from_utf8(self.body.clone())
            .map_err(|error| format!("the request body is not valid UTF-8: {error}"))
    }
}

/// Reads one request under the header and body caps. A request that
/// fails the caps or the grammar is a client fault; a socket fault is
/// not.
fn read_request(stream: &mut TcpStream) -> Result<Request, ReadFault> {
    let mut block = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => return Err(ReadFault::Closed),
            Ok(_) => {
                block.push(byte[0]);
                if block.len() > HEADER_BLOCK_CAP {
                    return Err(ReadFault::HeaderCap);
                }
                if block.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            Err(error) => return Err(ReadFault::Io(error.to_string())),
        }
    }
    let head = parse_head(&block)?;
    let content_length = head
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.parse::<usize>())
        .transpose()
        .map_err(|_| ReadFault::Client("content_length_unparsable".to_owned()))?
        .unwrap_or(0);
    if content_length > BODY_CAP {
        return Err(ReadFault::BodyCap);
    }
    let mut body = vec![0u8; content_length];
    let mut read = 0usize;
    while read < content_length {
        match stream.read(&mut body[read..]) {
            Ok(0) => return Err(ReadFault::Closed),
            Ok(count) => read += count,
            Err(error) => return Err(ReadFault::Io(error.to_string())),
        }
    }
    Ok(Request {
        method: head.method,
        target: head.target,
        headers: head.headers,
        body,
    })
}

/// The read faults: `Client` shapes carry a typed 400 code back; the
/// others close the connection.
enum ReadFault {
    Client(String),
    HeaderCap,
    BodyCap,
    Closed,
    Io(String),
}

impl ReadFault {
    fn is_client_fault(&self) -> bool {
        matches!(self, Self::Client(_))
    }

    /// The typed 400 response for a readable-but-invalid request head.
    fn bad_request_response(&self) -> Option<(u16, &'static str, JsonValue)> {
        match self {
            Self::Client(code) => Some((
                400,
                "Bad Request",
                error_json(code, "the request head is not a servable HTTP/1.1 request"),
            )),
            Self::HeaderCap => Some((
                400,
                "Bad Request",
                error_json(
                    "request_too_large",
                    "the request head exceeds the surface's bounded request size",
                ),
            )),
            Self::BodyCap => Some((
                400,
                "Bad Request",
                error_json(
                    "body_too_large",
                    "the request body exceeds the surface's bounded body size",
                ),
            )),
            Self::Closed | Self::Io(_) => None,
        }
    }
}

impl core::fmt::Display for ReadFault {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Client(code) => write!(f, "the request is malformed ({code})"),
            Self::HeaderCap => write!(f, "the request head exceeds the cap"),
            Self::BodyCap => write!(f, "the request body exceeds the cap"),
            Self::Closed => write!(f, "the connection closed mid-request"),
            Self::Io(detail) => write!(f, "{detail}"),
        }
    }
}

/// Parses the request head: method, target, headers (the grammar subset
/// the surface serves).
fn parse_head(block: &[u8]) -> Result<ParsedRequestHead, ReadFault> {
    let text =
        std::str::from_utf8(block).map_err(|_| ReadFault::Client("head_not_utf8".to_owned()))?;
    let mut lines = text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| ReadFault::Client("empty_request".to_owned()))?;
    let mut parts = request_line.split(' ');
    let method = parts
        .next()
        .filter(|token| !token.is_empty())
        .ok_or_else(|| ReadFault::Client("missing_method".to_owned()))?;
    let target = parts
        .next()
        .filter(|token| !token.is_empty())
        .ok_or_else(|| ReadFault::Client("missing_target".to_owned()))?;
    let _version = parts
        .next()
        .filter(|token| token.starts_with("HTTP/"))
        .ok_or_else(|| ReadFault::Client("missing_version".to_owned()))?;
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(ReadFault::Client("malformed_header".to_owned()));
        };
        headers.push((name.trim().to_owned(), value.trim().to_owned()));
    }
    Ok(ParsedRequestHead {
        method: method.to_owned(),
        target: target.to_owned(),
        headers,
    })
}

/// The parsed head handed to the request builder.
struct ParsedRequestHead {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
}

/// A built response: status, reason, body, and the optional `Allow`
/// header value.
struct Response {
    status: u16,
    reason: &'static str,
    body: JsonValue,
    allow: Option<String>,
}

impl Response {
    fn ok(body: JsonValue) -> Self {
        Self {
            status: 200,
            reason: "OK",
            body,
            allow: None,
        }
    }

    fn bad_request(code: &str, message: &str) -> Self {
        Self {
            status: 400,
            reason: "Bad Request",
            body: error_json(code, message),
            allow: None,
        }
    }

    fn not_found(message: &str) -> Self {
        Self {
            status: 404,
            reason: "Not Found",
            body: error_json("not_found", message),
            allow: None,
        }
    }

    fn method_not_allowed(allow: &str) -> Self {
        Self {
            status: 405,
            reason: "Method Not Allowed",
            body: error_json(
                "method_not_allowed",
                &format!("this route serves {allow}; the request method is refused"),
            ),
            allow: Some(allow.to_owned()),
        }
    }

    /// A typed refusal: the action was decided and refused; nothing was
    /// written (409 — the operator's action conflicts with the durable
    /// state the service refuses past).
    fn refused(code: &str, message: &str) -> Self {
        Self {
            status: 409,
            reason: "Conflict",
            body: error_json(code, message),
            allow: None,
        }
    }

    /// An infrastructure failure: the action could not be decided.
    fn action_failed(code: &str, message: &str) -> Self {
        Self {
            status: 500,
            reason: "Internal Server Error",
            body: error_json(code, message),
            allow: None,
        }
    }

    /// A snapshot/read assembly failure (§21.3: reported, never fatal).
    fn read_failed(detail: String) -> Self {
        Self {
            status: 500,
            reason: "Internal Server Error",
            body: error_json("snapshot_failed", &detail),
            allow: None,
        }
    }
}

/// The access log line (COR-1 C4): the request target is attacker-
/// controlled untrusted input, so it lands DEBUG-ESCAPED — the same
/// posture the §20.3 action log applies to the operator identity — and
/// no raw control byte can reach the operator's diagnostics. COR-2
/// (falsifier round 2, R2-F-2): the METHOD is attacker-controlled the
/// same way (the head parser applies no token grammar to it), so it
/// lands DEBUG-ESCAPED too. Log-sink only: parser method handling and
/// all 405/404 semantics are unchanged.
fn access_log_line(method: &str, target: &str, status: u16) -> String {
    format!("surface: {method:?} {target:?} -> {status}")
}

/// Writes one response with `Connection: close` (the surface serves one
/// request per connection — small, boring, bounded).
fn write_response<W: Write>(
    stream: &mut W,
    status: u16,
    reason: &str,
    body: &JsonValue,
    allow: Option<&str>,
) -> std::io::Result<()> {
    let body_text = body.to_json_text();
    let mut head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        body_text.len()
    );
    if let Some(allow) = allow {
        head.push_str("Allow: ");
        head.push_str(allow);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body_text.as_bytes())?;
    stream.flush()
}

fn json_error_text(error: &JsonError) -> String {
    format!("the request body did not parse as JSON: {error}")
}

/// The per-request port deadline (the reads' own bound) — derived from
/// the wall clock, never from a literal in a call site.
fn port_deadline() -> Deadline {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or(0);
    Deadline::at_epoch(EpochSeconds::new(now + REQUEST_SPAN_SECONDS))
}

/// The socket timeout span for one request.
fn request_deadline() -> Duration {
    Duration::from_secs(REQUEST_SPAN_SECONDS)
}

#[cfg(test)]
mod accept_and_log {
    use super::*;
    use std::collections::VecDeque;

    /// COR-1 C4: the access log debug-escapes the attacker-controlled
    /// request target — exactly how the §20.3 action log escapes the
    /// operator identity — so no raw control byte reaches the operator's
    /// diagnostics. COR-2 (falsifier round 2, R2-F-2): the METHOD is
    /// attacker-controlled too (the parser applies no token grammar to
    /// it), so it gets the same escape treatment.
    #[test]
    fn the_access_log_line_escapes_the_request_target() {
        let line = access_log_line("GET", "/api/v1/\u{1b}[31mred-alert", 404);
        assert!(
            !line.contains('\u{1b}'),
            "no raw escape byte reaches the log: {line:?}"
        );
        assert!(
            line.contains("\\u{1b}"),
            "the target lands in its escaped form: {line:?}"
        );
        let plain = access_log_line("GET", "/api/v1/state", 200);
        assert!(
            plain.contains("/api/v1/state"),
            "a plain target reads as itself: {plain}"
        );
        // COR-2 (R2-F-2): control bytes riding the METHOD (ESC + BEL, the
        // falsifier's executed 405 probe) land escaped, never raw.
        let hostile_method = access_log_line("\u{1b}[31mFORGED\u{7}", "/api/v1/state", 405);
        assert!(
            !hostile_method.contains('\u{1b}') && !hostile_method.contains('\u{7}'),
            "no raw control byte from the method reaches the log: {hostile_method:?}"
        );
        assert!(
            hostile_method.contains("\\u{1b}[31mFORGED\\u{7}"),
            "the method lands in its escaped form: {hostile_method:?}"
        );
        let plain_method = access_log_line("GET", "/api/v1/state", 200);
        assert!(
            plain_method.contains("\"GET\""),
            "a plain method reads as itself, quoted like the target: {plain_method}"
        );
    }

    /// COR-1 C3: the classification — the transient accept-failure
    /// classes continue; the listener-fatal classes (EBADF/INVAL-class,
    /// a NotFound after a deliberate close) break the loop honestly.
    #[test]
    fn transient_accept_errors_continue_and_listener_fatal_ones_break() {
        let transient = [
            std::io::Error::from_raw_os_error(4),   // EINTR
            std::io::Error::from_raw_os_error(103), // ECONNABORTED
            std::io::Error::from_raw_os_error(104), // ECONNRESET
            std::io::Error::from_raw_os_error(32),  // EPIPE
            std::io::Error::from_raw_os_error(24),  // EMFILE
            std::io::Error::from_raw_os_error(23),  // ENFILE
            std::io::Error::new(std::io::ErrorKind::WouldBlock, "eagain"),
            std::io::Error::new(std::io::ErrorKind::OutOfMemory, "enomem"),
            std::io::Error::new(std::io::ErrorKind::StorageFull, "table full"),
        ];
        for error in &transient {
            assert!(
                accept_error_is_transient(error),
                "{error} is a continue class"
            );
        }
        let fatal = [
            std::io::Error::from_raw_os_error(9), // EBADF: the listener is gone
            std::io::Error::from_raw_os_error(22), // EINVAL
            std::io::Error::from_raw_os_error(2), // ENOENT (a closed listener's shape)
            std::io::Error::new(std::io::ErrorKind::NotFound, "after a deliberate close"),
        ];
        for error in &fatal {
            assert!(
                !accept_error_is_transient(error),
                "{error} is a break class"
            );
        }
    }

    /// The injected accept source: one scripted result per `accept`
    /// call, then a plain error if the loop keeps going past the script.
    struct ScriptedAccept {
        steps: std::sync::Mutex<VecDeque<std::io::Result<TcpStream>>>,
    }

    impl AcceptSource for ScriptedAccept {
        fn accept(&self) -> std::io::Result<(TcpStream, SocketAddr)> {
            let mut steps = self.steps.lock().expect("the accept script locks");
            match steps.pop_front() {
                Some(step) => {
                    let addr: SocketAddr = "127.0.0.1:0".parse().expect("a socket addr");
                    step.map(|stream| (stream, addr))
                }
                None => Err(std::io::Error::other("the accept script is exhausted")),
            }
        }
    }

    /// The read seam the loop test needs: every read fails, so the one
    /// accepted step answers with the typed 500 (proof the step was
    /// served) without any snapshot plumbing.
    struct FailingSurface;

    impl gauntlet_ports::operator_surface::OperatorSurface for FailingSurface {
        fn snapshot(
            &self,
            _deadline: Deadline,
        ) -> Result<
            gauntlet_ports::operator_surface::RuntimeSnapshot,
            gauntlet_ports::operator_surface::OperatorSurfaceError,
        > {
            Err(
                gauntlet_ports::operator_surface::OperatorSurfaceError::SnapshotFailed {
                    detail: "the loop test's read seam".to_owned(),
                },
            )
        }

        fn actions(&self) -> Vec<gauntlet_ports::operator_surface::OperatorAction> {
            Vec::new()
        }
    }

    struct NoReaderSeam;

    impl crate::views::RunScopedReader for NoReaderSeam {
        fn run_view(
            &self,
            _run: &RunId,
            _deadline: Deadline,
        ) -> Result<crate::views::RunView, crate::views::RunScopedError> {
            Err(crate::views::RunScopedError::NotFound {
                detail: "unused".to_owned(),
            })
        }

        fn unit_view(
            &self,
            _run: &RunId,
            _unit: &UnitId,
            _deadline: Deadline,
        ) -> Result<crate::views::UnitView, crate::views::RunScopedError> {
            Err(crate::views::RunScopedError::NotFound {
                detail: "unused".to_owned(),
            })
        }
    }

    struct NoActions;

    impl OperatorActionHandler for NoActions {
        fn perform(&self, _command: ActionCommand, _deadline: Deadline) -> ActionOutcome {
            ActionOutcome::Failed(crate::action::ActionFailure {
                code: "unused".to_owned(),
                message: "unused".to_owned(),
            })
        }
    }

    struct NoDisclosure;

    impl SnapshotDisclosure for NoDisclosure {
        fn run_derivation(&self) -> String {
            "unused".to_owned()
        }
    }

    /// COR-1 C3's behavioral half: the loop CONTINUES past transient
    /// accept errors (the accepted step is still served) and BREAKS on
    /// the listener-fatal one — consuming exactly through it, with the
    /// `running` flag never falling (the break is the error's, not the
    /// flag's).
    #[test]
    fn the_serve_loop_continues_past_transient_errors_and_stops_on_a_listener_fatal_one() {
        // A real connected pair for the one accepted step: the request
        // is pre-written; the answer is read on its own thread (the
        // serial server runs on this one).
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("the pair binds");
        let addr = listener.local_addr().expect("the pair's address");
        let mut client = TcpStream::connect(addr).expect("the pair connects");
        client
            .write_all(b"GET /api/v1/state HTTP/1.1\r\nHost: t\r\n\r\n")
            .expect("the request writes");
        let (server_side, _) = listener.accept().expect("the pair accepts");
        let reader = std::thread::spawn(move || {
            let mut buffer = Vec::new();
            let _ = client.take(64 * 1024).read_to_end(&mut buffer);
            buffer
        });

        let steps: VecDeque<std::io::Result<TcpStream>> = VecDeque::from(vec![
            Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "eintr",
            )),
            Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "aborted",
            )),
            Ok(server_side),
            Err(std::io::Error::from_raw_os_error(9)), // EBADF: the listener died
            Err(std::io::Error::other(
                "never reached if the fatal error breaks",
            )),
        ]);
        let source = ScriptedAccept {
            steps: std::sync::Mutex::new(steps),
        };
        let server = SurfaceHttp::new(
            Box::new(FailingSurface),
            Box::new(NoReaderSeam),
            Box::new(NoActions),
            Box::new(NoDisclosure),
        );
        let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        server.serve_accept_loop(&source, std::sync::Arc::clone(&running));
        assert!(
            running.load(std::sync::atomic::Ordering::SeqCst),
            "the flag never fell — the loop ended on the fatal accept, not the flag"
        );
        {
            let steps = source.steps.lock().expect("the accept script locks");
            assert_eq!(
                steps.len(),
                1,
                "the loop consumed exactly through the listener-fatal error"
            );
        }
        let response = reader.join().expect("the reader joins");
        assert!(
            response.starts_with(b"HTTP/1.1 "),
            "the accepted step was served AFTER the transient errors: {}",
            String::from_utf8_lossy(&response)
        );
    }

    /// COR-2 (falsifier round 2, R2-F-3): the transient branch pays the
    /// bounded backoff between continues — N consecutive transient errors
    /// cost at least N × (backoff − slack) — so a PERSISTENT transient
    /// condition (the executed repro held RLIMIT_NOFILE: every accept
    /// returned EMFILE, ~3.1M "continuing" lines in 3 s at 100% of a
    /// core) cannot hot-spin the accept loop. Generous-slack LOWER bound:
    /// the sleep itself is at-least semantics, only the measured floor
    /// carries slack, and the loop's consumption is asserted exactly, so
    /// the timing cannot pass by exiting early.
    #[test]
    fn consecutive_transient_accept_errors_cannot_hot_spin_the_loop() {
        const CONSECUTIVE: usize = 4;
        let mut steps: VecDeque<std::io::Result<TcpStream>> = VecDeque::new();
        for _ in 0..CONSECUTIVE {
            // EMFILE: the persistent fd-exhaustion class from the repro.
            steps.push_back(Err(std::io::Error::from_raw_os_error(24)));
        }
        // EBADF: the listener-fatal class that honestly ends the loop.
        steps.push_back(Err(std::io::Error::from_raw_os_error(9)));
        let source = ScriptedAccept {
            steps: std::sync::Mutex::new(steps),
        };
        let server = SurfaceHttp::new(
            Box::new(FailingSurface),
            Box::new(NoReaderSeam),
            Box::new(NoActions),
            Box::new(NoDisclosure),
        );
        let started = std::time::Instant::now();
        server.serve_accept_loop(
            &source,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        );
        let elapsed = started.elapsed();
        let floor = (TRANSIENT_ACCEPT_BACKOFF - Duration::from_millis(20)) * CONSECUTIVE as u32;
        assert!(
            elapsed >= floor,
            "{CONSECUTIVE} transient continues cost at least the bounded backoff \
             ({floor:?}): {elapsed:?}"
        );
        {
            let steps = source.steps.lock().expect("the accept script locks");
            assert!(
                steps.is_empty(),
                "the loop consumed exactly through the listener-fatal error, not short: \
                 {} steps left",
                steps.len()
            );
        }
    }
}
