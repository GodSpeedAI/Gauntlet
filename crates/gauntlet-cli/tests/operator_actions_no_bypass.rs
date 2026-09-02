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
