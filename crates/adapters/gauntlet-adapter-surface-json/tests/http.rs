//! The JSON surface's HTTP behavior, over REAL loopback sockets: the
//! §20.2 routes, the error body shape, `405` (+ `Allow`), `404`, `400`,
//! the loopback-only bind, the snapshot field order (§20.2 priority,
//! escalation first), and the bounded request shape. The action outcome
//! mapping (200/409/500) is exercised with a scripted handler — the REAL
//! handler (the composition root's shared operator-action path) is what
//! the CLI integration tests drive end to end.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gauntlet_adapter_surface_json::action::{
    ActionCommand, ActionKind, ActionOutcome, ActionRefusal, OperatorActionHandler,
    SnapshotDisclosure,
};
use gauntlet_adapter_surface_json::views::{RunScopedReader, RunView, UnitView};
use gauntlet_adapter_surface_json::{MemorySurface, SurfaceHttp, snapshot_json};
use gauntlet_domain::{BlastRadius, MicroUsd, RunId, UnitId, UnitStatus};
use gauntlet_ports::budget_meter::Remaining;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::json::JsonValue;
use gauntlet_ports::operator_surface::{RuntimeSnapshot, UnitSnapshot};

/// A scripted action handler: one canned outcome per token.
struct ScriptedActions(ActionOutcome);

impl OperatorActionHandler for ScriptedActions {
    fn perform(&self, command: ActionCommand, _deadline: Deadline) -> ActionOutcome {
        let _ = (command.kind, command.unit, command.operator, command.note);
        self.0.clone()
    }
}

/// A no-op run-scoped reader: the routes under test here do not reach
/// it; the integration tests exercise the real one.
struct NoReader;

impl RunScopedReader for NoReader {
    fn run_view(
        &self,
        _run: &RunId,
        _deadline: Deadline,
    ) -> Result<RunView, gauntlet_adapter_surface_json::views::RunScopedError> {
        Ok(RunView::default())
    }

    fn unit_view(
        &self,
        _run: &RunId,
        _unit: &UnitId,
        _deadline: Deadline,
    ) -> Result<UnitView, gauntlet_adapter_surface_json::views::RunScopedError> {
        Ok(UnitView::default())
    }
}

struct FixedDisclosure;

impl SnapshotDisclosure for FixedDisclosure {
    fn run_derivation(&self) -> String {
        "derived from the open escalation tickets; the newest ULID anchors the view (test \
         disclosure)"
            .to_owned()
    }
}

/// A fully populated snapshot (§20.2's priority order, escalation first).
/// The canned no-op action outcome the read-only route tests share.
fn refused() -> ActionOutcome {
    ActionOutcome::Refused(ActionRefusal {
        code: "unused".to_owned(),
        message: "unused".to_owned(),
    })
}

fn seeded_snapshot() -> RuntimeSnapshot {
    RuntimeSnapshot {
        run: Some(RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id")),
        escalated: vec![
            gauntlet_ports::human_escalation::TicketId::new("t-1").expect("valid ticket id"),
        ],
        in_flight: Vec::new(),
        units: vec![UnitSnapshot {
            id: UnitId::new("AK-001").expect("valid unit id"),
            status: UnitStatus::Open,
            tier: BlastRadius::Contained,
        }],
        budget: Remaining {
            rounds: 4,
            parallel: 2,
            depth: 1,
            wall_seconds: 240,
            tokens: 640,
            cost_usd: MicroUsd::new(200),
        },
        sandbox_denials: vec![
            "sandbox policy p failed validation for unit AK-001 (1 \
                               finding(s))"
                .to_owned(),
        ],
        open_residuals: Vec::new(),
    }
}

/// Runs the client closure against a REAL server over a loopback socket:
/// the server serves on the CALLING thread (the port seams carry no
/// thread bounds — the surface is serial by design); the client runs on
/// its own thread, always stops the server (even on a failed assertion),
/// and its outcome is re-checked here.
fn with_server(
    snapshot: RuntimeSnapshot,
    actions: ActionOutcome,
    client: impl FnOnce(&str) + Send + 'static,
) {
    let listener = SurfaceHttp::bind("127.0.0.1:0").expect("the loopback bind opens");
    let addr = SurfaceHttp::local_addr(&listener).expect("the local address reads");
    let address = format!("{addr}");
    let running = Arc::new(AtomicBool::new(true));
    let stop = Arc::clone(&running);
    let outcome = std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client(&address);
        }));
        running.store(false, Ordering::SeqCst);
        // Unblocks the serial accept loop so serve_while can return.
        if let Ok(mut stream) = TcpStream::connect(&address) {
            let _ = stream.write_all(b"GET /api/v1/state HTTP/1.1\r\nHost: stop\r\n\r\n");
            let _ = stream.flush();
        }
        result
    });
    let server = SurfaceHttp::new(
        Box::new(MemorySurface::new(snapshot)),
        Box::new(NoReader),
        Box::new(ScriptedActions(actions)),
        Box::new(FixedDisclosure),
    );
    server.serve_while(listener, stop);
    let joined = outcome.join().expect("the client thread joins");
    assert!(joined.is_ok(), "the client assertions failed: {:?}", joined);
}

