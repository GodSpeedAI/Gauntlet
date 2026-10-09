//! P10 proof (plan Task 22; spec §23.1 P10 / §19.1–19.2; frozen unit
//! AK-055): over the REAL composed ledger substrate —
//!
//! 1. `one_successful_run_stays_observed` — a single successful run's
//!    developmental memory imports, the capability registers `observed`,
//!    and evaluation REFUSES: the configured minimum is not met (§19.2:
//!    one successful run is not capability);
//! 2. `three_identical_axis_settlements_are_refused` — N ≥ 3 settlements
//!    whose CONTENT axes are identical are refused promotion with
//!    `insufficient_variation` recorded: run ids and timestamps differ
//!    across every run, so only content is variation (the plan's
//!    Done-when teeth);
//! 3. `the_positive_path_is_the_only_road_to_promoted` — the FULL §19.2
//!    conjunction (≥3 settlements under real content variation + a
//!    handled recovery + non-increasing failure rate + no gate erosion +
//!    non-increasing orchestration burden) promotes through a durable
//!    decision record, and the decision survives reopening the ledger —
//!    the store is cross-run by design;
//! 4. `revoke_is_sticky` — a promoted capability revokes, evaluation
//!    thereafter refuses without writing, and no transition leaves
//!    `revoked`.
//!
//! Locator note (the p4/p7 precedent): AK-055's `evidence_required`
//! locator is `tests/proofs/p10_no_false_learning` — a repo-root proof
//! spelling that cannot be a cargo target of a core crate — so the proof
//! materializes as an integration test of the composition root whose
//! target NAME is `p10_no_false_learning`, making the plan's focused gate
//! run verbatim. The divergence is recorded in
//! `.agents/reports/reference-candidates.md` (AK-055 entry, 2026-09-03).
//!
//! Composition inventory (spec §4.1 — everything REAL unless this header
//! names the proof-local replacement and why):
//!
//! - the capability ledger: the REAL embedded-SQL capability ledger on a
//!   scratch database file (the same file the run state lives on);
//! - the run substrate: the REAL embedded-SQL state store, written ONLY
//!   through the earned production path — run identity, the durable
//!   checkpoint, the verification-record authority's mints, and the
//!   settlement-commit authority's admit + §16.3 commit. No settlement
//!   row or verification record is ever hand-built into state;
//! - the recovery seam: the REAL §18.3 `complete_pending_settlements`
//!   replay-completion driven over the adapter's documented
//!   damage-injection instrument (the settlement ROWS of one unit are
//!   deleted out of band — the COR-R5 reproduction class — and the replay
//!   must complete them); the recovery observation is appended by the
//!   import's `record_recovery` AFTER the completion returns a count
//!   greater than zero, never inside the settlement machinery;
//! - clock: the deterministic virtual clock (proof-local; the services'
//!   recording instants must be deterministic, and window ordering is the
//!   ledger's append order, never the clock);
//! - runner, observer, comparator, verifier: NOT INVOLVED — this slice's
//!   claim is about what the durable substrate proves after the loop has
//!   run, and every fact those roles would have produced is already
//!   durable in the substrate the import reads.
//!
//! Teeth declarations (each anti-false-learning assertion names the
//! mechanism it exercises; the falsifier mutates them). Honesty note
//! (COR-1 C5, from executed mutations A1/A2/A4): defense in depth is
//! real — removing ONE refusal condition does not produce a promotion,
//! because the remaining conditions still refuse the same window. The
//! per-leg teeth are therefore the named rule-presence assertions
//! themselves: mutating (or removing) a condition leaves the refusal to
//! the remaining conditions, and the named assertion is the tooth that
//! turns RED, proving the rule was present and load-bearing.
//!
//! - leg 1's tooth is the minimum-settlements condition in
//!   `PromotionService::evaluate` (`insufficient_settlements`): remove it
//!   and the refusal falls to the remaining conditions — the run still
//!   never promotes, and this leg's `insufficient_settlements` assertion
//!   turns RED.
//! - leg 2's tooth is the recorded-variation condition's content-axis
//!   comparison over `VariationAxis` (`insufficient_variation`): remove
//!   it and the refusal falls to the remaining conditions — three runs
//!   over identical content still never promote, and this leg's
//!   `insufficient_variation` assertion turns RED. (What variation may
//!   never rest on is absolute: run ids and timestamps are not axes —
//!   exactly the false learning AK-055 forbids.)
//! - leg 3's tooth is the CONJUNCTION itself plus its durability: each of
//!   the five conditions refuses independently (the module tests break
//!   each in isolation), the recovery observation exists only because the
//!   §18.3 replay completed real damage, and the promotion decision is a
//!   durable transition record that a freshly reopened ledger still
//!   reads.
//! - leg 4's tooth is the transition state machine's terminal `revoked`
//!   (`is_legal_transition` + the record types refusing to compose a move
//!   out of it): remove it and a revoked capability flips back without
//!   re-earning anything.

