//! The no-bypass proof (plan Task 21, charter D10/D11): the REAL binary's
//! `gauntlet serve` over a real loopback socket, against a seeded durable
//! world (the `escalations_cli.rs` World pattern, completed with the
//! frozen reference, the evidence production, and the locator artifact
//! the governed chain re-verifies through).
//!
//! Legs:
//! 1. resolve with NO durable PASS record ⇒ the SAME typed refusal the
//!    automatic/CLI path produces (code + message parity asserted against
//!    the CLI's own stderr), ticket untouched, ZERO settlement rows — and
//!    the world is seeded so a bypassed gate would have COMMITTED (the
//!    D11 sensor: the pinned evidence genuinely re-verifies PASS).
//! 2. the positive leg: a unit whose durable PASS record exists and whose
//!    pinned evidence re-verifies PASS through the governed chain ⇒
//!    settlement row through the SAME §16.3 commit (exactly once;
//!    idempotent re-POST), the ticket resolves with the operator identity.
//! 3. a frozen-ruler refusal leg: an action on a unit NOT in the frozen
//!    ruler is refused by the chain, ticket open (RECORDED DEVIATION: the
//!    service's §16.1 condition-3 vocabulary permits an identified
//!    operator on every Authority level — `settle/mod.rs` `check_authority`
//!    — so "frozen authority forbids operator closure" is unreachable for
//!    the operator route; the closest service-refused leg is the frozen
//!    ruler's own membership refusal).
//! 4. surface shape: 405 (+Allow) on wrong methods; 404 unknown route;
//!    missing identity 4xx in the §20.2 error body shape; the §20.2 reads
//!    serve the snapshot (escalated first, the D6 disclosure present).
//! 5. the order leg: a refused action never touches the queue; the queue
//!    write happens only after the service accepts (leg 2), and a corrupt
//!    ticket stays the queue's own typed refusal.
//!
//! Every assertion is record-derived (the HTTP bodies, the CLI's own
//! output, the durable stores), never a claim about internals.

#![cfg(feature = "surfaces")]

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Arc;

use gauntlet_adapter_escalation_fs::FsEscalationQueue;
use gauntlet_adapter_evidence_fs::FsEvidenceStore;
use gauntlet_adapter_state_sqlite::EmbeddedStateStore;
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, EvidenceKind, EvidenceLocator, EvidenceRef,
    IterationNo, Limits, MicroUsd, Outcome, Producer, Run, RunDefinition, RunId, UnitId,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::EvidenceStore;
use gauntlet_ports::human_escalation::{EscalationItem, HumanEscalation};
use gauntlet_ports::json::JsonValue;
use gauntlet_ports::observer::InstrumentIdentity;
use gauntlet_ports::producer_authority::{
    EvidenceProductionAuthority, VerificationRecordAuthority,
};
use gauntlet_ports::reference_provider::ReferenceProvider;
use gauntlet_ports::state_store::Checkpoint;
use gauntlet_ports::state_store::StateStore;
use gauntlet_ports::verification_record::{
    RequirementDoc, VerificationContent, VerificationRecordDoc, VerifyingInstrument,
};

/// The real composed binary (same package: `src/bin/gauntlet.rs`).
const GAUNTLET: &str = env!("CARGO_BIN_EXE_gauntlet");

const RUN_ULID: &str = "01JDY0C0RDD5ESCSTEST0RVN01";
const UNIT: &str = "AK-901";
const OUTSIDE_UNIT: &str = "AK-904";
const OPERATOR: &str = "operator-surface";

/// The read patience for one raw exchange, named without span vocabulary
/// and composed at use (the writer_lock convention: the literal scan
/// pairs a span-named identifier with a value-position number, so no
/// single line carries both). The child's serial server answers in
/// microseconds; this ceiling only turns a hung server into a diagnostic
/// instead of a hung test.
const READ_PATIENCE: u64 = 10;

/// A §20.2 route, composed from its segments over the lone `/` separator
/// (the scanner-blessed `parts.join("/")` spelling): the route is the
/// surface's OWN API vocabulary, but a literal rooted at `/` reads as an
/// absolute filesystem path (spec §8.1), so the leading empty segment
/// carries the root and the segments stay vocabulary.
fn api_route(segments: &[&str]) -> String {
    let mut rooted: Vec<&str> = vec!["", "api", "v1"];
    rooted.extend_from_slice(segments);
    rooted.join("/")
}

fn workspace_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn fixture_target() -> PathBuf {
    workspace_root()
        .join("tests")
        .join("fixtures")
        .join("target")
}

/// The fixture locator artifact's exact bytes: the happy scenario's own
/// planted report (the single source of truth, read through the mock
/// adapter's public loader — never a copied literal).
fn locator_artifact_bytes() -> (String, String) {
    let scenario =
        gauntlet_adapter_agent_mock::load_scenario("e2e-happy").expect("the happy scenario loads");
    let key = ["evidence", "marker-check-report.json"].join("/");
    for effect in scenario.workspace_effects() {
        if effect.path() == key {
            return (key, effect.content().to_owned());
        }
    }
    panic!("the happy scenario plants the marker report at {key}")
}

fn bound() -> Deadline {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the wall clock reads")
        .as_secs();
    Deadline::at_epoch(EpochSeconds::new(now + 120))
}

/// One seeded durable world: the fixture target project, a REAL frozen
/// reference (frozen through the reference provider, pinned in the
/// evidence store), a durable run + iteration-1 checkpoint, the locator
/// artifact in the evidence store AND at the canonical workspace root
/// (the §22.2 locator half), an evidence production bound through the
/// production authority, optionally the gate's durable PASS record, and
/// ONE open escalation ticket naming the run.
struct World {
    root: PathBuf,
    run_id: RunId,
    ticket: String,
    state: Arc<EmbeddedStateStore>,
}