/// One raw HTTP/1.1 request over a fresh connection; returns the status
/// line and the body text.
fn request(address: &str, raw: &str) -> (String, String) {
    let mut stream =
        TcpStream::connect(address.trim_start_matches("http://")).expect("the connection opens");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("the read timeout sets");
    stream
        .write_all(raw.as_bytes())
        .expect("the request writes");
    let mut response = Vec::new();
    let _ = stream.take(1024 * 1024).read_to_end(&mut response);
    let response = String::from_utf8_lossy(&response).into_owned();
    let (head, body) = response
        .split_once("\r\n\r\n")
        .unwrap_or((response.as_str(), ""));
    (head.to_owned(), body.to_owned())
}

fn get(address: &str, path: &str) -> (String, String) {
    request(
        address,
        &format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"),
    )
}

#[test]
fn the_bind_refuses_non_loopback_hosts_before_opening_a_socket() {
    // (No server needed: the bind's own rules are checked directly.)
    let refused = SurfaceHttp::bind("192.0.2.1:8080").expect_err("a non-loopback bind refuses");
    assert!(
        refused.to_string().contains("not a loopback address"),
        "the refusal names the loopback rule: {refused}"
    );
    let unparsable = SurfaceHttp::bind("   ").expect_err("an empty bind refuses");
    assert!(
        unparsable.to_string().contains("not usable"),
        "an empty bind is a typed refusal: {unparsable}"
    );
    let loopback = SurfaceHttp::bind("127.0.0.1:0").expect("the loopback bind opens");
    assert_ne!(
        SurfaceHttp::local_addr(&loopback)
            .expect("the local address reads")
            .port(),
        0,
        "port 0 binds to an OS-chosen port"
    );
}

#[test]
fn the_state_route_serves_the_snapshot_in_the_2022_priority_order() {
    with_server(seeded_snapshot(), refused(), |address| {
        let (head, body) = get(address, "/api/v1/state");
        assert!(
            head.starts_with("HTTP/1.1 200 OK"),
            "the state serves: {head}"
        );
        // Escalated FIRST (§20.2 priority), the disclosure last (D6).
        let escalated = body
            .find("\"escalated\"")
            .expect("the escalated key serves");
        let units = body.find("\"units\"").expect("the units key serves");
        let budget = body.find("\"budget\"").expect("the budget key serves");
        let denials = body
            .find("\"sandbox_denials\"")
            .expect("the denials key serves");
        let derivation = body
            .find("\"run_derivation\"")
            .expect("the derivation disclosure serves");
        assert!(escalated < units && units < budget && budget < denials);
        assert!(
            derivation > denials,
            "the run_derivation disclosure is served with the snapshot"
        );
        let parsed = gauntlet_ports::json::parse(&body).expect("the body is valid JSON");
        assert!(
            matches!(parsed.get("run"), Some(JsonValue::Str(_))),
            "the derived active run is served: {body}"
        );
    });
}