use std::path::PathBuf;
use std::sync::Arc;

use gauntlet_adapter_agent_mock::DeterministicClock;
use gauntlet_adapter_state_postgres::{EmbeddedCapabilityLedger, EmbeddedStateStore, test_support};
use gauntlet_app::memory::developmental::DevelopmentalImport;
use gauntlet_app::memory::promotion::{PromotionDecision, PromotionService, PromotionServiceError};
use gauntlet_domain::{
    Budget, CapabilityId, CapabilityStatus, DiagnosticAllowance, Digest, EpochSeconds,
    EvidenceKind, Limits, MicroUsd, Outcome, ReferenceId, ReferenceVersion, Run, RunDefinition,
    RunId, UnitId,
};
use gauntlet_ports::capability_ledger::{CapabilityEntry, CapabilityLedger};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::producer_authority::{SettlementCommitAuthority, VerificationRecordAuthority};
use gauntlet_ports::state_store::{Checkpoint, SettlementCommit, StateStore};
use gauntlet_ports::verification_record::{
    RequirementDoc, VerificationContent, VerificationRecordDoc, VerifyingInstrument,
};

// ---------------------------------------------------------------------------
// Named fixture figures (the literal-scan idiom)
// ---------------------------------------------------------------------------

const BASE_INSTANT: u64 = 1_700_000_000;
/// The bound every durable call runs under, on its own line (numeric
/// literals never share a line with a duration-shaped identifier).
const BOUND_SPAN: u64 = 600;
const CHECKPOINT_ITERATION: u32 = 1;
const MIN_SETTLEMENTS: u32 = 3;
/// The durable spend counters the fixture checkpoint carries (the payment
/// observation's substrate fact).
const SPENT_ROUNDS: u32 = 1;
const SPENT_WALL_SPAN: u64 = 30;
const SPENT_TOKENS: u64 = 400;
const SPENT_COST_MICROS: u64 = 5;
const BUDGET_ROUNDS: u32 = 8;
const BUDGET_PARALLEL: u32 = 2;
const BUDGET_DEPTH: u32 = 1;
const BUDGET_WALL_SPAN: u64 = 3_600;
const BUDGET_TOKENS: u64 = 2_000_000;
const BUDGET_COST_MICROS: u64 = 500_000;
const RESERVE_PCT: u8 = 15;
const DIAGNOSTIC_TOKENS: u64 = 100;
const DIAGNOSTIC_COST_MICROS: u64 = 50;

/// The frozen reference every fixture run settles against — ONE reference,
/// so the reference axis never varies and variation must come from
/// content the runs actually differ on.
const REFERENCE_ID: &str = "ref-p10-proof";
const REFERENCE_VERSION: u32 = 1;
const REFERENCE_DIGEST: &str = "sha256:1a2b3c4d5e6f1a2b";
const INTENT: &str = "the p10 proof's fixture run";