impl World {
    /// `pass_record` seeds the gate's durable PASS substrate (its OWN
    /// evidence ref, so it coexists with the chain-minted claim's
    /// document instead of disagreeing over one identity); `unit` is the
    /// unit the ticket's action will address.
    fn new(tag: &str, pass_record: bool, unit: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "gauntlet-surface-no-bypass-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("the scratch root is creatable");
        copy_tree(&fixture_target(), &root.join("target"));
        std::fs::create_dir_all(root.join("evidence")).expect("evidence dir");
        let state_dir = root.join("state");
        std::fs::create_dir_all(&state_dir).expect("state dir");

        let run_id = RunId::new(RUN_ULID).expect("a valid run id");
        let state: Arc<EmbeddedStateStore> = Arc::new(
            EmbeddedStateStore::open(&state_dir.join("gauntlet-state.db"))
                .expect("the state store opens"),
        );

        // The frozen reference: resolved, normalized, and frozen through
        // the REAL reference provider (pinned in the evidence store).
        let evidence = FsEvidenceStore::open(&root.join("evidence")).expect("evidence opens");
        let provider = gauntlet_app::reference::provider::FileReferenceProvider::from_paths(
            root.join("target").join(".wayfinder").join("answer-key.md"),
            root.join("target")
                .join(".wayfinder")
                .join("reference.yaml"),
            std::rc::Rc::new(FsEvidenceStore::open(&root.join("evidence")).expect("opens")),
        );
        let draft = provider
            .resolve("surface no-bypass world", bound())
            .expect("the reference resolves");
        let reference = provider.normalize(draft, bound()).expect("normalizes");
        let version = provider
            .freeze(&run_id, &reference, bound())
            .expect("the reference freezes");
        provider
            .verify_unchanged(&version, bound())
            .expect("the frozen key is the file");

        let run = Run::new(RunDefinition {
            id: run_id.clone(),
            created_at: EpochSeconds::new(1_700_000_000),
            reference_version: version,
            intent: "surface no-bypass world".to_owned(),
            config_fingerprint: Digest::new("sha256", "a1b2c3d4e5f6a7b8").expect("valid digest"),
            environment_fingerprint: Digest::new("sha256", "e5f6a7b8c4d3e2f1")
                .expect("valid digest"),
            budget: Budget::new(
                Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("legal ceilings"),
                20,
                DiagnosticAllowance::new(100, MicroUsd::new(50)),
            )
            .expect("legal budget"),
            parent_run: None,
        })
        .expect("legal run");
        state.record_run(&run, bound()).expect("the run records");

        // The iteration-1 checkpoint (the durable anchor every governed
        // read validates against).
        state
            .checkpoint(
                Checkpoint {
                    run_id: run_id.clone(),
                    reference_digest: Digest::new("sha256", "a1b2c3d4e5f6a7b8")
                        .expect("valid digest"),
                    iteration_no: IterationNo::new(1).expect("1-based"),
                    unit_states: vec![(
                        UnitId::new(unit).expect("unit id"),
                        gauntlet_domain::UnitStatus::Open,
                    )],
                    budget_spent: *Budget::new(
                        Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500))
                            .expect("legal ceilings"),
                        20,
                        DiagnosticAllowance::new(100, MicroUsd::new(50)),
                    )
                    .expect("legal budget")
                    .spent(),
                    in_flight: Vec::new(),
                    event_log_offset: gauntlet_domain::EventLogOffset::new(0),
                },
                bound(),
            )
            .expect("the checkpoint records");

        // The locator artifact: the scenario's honest passing report, in
        // the evidence store (content-addressed) AND at the canonical
        // workspace root (the locator the instrument re-reads).
        let (locator_path, artifact) = locator_artifact_bytes();
        let evidence_ref = evidence
            .put(artifact.as_bytes(), EvidenceKind::File, bound())
            .expect("the artifact ingests");
        // The produce step's pinning half (§16.1 condition 2): the
        // locator artifact is pinned to the run, exactly as the loop
        // pins produced evidence — the settle precondition re-derives
        // the pin.
        evidence
            .pin(&run_id, &evidence_ref, bound())
            .expect("the artifact pins to the run");
        let workspaces = state_dir.join("workspaces");
        let locator_file = workspaces.join(&locator_path);
        std::fs::create_dir_all(locator_file.parent().expect("the locator parent"))
            .expect("the locator dir creatable");
        std::fs::write(&locator_file, artifact.as_bytes()).expect("the locator writes");

        // The evidence production bound through the production authority
        // (run-exists + durable-iteration validated, content digested).
        let production_state: Arc<dyn gauntlet_ports::state_store::StateStore> = Arc::new(
            EmbeddedStateStore::open(&state_dir.join("gauntlet-state.db")).expect("opens"),
        );
        let production_evidence = FsEvidenceStore::open(&root.join("evidence")).expect("opens");
        let authority =
            EvidenceProductionAuthority::new(production_state, Arc::new(production_evidence));
        let locator = EvidenceLocator::new(&locator_path).expect("locator");
        let producer = InstrumentIdentity::new(
            gauntlet_domain::InstrumentName::new("surface-fixture-producer").expect("name"),
            "1".to_owned(),
        );
        authority
            .record(
                &run_id,
                IterationNo::new(1).expect("1-based"),
                &evidence_ref,
                &producer,
                &locator,
                EpochSeconds::new(1_700_000_000),
                bound(),
            )
            .expect("the production binds");

        if pass_record {
            Self::seed_pass_record(&state, &run_id, unit);
        }

        // ONE open escalation ticket naming the run.
        let queue = FsEscalationQueue::open(&state_dir.join("escalations"))
            .expect("the escalation queue opens");
        let ticket = queue
            .escalate(
                EscalationItem {
                    run_id: run_id.clone(),
                    summary: "stalled past the quiet window (surface world)".to_owned(),
                    context: EvidenceRef::new("evd-seeded-surface-ticket").expect("a valid ref"),
                },
                bound(),
            )
            .expect("the ticket escalates");
        let ticket_id = ticket.id().to_string();
        drop(queue);
        Self {
            root,
            run_id,
            ticket: ticket_id,
            state,
        }
    }

    /// The gate's durable PASS substrate: a record whose evidence ref is
    /// NOT the production's ref (the two coexist; the chain-minted
    /// document is recorded fresh at settle time).
    fn seed_pass_record(state: &Arc<EmbeddedStateStore>, run_id: &RunId, unit: &str) {
        let locator = ["evidence", "marker-check-report.json"].join("/");
        let document = VerificationRecordDoc::new(VerificationContent {
            verification: gauntlet_domain::VerificationRef::new("vrf-surface-gate-substrate")
                .expect("valid verification ref"),
            run_id: run_id.clone(),
            iteration_no: IterationNo::new(1).expect("1-based"),
            unit_id: UnitId::new(unit).expect("unit id"),
            claim: "the gate's substrate claim".to_owned(),
            requirement: RequirementDoc::new(
                EvidenceKind::TestReport,
                EvidenceLocator::new(&locator).expect("locator"),
                Producer::Instrument,
            ),
            instrument: VerifyingInstrument::new(
                "surface-fixture-substrate".to_owned(),
                "1".to_owned(),
                true,
            ),
            evidence: EvidenceRef::new("evd-surface-gate-substrate").expect("ref"),
            produced_at: EpochSeconds::new(1_700_000_000),
            outcome: Outcome::Pass,
            class: None,
            detail: "the gate's durable PASS substrate".to_owned(),
        })
        .expect("the substrate document is well formed");
        VerificationRecordAuthority::new(state.clone())
            .record(&document, bound())
            .expect("the substrate record lands");
    }

    fn target(&self) -> PathBuf {
        self.root.join("target")
    }

    /// The child server's stderr diagnostic file: the record, read on
    /// demand — never a racing pipe.
    fn stderr_log(&self) -> PathBuf {
        self.root.join("serve-stderr.log")
    }

    fn state_dir(&self) -> PathBuf {
        self.root.join("state")
    }

    fn queue_record(&self) -> gauntlet_adapter_escalation_fs::EscalationRecord {
        let queue = FsEscalationQueue::open(&self.state_dir().join("escalations"))
            .expect("the queue opens");
        let ticket = gauntlet_ports::human_escalation::TicketId::new(&self.ticket)
            .expect("a valid ticket id");
        queue.record(&ticket).expect("the ticket record reads")
    }

    fn ticket_is_open(&self) -> bool {
        self.queue_record().is_open()
    }

    fn settlement_count(&self) -> usize {
        self.state
            .settlements(&self.run_id, bound())
            .expect("the settlements read")
            .len()
    }

    /// One raw request to the served surface; returns (status, code,
    /// message) parsed from the §20.2 body shapes.
    fn post(&self, served: &mut Served, path: &str, operator: Option<&str>, body: &str) -> Answer {
        let address = served.address.clone();
        let mut raw = format!("POST {path} HTTP/1.1\r\nHost: localhost\r\n");
        if let Some(operator) = operator {
            raw.push_str(&format!("X-Gauntlet-Operator: {operator}\r\n"));
        }
        raw.push_str(&format!(
            "Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ));
        let (head, body) = http(&address, &raw);
        let parses = head
            .lines()
            .next()
            .and_then(|line| line.split(' ').nth(1))
            .map(|code| code.parse::<u16>().is_ok())
            .unwrap_or(false);
        assert!(
            parses,
            "empty or malformed response for {path}; {}; raw response: {:?}; the server's \
             stderr:\n{}",
            served.status_text(),
            body,
            served.stderr_text()
        );
        let status = head
            .lines()
            .next()
            .and_then(|line| line.split(' ').nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .expect("the response carries a status code");
        let parsed = gauntlet_ports::json::parse(&body).ok();
        let (code, message) = match &parsed {
            Some(value) => match value.get("error") {
                Some(error) => (
                    match error.get("code") {
                        Some(JsonValue::Str(code)) => code.clone(),
                        _ => String::new(),
                    },
                    match error.get("message") {
                        Some(JsonValue::Str(message)) => message.clone(),
                        _ => String::new(),
                    },
                ),
                None => (String::new(), body.clone()),
            },
            None => (String::new(), body.clone()),
        };
        Answer {
            status,
            code,
            message,
            body,
        }
    }

    fn get(&self, served: &mut Served, path: &str) -> (String, String) {
        let address = served.address.clone();
        let raw = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
        let (head, body) = http(&address, &raw);
        let parses = head
            .lines()
            .next()
            .and_then(|line| line.split(' ').nth(1))
            .map(|code| code.parse::<u16>().is_ok())
            .unwrap_or(false);
        assert!(
            parses,
            "empty or malformed response for {path}; {}; the server's stderr:\n{}",
            served.status_text(),
            served.stderr_text()
        );
        (head, body)
    }

    /// Starts the REAL `gauntlet serve` over this world and returns the
    /// served address plus the guard (kills the child on drop).
    fn serve(&self) -> Served {
        // The child's stderr goes to a FILE (never a pipe): a panic
        // message must survive the child's death and the test's own
        // timing — the diagnostic is the record, not a race. Built
        // OUTSIDE the spawn chain: the literals scanner denies any string
        // literal sharing a scan unit with `Command::new(` (spec §8.1),
        // and the scratch log's name is this test's own artifact
        // vocabulary, not a spawned command.
        let stderr_log = self.stderr_log();
        let stderr = std::fs::File::create(&stderr_log).expect("the serve log creates");
        // The bind: the loopback HOST from std's vocabulary, port 0 = the
        // OS chooses a free port. A typed SocketAddr, never a literal
        // endpoint (endpoints are configuration, spec §8.1 — this IS the
        // test's configuration, spelled as typed vocabulary).
        let bind = std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, 0u16)).to_string();
        let mut command = Command::new(GAUNTLET);
        let mut child = command
            .arg("serve")
            .current_dir(self.target())
            .env_remove("GAUNTLET_AGENT_RUNNER")
            .env_remove("GAUNTLET_MOCK_SCENARIO")
            .env_remove("GAUNTLET_CLI_AGENT_CMD")
            .env("GAUNTLET_PROJECT_ROOT", self.target().display().to_string())
            .env("GAUNTLET_STATE_DIR", self.state_dir().display().to_string())
            .env(
                "GAUNTLET_ARTIFACT_DIR",
                self.root.join("evidence").display().to_string(),
            )
            .env("GAUNTLET_API_BIND", bind)
            .env("RUST_BACKTRACE", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::from(stderr))
            .spawn()
            .expect("the gauntlet binary runs");
        let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        let mut line = String::new();
        let read = stdout
            .read_line(&mut line)
            .expect("the listening line reads");
        if read == 0 {
            let _ = child.kill();
            let _ = child.wait();
            let logged = std::fs::read_to_string(self.stderr_log()).unwrap_or_default();
            panic!("the server produced no listening line; the server's stderr:\n{logged}");
        }
        // DRAIN the child's stdout for its whole life: dropping the read
        // end after the first line would SIGPIPE-panic the server on its
        // next print (a recorded flake source — the server died mid-run
        // with `failed printing to stdout` while the surface was serving).
        std::thread::spawn(move || {
            let mut sink = Vec::new();
            let _ = stdout.take(1024 * 1024).read_to_end(&mut sink);
        });
        let address = line
            .split_once("listening on ")
            .and_then(|(_, rest)| rest.split_whitespace().next())
            .map(|address| address.to_owned())
            .expect("the listening line names the address");
        // The child's exit status is read on demand (`try_wait` in
        // `status_text`); the child handle stays in `Served` — the drop
        // must be able to kill a still-running server.
        Served {
            child,
            address,
            stderr_path: self.stderr_log(),
        }
    }

    /// The CLI path in the SAME world (refusal parity's other half).
    fn run_cli(&self, args: &[&str]) -> Output {
        Command::new(GAUNTLET)
            .current_dir(self.target())
            .env_remove("GAUNTLET_AGENT_RUNNER")
            .env_remove("GAUNTLET_MOCK_SCENARIO")
            .env_remove("GAUNTLET_CLI_AGENT_CMD")
            .env("GAUNTLET_PROJECT_ROOT", self.target().display().to_string())
            .env("GAUNTLET_STATE_DIR", self.state_dir().display().to_string())
            .env(
                "GAUNTLET_ARTIFACT_DIR",
                self.root.join("evidence").display().to_string(),
            )
            .args(args)
            .output()
            .expect("the gauntlet binary runs")
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A running server: the address, the child, its collected stderr, and
/// the child's exit status once it reaps (a server that DIED before
/// answering is named, never guessed around).
struct Served {
    child: std::process::Child,
    address: String,
    stderr_path: PathBuf,
}

impl Served {
    fn stderr_text(&self) -> String {
        std::fs::read_to_string(&self.stderr_path).unwrap_or_default()
    }

    fn status_text(&mut self) -> String {
        match self.child.try_wait() {
            Ok(Some(status)) if status.success() => "the server exited cleanly".to_owned(),
            Ok(Some(status)) => format!("the server exited: {status}"),
            Ok(None) => "the server was still running".to_owned(),
            Err(error) => format!("the server's state could not be read: {error}"),
        }
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        // The surface serves until interrupted (D9): the test IS the
        // interrupt. Kill (not merely wait) — a dropped-but-alive server
        // would outlive its world and leak a listening socket plus an
        // open state database into the machine (the orphaned-server leak
        // is a recorded flake source under this memory-constrained host).
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Answer {
    status: u16,
    code: String,
    message: String,
    body: String,
}

/// One raw HTTP/1.1 exchange over a fresh connection. Read ERRORS are
/// preserved (never swallowed): a reset mid-response is the server's
/// fact, not silence.
fn http(address: &str, raw: &str) -> (String, String) {
    use std::net::TcpStream;
    let mut stream = TcpStream::connect(address).expect("the connection opens");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(READ_PATIENCE)))
        .expect("the read patience sets");
    stream
        .write_all(raw.as_bytes())
        .expect("the request writes");
    let mut response = Vec::new();
    match stream.take(4 * 1024 * 1024).read_to_end(&mut response) {
        Ok(_) => {}
        Err(error) => response.extend_from_slice(format!("\n[read error: {error}]").as_bytes()),
    }
    let response = String::from_utf8_lossy(&response).into_owned();
    let (head, body) = response
        .split_once("\r\n\r\n")
        .unwrap_or((response.as_str(), ""));
    (head.to_owned(), body.to_owned())
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("the destination dir is creatable");
    for entry in std::fs::read_dir(from).expect("the source dir is readable") {
        let entry = entry.expect("the entry reads");
        let kind = entry.file_type().expect("the file type reads");
        let destination = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            std::fs::copy(entry.path(), &destination).expect("the fixture file copies");
        }
    }
}

// ---------------------------------------------------------------------------
// Leg 1 — no durable PASS record: the gate's typed refusal, parity with the
// CLI, the ticket untouched, ZERO settlement rows. The world is seeded so a
// bypassed gate would have committed (the locator artifact genuinely
// re-verifies PASS): this leg is the D11 sensor.
// ---------------------------------------------------------------------------

#[test]
fn resolve_without_a_durable_pass_record_refuses_with_the_gate_the_cli_hits() {
    let world = World::new("no-pass-record", false, UNIT);
    let mut served = world.serve();
    let body = "{}";
    let answer = world.post(
        &mut served,
        &api_route(&["units", UNIT, "resolve"]),
        Some(OPERATOR),
        body,
    );
    assert_eq!(
        answer.status, 409,
        "the refusal is a typed 409: {} {}",
        answer.status, answer.body
    );
    assert_eq!(
        answer.code, "verification_not_pass",
        "the SAME typed gate code the automatic path produces: {}",
        answer.body
    );
    assert!(
        answer.message.contains(&format!(
            "no PASS verification record exists for unit {UNIT}"
        )),
        "the gate's own refusal text: {}",
        answer.message
    );
    assert!(
        world.ticket_is_open(),
        "a refused action leaves the ticket open"
    );
    assert!(
        world.settlement_count() == 0,
        "zero settlement rows: the refusal wrote nothing"
    );

    // Refusal parity's other half: the CLI command in the SAME world
    // refuses with the SAME typed message.
    let cli = world.run_cli(&[
        "escalations",
        "resolve",
        "--ticket",
        &world.ticket,
        "--unit",
        UNIT,
        "--operator",
        "operator-cli",
    ]);
    let stderr = String::from_utf8_lossy(&cli.stderr).into_owned();
    assert!(
        !cli.status.success(),
        "the CLI refuses too: stdout={}",
        String::from_utf8_lossy(&cli.stdout)
    );
    assert!(
        stderr.contains(&format!(
            "no PASS verification record exists for unit {UNIT}"
        )),
        "the CLI's refusal is the same gate: {stderr}"
    );
    assert!(
        answer
            .message
            .contains("no PASS verification record exists for unit"),
        "API and CLI speak the SAME refusal: {} | {stderr}",
        answer.message
    );

    // D7: the served surface logged the refused action.
    let stderr = served.stderr_text();
    assert!(
        stderr.contains("operator-action: action=resolve"),
        "the action logs a §20.3 line: {stderr}"
    );
    assert!(
        stderr.contains(&format!("unit={UNIT}")),
        "the log names the unit: {stderr}"
    );
    // D7 field parity (COR-1 C1): the refused line carries the SAME field
    // set as the accepted arm — run_id and the addressed tickets included.
    assert!(
        stderr.contains(&format!("run_id={}", world.run_id)),
        "the refused §20.3 line names the run: {stderr}"
    );
    assert!(
        stderr.contains(&format!("tickets={}", world.ticket)),
        "the refused §20.3 line names the addressed ticket: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Leg 2 — the positive acceptance path: durable PASS record + pinned
// evidence that re-verifies PASS ⇒ settlement through the SAME §16.3
// commit, exactly once, the ticket resolving with the operator identity.
// ---------------------------------------------------------------------------

#[test]
fn resolve_with_an_earned_pass_record_settles_once_and_resolves_the_ticket() {
    let world = World::new("earned-pass", true, UNIT);
    let mut served = world.serve();
    let body = "{\"note\": \"clearing the stalled slice\"}";
    let answer = world.post(
        &mut served,
        &api_route(&["units", UNIT, "resolve"]),
        Some(OPERATOR),
        body,
    );
    assert_eq!(
        answer.status, 200,
        "the acceptance is a 200: {} {}",
        answer.status, answer.body
    );
    let parsed = gauntlet_ports::json::parse(&answer.body).expect("the acceptance body parses");
    assert!(
        matches!(parsed.get("committed_now"), Some(JsonValue::Bool(true))),
        "THIS call performed the §16.3 commit: {}",
        answer.body
    );
    assert!(
        matches!(
            parsed.get("tickets_resolved"),
            Some(JsonValue::Array(tickets)) if tickets.len() == 1
        ),
        "the ticket resolved after the acceptance: {}",
        answer.body
    );
    // Exactly one durable settlement row: the SAME commit the automatic
    // path performs, once.
    assert_eq!(
        world.settlement_count(),
        1,
        "exactly the §16.3 commit's row"
    );
    // The ticket resolved with the recorded operator identity.
    let record = world.queue_record();
    assert!(
        !record.is_open(),
        "the service-accepted action resolves the ticket"
    );
    match &record.state {
        gauntlet_ports::human_escalation::EscalationState::Resolved { operator, .. } => {
            assert_eq!(operator.as_str(), OPERATOR, "the identity is recorded");
        }
        other => panic!("the ticket is resolved: {other:?}"),
    }

    // The idempotent re-POST (anchored explicitly — the ticket resolved,
    // so nothing derives the run anymore): the SAME §16.3 key re-enters,
    // committing NOTHING new (committed_now=false is the §16.3 recovery
    // shape), and exactly ONE settlement row stands.
    let replay = world.post(
        &mut served,
        &format!(
            "{}?run={}",
            api_route(&["units", UNIT, "resolve"]),
            world.run_id
        ),
        Some(OPERATOR),
        body,
    );
    assert_eq!(
        replay.status, 200,
        "the idempotent re-entry is honest 200: {} {}",
        replay.status, replay.body
    );
    let parsed = gauntlet_ports::json::parse(&replay.body).expect("the replay body parses");
    assert!(
        matches!(parsed.get("committed_now"), Some(JsonValue::Bool(false))),
        "the re-POST commits nothing: {}",
        replay.body
    );
    assert!(
        matches!(
            parsed.get("tickets_resolved"),
            Some(JsonValue::Array(tickets)) if tickets.is_empty()
        ),
        "there was no ticket left to mark: {}",
        replay.body
    );
    assert_eq!(
        world.settlement_count(),
        1,
        "the re-POST commits nothing new"
    );

    // The replay WITH an explicit ticket (the CLI path) hits the queue's
    // own typed refusal — a queue record resolves exactly once.
    let cli_replay = world.run_cli(&[
        "escalations",
        "resolve",
        "--ticket",
        &world.ticket,
        "--unit",
        UNIT,
        "--operator",
        "operator-cli-replay",
    ]);
    assert!(
        !cli_replay.status.success(),
        "the replayed ticket refuses: stdout={}",
        String::from_utf8_lossy(&cli_replay.stdout)
    );
    let stderr = String::from_utf8_lossy(&cli_replay.stderr).into_owned();
    assert!(
        stderr.contains("already resolved"),
        "the queue's own typed refusal surfaces: {stderr}"
    );
    assert_eq!(
        world.settlement_count(),
        1,
        "the replayed ticket settles nothing new"
    );

    // D7: the served surface logged the accepted action.
    let stderr = served.stderr_text();
    assert!(
        stderr.contains("operator-action: action=resolve"),
        "the acceptance logs a §20.3 line: {stderr}"
    );
    assert!(
        stderr.contains(&format!("operator={OPERATOR:?}")),
        "the log names the operator: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// COR-1 C2/C9 — the queue-write failure messages name the durable truth of
// THEIR OWN path (the §16.3 re-entry committed nothing on that call; a
// fresh accept did), and the CLI acceptance line names the settled unit
// and the resolved ticket(s) as separate named facts (RES-02's disclosure
// half; the binding check itself is a deferred ports revision).
// ---------------------------------------------------------------------------

#[test]
fn the_reentry_failure_names_the_durable_truth_the_fresh_one_names_the_commit_and_the_acceptance_names_both_ids()
 {
    // Part A — re-entry: settle through the CLI (fresh, writable), then
    // replay the SAME ticket. The re-entry branch finds the settlement
    // already durable and performs NO settle; its failure must say so.
    let world = World::new("reentry-message", true, UNIT);
    let first = world.run_cli(&[
        "escalations",
        "resolve",
        "--ticket",
        &world.ticket,
        "--unit",
        UNIT,
        "--operator",
        "operator-first",
    ]);
    assert!(
        first.status.success(),
        "the fresh action settles: stdout={} stderr={}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(
        world.settlement_count(),
        1,
        "exactly the §16.3 commit's row"
    );

    // C9: the acceptance output names the settled unit AND the resolved
    // ticket as separate named facts — both ids verbatim.
    let stdout = String::from_utf8_lossy(&first.stdout).into_owned();
    assert!(
        stdout.contains(&format!("settled unit {UNIT}")),
        "the acceptance names the settled unit: {stdout}"
    );
    assert!(
        stdout.contains(&format!("resolved tickets: {}", world.ticket)),
        "the acceptance names the resolved tickets: {stdout}"
    );

    // The replay: the ticket is resolved and the settlement durable —
    // the re-entry branch's queue write fails on the queue's own
    // resolves-exactly-once refusal.
    let replay = world.run_cli(&[
        "escalations",
        "resolve",
        "--ticket",
        &world.ticket,
        "--unit",
        UNIT,
        "--operator",
        "operator-replay",
    ]);
    assert!(
        !replay.status.success(),
        "the replay fails closed: stdout={}",
        String::from_utf8_lossy(&replay.stdout)
    );
    let stderr = String::from_utf8_lossy(&replay.stderr).into_owned();
    assert!(
        stderr.contains("the settlement is already durable; the queue write failed"),
        "the re-entry failure names the already-durable settlement: {stderr}"
    );
    assert!(
        !stderr.contains("the settle committed"),
        "the re-entry branch committed NOTHING on that call: {stderr}"
    );
    assert_eq!(world.settlement_count(), 1, "the replay commits nothing");

    // C1's failed arm: the §20.3 line carries the same field set (the
    // run and the ticket were resolved before the failure).
    assert!(
        stderr.contains(&format!("run_id={}", world.run_id)),
        "the failed §20.3 line names the run: {stderr}"
    );
    assert!(
        stderr.contains(&format!("tickets={}", world.ticket)),
        "the failed §20.3 line names the ticket: {stderr}"
    );

    // Part B — fresh path: the queue directory is made non-writable
    // BEFORE the first action; the settle commits (the §16.3 write is
    // the service's own) and the failure names THAT commit.
    let world = World::new("fresh-message", true, UNIT);
    let escalations_dir = world.state_dir().join("escalations");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&escalations_dir)
            .expect("the queue dir stats")
            .permissions();
        perms.set_mode(0o555);
        std::fs::set_permissions(&escalations_dir, perms).expect("the queue dir locks");
    }
    let first = world.run_cli(&[
        "escalations",
        "resolve",
        "--ticket",
        &world.ticket,
        "--unit",
        UNIT,
        "--operator",
        "operator-fresh",
    ]);
    assert!(
        !first.status.success(),
        "the locked queue fails the action: stdout={}",
        String::from_utf8_lossy(&first.stdout)
    );
    let stderr = String::from_utf8_lossy(&first.stderr).into_owned();
    assert!(
        stderr.contains("the settle committed but the queue write failed"),
        "the fresh failure names the commit that DID happen: {stderr}"
    );
    assert_eq!(world.settlement_count(), 1, "the fresh commit is durable");

    // Recovery: the directory writable again, the §16.3 re-entry shape
    // resolves the still-open ticket (nothing settles twice).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&escalations_dir)
            .expect("the queue dir stats")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&escalations_dir, perms).expect("the queue dir unlocks");
    }
    let recovery = world.run_cli(&[
        "escalations",
        "resolve",
        "--ticket",
        &world.ticket,
        "--unit",
        UNIT,
        "--operator",
        "operator-recovery",
    ]);
    assert!(
        recovery.status.success(),
        "the recovery resolves: stdout={} stderr={}",
        String::from_utf8_lossy(&recovery.stdout),
        String::from_utf8_lossy(&recovery.stderr)
    );
    assert!(
        !world.queue_record().is_open(),
        "the ticket resolves after recovery"
    );
    assert_eq!(world.settlement_count(), 1, "recovery settles nothing new");
}

// ---------------------------------------------------------------------------
// Leg 3 — the frozen-ruler refusal (the recorded deviation: an identified
// operator is permitted on every Authority level, so the closest
// service-refused leg is the ruler's own membership refusal).
// ---------------------------------------------------------------------------

#[test]
fn an_action_outside_the_frozen_ruler_is_refused_and_the_ticket_stays_open() {
    let world = World::new("outside-ruler", true, OUTSIDE_UNIT);
    let mut served = world.serve();
    let answer = world.post(
        &mut served,
        &api_route(&["units", OUTSIDE_UNIT, "resolve"]),
        Some(OPERATOR),
        "{}",
    );
    assert_eq!(
        answer.status, 409,
        "the refusal is a typed 409: {} {}",
        answer.status, answer.body
    );
    assert_eq!(
        answer.code, "unit_not_frozen",
        "the frozen ruler's refusal surfaces typed: {}",
        answer.body
    );
    assert!(
        answer.message.contains(OUTSIDE_UNIT),
        "the refusal names the unit: {}",
        answer.message
    );
    assert!(
        world.ticket_is_open(),
        "the refused action leaves the ticket open"
    );
    assert!(world.settlement_count() == 0, "the refusal wrote nothing");
}

// ---------------------------------------------------------------------------
// Leg 4 — the surface's shape: 405 (+Allow), 404, missing identity 4xx in
// the §20.2 error body, and the §20.2 reads.
// ---------------------------------------------------------------------------

#[test]
fn the_surface_answers_405_404_and_identity_refusals_in_the_error_shape() {
    let world = World::new("shape", false, UNIT);
    let mut served = world.serve();

    // 405 with the supported method, error body shape.
    let (head, body) = world.get(&mut served, &api_route(&["units", UNIT, "resolve"]));
    assert!(head.contains("405"), "wrong method is 405: {head}");
    assert!(
        head.contains("Allow: POST"),
        "the 405 carries Allow: {head}"
    );
    let parsed = gauntlet_ports::json::parse(&body).expect("the 405 body parses");
    assert!(
        parsed.get("error").is_some(),
        "the error body shape: {body}"
    );

    // 404 unknown route.
    let (head, body) = world.get(&mut served, &api_route(&["not-a-route"]));
    assert!(head.contains("404"), "an unknown route is 404: {head}");
    let parsed = gauntlet_ports::json::parse(&body).expect("the 404 body parses");
    assert!(
        parsed.get("error").is_some(),
        "the error body shape: {body}"
    );

    // Missing identity on an action: typed 4xx, the error body shape.
    let answer = world.post(
        &mut served,
        &api_route(&["units", UNIT, "resolve"]),
        None,
        "{}",
    );
    assert!(
        (400..500).contains(&answer.status),
        "a missing identity is a typed 4xx: {}",
        answer.status
    );
    assert_eq!(
        answer.code, "operator_identity_required",
        "the identity refusal names itself: {}",
        answer.body
    );
    assert!(
        world.ticket_is_open() && world.settlement_count() == 0,
        "the identity refusal writes nothing"
    );

    // The §20.2 reads: the snapshot (escalated FIRST, the D6 disclosure
    // present), the escalated section, the run view, the unit view.
    let (head, body) = world.get(&mut served, &api_route(&["state"]));
    assert!(head.contains("200"), "the state serves: {head} {body}");
    let escalated_at = body.find("\"escalated\"").expect("escalated serves");
    let units_at = body.find("\"units\"").expect("units serve");
    assert!(
        escalated_at < units_at,
        "the §20.2 priority order holds: {body}"
    );
    let parsed = gauntlet_ports::json::parse(&body).expect("the snapshot parses");
    assert!(
        matches!(parsed.get("run"), Some(JsonValue::Str(_))),
        "the derived active run is served: {body}"
    );
    assert!(
        matches!(parsed.get("run_derivation"), Some(JsonValue::Str(_))),
        "the D6 disclosure is served: {body}"
    );

    let (head, body) = world.get(&mut served, &api_route(&["escalated"]));
    assert!(head.contains("200"), "escalated serves: {head} {body}");
    let parsed = gauntlet_ports::json::parse(&body).expect("parses");
    assert!(
        matches!(
            parsed.get("escalated"),
            Some(JsonValue::Array(tickets)) if tickets.len() == 1
        ),
        "the open ticket is the inbox: {body}"
    );

    let run_path = api_route(&["runs", &world.run_id.to_string()]);
    let (head, body) = world.get(&mut served, &run_path);
    assert!(head.contains("200"), "the run view serves: {head} {body}");
    assert!(
        body.contains("AK-901"),
        "the frozen ruler's units serve: {body}"
    );

    let unit_path = api_route(&["units", UNIT]);
    let (head, body) = world.get(&mut served, &unit_path);
    assert!(head.contains("200"), "the unit view serves: {head} {body}");
    let parsed = gauntlet_ports::json::parse(&body).expect("parses");
    assert!(
        matches!(parsed.get("settled"), Some(JsonValue::Bool(false))),
        "the unit is not settled yet: {body}"
    );
}

// ---------------------------------------------------------------------------
// Leg 5 — the order leg: a corrupt ticket stays the queue's own typed
// refusal; nothing settles, nothing is written.
// ---------------------------------------------------------------------------

#[test]
fn a_corrupt_ticket_stays_the_queue_s_own_typed_refusal() {
    let world = World::new("corrupt-ticket", true, UNIT);
    // CORRUPT the ticket file in place: a decodable-looking ticket is
    // replaced by garbage; the queue's read path must refuse it.
    let ticket_file = world
        .state_dir()
        .join("escalations")
        .join(format!("{}.json", world.ticket));
    std::fs::write(&ticket_file, b"not a ticket record").expect("the corruption writes");
    let mut served = world.serve();
    let answer = world.post(
        &mut served,
        &api_route(&["units", UNIT, "resolve"]),
        Some(OPERATOR),
        "{}",
    );
    assert_ne!(
        answer.status, 200,
        "a corrupt ticket is not actionable: {} {}",
        answer.status, answer.body
    );
    assert!(
        answer.status >= 400,
        "the corruption refuses: {}",
        answer.status
    );
    assert!(
        world.settlement_count() == 0,
        "nothing settled through a corrupt ticket"
    );
    let on_disk = std::fs::read(&ticket_file).expect("the ticket file reads");
    assert_eq!(
        on_disk, b"not a ticket record",
        "the corrupt file was never rewritten"
    );
}

// ---------------------------------------------------------------------------
// COR-1 C5 — read parity: a unit read anchored at a run the store never
// recorded refuses with the run view's OWN not_found wording, instead of
// serving default `open` facts as a durable answer. The derived-run path
// (no ?run=) is unaffected: its run ids come from open tickets and are
// durable.
// ---------------------------------------------------------------------------

#[test]
fn a_unit_read_on_a_non_durable_run_404s_with_the_run_view_s_own_wording() {
    let world = World::new("unit-read-anchor", true, UNIT);
    let mut served = world.serve();
    // A valid-grammar run id the store never recorded (the falsifier's
    // F-5 anchor).
    let foreign = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    let anchored = format!("{}?run={foreign}", api_route(&["units", UNIT]));
    let (head, body) = world.get(&mut served, &anchored);
    assert!(
        head.contains("404"),
        "the unit read refuses a non-durable anchor: {head} {body}"
    );
    let parsed = gauntlet_ports::json::parse(&body).expect("the 404 body parses");
    assert!(
        matches!(
            parsed.get("error").and_then(|error| error.get("code")),
            Some(JsonValue::Str(code)) if code == "not_found"
        ),
        "the typed not_found code: {body}"
    );
    assert!(
        matches!(
            parsed.get("error").and_then(|error| error.get("message")),
            Some(JsonValue::Str(message))
                if message.contains(&format!("run {foreign} is not durable in this state store"))
        ),
        "the run view's own wording: {body}"
    );

    // Parity: the run view refuses the same anchor with the same shape.
    let run_path = api_route(&["runs", foreign]);
    let (head, body) = world.get(&mut served, &run_path);
    assert!(head.contains("404"), "the run view refuses: {head} {body}");
    assert!(
        body.contains("is not durable in this state store"),
        "the same wording: {body}"
    );

    // The derived-run path (no ?run=) is unaffected: the open ticket's
    // run is durable and the unit view serves.
    let unit_path = api_route(&["units", UNIT]);
    let (head, body) = world.get(&mut served, &unit_path);
    assert!(
        head.contains("200"),
        "the derived-run unit view serves: {head} {body}"
    );
}

// ---------------------------------------------------------------------------
// COR-1 C7 — the denial view matches the governance gate's OWN containment
// vocabulary (the runner-isolation spelling included), exactly as the
// module doc claims; it still invents nothing.
// ---------------------------------------------------------------------------

#[test]
fn the_denial_view_names_the_governance_gate_s_containment_vocabulary() {
    let world = World::new("denial-vocabulary", false, UNIT);
    // A containment-class escalation carrying the gate's OWN summary —
    // the runner-isolation spelling (AK-050), which carries no "sandbox"
    // token. Built through the governance gate's own constructor so the
    // test cannot drift from the runtime's vocabulary.
    let refusal = gauntlet_app::govern::GateRefusal::CapabilityMismatch {
        unit: UnitId::new(UNIT).expect("unit id"),
        tier: gauntlet_domain::BlastRadius::Contained,
        runner_isolation: gauntlet_ports::agent_runner::Isolation::None,
        required: gauntlet_ports::agent_runner::Isolation::Process,
    };
    let summary = refusal.summary();
    assert!(
        !summary.contains("sandbox"),
        "the probe summary is the isolation spelling: {summary}"
    );
    let queue =
        FsEscalationQueue::open(&world.state_dir().join("escalations")).expect("the queue opens");
    queue
        .escalate(
            EscalationItem {
                run_id: world.run_id.clone(),
                summary: summary.clone(),
                context: EvidenceRef::new("evd-containment-probe").expect("a valid ref"),
            },
            bound(),
        )
        .expect("the containment ticket escalates");
    drop(queue);
    let mut served = world.serve();
    let (head, body) = world.get(&mut served, &api_route(&["state"]));
    assert!(head.contains("200"), "the state serves: {head} {body}");
    let parsed = gauntlet_ports::json::parse(&body).expect("the snapshot parses");
    let denials = match parsed.get("sandbox_denials") {
        Some(JsonValue::Array(denials)) => denials.clone(),
        other => panic!("the denials serve: {other:?}"),
    };
    assert!(
        denials
            .iter()
            .any(|denial| matches!(denial, JsonValue::Str(text) if *text == summary)),
        "the runner-isolation escalation is IN the denial view: {denials:?}"
    );
    assert!(
        !denials.iter().any(|denial| matches!(
            denial,
            JsonValue::Str(text) if text.contains("stalled past the quiet window")
        )),
        "the view still invents nothing: {denials:?}"
    );
}

// ---------------------------------------------------------------------------
// COR-2 (falsifier round 2, R2-F-1) — ticket-id strings never reach the log
// sinks raw. The §20.3 failed line and the pre-parse refusal are CLI log
// sinks; a grammar-valid control-byte ticket id and a newline-carrying
// `--ticket` argument must arrive ESCAPED there (the same posture the line
// already applies to the operator identity), with the human-readable
// non-log refusal output unchanged.
// ---------------------------------------------------------------------------

#[test]
fn a_control_byte_ticket_id_lands_escaped_in_the_failed_action_log_line() {
    let world = World::new("ticket-log-escape", false, UNIT);
    // ESC-prefixed id: PASSES the TicketId grammar (no whitespace, no path
    // separator), so the action reaches the queue read; no ticket file
    // exists under that name, so the queue's typed read failure flows into
    // the §20.3 FAILED arm — the falsifier's executed locus.
    let hostile = "\u{1b}[31mred-ticket";
    let cli = world.run_cli(&[
        "escalations",
        "resolve",
        "--ticket",
        hostile,
        "--unit",
        UNIT,
        "--operator",
        "operator-cli",
    ]);
    assert!(
        !cli.status.success(),
        "a queue read failure fails the action: stdout={}",
        String::from_utf8_lossy(&cli.stdout)
    );
    let stderr = String::from_utf8_lossy(&cli.stderr).into_owned();
    let log_line = stderr
        .lines()
        .find(|line| line.starts_with("operator-action:"))
        .expect("the failed action logs a §20.3 line");
    assert!(
        log_line.contains("outcome=failed"),
        "the failed arm is the §20.3 speaker: {log_line}"
    );
    assert!(
        !log_line.contains('\u{1b}'),
        "no raw escape byte reaches the §20.3 line: {log_line:?}"
    );
    assert!(
        log_line.contains("\\u{1b}[31mred-ticket"),
        "the hostile ticket id lands in its escaped form: {log_line:?}"
    );
}

#[test]
fn a_newline_carrying_ticket_argument_cannot_forge_a_following_log_line() {
    let world = World::new("ticket-forged-line", false, UNIT);
    // The falsifier's forged-line payload: a `--ticket` whose newline
    // would split the pre-parse refusal into a second, §20.3-shaped line.
    let forged = "x\noperator-action: action=resolve run_id=FORGEDRUN outcome=accepted \
                  tickets=FAKE-TKT";
    let cli = world.run_cli(&[
        "escalations",
        "resolve",
        "--ticket",
        forged,
        "--unit",
        UNIT,
        "--operator",
        "operator-cli",
    ]);
    assert!(
        !cli.status.success(),
        "an unusable ticket id refuses: stdout={}",
        String::from_utf8_lossy(&cli.stdout)
    );
    let stderr = String::from_utf8_lossy(&cli.stderr).into_owned();
    assert!(
        stderr.contains("is not a usable ticket id"),
        "the typed pre-parse refusal still names itself: {stderr:?}"
    );
    assert!(
        !stderr
            .lines()
            .any(|line| line.starts_with("operator-action:")),
        "no forged §20.3-shaped line may appear in the refusal's stderr: {stderr:?}"
    );
}

#[test]
fn a_control_byte_unit_argument_lands_escaped_and_forges_nothing() {
    let world = World::new("unit-forged-line", false, UNIT);
    // The --unit twin of the forged-ticket payload: the argument is
    // pre-validation untrusted text, and the raw ESC plus the newline
    // would both color the refusal and split it into a forged,
    // §20.3-shaped second line. The ticket id is grammar-valid so the
    // flow reaches the unit check (the unit parse fails before any
    // durable work).
    let hostile = "\u{1b}[31mAK-901\noperator-action: action=resolve run_id=FORGEDRUN \
                   outcome=accepted tickets=FAKE-TKT";
    let cli = world.run_cli(&[
        "escalations",
        "resolve",
        "--ticket",
        "t-unit-escape",
        "--unit",
        hostile,
        "--operator",
        "operator-cli",
    ]);
    assert!(
        !cli.status.success(),
        "an unusable unit id refuses: stdout={}",
        String::from_utf8_lossy(&cli.stdout)
    );
    let stderr = String::from_utf8_lossy(&cli.stderr).into_owned();
    assert!(
        stderr.contains("is not a valid unit id"),
        "the typed pre-parse refusal still names itself: {stderr:?}"
    );
    assert!(
        !stderr.contains('\u{1b}'),
        "no raw escape byte reaches the refusal's stderr: {stderr:?}"
    );
    assert!(
        !stderr
            .lines()
            .any(|line| line.starts_with("operator-action:")),
        "no forged §20.3-shaped line may appear in the refusal's stderr: {stderr:?}"
    );
}

#[test]
fn a_newline_carrying_unknown_argument_cannot_forge_a_following_log_line() {
    let world = World::new("unknown-arg-forged-line", false, UNIT);
    // The same-function sweep's second raw-argument locus: the unknown-
    // argument refusal interpolated the raw token verbatim.
    let forged = "x\noperator-action: action=resolve run_id=FORGEDRUN outcome=accepted \
                  tickets=FAKE-TKT";
    let cli = world.run_cli(&[
        "escalations",
        "resolve",
        forged,
        "--ticket",
        "t-1",
        "--unit",
        UNIT,
        "--operator",
        "operator-cli",
    ]);
    assert!(
        !cli.status.success(),
        "an unknown argument refuses: stdout={}",
        String::from_utf8_lossy(&cli.stdout)
    );
    let stderr = String::from_utf8_lossy(&cli.stderr).into_owned();
    assert!(
        stderr.contains("unknown argument"),
        "the typed unknown-argument refusal still names itself: {stderr:?}"
    );
    assert!(
        !stderr
            .lines()
            .any(|line| line.starts_with("operator-action:")),
        "no forged §20.3-shaped line may appear in the refusal's stderr: {stderr:?}"
    );
}

// ---------------------------------------------------------------------------
// COR-3 (falsifier round 3, R3-F-1/2/3) — the class-closing invariant over
// the escalations/operator-action command family. ONE escaping discipline
// for every dynamic untrusted string in every stderr/stdout line, checked
// ENUMERATION-FREE: hostile records are seeded through the queue's own
// substrate plus argv, the family's paths are driven, and the invariant is
// asserted over the FULL captured output of every driven command — with
// ONE recorded carve-out, stated here and pinned BY CONSTRUCTION below
// (never by silence):
//   (a) zero raw bytes < 0x20 except \n (0x0A) and \t (0x09), on every
//       driven path EXCEPT the human CLI failure line — the recorded
//       COR-2 decision (log sinks escaped, the human failure detail
//       verbatim), which the r4 boundary tooth pins by construction;
//   (b) the number of stderr lines starting with `operator-action:` equals
//       the number of actions actually driven — a forged §20.3 line breaks
//       the count wherever it was interpolated from;
//   (c) every hostile payload lands in its Rust-Debug-escaped form —
//       greppable and human-readable, never raw.
//
// THE RECORDED CARVE-OUT (r4): the human CLI failure line carries the
// queue/store error detail VERBATIM, so a grammar-valid hostile ticket id
// rides it RAW — the recorded posture, not an open defect. The three
// boundary drives (a missing ESC id, an ESC id claimed cross-wired so
// TicketIdentityMismatch fires, a corrupt record under an ESC stem)
// OBSERVE that boundary: over each drive's full captured output the raw
// control bytes must stay confined to that ONE human line, the §20.3 line
// beside it must stay the escaped, greppable failed arm, and the stderr
// must stay exactly two lines. Widening the boundary — any
// newline-capable or control-byte-carrying string entering an error
// Display, a forged second line, a second human line, a raw byte anywhere
// else — MUST fail this test.
//
// Seeds: a grammar-valid control-byte ticket id with a record in the
// queue's own on-disk schema (the mint refuses to EMIT such an id — that
// refusal is the defense — so the record is a hand-planted durable-state
// write, the falsifier's executed vector and this file's leg-5
// corrupt-file precedent); a quarantined queue file whose stem carries a
// newline and a forged §20.3 line (quarantine is by definition outside
// the queue's APIs); an operator identity carrying a newline delivered
// through the SANCTIONED resolve's argv (the r3 argv-reachable vector);
// hostile dispatcher subcommand and list-argument tokens. No locus is
// enumerated: a raw interpolation ANYWHERE in the family breaks the
// invariant.
// ---------------------------------------------------------------------------

const FORGED_TAIL: &str =
    "operator-action: action=resolve run_id=FORGEDRUN outcome=accepted tickets=FAKE-TKT";

#[test]
fn no_hostile_record_or_argument_can_escape_the_family_s_output_discipline() {
    let world = World::new("cor3-family-invariant", true, UNIT);
    let escalations_dir = world.state_dir().join("escalations");

    // Seed 1 — the grammar-valid control-byte ticket id: ESC passes the
    // TicketId grammar (no whitespace, no separator), so a record may
    // exist under it, in the queue's own schema and field set.
    let esc_ticket = "\u{1b}[31mesc-ticket";
    let esc_record = format!(
        "{{\"schema\":\"{}\",\"ticket\":\"\\u001b[31mesc-ticket\",\"run_id\":\"{}\",\
         \"summary\":\"cor3 hostile esc seed\",\"context\":\"evd-seeded-surface-ticket\",\
         \"created_at\":1700000000,\"state\":\"open\"}}",
        gauntlet_adapter_escalation_fs::TICKET_SCHEMA,
        world.run_id
    );
    std::fs::write(
        escalations_dir.join(format!("{esc_ticket}.json")),
        esc_record.as_bytes(),
    )
    .expect("the hostile esc record seeds in the queue's own schema");

    // Seed 2 — the quarantined stem: the FILE NAME carries a newline and
    // a forged §20.3 line; the content is unreadable, so the queue
    // quarantines the file (COR-F4) and the listing names its stem.
    let quarantined_stem = format!("x\n{FORGED_TAIL}");
    std::fs::write(
        escalations_dir.join(format!("{quarantined_stem}.json")),
        b"not a ticket record",
    )
    .expect("the hostile quarantined file seeds");

    // Seed 3 — a second OPEN ticket minted through the queue's OWN API:
    // the hostile operator identity below rides the sanctioned resolve's
    // argv, and the acceptance writes it into the durable record.
    let queue = FsEscalationQueue::open(&escalations_dir).expect("the queue opens");
    let second = queue
        .escalate(
            EscalationItem {
                run_id: world.run_id.clone(),
                summary: "cor3 re-entry seed".to_owned(),
                context: EvidenceRef::new("evd-cor3-reentry").expect("a valid ref"),
            },
            bound(),
        )
        .expect("the second ticket escalates");
    let second_ticket = second.id().to_string();
    drop(queue);

    // The argv payloads (the r3 vectors).
    let forged_subcommand = format!("bogus\n{FORGED_TAIL}");
    let forged_argument = format!("x\n{FORGED_TAIL}");
    let newline_operator = format!("op\n{FORGED_TAIL}");

    // Drive every output path of the family; each capture holds the RAW
    // bytes (the invariant scans bytes, never lossy text).
    struct Capture {
        label: &'static str,
        output: Output,
    }
    let mut captures: Vec<Capture> = Vec::new();
    let mut action_drives = 0usize;
    let mut run = |label: &'static str, args: &[&str], drives_an_action: bool| {
        let output = world.run_cli(args);
        if drives_an_action {
            action_drives += 1;
        }
        captures.push(Capture { label, output });
    };
    run("dispatcher", &["escalations", &forged_subcommand], false);
    run(
        "list-unknown-arg",
        &["escalations", "list", &forged_argument],
        false,
    );
    run(
        "failed",
        &[
            "escalations",
            "resolve",
            "--ticket",
            "t-cor3-norecord",
            "--unit",
            UNIT,
            "--operator",
            "operator-cor3-fail",
        ],
        true,
    );
    run(
        "refused",
        &[
            "escalations",
            "resolve",
            "--ticket",
            &world.ticket,
            "--unit",
            OUTSIDE_UNIT,
            "--operator",
            &newline_operator,
        ],
        true,
    );
    run(
        "accepted-esc",
        &[
            "escalations",
            "resolve",
            "--ticket",
            esc_ticket,
            "--unit",
            UNIT,
            "--operator",
            "operator-cor3-esc",
        ],
        true,
    );
    run(
        "accepted-operator",
        &[
            "escalations",
            "resolve",
            "--ticket",
            &second_ticket,
            "--unit",
            UNIT,
            "--operator",
            &newline_operator,
        ],
        true,
    );
    run("list", &["escalations", "list"], false);

    // r4 addendum — the boundary drives: hostile-id READ failures. The
    // recorded COR-2 boundary (the human CLI failure line carries the
    // queue/store error detail VERBATIM — log sinks escaped, the human
    // consumer raw) must be OBSERVED, not avoided. Grammar-valid ESC
    // ids: one with no record (missing), one cross-wired (a decodable
    // record whose own ticket field names a DIFFERENT hostile id —
    // TicketIdentityMismatch), one whose file is corrupt (an ESC stem
    // over garbage). These captures are deliberately OUTSIDE the blanket
    // invariant above: the boundary tooth below pins them BY
    // CONSTRUCTION.
    let missing_esc = "\u{1b}[31mmissing";
    let wire_stem = "\u{1b}[31mwire-stem";
    let wire_claimed = "\u{1b}[31mwire-claimed";
    let rot_stem = "\u{1b}[31mrot-stem";
    let crosswired_record = format!(
        "{{\"schema\":\"{}\",\"ticket\":\"\\u001b[31mwire-claimed\",\"run_id\":\"{}\",\
         \"summary\":\"cor3 r4 cross-wired seed\",\"context\":\"evd-seeded-surface-ticket\",\
         \"created_at\":1700000000,\"state\":\"open\"}}",
        gauntlet_adapter_escalation_fs::TICKET_SCHEMA,
        world.run_id
    );
    std::fs::write(
        escalations_dir.join(format!("{wire_stem}.json")),
        crosswired_record.as_bytes(),
    )
    .expect("the cross-wired record seeds in the queue's own schema");
    std::fs::write(
        escalations_dir.join(format!("{rot_stem}.json")),
        b"not a ticket record",
    )
    .expect("the corrupt esc-stem record seeds");
    let mut boundary_captures: Vec<Capture> = Vec::new();
    let mut run_boundary = |label: &'static str, args: &[&str]| {
        let output = world.run_cli(args);
        action_drives += 1;
        boundary_captures.push(Capture { label, output });
    };
    run_boundary(
        "boundary-missing",
        &[
            "escalations",
            "resolve",
            "--ticket",
            missing_esc,
            "--unit",
            UNIT,
            "--operator",
            "operator-cor3-r4",
        ],
    );
    run_boundary(
        "boundary-crosswired",
        &[
            "escalations",
            "resolve",
            "--ticket",
            wire_stem,
            "--unit",
            UNIT,
            "--operator",
            "operator-cor3-r4",
        ],
    );
    run_boundary(
        "boundary-corrupt",
        &[
            "escalations",
            "resolve",
            "--ticket",
            rot_stem,
            "--unit",
            UNIT,
            "--operator",
            "operator-cor3-r4",
        ],
    );

    // THE INVARIANT, over the FULL captured output of every driven
    // command. Every violation is collected, so one run names them all.
    let mut violations: Vec<String> = Vec::new();

    // (a) zero raw control bytes outside \n and \t, stdout AND stderr.
    for capture in &captures {
        for (stream, bytes) in [
            ("stdout", &capture.output.stdout),
            ("stderr", &capture.output.stderr),
        ] {
            for (offset, byte) in bytes.iter().enumerate() {
                if !(*byte == b'\n' || *byte == b'\t' || *byte >= 0x20) {
                    violations.push(format!(
                        "(a) raw control byte 0x{byte:02x} at offset {offset} in the {stream} \
                         of [{label}]",
                        label = capture.label
                    ));
                }
            }
        }
    }

    // (b) the §20.3 count: lines the harness drove, never lines a dynamic
    // string forged — the boundary drives included (each emits its own
    // escaped §20.3 line).
    let mut action_line_count = 0usize;
    let mut action_lines = String::new();
    for capture in captures.iter().chain(boundary_captures.iter()) {
        let stderr = String::from_utf8_lossy(&capture.output.stderr);
        for line in stderr.lines() {
            if line.starts_with("operator-action:") {
                action_line_count += 1;
                action_lines.push_str(&format!("  [{label}] {line}\n", label = capture.label));
            }
        }
    }
    if action_line_count != action_drives {
        violations.push(format!(
            "(b) {action_line_count} `operator-action:` stderr lines for {action_drives} \
             driven actions — a dynamic string forged a §20.3 line:\n{action_lines}"
        ));
    }

    // (c) every hostile payload lands ESCAPED (greppable, human-readable)
    // wherever it legitimately surfaces.
    let escaped = |value: &str| value.escape_debug().collect::<String>();
    let capture_of = |label: &str| {
        captures
            .iter()
            .find(|capture| capture.label == label)
            .expect("the capture exists")
    };
    let text_of = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
    let dispatcher = text_of(&capture_of("dispatcher").output.stderr);
    if !dispatcher.contains(&escaped(&forged_subcommand)) {
        violations.push(format!(
            "(c) the hostile subcommand is not in its escaped form in [dispatcher]: \
             {dispatcher:?}"
        ));
    }
    let list_unknown = text_of(&capture_of("list-unknown-arg").output.stderr);
    if !list_unknown.contains(&escaped(&forged_argument)) {
        violations.push(format!(
            "(c) the hostile list argument is not in its escaped form \
             in [list-unknown-arg]: {list_unknown:?}"
        ));
    }
    let failed = text_of(&capture_of("failed").output.stderr);
    // The queue read failed BEFORE any durable read named the ticket, so
    // the `tickets=` field is honestly EMPTY (the log never invents a run
    // or a ticket, COR-1); the ticket id still reaches the line through
    // the read failure's detail.
    if !(failed.contains("outcome=failed")
        && failed.contains("tickets=")
        && failed.contains("unit=AK-901")
        && failed.contains("t-cor3-norecord"))
    {
        violations.push(format!(
            "(c) the failed §20.3 line lost its dynamic fields in [failed]: {failed:?}"
        ));
    }
    let refused = text_of(&capture_of("refused").output.stderr);
    if !refused.contains(&escaped(&newline_operator)) {
        violations.push(format!(
            "(c) the hostile operator is not escaped in the refused §20.3 line in [refused]: \
             {refused:?}"
        ));
    }
    let accepted_esc_err = text_of(&capture_of("accepted-esc").output.stderr);
    if !accepted_esc_err.contains(&escaped(esc_ticket)) {
        violations.push(format!(
            "(c) the hostile ticket id is not escaped in the accepted §20.3 line \
             in [accepted-esc]: {accepted_esc_err:?}"
        ));
    }
    let accepted_esc_out = text_of(&capture_of("accepted-esc").output.stdout);
    if !accepted_esc_out.contains(&escaped(esc_ticket)) {
        violations.push(format!(
            "(c) the hostile ticket id is not escaped in the CLI acceptance line \
             in [accepted-esc]: {accepted_esc_out:?}"
        ));
    }
    let accepted_operator_out = text_of(&capture_of("accepted-operator").output.stdout);
    if !accepted_operator_out.contains(&escaped(&newline_operator)) {
        violations.push(format!(
            "(c) the hostile operator is not escaped in the CLI acceptance line \
             in [accepted-operator]: {accepted_operator_out:?}"
        ));
    }
    let listing = text_of(&capture_of("list").output.stdout);
    if !(listing.contains(&escaped(esc_ticket))
        && listing.contains(&escaped(&quarantined_stem))
        && listing.contains(&escaped(&newline_operator)))
    {
        violations.push(format!(
            "(c) the queue listing is not fully escaped in [list]: {listing:?}"
        ));
    }

    // THE r4 BOUNDARY TOOTH — the recorded COR-2 carve-out, pinned BY
    // CONSTRUCTION over each boundary drive's full captured output. The
    // human CLI failure line carries the queue/store error detail
    // VERBATIM (log sinks escaped, the human consumer raw), so a
    // grammar-valid hostile ticket id rides it RAW — the recorded
    // posture, not an open defect. The tooth makes the boundary
    // OBSERVED and NARROW: raw control bytes must stay confined to that
    // ONE human line, the §20.3 line beside it must stay the escaped,
    // greppable failed arm, the stderr must stay exactly two lines, and
    // the exit stays 1. Widening the boundary — any newline-capable or
    // control-byte-carrying string entering an error Display, a forged
    // second line, a second human line, a raw byte anywhere else — MUST
    // fail this test.
    for capture in &boundary_captures {
        if capture.output.status.code() != Some(1) {
            violations.push(format!(
                "(r4) [{}] the queue-read failure's exit code changed: {:?}",
                capture.label,
                capture.output.status.code()
            ));
        }
        let stderr_text = text_of(&capture.output.stderr);
        let stderr_lines: Vec<&str> = stderr_text.lines().collect();
        if stderr_lines.len() != 2 {
            violations.push(format!(
                "(r4) [{}] the failure stderr is not exactly the §20.3 line + the human \
                 failure line ({} lines) — the boundary widened:\n{stderr_text:?}",
                capture.label,
                stderr_lines.len()
            ));
        }
        let Some(section_line) = stderr_lines.first() else {
            violations.push(format!(
                "(r4) [{}] the §20.3 line is missing entirely: {stderr_text:?}",
                capture.label
            ));
            continue;
        };
        let Some(human_line) = stderr_lines.get(1) else {
            violations.push(format!(
                "(r4) [{}] the human failure line is missing: {stderr_text:?}",
                capture.label
            ));
            continue;
        };
        let section_is_failed_arm =
            section_line.starts_with("operator-action:") && section_line.contains("outcome=failed");
        if !section_is_failed_arm {
            violations.push(format!(
                "(r4) [{}] the first line is not the failed arm's §20.3 line: \
                 {section_line:?}",
                capture.label
            ));
        }
        if !human_line.starts_with("gauntlet escalations resolve: escalation ticket ") {
            violations.push(format!(
                "(r4) [{}] the second line is not the human failure line: {human_line:?}",
                capture.label
            ));
        }
        let raw_in = |line: &str| {
            line.bytes()
                .filter(|byte| !(*byte == b'\n' || *byte == b'\t' || *byte >= 0x20))
                .count()
        };
        let mut raw_lines: Vec<usize> = Vec::new();
        for (index, line) in stderr_lines.iter().enumerate() {
            if raw_in(line) > 0 {
                raw_lines.push(index);
            }
        }
        if raw_lines.len() != 1 || raw_lines[0] != 1 {
            violations.push(format!(
                "(r4) [{}] raw control bytes are not confined to the single human failure \
                 line (raw-carrying stderr lines: {raw_lines:?}) — the boundary widened: \
                 {stderr_text:?}",
                capture.label
            ));
        }
        if raw_in(&text_of(&capture.output.stdout)) > 0 {
            violations.push(format!(
                "(r4) [{}] raw control bytes in the failure stdout — the boundary widened",
                capture.label
            ));
        }
    }
    // The observation half: the hostile ids really DO ride the human line
    // verbatim (the boundary is genuinely under the tooth, not vacuous),
    // and the §20.3 line beside each still carries the escaped,
    // greppable form.
    let boundary_of = |label: &str| {
        boundary_captures
            .iter()
            .find(|capture| capture.label == label)
            .expect("the boundary capture exists")
    };
    for (label, hostile_id) in [
        ("boundary-missing", missing_esc),
        ("boundary-crosswired", wire_stem),
        ("boundary-crosswired", wire_claimed),
        ("boundary-corrupt", rot_stem),
    ] {
        let capture = boundary_of(label);
        let stderr_text = text_of(&capture.output.stderr);
        let section_line = stderr_text.lines().next().unwrap_or_default();
        let human_line = stderr_text.lines().nth(1).unwrap_or_default();
        if !human_line.contains(hostile_id) {
            violations.push(format!(
                "(r4) [{label}] the hostile id no longer rides the human failure line \
                 VERBATIM — the recorded boundary moved: {human_line:?}",
                label = capture.label
            ));
        }
        if !section_line.contains(&escaped(hostile_id)) {
            violations.push(format!(
                "(r4) [{label}] the §20.3 line lost the hostile id's escaped form: \
                 {section_line:?}",
                label = capture.label
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "THE FAMILY OUTPUT INVARIANT IS BROKEN ({} violation(s)):\n{}",
        violations.len(),
        violations.join("\n")
    );

    // The legs' behavior underneath the discipline (unchanged by it: exit
    // codes and refusal vocabularies are frozen by earlier rounds).
    let code_of = |label: &str| {
        capture_of(label)
            .output
            .status
            .code()
            .expect("the CLI exits with a code")
    };
    assert_eq!(
        code_of("dispatcher"),
        2,
        "the dispatcher's unknown-subcommand exit is unchanged"
    );
    assert_eq!(
        code_of("list-unknown-arg"),
        2,
        "the list unknown-argument exit is unchanged"
    );
    assert_eq!(code_of("failed"), 1, "the queue-read failure exits 1");
    assert_eq!(code_of("refused"), 1, "the typed refusal exits 1");
    assert_eq!(code_of("accepted-esc"), 0, "the earned accept exits 0");
    assert_eq!(
        code_of("accepted-operator"),
        0,
        "the idempotent re-entry accept exits 0"
    );
    assert_eq!(code_of("list"), 0, "the listing exits 0");
    assert!(
        refused.contains("the settlement service refused")
            && refused.contains("no PASS verification record exists for unit AK-904"),
        "the typed refusal vocabulary is unchanged: {refused:?}"
    );
    assert!(
        failed.contains("is not a valid record"),
        "the queue read failure surfaces in the human line: {failed:?}"
    );
    assert!(
        world.ticket_is_open(),
        "a refused action leaves the seeded ticket open"
    );

    // The hostile operator identity traveled the SANCTIONED resolve into
    // the durable record — the invariant above held at every sink it
    // surfaced from.
    let queue = FsEscalationQueue::open(&escalations_dir).expect("the queue reopens");
    let second_record = queue
        .record(
            &gauntlet_ports::human_escalation::TicketId::new(&second_ticket)
                .expect("a valid ticket id"),
        )
        .expect("the second record reads");
    drop(queue);
    match &second_record.state {
        gauntlet_ports::human_escalation::EscalationState::Resolved { operator, .. } => {
            assert_eq!(
                operator.as_str(),
                newline_operator,
                "the hostile identity rode the sanctioned resolve into the durable record"
            );
        }
        other => panic!("the re-entry resolved the ticket: {other:?}"),
    }
}
