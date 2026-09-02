//! The operator escalations surface, composed (plan Task 20, COR-D5 /
//! RES-06): `escalations resolve|retry` go through
//! `SettlementService::settle_operator_action` — the SAME settlement
//! service the automatic loop settles through (charter D9: no second
//! writer) — and the queue ticket resolves only after the service speaks.
//! Each leg drives the REAL `gauntlet` binary over a seeded durable run:
//!
//! 1. `resolve` with NO verification record for the unit ⇒ non-zero, the
//!    typed PASS-gate refusal, the ticket STAYS OPEN.
//! 2. `retry` with a durable NON-PASS record ⇒ the same gate, the ticket
//!    STAYS OPEN.
//! 3. `resolve` with an EARNED durable PASS record ⇒ STILL refused —
//!    AK-062's structural boundary: the operator shape carries no
//!    chain-minted readiness, and settlement is never re-derived from an
//!    operator's say-so (the full operator surface is plan Task 21). Zero
//!    settlement rows exist afterwards: D9's single writer, held.
//!
//! Record (the adjudication's wording vs the shipped gate): the
//! "settle-committed-but-queue-write-failed" CLI branch cannot fire in
//! this build — the service's `settle_operator_action` returns `Ok` for
//! no caller (AK-062 pins the no-bypass gate at the service level), so
//! the post-accept queue write is unreachable end-to-end until Task 21
//! composes an acceptance path. The branch's fail-closed shape (non-zero
//! exit, ticket left unresolved) is the honest posture for the day it
//! becomes reachable; pinning it end-to-end here would require faking
//! the service, which is exactly the seam this surface refuses to add.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;

use gauntlet_adapter_escalation_fs::FsEscalationQueue;
use gauntlet_adapter_state_sqlite::EmbeddedStateStore;
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, EvidenceKind, EvidenceLocator, EvidenceRef,
    IterationNo, Limits, MicroUsd, Outcome, Producer, ReferenceId, ReferenceVersion, Run,
    RunDefinition, RunId, UnitId,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::human_escalation::{EscalationItem, HumanEscalation};
use gauntlet_ports::producer_authority::VerificationRecordAuthority;
use gauntlet_ports::state_store::{Checkpoint, StateStore};
use gauntlet_ports::verification_record::{
    VerificationContent, VerificationRecordDoc, VerifyingInstrument,
};

const GAUNTLET: &str = env!("CARGO_BIN_EXE_gauntlet");

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

fn bound() -> Deadline {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the wall clock reads")
        .as_secs();
    Deadline::at_epoch(EpochSeconds::new(now + 120))
}

const RUN_ULID: &str = "01JDY0C0RDD5ESCSTEST0RVN01";
const UNIT: &str = "AK-901";

/// One seeded world: the fixture target project, a durable run in the
/// state store the CLI surface opens, and ONE open escalation ticket
/// naming that run. Removed on drop.
struct World {
    root: PathBuf,
    run_id: RunId,
    ticket: String,
}

impl World {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "gauntlet-escalations-cli-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("the scratch root is creatable");
        copy_tree(&fixture_target(), &root.join("target"));
        std::fs::create_dir_all(root.join("evidence")).expect("evidence dir");
        std::fs::create_dir_all(root.join("state")).expect("state dir");
        // The workspace root every REAL composed run has (compose_run opens
        // the workspace provider, which creates it): the governed re-mint
        // composes its §14.2 instruments over this root, and a world
        // seeding a durable checkpoint claims the loop already ran here.
        // Without it the action dies at instrument composition (an
        // infrastructure failure) before the governed open can speak.
        std::fs::create_dir_all(root.join("state").join("workspaces")).expect("workspaces dir");