/// The fixture units: leg 2 settles THE SAME unit three times (identical
/// axes); leg 3 settles three DIFFERENT units (real variation).
const IDENTICAL_UNIT: &str = "AK-9701";
const VARIED_UNITS: [&str; 3] = ["AK-9701", "AK-9702", "AK-9703"];
const EVIDENCE_LOCATOR: &str = "p10-fixture-report.json";
const INSTRUMENT_NAME: &str = "gauntlet-p10-fixture-instrument";
const CLAIM: &str = "the unit's behavioral claim holds";

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + BOUND_SPAN))
}

fn clock() -> Arc<DeterministicClock> {
    Arc::new(DeterministicClock::start_at(EpochSeconds::new(
        BASE_INSTANT,
    )))
}

fn reference_digest() -> Digest {
    Digest::parse(REFERENCE_DIGEST).expect("the fixture digest is well formed")
}

// ---------------------------------------------------------------------------
// Scratch
// ---------------------------------------------------------------------------

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    /// One database file per leg: the ledger and the run state share it,
    /// as they would in production.
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("gauntlet-p10-proof-{tag}-{}", std::process::id()));
        std::fs::remove_file(&path).ok();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("the p10 scratch parent is creatable");
        }
        Self { path }
    }

    fn open_state(&self) -> Arc<EmbeddedStateStore> {
        Arc::new(EmbeddedStateStore::open_at(&self.path).expect("the p10 state store opens"))
    }

    fn open_ledger(&self) -> Arc<EmbeddedCapabilityLedger> {
        Arc::new(EmbeddedCapabilityLedger::open_at(&self.path).expect("the p10 ledger opens"))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_file(&self.path).ok();
    }
}

// ---------------------------------------------------------------------------
// The earned substrate: every run fact lands through the production path
// ---------------------------------------------------------------------------

fn durable_run(id: &RunId) -> Run {
    Run::new(RunDefinition {
        id: id.clone(),
        created_at: EpochSeconds::new(BASE_INSTANT),
        reference_version: ReferenceVersion::new(
            ReferenceId::new(REFERENCE_ID).expect("reference id"),
            REFERENCE_VERSION,
            reference_digest(),
        ),
        intent: INTENT.to_owned(),
        config_fingerprint: Digest::new("sha256", "0f1e2d3c4b5a6978").expect("digest"),
        environment_fingerprint: Digest::new("sha256", "78695a4b3c2d1e0f").expect("digest"),
        budget: Budget::new(
            Limits::new(
                BUDGET_ROUNDS,
                BUDGET_PARALLEL,
                BUDGET_DEPTH,
                BUDGET_WALL_SPAN,
                BUDGET_TOKENS,
                MicroUsd::new(BUDGET_COST_MICROS),
            )
            .expect("the fixture limits are usable"),
            RESERVE_PCT,
            DiagnosticAllowance::new(DIAGNOSTIC_TOKENS, MicroUsd::new(DIAGNOSTIC_COST_MICROS)),
        )
        .expect("the fixture budget is usable"),
        parent_run: None,
    })
    .expect("the fixture run is valid")
}

fn pass_record(run_id: &RunId, unit: &str, sequence: usize) -> VerificationRecordDoc {
    let verification = format!("vrf-p10-{sequence}");
    VerificationRecordDoc::new(VerificationContent {
        verification: gauntlet_domain::VerificationRef::new(&verification).expect("ref"),
        run_id: run_id.clone(),
        iteration_no: gauntlet_domain::IterationNo::new(CHECKPOINT_ITERATION)
            .expect("iterations are 1-based"),
        unit_id: UnitId::new(unit).expect("unit id"),
        claim: CLAIM.to_owned(),
        requirement: RequirementDoc::new(
            EvidenceKind::TestReport,
            gauntlet_domain::EvidenceLocator::new(EVIDENCE_LOCATOR).expect("locator"),
            gauntlet_domain::Producer::Instrument,
        ),
        instrument: VerifyingInstrument::new(INSTRUMENT_NAME.to_owned(), "1".to_owned(), true),
        evidence: gauntlet_domain::EvidenceRef::new(&format!("evd-p10-{sequence}"))
            .expect("evidence ref"),
        produced_at: EpochSeconds::new(BASE_INSTANT + 10),
        outcome: Outcome::Pass,
        class: None,
        detail: "the deterministic instrument decided PASS".to_owned(),
    })
    .expect("the fixture record is well formed")
}