#[test]
fn wrong_methods_405_with_allow_and_unknown_routes_404_in_the_error_shape() {
    with_server(seeded_snapshot(), refused(), |address| {
        let (head, body) = request(
            address,
            "POST /api/v1/state HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
        );
        assert!(head.contains("405"), "a wrong method is 405: {head}");
        assert!(
            head.contains("Allow: GET"),
            "the 405 carries the supported method: {head}"
        );
        let parsed = gauntlet_ports::json::parse(&body).expect("the 405 body is the error shape");
        assert!(
            matches!(parsed.get("error"), Some(JsonValue::Object(_))),
            "the error body shape holds: {body}"
        );

        let (head, body) = get(address, "/api/v1/definitely-not-a-route");
        assert!(head.contains("404"), "an unknown route is 404: {head}");
        let parsed = gauntlet_ports::json::parse(&body).expect("the 404 body is the error shape");
        assert!(
            parsed.get("error").is_some(),
            "the error body shape: {body}"
        );

        let (head, _body) = get(address, "/outside/the/api/tree");
        assert!(
            head.contains("404"),
            "a route outside the tree is 404: {head}"
        );
    });
}

#[test]
fn malformed_requests_are_typed_400s() {
    with_server(seeded_snapshot(), refused(), |address| {
        // A malformed request head.
        let (head, body) = request(address, "GARBAGE\r\n\r\n");
        assert!(head.contains("400"), "a malformed head is 400: {head}");
        let parsed = gauntlet_ports::json::parse(&body).expect("the 400 body is the error shape");
        assert!(
            parsed.get("error").is_some(),
            "the error body shape: {body}"
        );

        // A malformed action body (valid head, invalid JSON).
        let payload = "{not json}";
        let raw = format!(
            "POST /api/v1/units/AK-001/resolve HTTP/1.1\r\nHost: x\r\nX-Gauntlet-Operator: o\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{payload}",
            payload.len()
        );
        let (head, body) = request(address, &raw);
        assert!(head.contains("400"), "a malformed body is 400: {head}");
        let parsed = gauntlet_ports::json::parse(&body).expect("the body parses");
        assert!(
            matches!(
                parsed.get("error").and_then(|error| error.get("code")),
                Some(JsonValue::Str(code)) if code == "body_unparsable"
            ),
            "the malformed body names itself: {body}"
        );

        // A missing identity on an action is a typed 4xx.
        let payload = "{}";
        let raw = format!(
            "POST /api/v1/units/AK-001/resolve HTTP/1.1\r\nHost: x\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{payload}",
            payload.len()
        );
        let (head, body) = request(address, &raw);
        assert!(head.contains("400"), "a missing identity is 4xx: {head}");
        let parsed = gauntlet_ports::json::parse(&body).expect("the body parses");
        assert!(
            matches!(
                parsed.get("error").and_then(|error| error.get("code")),
                Some(JsonValue::Str(code)) if code == "operator_identity_required"
            ),
            "the missing identity names itself: {body}"
        );

        // COR-1 C8: a non-string note is a TYPED 400 — untrusted input is
        // never silently coerced to an empty note. (Unknown fields stay
        // ignored, the documented posture.)
        let payload = "{\"note\": 123}";
        let raw = format!(
            "POST /api/v1/units/AK-001/resolve HTTP/1.1\r\nHost: x\r\nX-Gauntlet-Operator: o\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{payload}",
            payload.len()
        );
        let (head, body) = request(address, &raw);
        assert!(
            head.contains("400"),
            "a non-string note is 400: {head} {body}"
        );
        let parsed = gauntlet_ports::json::parse(&body).expect("the body parses");
        assert!(
            matches!(
                parsed.get("error").and_then(|error| error.get("code")),
                Some(JsonValue::Str(code)) if code == "body_unparsable"
            ),
            "the non-string note names itself: {body}"
        );
    });
}

