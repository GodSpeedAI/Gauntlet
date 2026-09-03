//! The developmental import (plan Task 22, charter D3): projects one run's
//! durable substrate — the typed `StateStore` reads the port CAN answer —
//! into the capability's observation log.
//!
//! ## Import, not dual-write
//!
//! Nothing in the run loop writes the ledger. The import is a projection
//! over reads: run identity (the frozen reference digest), the latest
//! checkpoint's durable spend, the verification records (typed outcomes,
//! including FAIL and INCONCLUSIVE), the correction selections, and the
//! settlement rows. Invocation-level records have NO port reader, and
//! comparison documents are write-only on the store — so attempts and
//! failures are projected from what the port CAN read (verification
//! outcomes), and comparison-class observations are never invented. The
//! row type records exactly the facts available; the §19.1 vocabulary is
//! honest about absence.
//!
//! ## Ungated and idempotent
//!
//! The import appends every projected row with the same eagerness — a
//! failed attempt is still memory (spec §19.1: "Raw. Immediate,
//! append-only"). Appends are idempotent on the declared observation
//! identity, so re-importing a run (a recovery replay, a second evaluation
//! pass) appends nothing and changes nothing; an identity whose facts
//! disagree is the ledger's typed conflict, never a silent overwrite.
//!
//! Each row's `at` is the RECORDING instant (this service's `Clock`
//! reading) — never a claimed fact of the substrate. The substrate's own
//! occurrence instant is projected where it exists: every attempt,
//! failure, and settlement row carries `occurred_at` from its verification
//! record's `produced_at` (the settlement from its MATCHING record), so
//! promotion windows decide on substrate chronology, never on the import
//! call order. Payment, correction, and recovery rows are not decided on;
//! they invent no occurrence fact.
//!
//! ## Call sites (charter D3)
//!
//! Before any promotion evaluation, and after a run reaches a terminal
//! state in the report path. Both are append-only over the ledger and
//! neither touches the settlement path. The production wiring of those two
//! call sites arrives with the consumers (the plan's optimizer task); this
//! slice ships the service and its proof.

use std::sync::Arc;

use gauntlet_domain::{CapabilityId, Outcome, RunId};
use gauntlet_ports::capability_ledger::{
    CapabilityLedger, CapabilityLedgerError, ObservationClass, ObservationIdentity,
    ObservationRecord,
};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::state_store::{StateStore, StateStoreError};

/// Projects a run's durable substrate into a capability's developmental
/// memory.
#[derive(Clone)]
pub struct DevelopmentalImport {
    state: Arc<dyn StateStore>,
    ledger: Arc<dyn CapabilityLedger>,
    clock: Arc<dyn Clock>,
}

/// What one import appended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportSummary {
    /// Rows this call appended (new memories).
    pub appended: usize,
    /// Rows the ledger already carried under the same identity with
    /// agreeing facts (the idempotent no-ops).
    pub already_present: usize,
}

/// Import failures — typed (spec §21.2). A ledger append failure is
/// surfaced mid-import: the appends before it are durable memory (the log
/// is append-only), and a re-import is idempotent, so a retry cannot
/// duplicate them.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportError {
    /// The run has no durable identity — nothing can be projected.
    RunNotDurable { run_id: RunId },
    /// A recovery record was claimed with a completion count of zero —
    /// `complete_pending_settlements` completed nothing, so there is no
    /// handled recovery to record. Nothing was appended.
    RecoveryNotEarned { completed: usize },
    /// The substrate read failed.
    State(StateStoreError),
    /// An observation append failed or conflicted.
    Ledger(CapabilityLedgerError),
}

impl core::fmt::Display for ImportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RunNotDurable { run_id } => {
                write!(
                    f,
                    "run {run_id} is not durable; there is no substrate to project"
                )
            }
            Self::RecoveryNotEarned { completed } => write!(
                f,
                "a recovery record with a completion count of {completed} is not an earned \
                 write — §18.3 completed nothing, so there is no handled recovery to record \
                 (spec 19.2: plus a recovery event handled)"
            ),
            Self::State(error) => write!(f, "the developmental import's read failed: {error}"),
            Self::Ledger(error) => write!(f, "the developmental import's append failed: {error}"),
        }
    }
}

impl std::error::Error for ImportError {}