/// Lands ONE run's full successful substrate through the earned path:
/// identity, checkpoint, the authority-minted PASS record, and the
/// authority-admitted §16.3 settlement commit.
fn settle_one_run(
    state: &Arc<EmbeddedStateStore>,
    clock: &DeterministicClock,
    run_sequence: usize,
    unit: &str,
) {
    let run_id = RunId::new(&format!("01ARZ3NDEKTSV4RRFFQ69G5F{run_sequence:02X}"))
        .expect("the fixture run id is a ULID");
    state
        .record_run(&durable_run(&run_id), bound())
        .expect("the run identity records");
    state
        .checkpoint(
            Checkpoint {
                run_id: run_id.clone(),
                reference_digest: reference_digest(),
                iteration_no: gauntlet_domain::IterationNo::new(CHECKPOINT_ITERATION)
                    .expect("iterations are 1-based"),
                unit_states: Vec::new(),
                budget_spent: gauntlet_domain::Spent::from_counters(
                    SPENT_ROUNDS,
                    SPENT_WALL_SPAN,
                    SPENT_TOKENS,
                    MicroUsd::new(SPENT_COST_MICROS),
                    0,
                    MicroUsd::new(0),
                ),
                in_flight: Vec::new(),
                event_log_offset: gauntlet_domain::EventLogOffset::new(0),
            },
            bound(),
        )
        .expect("the checkpoint lands");
    let record = pass_record(&run_id, unit, run_sequence);
    VerificationRecordAuthority::new(state.clone())
        .record(&record, bound())
        .expect("the PASS record is earned and durable");
    let commit = SettlementCommit {
        run_id: run_id.clone(),
        unit_id: UnitId::new(unit).expect("unit id"),
        evidence: record.evidence().clone(),
        verification: record.verification().clone(),
        reference_digest: reference_digest(),
        at: clock.now(),
    };
    let admitted = SettlementCommitAuthority::new(state.clone())
        .admit(commit, bound())
        .expect("the settlement admits");
    state
        .commit_settlement(&admitted, bound())
        .expect("the settlement commits");
}

fn import_for(
    state: &Arc<EmbeddedStateStore>,
    ledger: &Arc<EmbeddedCapabilityLedger>,
    clock: &Arc<DeterministicClock>,
) -> DevelopmentalImport {
    DevelopmentalImport::new(state.clone(), ledger.clone(), clock.clone())
}

fn service_for(ledger: &Arc<EmbeddedCapabilityLedger>) -> PromotionService {
    PromotionService::new(ledger.clone(), MIN_SETTLEMENTS, clock())
        .expect("the spec default minimum is valid")
}

fn capability() -> CapabilityId {
    CapabilityId::new("cap-p10-no-false-learning").expect("capability id")
}

fn entry_of(ledger: &EmbeddedCapabilityLedger) -> CapabilityEntry {
    ledger
        .capability(&capability(), bound())
        .expect("the register reads")
        .expect("the capability is registered")
}

// ---------------------------------------------------------------------------
// Leg 1 (AK-055, first half): one successful run is not capability
// ---------------------------------------------------------------------------