#[test]
fn action_outcomes_map_to_typed_statuses_and_the_refusal_carries_code_and_message() {
    // A typed refusal surfaces as 409 with the handler's exact code and
    // message (refusal parity is asserted end to end by the CLI tests).
    with_server(
        seeded_snapshot(),
        ActionOutcome::Refused(ActionRefusal {
            code: "verification_not_pass".to_owned(),
            message: "the typed refusal text".to_owned(),
        }),
        |address| {
            let payload = "{}";
            let raw = format!(
                "POST /api/v1/units/AK-001/retry HTTP/1.1\r\nHost: x\r\nX-Gauntlet-Operator: op\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let (head, body) = request(address, &raw);
            assert!(head.contains("409"), "a refusal is 409: {head}");
            let parsed = gauntlet_ports::json::parse(&body).expect("the refusal body parses");
            let error = parsed.get("error").expect("the error object serves");
            assert!(
                matches!(
                    error.get("code"),
                    Some(JsonValue::Str(code)) if code == "verification_not_pass"
                ),
                "the refusal code is the handler's: {body}"
            );
            assert!(
                matches!(
                    error.get("message"),
                    Some(JsonValue::Str(message)) if message == "the typed refusal text"
                ),
                "the refusal message is the handler's: {body}"
            );
        },
    );

    // An infrastructure failure is a 500, distinct from a refusal.
    with_server(
        seeded_snapshot(),
        ActionOutcome::Failed(gauntlet_adapter_surface_json::ActionFailure {
            code: "queue_unreadable".to_owned(),
            message: "the queue read failed".to_owned(),
        }),
        |address| {
            let payload = "{}";
            let raw = format!(
                "POST /api/v1/units/AK-001/resolve HTTP/1.1\r\nHost: x\r\nX-Gauntlet-Operator: op\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let (head, body) = request(address, &raw);
            assert!(head.contains("500"), "a failure is 500: {head}");
            let parsed = gauntlet_ports::json::parse(&body).expect("the failure body parses");
            assert!(
                matches!(
                    parsed
                        .get("error")
                        .and_then(|error| error.get("code")),
                    Some(JsonValue::Str(code)) if code == "queue_unreadable"
                ),
                "the failure names itself: {body}"
            );
        },
    );

    // An acceptance is a 200 with the accepted facts.
    with_server(
        seeded_snapshot(),
        ActionOutcome::Accepted(gauntlet_adapter_surface_json::ActionAccepted {
            run_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
            unit_id: "AK-001".to_owned(),
            tickets_resolved: vec!["t-1".to_owned()],
            committed_now: true,
        }),
        |address| {
            let payload = "{\"note\": \"clearing the escalation\"}";
            let raw = format!(
                "POST /api/v1/units/AK-001/resolve HTTP/1.1\r\nHost: x\r\nX-Gauntlet-Operator: op\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let (head, body) = request(address, &raw);
            assert!(head.contains("200"), "an acceptance is 200: {head}");
            let parsed = gauntlet_ports::json::parse(&body).expect("the acceptance body parses");
            assert!(
                matches!(parsed.get("committed_now"), Some(JsonValue::Bool(true))),
                "the acceptance carries the commit fact: {body}"
            );
        },
    );
    let _ = ActionKind::Resolve;
}

#[test]
fn the_snapshot_json_carries_the_2022_field_vocabulary_in_order() {
    let snapshot = seeded_snapshot();
    let text = snapshot_json(&snapshot, "the derivation").to_json_text();
    let run = text.find("\"run\"").expect("run serves");
    let escalated = text.find("\"escalated\"").expect("escalated serves");
    let in_flight = text.find("\"in_flight\"").expect("in_flight serves");
    let units = text.find("\"units\"").expect("units serves");
    let budget = text.find("\"budget\"").expect("budget serves");
    let denials = text.find("\"sandbox_denials\"").expect("denials serve");
    let residuals = text.find("\"open_residuals\"").expect("residuals serve");
    let derivation = text.find("\"run_derivation\"").expect("derivation serves");
    assert!(
        run < escalated
            && escalated < in_flight
            && in_flight < units
            && units < budget
            && budget < denials
            && denials < residuals
            && residuals < derivation,
        "the §20.2 priority order holds with the disclosure last: {text}"
    );
    let parsed = gauntlet_ports::json::parse(&text).expect("the encoding round-trips");
    assert!(
        matches!(
            parsed
                .get("budget")
                .and_then(|budget| budget.get("cost_usd_micro")),
            Some(JsonValue::Number(_))
        ),
        "cost stays an integer on the wire: {text}"
    );
}