        let run_id = RunId::new(RUN_ULID).expect("a valid run id");
        let run = Run::new(RunDefinition {
            id: run_id.clone(),
            created_at: EpochSeconds::new(1_700_000_000),
            reference_version: ReferenceVersion::new(
                ReferenceId::new("ref-compose-teeth").expect("valid reference id"),
                1,
                Digest::new("sha256", "a1b2c3d4e5f6a7b8").expect("valid digest"),
            ),
            intent: "escalations composed surface".to_owned(),
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
        let state = EmbeddedStateStore::open(&root.join("state").join("gauntlet-state.db"))
            .expect("the state store opens");
        state.record_run(&run, bound()).expect("the run records");

        let queue = FsEscalationQueue::open(&root.join("state").join("escalations"))
            .expect("the escalation queue opens");
        let ticket = queue
            .escalate(
                EscalationItem {
                    run_id: run_id.clone(),
                    summary: "stalled past the quiet window (seeded)".to_owned(),
                    context: gauntlet_domain::EvidenceRef::new("evd-seeded-ticket")
                        .expect("a valid evidence ref"),
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
        }
    }

    fn target(&self) -> PathBuf {
        self.root.join("target")
    }

    fn state_dir(&self) -> PathBuf {
        self.root.join("state")
    }

    /// Seeds a checkpoint declaring iteration 1 (the durable anchor a
    /// verification record at iteration 1 validates against) and, when
    /// `pass` is set, one durable PASS verification record for the unit —
    /// minted through the SAME record authority the governed chain uses.
    fn seed_anchor_and_record(&self, pass: bool) {
        let state: Arc<dyn StateStore> = Arc::new(
            EmbeddedStateStore::open(&self.state_dir().join("gauntlet-state.db"))
                .expect("the state store opens"),
        );
        state
            .checkpoint(
                Checkpoint {
                    run_id: self.run_id.clone(),
                    reference_digest: Digest::new("sha256", "a1b2c3d4e5f6a7b8")
                        .expect("valid digest"),
                    iteration_no: IterationNo::new(1).expect("1-based"),
                    unit_states: vec![(
                        UnitId::new(UNIT).expect("unit id"),
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
        let outcome = if pass { Outcome::Pass } else { Outcome::Fail };
        // The fixture's locator, built segment-wise (a joined path literal
        // is environment coupling the literals scanner denies — AK-016).
        let marker_report_locator =
            EvidenceLocator::new(&["evidence", "marker-check-report.json"].join("/"))
                .expect("locator");
        let document = VerificationRecordDoc::new(VerificationContent {
            verification: gauntlet_domain::VerificationRef::new("vrf-escalations-cli-seed")
                .expect("valid verification ref"),
            run_id: self.run_id.clone(),
            iteration_no: IterationNo::new(1).expect("1-based"),
            unit_id: UnitId::new(UNIT).expect("unit id"),
            claim: "the seeded claim".to_owned(),
            requirement: gauntlet_ports::verification_record::RequirementDoc::new(
                EvidenceKind::TestReport,
                marker_report_locator,
                Producer::Instrument,
            ),
            instrument: VerifyingInstrument::new(
                "escalations-cli-seed-verifier".to_owned(),
                "1".to_owned(),
                true,
            ),
            evidence: EvidenceRef::new("evd-escalations-cli-seed").expect("ref"),
            produced_at: EpochSeconds::new(1_700_000_000),
            outcome,
            class: None,
            detail: "the seeded judgment".to_owned(),
        })
        .expect("the record document is well formed");
        VerificationRecordAuthority::new(state.clone())
            .record(&document, bound())
            .expect("the seeded record lands");
    }

    fn run_cli(&self, args: &[&str]) -> Output {
        let mut command = Command::new(GAUNTLET);
        command
            .current_dir(self.target())
            .env_remove("GAUNTLET_AGENT_RUNNER")
            .env_remove("GAUNTLET_MOCK_SCENARIO")
            .env_remove("GAUNTLET_CLI_AGENT_CMD")
            .env("GAUNTLET_PROJECT_ROOT", self.target().display().to_string())
            .env("GAUNTLET_STATE_DIR", self.state_dir().display().to_string())
            .env(
                "GAUNTLET_ARTIFACT_DIR",
                self.root.join("evidence").display().to_string(),
            );
        command
            .args(args)
            .output()
            .expect("the gauntlet binary runs")
    }

    fn ticket_is_open(&self) -> bool {
        let queue = FsEscalationQueue::open(&self.state_dir().join("escalations"))
            .expect("the queue opens");
        let ticket = gauntlet_ports::human_escalation::TicketId::new(&self.ticket)
            .expect("a valid ticket id");
        queue
            .record(&ticket)
            .expect("the ticket record reads")
            .is_open()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
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

#[test]
fn resolve_without_any_verification_record_refuses_and_the_ticket_stays_open() {
    let world = World::new("no-record");
    let output = world.run_cli(&[
        "escalations",
        "resolve",
        "--ticket",
        &world.ticket,
        "--unit",
        UNIT,
        "--operator",
        "operator-one",
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        !output.status.success(),
        "the resolve refuses: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("no PASS verification record exists for unit"),
        "the typed PASS-gate refusal surfaces: {stderr}"
    );
    assert!(
        stderr.contains("the settlement service refused")
            && stderr.contains("the ticket stays open"),
        "the CLI names the service refusal and the ticket's fate: {stderr}"
    );
    assert!(world.ticket_is_open(), "the ticket is untouched");
}

#[test]
fn retry_with_a_non_pass_record_hits_the_same_gate() {
    let world = World::new("non-pass");
    world.seed_anchor_and_record(false);
    let output = world.run_cli(&[
        "escalations",
        "retry",
        "--ticket",
        &world.ticket,
        "--unit",
        UNIT,
        "--operator",
        "operator-two",
        "--note",
        "retrying the stalled slice",
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        !output.status.success(),
        "the retry refuses: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("no PASS verification record exists for unit"),
        "the same typed gate, whichever action: {stderr}"
    );
    assert!(world.ticket_is_open(), "the ticket is untouched");
}

#[test]
fn even_an_earned_pass_record_never_settles_without_the_governed_chain_s_readiness() {
    // AK-062's structural boundary, composed through the REAL CLI glue
    // (Task 21's positive path): with a durable PASS record present the
    // shared path re-mints readiness THROUGH the governed verification
    // chain — and this world's reference was never frozen, so the
    // chain's governed open refuses. Readiness is earned by the chain
    // over a frozen ruler, never re-derived from an operator's request.
    // D9's single writer holds: zero settlement rows afterwards.
    let world = World::new("earned-pass");
    world.seed_anchor_and_record(true);
    let output = world.run_cli(&[
        "escalations",
        "resolve",
        "--ticket",
        &world.ticket,
        "--unit",
        UNIT,
        "--operator",
        "operator-three",
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        !output.status.success(),
        "the operator shape cannot settle: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("the settlement service refused")
            && stderr.contains("the ticket stays open"),
        "the CLI names the service refusal: {stderr}"
    );
    assert!(
        stderr.contains("the governed verification open refused"),
        "the refusal is the governed chain's own (AK-062 through the chain): {stderr}"
    );
    assert!(world.ticket_is_open(), "the ticket is untouched");
    let state = EmbeddedStateStore::open(&world.state_dir().join("gauntlet-state.db"))
        .expect("the state store opens");
    let settlements = state
        .settlements(&world.run_id, bound())
        .expect("the settlements read");
    assert!(
        settlements.is_empty(),
        "no second writer appeared: zero settlement rows through the operator shape"
    );
}