/// Tooth: the `insufficient_settlements` condition in
/// `PromotionService::evaluate`. Removing it leaves the refusal to the
/// remaining conditions — the run still never promotes — and this leg's
/// rule-presence assertion is the tooth that turns RED.
#[test]
fn one_successful_run_is_observed_and_never_promoted() {
    let scratch = Scratch::new("single-success");
    let state = scratch.open_state();
    let ledger = scratch.open_ledger();
    let clock = clock();
    settle_one_run(&state, &clock, 1, IDENTICAL_UNIT);

    // The developmental import records the run's memory ungated: the
    // settlement, the PASS attempt, and the checkpoint's payment fact.
    let summary = import_for(&state, &ledger, &clock)
        .import_run(&capability(), &run_id_of(1), bound())
        .expect("the import projects the run");
    assert!(
        summary.appended >= 3,
        "the settlement, the attempt, and the payment are memory: {summary:?}"
    );

    // The register entry: observed, never evaluated.
    let entry = entry_of(&ledger);
    assert_eq!(
        entry.status,
        CapabilityStatus::Observed,
        "a single successful run registers observed — import alone promotes nothing"
    );
    assert_eq!(entry.last_transition_at, None, "nothing was ever evaluated");

    // Evaluation refuses on the configured minimum, and the refusal writes
    // PROVISIONAL with the reasons durable — never promoted.
    let decision = service_for(&ledger)
        .evaluate(&capability(), bound())
        .expect("the evaluation runs");
    match decision {
        PromotionDecision::Refused { refusals, entry } => {
            assert!(
                refusals
                    .iter()
                    .any(|refusal| refusal.rule() == "insufficient_settlements"),
                "the single settlement is below the minimum: {refusals:?}"
            );
            assert_eq!(entry.status, CapabilityStatus::Provisional);
        }
        PromotionDecision::Promoted { .. } => {
            panic!("one successful run promoted — one successful run is NOT capability (spec 19.2)")
        }
    }
    let history = ledger
        .transitions(&capability(), bound())
        .expect("the history reads");
    assert_eq!(
        history.len(),
        1,
        "the refusal is durable, exactly one record"
    );
}

fn run_id_of(sequence: usize) -> RunId {
    RunId::new(&format!("01ARZ3NDEKTSV4RRFFQ69G5F{sequence:02X}")).expect("fixture run id")
}

// ---------------------------------------------------------------------------
// Leg 2 (AK-055, the plan's Done-when teeth): identical axes refuse
// ---------------------------------------------------------------------------

/// Tooth: the `insufficient_variation` content-axis comparison in
/// `PromotionService::evaluate`. Removing it leaves the refusal to the
/// remaining conditions — these three runs still never promote — and this
/// leg's rule-presence assertion is the tooth that turns RED. What
/// variation may never rest on is absolute: run ids and timestamps are
/// not axes.
#[test]
fn three_identical_axis_settlements_are_refused() {
    let scratch = Scratch::new("identical-axes");
    let state = scratch.open_state();
    let ledger = scratch.open_ledger();
    let clock = clock();
    // Three honest, fully settled runs — all over THE SAME unit, the SAME
    // frozen reference, the SAME evidence kind and locator class. Only the
    // run ids and timestamps differ.
    for sequence in 1..=3 {
        settle_one_run(&state, &clock, sequence, IDENTICAL_UNIT);
        let summary = import_for(&state, &ledger, &clock)
            .import_run(&capability(), &run_id_of(sequence), bound())
            .expect("the import projects the run");
        assert!(
            summary.appended >= 2,
            "each run's memory lands: {summary:?}"
        );
    }

    let decision = service_for(&ledger)
        .evaluate(&capability(), bound())
        .expect("the evaluation runs");
    match decision {
        PromotionDecision::Refused { refusals, entry } => {
            assert!(
                refusals
                    .iter()
                    .any(|refusal| refusal.rule() == "insufficient_variation"),
                "identical content axes are recorded as insufficient variation: {refusals:?}"
            );
            assert_eq!(entry.status, CapabilityStatus::Provisional);
        }
        PromotionDecision::Promoted { evidence, .. } => {
            panic!(
                "three settlements under identical content axes promoted — false learning \
                 through the run-id trap (variation_axes: {:?})",
                evidence.variation_axes
            )
        }
    }
    // The refusal reasons are durable.
    let history = ledger
        .transitions(&capability(), bound())
        .expect("the history reads");
    assert!(matches!(
        history.last().expect("the refusal is durable").reason(),
        gauntlet_ports::capability_ledger::TransitionReason::Refused(refusals)
            if refusals.iter().any(|r| r.rule() == "insufficient_variation")
    ));
}