impl DevelopmentalImport {
    pub fn new(
        state: Arc<dyn StateStore>,
        ledger: Arc<dyn CapabilityLedger>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            state,
            ledger,
            clock,
        }
    }

    /// Projects `run_id`'s durable facts into `capability`'s observation
    /// log. Ungated: the run's settlements, attempts, failures,
    /// corrections, and payments are memory regardless of outcome.
    pub fn import_run(
        &self,
        capability: &CapabilityId,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<ImportSummary, ImportError> {
        let run = self
            .state
            .load_run(run_id, deadline)
            .map_err(ImportError::State)?
            .ok_or_else(|| ImportError::RunNotDurable {
                run_id: run_id.clone(),
            })?;
        let reference_digest = run.reference_version().digest().clone();
        let at = self.clock.now();
        let mut summary = ImportSummary {
            appended: 0,
            already_present: 0,
        };
        // The durable payment fact: the latest checkpoint's spend counters.
        if let Some(checkpoint) = self
            .state
            .resume(run_id, deadline)
            .map_err(ImportError::State)?
        {
            let identity = ObservationIdentity::new(
                capability.clone(),
                ObservationClass::Payment,
                run_id.clone(),
                "checkpoint-spend".to_owned(),
            )
            .expect("the payment identity carries a fact");
            let record = ObservationRecord::new(
                identity,
                at,
                "the run's latest durable checkpoint spend".to_owned(),
            )
            .with_spend(checkpoint.budget_spent)
            .with_reference_digest(reference_digest.clone());
            self.append(record, &mut summary, deadline)?;
        }
        // The durable outcome facts: every verification record, PASS, FAIL,
        // or INCONCLUSIVE — an attempt that decided, an attempt that could
        // not decide, or a failure, exactly as the instrument recorded.
        let records = self
            .state
            .verification_records(run_id, deadline)
            .map_err(ImportError::State)?;
        for record in &records {
            let class = match record.outcome() {
                Outcome::Fail => ObservationClass::Failure,
                Outcome::Pass | Outcome::Inconclusive => ObservationClass::Attempt,
            };
            let identity = ObservationIdentity::new(
                capability.clone(),
                class,
                run_id.clone(),
                format!("verification:{}", record.verification().as_str()),
            )
            .expect("the verification identity carries a fact");
            let observation = ObservationRecord::new(identity, at, record.claim().to_owned())
                .with_unit(record.unit_id().clone())
                .with_outcome(record.outcome())
                .with_instrument_deterministic(record.instrument().deterministic())
                .with_evidence_kind(record.requirement().kind())
                .with_evidence_locator(record.requirement().locator().clone())
                .with_reference_digest(reference_digest.clone())
                .with_verification(record.verification().clone())
                // The substrate occurrence fact (spec §19.2 chronology):
                // when the instrument decided, so the promotion windows
                // order by substrate chronology, never import order.
                .with_occurred_at(record.produced_at());
            self.append(observation, &mut summary, deadline)?;
        }
        // The §15.1 selections: the orchestration-burden proxy's raw
        // material, recorded raw.
        let selections = self
            .state
            .correction_selections(run_id, deadline)
            .map_err(ImportError::State)?;
        for (index, selection) in selections.iter().enumerate() {
            let identity = ObservationIdentity::new(
                capability.clone(),
                ObservationClass::Correction,
                run_id.clone(),
                format!("correction:{index}"),
            )
            .expect("the correction identity carries a fact");
            let observation =
                ObservationRecord::new(identity, at, selection.chosen().summary().to_owned())
                    .with_reference_digest(reference_digest.clone());
            self.append(observation, &mut summary, deadline)?;
        }
        // The durable settlements: the §19.2 settlement window's rows. The
        // content axes come from the settlement row and its matching
        // verification record (unit, frozen reference, evidence kind and
        // locator class, instrument) — never from ids and timestamps,
        // which differ across every run and are the false-variation trap.
        let settlements = self
            .state
            .settlements(run_id, deadline)
            .map_err(ImportError::State)?;
        for settlement in &settlements {
            let decided = records
                .iter()
                .find(|record| record.verification() == settlement.verification());
            let identity = ObservationIdentity::new(
                capability.clone(),
                ObservationClass::Settlement,
                run_id.clone(),
                format!(
                    "settlement:{}:{}",
                    settlement.unit_id().as_str(),
                    settlement.verification().as_str()
                ),
            )
            .expect("the settlement identity carries a fact");
            let mut observation = ObservationRecord::new(
                identity,
                at,
                format!("unit {} settled", settlement.unit_id()),
            )
            .with_unit(settlement.unit_id().clone())
            .with_reference_digest(reference_digest.clone())
            .with_verification(settlement.verification().clone())
            // The typed orchestration-burden proxy (charter D4 condition 5):
            // the run's correction selections per settlement — a proxy over
            // recorded instruments, honestly not a semantic proof.
            .with_orchestration_cost(selections.len() as u64);
            if let Some(decided) = decided {
                observation = observation
                    .with_outcome(decided.outcome())
                    .with_instrument_deterministic(decided.instrument().deterministic())
                    .with_evidence_kind(decided.requirement().kind())
                    .with_evidence_locator(decided.requirement().locator().clone())
                    .with_evidence(decided.evidence().clone())
                    // The settlement's occurrence is ITS MATCHING
                    // verification record's produced_at.
                    .with_occurred_at(decided.produced_at());
            }
            self.append(observation, &mut summary, deadline)?;
        }
        Ok(summary)
    }

    /// Records one handled recovery (charter D4): the recovery-completion
    /// call site appends this AFTER `complete_pending_settlements` returns
    /// its count — an append-only observation, never a settlement-path
    /// change. `fact` discriminates the recovery event within the run (a
    /// same-run replay of the SAME recovery is the ledger's idempotent
    /// no-op).
    ///
    /// Two rooting refusals, both appending nothing (COR-1 C4): a
    /// `completed` count of zero is `RecoveryNotEarned` — §18.3 completed
    /// nothing, so there is no handled recovery; and the run must be
    /// durable (`state.load_run` is `Some`), else `RunNotDurable` —
    /// recovery credit for a run that exists nowhere is not an earned
    /// write.
    ///
    /// RECORDED boundary: `completed` is caller-supplied — the production
    /// caller is the recovery-completion call site, passing the count §18.3
    /// returned. Deeper sealing (a typed completion receipt minted by the
    /// store, the house authority pattern) is a recorded ports-revision
    /// candidate.
    pub fn record_recovery(
        &self,
        capability: &CapabilityId,
        run_id: &RunId,
        fact: String,
        completed: usize,
        detail: String,
        deadline: Deadline,
    ) -> Result<(), ImportError> {
        if completed == 0 {
            return Err(ImportError::RecoveryNotEarned { completed });
        }
        if self
            .state
            .load_run(run_id, deadline)
            .map_err(ImportError::State)?
            .is_none()
        {
            return Err(ImportError::RunNotDurable {
                run_id: run_id.clone(),
            });
        }
        let identity = ObservationIdentity::new(
            capability.clone(),
            ObservationClass::Recovery,
            run_id.clone(),
            fact,
        )
        .map_err(|error| {
            ImportError::Ledger(CapabilityLedgerError::Corrupt {
                detail: error.to_string(),
            })
        })?;
        let observation = ObservationRecord::new(identity, self.clock.now(), detail);
        let mut summary = ImportSummary {
            appended: 0,
            already_present: 0,
        };
        self.append(observation, &mut summary, deadline)?;
        Ok(())
    }

    /// One ungated append, counted into the summary.
    fn append(
        &self,
        observation: ObservationRecord,
        summary: &mut ImportSummary,
        deadline: Deadline,
    ) -> Result<(), ImportError> {
        let receipt = self
            .ledger
            .append_observation(observation, deadline)
            .map_err(ImportError::Ledger)?;
        if receipt.idempotent {
            summary.already_present += 1;
        } else {
            summary.appended += 1;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gauntlet_domain::{
        Budget, BudgetFacts, CapabilityStatus, CorrectionDescriptor, CorrectionSelection,
        CorrectionShape, DecisionStep, Digest, EpochSeconds, EventLogOffset, EvidenceKind,
        EvidenceLocator, EvidenceRef, IterationNo, Limits, MicroUsd, Outcome, Producer,
        ReferenceId, ReferenceVersion, RejectedAlternative, RejectionClass,
        RejectionReason as Reason, Run, RunDefinition, RunId, SelectionDimension, SettlementRecord,
        UnitId, VerificationRef,
    };
    use gauntlet_ports::capability_ledger::{
        CapabilityLedger, CapabilityLedgerError, ObservationClass, ObservationIdentity,
        ObservationRecord,
    };
    use gauntlet_ports::clock::Clock;
    use gauntlet_ports::deadline::Deadline;
    use gauntlet_ports::state_store::{Checkpoint, RunEvent, StateStore, StateStoreError};
    use gauntlet_ports::verification_record::{
        RequirementDoc, VerificationContent, VerificationRecordDoc, VerifyingInstrument,
    };

    use super::{DevelopmentalImport, ImportError};
    use crate::memory::promotion::{PromotionDecision, PromotionService};
    use crate::memory::testing::{FixedClock, MemoryLedger, bound};

    const CAP: &str = "cap-import-tests";
    /// The fixture locator: a single-segment locator name (path-shaped
    /// literals are environment coupling wherever they are rooted).
    const FIXTURE_LOCATOR: &str = "p10-fixture-locator";
    /// The checkpoint fixture's figures, named for the same reason as the
    /// selection figures below (numeric literals never share a chunk with
    /// a duration-shaped identifier — the domain's fixture idiom).
    const CKPT_ITERATION: u32 = 3;
    const CKPT_ROUNDS: u32 = 3;
    const CKPT_SPAN: u64 = 180;
    const CKPT_TOKENS: u64 = 900;
    const CKPT_COST: u64 = 12;
    const CKPT_DIAGNOSTIC_TOKENS: u64 = 0;
    const CKPT_DIAGNOSTIC_COST: u64 = 0;
    const CKPT_OFFSET: u64 = 30;
    /// The selection fixture's figures, named (numeric literals never
    /// share a chunk with a duration-shaped identifier — the domain's
    /// correction.rs fixture idiom).
    const REQ_ROUNDS: u32 = 1;
    const REQ_SPAN: u64 = 60;
    const REQ_TOKENS: u64 = 400;
    const REQ_COST: u64 = 5;
    const REM_ROUNDS: u32 = 6;
    const REM_SPAN: u64 = 2_700;
    const REM_TOKENS: u64 = 400;
    const REM_COST: u64 = 395;
    const RUN: &str = "01ARZ3NDEKTSV4RRFFQ69G5FC1";
    const REF: &str = "sha256:ab00000000000000000000000000000000000000000000000000000000000000";
    const BASE: u64 = 1_700_000_000;

    use gauntlet_domain::CapabilityId;

    fn capability() -> CapabilityId {
        CapabilityId::new(CAP).expect("test capability id")
    }

    fn run_id() -> RunId {
        RunId::new(RUN).expect("test run id")
    }

    fn reference_digest() -> Digest {
        Digest::parse(REF).expect("test digest")
    }

    fn durable_run() -> Run {
        Run::new(RunDefinition {
            id: run_id(),
            created_at: EpochSeconds::new(BASE),
            reference_version: ReferenceVersion::new(
                ReferenceId::new("ref-import-tests").expect("reference id"),
                1,
                reference_digest(),
            ),
            intent: "the developmental import's fixture run".to_owned(),
            config_fingerprint: Digest::new("sha256", "a1b2c3d4e5f6a7b8").expect("digest"),
            environment_fingerprint: Digest::new("sha256", "1a2b3c4d5e6f7a8b").expect("digest"),
            budget: Budget::new(
                Limits::new(8, 2, 1, 3_600, 1_000, MicroUsd::new(500)).expect("test limits"),
                15,
                gauntlet_domain::DiagnosticAllowance::new(100, MicroUsd::new(50)),
            )
            .expect("test budget"),
            parent_run: None,
        })
        .expect("test run")
    }

    fn verification_record(
        verification: &str,
        unit: &str,
        outcome: Outcome,
        iteration: u32,
    ) -> VerificationRecordDoc {
        verification_record_produced_at(verification, unit, outcome, iteration, BASE + 10)
    }

    /// Same record with an explicit `produced_at` — the substrate
    /// occurrence instant the import projects onto the row.
    fn verification_record_produced_at(
        verification: &str,
        unit: &str,
        outcome: Outcome,
        iteration: u32,
        produced_at: u64,
    ) -> VerificationRecordDoc {
        VerificationRecordDoc::new(VerificationContent {
            verification: VerificationRef::new(verification).expect("ref"),
            run_id: run_id(),
            iteration_no: IterationNo::new(iteration).expect("1-based"),
            unit_id: UnitId::new(unit).expect("unit"),
            claim: "the unit's behavioral claim".to_owned(),
            requirement: RequirementDoc::new(
                EvidenceKind::TestReport,
                EvidenceLocator::new(FIXTURE_LOCATOR).expect("locator"),
                Producer::Instrument,
            ),
            instrument: VerifyingInstrument::new(
                "gauntlet-test-instrument".to_owned(),
                "1".to_owned(),
                true,
            ),
            evidence: EvidenceRef::new(&format!("evd-{verification}")).expect("evidence"),
            produced_at: EpochSeconds::new(produced_at),
            outcome,
            class: None,
            detail: "the instrument decided".to_owned(),
        })
        .expect("well-formed record")
    }

    fn settlement(unit: &str, verification: &str, offset: u64) -> SettlementRecord {
        SettlementRecord::new(
            run_id(),
            UnitId::new(unit).expect("unit"),
            EvidenceRef::new(&format!("evd-{verification}")).expect("evidence"),
            VerificationRef::new(verification).expect("ref"),
            EventLogOffset::new(offset),
        )
    }

    fn selection(sequence: usize) -> CorrectionSelection {
        let facts = BudgetFacts {
            requested_rounds: REQ_ROUNDS,
            requested_wall_seconds: REQ_SPAN,
            requested_tokens: REQ_TOKENS,
            requested_cost_usd: MicroUsd::new(REQ_COST),
            remaining_rounds: REM_ROUNDS,
            remaining_wall_seconds: REM_SPAN,
            remaining_tokens: REM_TOKENS,
            remaining_cost_usd: MicroUsd::new(REM_COST),
        };
        let descriptor = |summary: String| {
            CorrectionDescriptor::new(
                summary,
                CorrectionShape::Bounded,
                vec![UnitId::new("AK-9501").expect("unit")],
                facts,
            )
            .expect("complete descriptor")
        };
        CorrectionSelection::new(
            run_id(),
            IterationNo::new(2).expect("1-based"),
            descriptor(format!("bound the failing branch {sequence}")),
            vec![RejectedAlternative::new(
                descriptor("rewrite everything".to_owned()),
                RejectionClass::Budget,
                Reason::MeterRefusedReserve { facts },
            )],
            SelectionDimension::ALL
                .iter()
                .map(|dimension| {
                    DecisionStep::new(*dimension, format!("import-test fact: {dimension}"))
                })
                .collect(),
        )
        .expect("the selection satisfies the aggregate")
    }

    /// The in-memory substrate: exactly the reads the import projects.
    struct Substrate {
        run: Option<Run>,
        checkpoint: Option<Checkpoint>,
        records: Vec<VerificationRecordDoc>,
        selections: Vec<CorrectionSelection>,
        settlements: Vec<SettlementRecord>,
    }

    impl StateStore for Substrate {
        fn append_event(
            &self,
            _event: RunEvent,
            _deadline: Deadline,
        ) -> Result<EventLogOffset, StateStoreError> {
            Err(StateStoreError::WriteFailed {
                detail: "the import-test substrate never appends".to_owned(),
            })
        }

        fn load_run(
            &self,
            _run_id: &RunId,
            _deadline: Deadline,
        ) -> Result<Option<Run>, StateStoreError> {
            Ok(self.run.clone())
        }

        fn checkpoint(
            &self,
            _checkpoint: Checkpoint,
            _deadline: Deadline,
        ) -> Result<(), StateStoreError> {
            Err(StateStoreError::WriteFailed {
                detail: "the import-test substrate never checkpoints".to_owned(),
            })
        }

        fn resume(
            &self,
            _run_id: &RunId,
            _deadline: Deadline,
        ) -> Result<Option<Checkpoint>, StateStoreError> {
            Ok(self.checkpoint.clone())
        }

        fn export_jsonl(
            &self,
            _run_id: &RunId,
            _deadline: Deadline,
        ) -> Result<String, StateStoreError> {
            Ok(String::new())
        }

        fn verification_records(
            &self,
            _run_id: &RunId,
            _deadline: Deadline,
        ) -> Result<Vec<VerificationRecordDoc>, StateStoreError> {
            Ok(self.records.clone())
        }

        fn correction_selections(
            &self,
            _run_id: &RunId,
            _deadline: Deadline,
        ) -> Result<Vec<CorrectionSelection>, StateStoreError> {
            Ok(self.selections.clone())
        }

        fn settlements(
            &self,
            _run_id: &RunId,
            _deadline: Deadline,
        ) -> Result<Vec<SettlementRecord>, StateStoreError> {
            Ok(self.settlements.clone())
        }
    }

    fn checkpoint() -> Checkpoint {
        Checkpoint {
            run_id: run_id(),
            reference_digest: reference_digest(),
            iteration_no: IterationNo::new(CKPT_ITERATION).expect("1-based"),
            unit_states: Vec::new(),
            budget_spent: gauntlet_domain::Spent::from_counters(
                CKPT_ROUNDS,
                CKPT_SPAN,
                CKPT_TOKENS,
                MicroUsd::new(CKPT_COST),
                CKPT_DIAGNOSTIC_TOKENS,
                MicroUsd::new(CKPT_DIAGNOSTIC_COST),
            ),
            in_flight: Vec::new(),
            event_log_offset: EventLogOffset::new(CKPT_OFFSET),
        }
    }

    fn import(substrate: &Arc<Substrate>, ledger: &Arc<MemoryLedger>) -> DevelopmentalImport {
        DevelopmentalImport::new(
            substrate.clone() as Arc<dyn StateStore>,
            ledger.clone() as Arc<dyn CapabilityLedger>,
            Arc::<FixedClock>::new(FixedClock) as Arc<dyn Clock>,
        )
    }

    fn observations_of(
        ledger: &MemoryLedger,
    ) -> Vec<gauntlet_ports::capability_ledger::ObservationRecord> {
        ledger.observations(&capability(), bound()).expect("reads")
    }

    #[test]
    fn a_full_run_projects_every_available_class() {
        let substrate = Arc::new(Substrate {
            run: Some(durable_run()),
            checkpoint: Some(checkpoint()),
            records: vec![
                verification_record("vrf-pass", "AK-9501", Outcome::Pass, 2),
                verification_record("vrf-fail", "AK-9502", Outcome::Fail, 2),
            ],
            selections: vec![selection(0), selection(1)],
            settlements: vec![settlement("AK-9501", "vrf-pass", 21)],
        });
        let ledger = Arc::new(MemoryLedger::new());
        let summary = import(&substrate, &ledger)
            .import_run(&capability(), &run_id(), bound())
            .expect("imports");
        // 1 payment + 2 verifications + 2 corrections + 1 settlement.
        assert_eq!(summary.appended, 6);
        assert_eq!(summary.already_present, 0);
        let observations = observations_of(&ledger);
        let classes: Vec<ObservationClass> = observations
            .iter()
            .map(|record| record.identity().class())
            .collect();
        assert!(classes.contains(&ObservationClass::Payment));
        assert_eq!(
            classes
                .iter()
                .filter(|c| **c == ObservationClass::Attempt)
                .count(),
            1
        );
        assert_eq!(
            classes
                .iter()
                .filter(|c| **c == ObservationClass::Failure)
                .count(),
            1
        );
        assert_eq!(
            classes
                .iter()
                .filter(|c| **c == ObservationClass::Correction)
                .count(),
            2
        );
        assert_eq!(
            classes
                .iter()
                .filter(|c| **c == ObservationClass::Settlement)
                .count(),
            1
        );
        // No comparison observation is ever invented: the substrate's
        // comparisons are write-only on the port.
        assert!(!classes.contains(&ObservationClass::Comparison));
    }

    #[test]
    fn failures_and_inconclusives_are_memory_with_the_same_eagerness() {
        // Ungated-ness: an INCONCLUSIVE attempt is recorded exactly like a
        // PASS; nothing about the outcome gates the log.
        let substrate = Arc::new(Substrate {
            run: Some(durable_run()),
            checkpoint: None,
            records: vec![
                verification_record("vrf-pass", "AK-9501", Outcome::Pass, 1),
                verification_record("vrf-inc", "AK-9502", Outcome::Inconclusive, 1),
            ],
            selections: Vec::new(),
            settlements: Vec::new(),
        });
        let ledger = Arc::new(MemoryLedger::new());
        import(&substrate, &ledger)
            .import_run(&capability(), &run_id(), bound())
            .expect("imports");
        let observations = observations_of(&ledger);
        let inconclusive = observations
            .iter()
            .find(|record| record.identity().fact() == "verification:vrf-inc")
            .expect("the inconclusive attempt is recorded");
        assert_eq!(inconclusive.outcome(), Some(Outcome::Inconclusive));
        assert_eq!(inconclusive.identity().class(), ObservationClass::Attempt);
    }

    #[test]
    fn reimporting_a_run_appends_nothing() {
        let substrate = Arc::new(Substrate {
            run: Some(durable_run()),
            checkpoint: Some(checkpoint()),
            records: vec![verification_record("vrf-pass", "AK-9501", Outcome::Pass, 2)],
            selections: vec![selection(0)],
            settlements: vec![settlement("AK-9501", "vrf-pass", 21)],
        });
        let ledger = Arc::new(MemoryLedger::new());
        let importer = import(&substrate, &ledger);
        let first = importer
            .import_run(&capability(), &run_id(), bound())
            .expect("imports");
        let second = importer
            .import_run(&capability(), &run_id(), bound())
            .expect("re-imports");
        assert_eq!(second.appended, 0);
        assert_eq!(second.already_present, first.appended);
        assert_eq!(
            observations_of(&ledger).len(),
            first.appended,
            "the replay duplicated nothing"
        );
    }

    #[test]
    fn a_settlement_observation_carries_the_content_axes() {
        let substrate = Arc::new(Substrate {
            run: Some(durable_run()),
            checkpoint: None,
            records: vec![verification_record("vrf-pass", "AK-9501", Outcome::Pass, 2)],
            selections: vec![selection(0), selection(1)],
            settlements: vec![settlement("AK-9501", "vrf-pass", 21)],
        });
        let ledger = Arc::new(MemoryLedger::new());
        import(&substrate, &ledger)
            .import_run(&capability(), &run_id(), bound())
            .expect("imports");
        let observations = observations_of(&ledger);
        let settlement_observation = observations
            .iter()
            .find(|record| record.identity().class() == ObservationClass::Settlement)
            .expect("the settlement is recorded");
        // The axes are exactly the durable content facts: unit, frozen
        // reference, evidence kind, locator class — never the run id or a
        // timestamp.
        let axes = settlement_observation
            .axes()
            .iter()
            .map(|axis| axis.sort_key())
            .collect::<Vec<_>>();
        assert_eq!(axes.len(), 4);
        assert!(axes[0].starts_with("unit:AK-9501"));
        assert!(axes[1].starts_with("reference:sha256:"));
        assert!(axes[2].starts_with("kind:test_report"));
        assert!(axes[3].starts_with("locator:p10-fixture-locator"));
        // The burden proxy and the instrument fact ride along.
        assert_eq!(settlement_observation.orchestration_cost(), Some(2));
        assert_eq!(
            settlement_observation.instrument_deterministic(),
            Some(true)
        );
        assert_eq!(settlement_observation.outcome(), Some(Outcome::Pass));
    }

    #[test]
    fn an_undurable_run_is_a_typed_refusal() {
        let substrate = Arc::new(Substrate {
            run: None,
            checkpoint: None,
            records: Vec::new(),
            selections: Vec::new(),
            settlements: Vec::new(),
        });
        let ledger = Arc::new(MemoryLedger::new());
        let error = import(&substrate, &ledger)
            .import_run(&capability(), &run_id(), bound())
            .expect_err("nothing is projected from an undurable run");
        assert!(matches!(error, ImportError::RunNotDurable { .. }));
        assert!(observations_of(&ledger).is_empty());
    }

    #[test]
    fn a_ledger_conflict_surfaces_instead_of_passing() {
        // The identity agrees, the facts disagree — the import surfaces the
        // ledger's typed conflict rather than absorbing it.
        let substrate = Arc::new(Substrate {
            run: Some(durable_run()),
            checkpoint: None,
            records: vec![verification_record("vrf-pass", "AK-9501", Outcome::Pass, 2)],
            selections: Vec::new(),
            settlements: Vec::new(),
        });
        let ledger = Arc::new(MemoryLedger::new());
        let forged = ObservationRecord::new(
            ObservationIdentity::new(
                capability(),
                ObservationClass::Attempt,
                run_id(),
                "verification:vrf-pass".to_owned(),
            )
            .expect("identity"),
            EpochSeconds::new(BASE),
            "a different claim under the same identity".to_owned(),
        );
        ledger
            .append_observation(forged, bound())
            .expect("the forged row seeds");
        let error = import(&substrate, &ledger)
            .import_run(&capability(), &run_id(), bound())
            .expect_err("the conflict surfaces");
        assert!(
            matches!(
                error,
                ImportError::Ledger(CapabilityLedgerError::ObservationConflict { .. })
            ),
            "the conflict is the typed refusal, not a silent overwrite"
        );
    }

    #[test]
    fn recovery_completion_records_are_ungated_appends() {
        let substrate = Arc::new(Substrate {
            run: Some(durable_run()),
            checkpoint: None,
            records: Vec::new(),
            selections: Vec::new(),
            settlements: Vec::new(),
        });
        let ledger = Arc::new(MemoryLedger::new());
        let importer = import(&substrate, &ledger);
        importer
            .record_recovery(
                &capability(),
                &run_id(),
                "recovery:settlement-replay".to_owned(),
                1,
                "the replay completed one settlement".to_owned(),
                bound(),
            )
            .expect("records");
        let observations = observations_of(&ledger);
        assert_eq!(observations.len(), 1);
        assert_eq!(
            observations[0].identity().class(),
            ObservationClass::Recovery
        );
        // The same recovery re-recorded is the idempotent no-op.
        importer
            .record_recovery(
                &capability(),
                &run_id(),
                "recovery:settlement-replay".to_owned(),
                1,
                "the replay completed one settlement".to_owned(),
                bound(),
            )
            .expect("idempotent");
        assert_eq!(observations_of(&ledger).len(), 1);
    }

    #[test]
    fn occurrence_instants_are_substrate_facts_from_the_verification_records() {
        // `occurred_at` is the fact's OWN occurrence instant in the
        // substrate — the verification record's `produced_at` — never
        // invented for classes the substrate assigns no occurrence to.
        let pass_produced = BASE + 10;
        let fail_produced = BASE + 20;
        let substrate = Arc::new(Substrate {
            run: Some(durable_run()),
            checkpoint: Some(checkpoint()),
            records: vec![
                verification_record_produced_at(
                    "vrf-pass",
                    "AK-9501",
                    Outcome::Pass,
                    2,
                    pass_produced,
                ),
                verification_record_produced_at(
                    "vrf-fail",
                    "AK-9502",
                    Outcome::Fail,
                    2,
                    fail_produced,
                ),
            ],
            selections: vec![selection(0)],
            settlements: vec![settlement("AK-9501", "vrf-pass", 21)],
        });
        let ledger = Arc::new(MemoryLedger::new());
        import(&substrate, &ledger)
            .import_run(&capability(), &run_id(), bound())
            .expect("imports");
        let observations = observations_of(&ledger);
        let find = |fact: &str| {
            observations
                .iter()
                .find(|record| record.identity().fact() == fact)
                .unwrap_or_else(|| panic!("no row for fact {fact}"))
        };
        // Attempt and failure rows carry their record's produced_at.
        assert_eq!(
            find("verification:vrf-pass").occurred_at(),
            Some(EpochSeconds::new(pass_produced))
        );
        assert_eq!(
            find("verification:vrf-fail").occurred_at(),
            Some(EpochSeconds::new(fail_produced))
        );
        // The settlement carries ITS MATCHING verification record's
        // produced_at.
        assert_eq!(
            find("settlement:AK-9501:vrf-pass").occurred_at(),
            Some(EpochSeconds::new(pass_produced))
        );
        // Payment and correction rows are not decided on: no occurrence
        // fact is invented for them.
        assert_eq!(find("checkpoint-spend").occurred_at(), None);
        assert_eq!(find("correction:0").occurred_at(), None);
    }

    /// One run's honest substrate for the B3b reproduction: runs 1–3 pass
    /// and settle three distinct units, each produced LATER than the
    /// previous run; run 4 is the newest run and FAILS without settling.
    fn b3b_substrate(sequence: usize) -> Arc<Substrate> {
        let unit = format!("AK-950{sequence}");
        let verification = format!("vrf-b3b-{sequence}");
        let produced_at = BASE + 10 * sequence as u64;
        let record = verification_record_produced_at(
            &verification,
            &unit,
            if sequence == 4 {
                Outcome::Fail
            } else {
                Outcome::Pass
            },
            1,
            produced_at,
        );
        Arc::new(Substrate {
            run: Some(durable_run()),
            checkpoint: None,
            records: vec![record],
            selections: Vec::new(),
            settlements: if sequence == 4 {
                Vec::new()
            } else {
                vec![settlement(&unit, &verification, 21)]
            },
        })
    }

    fn b3b_run(sequence: usize) -> RunId {
        RunId::new(&format!("01ARZ3NDEKTSV4RRFFQ69G5F{sequence:02X}"))
            .expect("the b3b run id is a ULID")
    }

    #[test]
    fn the_verdict_is_a_function_of_recorded_facts_never_import_order_b3b() {
        // The falsifier's B3b, verbatim: an honest deteriorating substrate
        // — three PASS settlements over three units, then the NEWEST run
        // fails — imported newest-first (4,3,2,1) must produce the SAME
        // verdict as the in-order import. The failure-rate halves follow
        // substrate chronology (occurred_at, ledger seq), never the import
        // call order, so a rising rate is recorded as rising whichever
        // order the imports happened to land in.
        let reverse = Arc::new(MemoryLedger::new());
        for sequence in [4usize, 3, 2, 1] {
            import(&b3b_substrate(sequence), &reverse)
                .import_run(&capability(), &b3b_run(sequence), bound())
                .expect("imports");
        }
        let in_order = Arc::new(MemoryLedger::new());
        for sequence in [1usize, 2, 3, 4] {
            import(&b3b_substrate(sequence), &in_order)
                .import_run(&capability(), &b3b_run(sequence), bound())
                .expect("imports");
        }
        // One handled recovery, appended after a completion (the honest
        // seam; the completion's count is one).
        for ledger in [&reverse, &in_order] {
            import(&b3b_substrate(1), ledger)
                .record_recovery(
                    &capability(),
                    &b3b_run(1),
                    "recovery:settlement-replay".to_owned(),
                    1,
                    "the replay completed one settlement".to_owned(),
                    bound(),
                )
                .expect("records");
        }
        let expected = vec![
            gauntlet_ports::capability_ledger::PromotionRefusal::FailureRateIncreased {
                earlier_failures: 0,
                earlier_decided: 2,
                later_failures: 1,
                later_decided: 2,
            },
        ];
        for (label, ledger) in [("newest-first", &reverse), ("in-order", &in_order)] {
            let decision = PromotionService::new(ledger.clone(), 3, Arc::new(FixedClock))
                .expect("the default minimum is valid")
                .evaluate(&capability(), bound())
                .unwrap_or_else(|error| panic!("{label}: the evaluation runs: {error}"));
            match decision {
                PromotionDecision::Refused { refusals, entry } => {
                    assert_eq!(
                        refusals, expected,
                        "{label}: the recorded halves must be occurrence-ordered"
                    );
                    assert_eq!(entry.status, CapabilityStatus::Provisional);
                }
                PromotionDecision::Promoted { .. } => panic!(
                    "{label}: import order decided the verdict — the newest-first import \
                     PROMOTED a genuinely deteriorating trajectory (B3b)"
                ),
            }
        }
    }

    fn empty_substrate() -> Arc<Substrate> {
        Arc::new(Substrate {
            run: None,
            checkpoint: None,
            records: Vec::new(),
            selections: Vec::new(),
            settlements: Vec::new(),
        })
    }

    #[test]
    fn a_recovery_with_zero_completions_is_not_earned() {
        // The completion receipt: §18.3 completed ZERO settlements — there
        // is no handled recovery to record, even over a durable run.
        let substrate = Arc::new(Substrate {
            run: Some(durable_run()),
            checkpoint: None,
            records: Vec::new(),
            selections: Vec::new(),
            settlements: Vec::new(),
        });
        let ledger = Arc::new(MemoryLedger::new());
        let error = import(&substrate, &ledger)
            .record_recovery(
                &capability(),
                &run_id(),
                "recovery:settlement-replay".to_owned(),
                0,
                "the completion completed nothing".to_owned(),
                bound(),
            )
            .expect_err("a zero-completion recovery is not earned");
        assert!(
            matches!(error, ImportError::RecoveryNotEarned { completed: 0 }),
            "the typed refusal carries the count: {error}"
        );
        assert!(
            observations_of(&ledger).is_empty(),
            "the refused recovery appended nothing"
        );
    }

    #[test]
    fn a_recovery_for_a_run_that_exists_nowhere_is_refused_b5() {
        // The falsifier's B5: recovery credit must be rooted to a durable
        // run. `record_recovery` for a run `load_run` cannot find is
        // refused typed and appends nothing — recovery credit is an
        // earned write, never a minted row.
        let ledger = Arc::new(MemoryLedger::new());
        let error = import(&empty_substrate(), &ledger)
            .record_recovery(
                &capability(),
                &b3b_run(9),
                "recovery:settlement-replay".to_owned(),
                1,
                "recovery credit minted for a run that exists nowhere".to_owned(),
                bound(),
            )
            .expect_err("a recovery for a non-durable run is refused");
        assert!(matches!(error, ImportError::RunNotDurable { .. }));
        assert!(
            observations_of(&ledger).is_empty(),
            "the refused recovery appended nothing"
        );
    }

    #[test]
    fn recovery_credit_never_completes_the_conjunction_on_an_empty_substrate_b5() {
        // B5's full shape: a complete-minus-recovery window refuses
        // `no_recovery_observed`; a recovery row minted for a run that
        // exists NOWHERE must not complete the conjunction — the
        // evaluation still refuses, never promotes.
        let ledger = Arc::new(MemoryLedger::new());
        for (sequence, unit) in ["AK-9401", "AK-9402", "AK-9403"].iter().enumerate() {
            ledger
                .append_observation(
                    ObservationRecord::new(
                        ObservationIdentity::new(
                            capability(),
                            ObservationClass::Settlement,
                            b3b_run(sequence + 1),
                            format!("settlement:{unit}:vrf-b5-{sequence}"),
                        )
                        .expect("settlement identity"),
                        EpochSeconds::new(BASE),
                        format!("unit {unit} settled"),
                    )
                    .with_unit(UnitId::new(unit).expect("unit"))
                    .with_outcome(Outcome::Pass)
                    .with_instrument_deterministic(true)
                    .with_evidence_kind(EvidenceKind::TestReport)
                    .with_evidence_locator(EvidenceLocator::new(FIXTURE_LOCATOR).expect("locator"))
                    .with_reference_digest(reference_digest())
                    .with_verification(
                        VerificationRef::new(&format!("vrf-b5-{sequence}")).expect("ref"),
                    )
                    .with_evidence(EvidenceRef::new(&format!("evd-b5-{sequence}")).expect("evd"))
                    .with_orchestration_cost(0)
                    .with_occurred_at(EpochSeconds::new(BASE + sequence as u64)),
                    bound(),
                )
                .expect("seed");
        }
        for sequence in 0..3 {
            ledger
                .append_observation(
                    ObservationRecord::new(
                        ObservationIdentity::new(
                            capability(),
                            ObservationClass::Attempt,
                            b3b_run(sequence + 10),
                            format!("verification:vrf-b5-{sequence}"),
                        )
                        .expect("attempt identity"),
                        EpochSeconds::new(BASE),
                        "an outcome fact".to_owned(),
                    )
                    .with_outcome(Outcome::Pass)
                    .with_occurred_at(EpochSeconds::new(BASE + 10 + sequence as u64)),
                    bound(),
                )
                .expect("seed");
        }
        let service = PromotionService::new(ledger.clone(), 3, Arc::new(FixedClock))
            .expect("the default minimum is valid");
        let refusal_of = |decision: &PromotionDecision| match decision {
            PromotionDecision::Refused { refusals, .. } => refusals.clone(),
            PromotionDecision::Promoted { .. } => {
                panic!("the window promoted without a recovery on record")
            }
        };
        let first = service.evaluate(&capability(), bound()).expect("evaluates");
        assert!(
            refusal_of(&first)
                .iter()
                .any(|refusal| refusal.rule() == "no_recovery_observed"),
            "the window refuses on the missing recovery: {:?}",
            refusal_of(&first)
        );
        // The mint attempt: `record_recovery` for a run that exists nowhere.
        let error = import(&empty_substrate(), &ledger)
            .record_recovery(
                &capability(),
                &b3b_run(9),
                "recovery:settlement-replay".to_owned(),
                1,
                "recovery credit minted for a run that exists nowhere".to_owned(),
                bound(),
            )
            .expect_err("the mint is refused");
        assert!(matches!(error, ImportError::RunNotDurable { .. }));
        assert!(
            observations_of(&ledger)
                .iter()
                .all(|record| record.identity().class() != ObservationClass::Recovery),
            "the refused recovery appended nothing"
        );
        let second = service.evaluate(&capability(), bound()).expect("evaluates");
        let refusals = refusal_of(&second);
        assert!(
            refusals
                .iter()
                .any(|refusal| refusal.rule() == "no_recovery_observed"),
            "the conjunction still refuses after the refused mint: {refusals:?}"
        );
    }
}