// ---------------------------------------------------------------------------
// Leg 3: the full §19.2 positive path is the only road to promoted
// ---------------------------------------------------------------------------

/// Tooth: the conjunction — each condition refuses independently (broken
/// one at a time by the module tests), the recovery observation exists
/// only because the §18.3 replay completed real damage, and the promotion
/// decision is durable across a freshly reopened ledger.
#[test]
fn the_positive_path_is_the_only_road_to_promoted() {
    let scratch = Scratch::new("positive-path");
    let state = scratch.open_state();
    let ledger = scratch.open_ledger();
    let clock = clock();
    // Three settled runs under REAL variation: three different units.
    for (sequence, unit) in VARIED_UNITS.iter().enumerate() {
        settle_one_run(&state, &clock, sequence + 1, unit);
    }

    // The recovery seam, in the honest order: FIRST the documented damage
    // (the adapter's instrument deletes the settlement ROWS of the second
    // run's unit while the settlement EVENT stays durable), THEN the §18.3
    // replay-completion, which returns a count greater than zero. The
    // recovery observation is appended HERE — after the completion, by the
    // import's ungated append — never inside the settlement machinery.
    let damaged = test_support::delete_settlement_rows(&scratch.path, Some(VARIED_UNITS[1]))
        .expect("the damage lands");
    assert_eq!(
        damaged, 1,
        "exactly the second run's settlement row is damaged"
    );
    let completed = state
        .complete_pending_settlements(
            &run_id_of(2),
            gauntlet_domain::EventLogOffset::new(0),
            bound(),
        )
        .expect("the replay completes");
    assert_eq!(
        completed, 1,
        "the §18.3 completion added exactly the missing row"
    );
    import_for(&state, &ledger, &clock)
        .record_recovery(
            &capability(),
            &run_id_of(2),
            "recovery:settlement-replay".to_owned(),
            completed,
            "the §18.3 replay-completion handled one half-applied settlement".to_owned(),
            bound(),
        )
        .expect("the recovery is memory");

    // The developmental import projects all three runs.
    for sequence in 1..=3 {
        let summary = import_for(&state, &ledger, &clock)
            .import_run(&capability(), &run_id_of(sequence), bound())
            .expect("the import projects the run");
        assert!(
            summary.appended >= 2,
            "each run's memory lands: {summary:?}"
        );
    }

    // The evaluation: every §19.2 condition holds, and the road ends in a
    // durable promotion decision.
    let decision = service_for(&ledger)
        .evaluate(&capability(), bound())
        .expect("the evaluation runs");
    let evidence = match decision {
        PromotionDecision::Promoted { evidence, entry } => {
            assert_eq!(entry.status, CapabilityStatus::Promoted);
            evidence
        }
        PromotionDecision::Refused { refusals, .. } => {
            panic!("the full §19.2 evidence was refused: {refusals:?}")
        }
    };
    assert_eq!(evidence.settlements_observed, 3);
    assert_eq!(evidence.min_settlements_configured, MIN_SETTLEMENTS);
    assert_eq!(evidence.recovery_observations, 1);
    // Real variation: three distinct unit axes in the durable decision.
    let unit_axes: Vec<_> = evidence
        .variation_axes
        .iter()
        .filter(|axis| axis.sort_key().starts_with("unit:"))
        .collect();
    assert_eq!(
        unit_axes.len(),
        3,
        "the decision records the three units the variation is real over"
    );
    // The decision names the settlements' evidence.
    assert_eq!(evidence.settlements.len(), 3);

    // The decision record is DURABLE and the register is cross-run: a
    // freshly reopened ledger reads the promoted status and the promotion
    // transition with its evidence.
    drop(ledger);
    let reopened = scratch.open_ledger();
    let entry = entry_of(&reopened);
    assert_eq!(
        entry.status,
        CapabilityStatus::Promoted,
        "the promotion survives reopening the ledger — cross-run by design"
    );
    let history = reopened
        .transitions(&capability(), bound())
        .expect("the history reads");
    assert_eq!(history.len(), 1);
    match history[0].reason() {
        gauntlet_ports::capability_ledger::TransitionReason::Promoted(recorded) => {
            assert_eq!(recorded.settlements.len(), 3);
            assert_eq!(
                history[0].evidence().len(),
                3,
                "each settlement's evidence is named"
            );
        }
        other => panic!("the durable decision record is not a promotion: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Leg 4: revoke is sticky
// ---------------------------------------------------------------------------

/// Tooth: the transition state machine's terminal `revoked`
/// (`is_legal_transition` + the record types) — remove it and a revoked
/// capability flips back without re-earning.
#[test]
fn revoke_is_sticky_and_has_no_flip_back() {
    let scratch = Scratch::new("revoke-sticky");
    let state = scratch.open_state();
    let ledger = scratch.open_ledger();
    let clock = clock();
    // The positive path earns the promotion the revoke revokes.
    for (sequence, unit) in VARIED_UNITS.iter().enumerate() {
        settle_one_run(&state, &clock, sequence + 1, unit);
    }
    let damaged = test_support::delete_settlement_rows(&scratch.path, Some(VARIED_UNITS[1]))
        .expect("the damage lands");
    assert_eq!(damaged, 1);
    let completed = state
        .complete_pending_settlements(
            &run_id_of(2),
            gauntlet_domain::EventLogOffset::new(0),
            bound(),
        )
        .expect("the replay completes");
    assert_eq!(completed, 1);
    let importer = import_for(&state, &ledger, &clock);
    importer
        .record_recovery(
            &capability(),
            &run_id_of(2),
            "recovery:settlement-replay".to_owned(),
            completed,
            "the §18.3 replay-completion handled one half-applied settlement".to_owned(),
            bound(),
        )
        .expect("the recovery is memory");
    for sequence in 1..=3 {
        importer
            .import_run(&capability(), &run_id_of(sequence), bound())
            .expect("the import projects the run");
    }
    let service = service_for(&ledger);
    service
        .evaluate(&capability(), bound())
        .expect("the full evidence promotes");
    assert_eq!(entry_of(&ledger).status, CapabilityStatus::Promoted);

    // The revoke.
    let entry = service
        .revoke(
            &capability(),
            "the operator revoked the capability after a regression".to_owned(),
            bound(),
        )
        .expect("the revoke applies");
    assert_eq!(entry.status, CapabilityStatus::Revoked);

    // Evaluation thereafter refuses WITHOUT writing, and the register
    // stays revoked.
    assert!(matches!(
        service.evaluate(&capability(), bound()),
        Err(PromotionServiceError::RevokedIsSticky { .. })
    ));
    assert_eq!(entry_of(&ledger).status, CapabilityStatus::Revoked);

    // A second revoke is the same sticky refusal: no flip, no redo.
    assert!(matches!(
        service.revoke(&capability(), "again".to_owned(), bound()),
        Err(PromotionServiceError::RevokedIsSticky { .. })
    ));

    // The history ends at the promotion and the revoke — nothing after.
    let history = ledger
        .transitions(&capability(), bound())
        .expect("the history reads");
    assert_eq!(history.len(), 2, "the sticky refusals wrote nothing");
    assert!(matches!(
        history[1].reason(),
        gauntlet_ports::capability_ledger::TransitionReason::Revoked(_)
    ));
}
