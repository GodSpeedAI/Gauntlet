//! The `StateStore` shared contract suite (spec §6.2; §18, §16.3).
//!
//! Pinned semantics: a checkpoint written is a checkpoint resumable — the
//! run's latest checkpoint comes back exactly (spec §18: resume from the
//! durable checkpoint); a run that was checkpointed LOADS with its identity
//! and frozen reference digest intact; a run never stored is `None`; and
//! export is deterministic — same run history, byte-identical JSONL (spec
//! §6.2: "Export is deterministic").
//!
//! The two owner writes (`record_evidence_production`,
//! `bind_observation_bundle`) are capability-gated (plan Task 12): the
//! suite EARNS them through the ports minting authorities exactly the way
//! production does — no minting fixture exists anywhere, and the store
//! methods themselves accept only the minted values. What the suite holds
//! the ADAPTER to is everything downstream of the mint: the production
//! facts persist exactly (write-once per ref, idempotent for identical
//! facts, refused for different facts), the observation binding persists
//! the derived §12.4 digest, and both read back through the port's own
//! queries.

use gauntlet_domain::{
    Budget, CorrectionDescriptor, CorrectionSelection, CorrectionShape, DecisionStep,
    DiagnosticAllowance, Digest, EpochSeconds, EventLogOffset, EvidenceKind, EvidenceRef,
    InstrumentName, InvocationId, IterationNo, Limits, MicroUsd, ReferenceId, RejectedAlternative,
    RejectionClass, RejectionReason, ResultContractPath, Run, RunDefinition, RunError, RunId,
    SelectionDimension, UnitId, UnitStatus, VerificationRef,
};

use crate::contract::ContractFinding;
use crate::deadline::Deadline;
use crate::evidence_store::{EvidenceStore, EvidenceStoreError};
use crate::observer::InstrumentIdentity;
use crate::producer_authority::{
    EvidenceProductionAuthority, ObservationBindingAuthority, ProducerAuthorityError,
};
use crate::state_store::{Checkpoint, RunEvent, RunEventKind, StateStore};

/// The suite's name in findings.
pub const SUITE: &str = "state_store";

const BASE_INSTANT: u64 = 1_700_000_000;
/// The suite's bound span, on its own line (numeric literals never share a
/// line with a duration-shaped identifier).
const BOUND_SPAN: u64 = 60;

/// Constructs the store under test. `build` must return an ISOLATED, EMPTY
/// instance.
pub trait StateStoreFactory {
    fn label(&self) -> &'static str;

    fn build(&self) -> Result<Box<dyn StateStore>, String>;
}

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding::new(SUITE, case, rule, detail)
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + BOUND_SPAN))
}

fn run(run_id: &RunId) -> Result<Run, RunError> {
    Run::new(RunDefinition {
        id: run_id.clone(),
        created_at: EpochSeconds::new(BASE_INSTANT),
        reference_version: gauntlet_domain::ReferenceVersion::new(
            ReferenceId::new("ref-suite-01").expect("suite reference id"),
            3,
            Digest::new("sha256", "c4d3e2f1a0b1c2d3").expect("suite digest"),
        ),
        intent: "prove the slice against the frozen standard".to_owned(),
        config_fingerprint: Digest::new("sha256", "a1b2c3d4e5f6a7b8").expect("suite digest"),
        environment_fingerprint: Digest::new("sha256", "e5f6a7b8c4d3e2f1").expect("suite digest"),
        budget: Budget::new(
            Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("suite limits usable"),
            20,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
        )
        .expect("suite budget usable"),
        parent_run: None,
    })
}

fn checkpoint(run_id: &RunId) -> Result<Checkpoint, RunError> {
    let run = run(run_id)?;
    Ok(Checkpoint {
        run_id: run_id.clone(),
        reference_digest: run.reference_version().digest().clone(),
        iteration_no: IterationNo::new(2).expect("suite iteration is 1-based"),
        unit_states: vec![
            (
                UnitId::new("AK-014").expect("suite unit id"),
                UnitStatus::Open,
            ),
            (
                UnitId::new("AK-015").expect("suite unit id"),
                UnitStatus::Settled,
            ),
        ],
        budget_spent: *run.budget().spent(),
        in_flight: vec![(
            InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAW").expect("suite invocation id"),
            ResultContractPath::new("suite-result-document").expect("suite contract path"),
        )],
        event_log_offset: EventLogOffset::new(2),
    })
}

fn append_events(store: &dyn StateStore, run_id: &RunId, count: usize) -> Vec<ContractFinding> {
    const CASE: &str = "append_events_feed";
    const RULE: &str =
        "spec 6.2 StateStore: export is deterministic — same run history, byte-identical output";
    let mut findings = Vec::new();
    for sequence in 0..count {
        let kind = match sequence % 2 {
            0 => RunEventKind::InvocationAdmitted {
                invocation_id: InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAW")
                    .expect("suite invocation id"),
            },
            _ => RunEventKind::SettlementCommitted {
                unit_id: UnitId::new("AK-014").expect("suite unit id"),
                evidence: EvidenceRef::new("evd-000042").expect("suite evidence ref"),
                verification: VerificationRef::new("vrf-000042").expect("suite verification ref"),
                reference_digest: Digest::new("sha256", "c4d3e2f1a0b1c2d3").expect("suite digest"),
            },
        };
        let event = RunEvent {
            run_id: run_id.clone(),
            kind,
            at: EpochSeconds::new(BASE_INSTANT + sequence as u64),
        };
        if let Err(error) = store.append_event(event, bound()) {
            findings.push(finding(
                CASE,
                RULE,
                format!("appending event {sequence} failed: {error}"),
            ));
            return findings;
        }
    }
    findings
}

/// Case: a fresh run has no checkpoint; the written checkpoint resumes
/// exactly (spec §18: every iteration boundary makes a durable checkpoint;
/// resume returns the latest).
fn checkpoint_and_resume_round_trip(factory: &dyn StateStoreFactory) -> Vec<ContractFinding> {
    const CASE: &str = "checkpoint_and_resume_round_trip";
    const RULE: &str = "spec 18: resume returns the run's latest durable checkpoint, exactly as written; a run with no checkpoint resumes None";
    let Ok(store) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a store for the case".to_owned(),
        )];
    };
    let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID");
    let mut findings = Vec::new();
    match store.resume(&run_id, bound()) {
        Ok(None) => {}
        Ok(other) => findings.push(finding(
            CASE,
            RULE,
            format!("a fresh run resumed {other:?} — it has no checkpoint"),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("resume on a fresh run errored instead of None: {error}"),
        )),
    }
    let Ok(checkpoint) = checkpoint(&run_id) else {
        return vec![finding(
            CASE,
            RULE,
            "the suite checkpoint is unusable".to_owned(),
        )];
    };
    if let Err(error) = store.checkpoint(checkpoint.clone(), bound()) {
        findings.push(finding(
            CASE,
            RULE,
            format!("writing the checkpoint failed: {error}"),
        ));
        return findings;
    }
    match store.resume(&run_id, bound()) {
        Ok(Some(resumed)) if resumed == checkpoint => {}
        Ok(other) => findings.push(finding(
            CASE,
            RULE,
            format!("resume returned {other:?} instead of the checkpoint that was written"),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("resume after checkpoint errored: {error}"),
        )),
    }
    findings
}

/// Case: a checkpointed run loads — with its identity and frozen reference
/// digest intact (spec §5.3.3: one frozen reference_version per run's
/// life); a never-stored run is `None`, not an error.
fn checkpointed_runs_load(factory: &dyn StateStoreFactory) -> Vec<ContractFinding> {
    const CASE: &str = "checkpointed_runs_load";
    const RULE: &str = "spec 18 via 5.3.3: load_run returns the run's durable state with its frozen reference; an unstored run is None";
    let Ok(store) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a store for the case".to_owned(),
        )];
    };
    let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID");
    let Ok(checkpoint) = checkpoint(&run_id) else {
        return vec![finding(
            CASE,
            RULE,
            "the suite checkpoint is unusable".to_owned(),
        )];
    };
    if let Err(error) = store.checkpoint(checkpoint.clone(), bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("writing the checkpoint failed: {error}"),
        )];
    }
    let mut findings = Vec::new();
    match store.load_run(&run_id, bound()) {
        Ok(Some(loaded)) => {
            if loaded.id() != &run_id {
                findings.push(finding(
                    CASE,
                    RULE,
                    format!(
                        "the loaded run carries id {} instead of {run_id}",
                        loaded.id()
                    ),
                ));
            }
            if loaded.reference_version().digest() != &checkpoint.reference_digest {
                findings.push(finding(
                    CASE,
                    RULE,
                    "the loaded run's frozen reference digest does not match the checkpoint"
                        .to_owned(),
                ));
            }
        }
        Ok(None) => findings.push(finding(
            CASE,
            RULE,
            "load_run returned None for a run that was checkpointed".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("load_run after checkpoint errored: {error}"),
        )),
    }
    let stranger = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FZZ").expect("distinct ULID");
    match store.load_run(&stranger, bound()) {
        Ok(None) => {}
        Ok(other) => findings.push(finding(
            CASE,
            RULE,
            format!("a never-stored run loaded as {other:?} — it must be None"),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("load_run of a never-stored run errored instead of None: {error}"),
        )),
    }
    findings
}

/// Case: export is byte-identical for the same history AND faithful to the
/// history's growth — two exports of the same run are byte-identical, and a
/// longer history exports differently (the export is a faithful picture,
/// not a constant).
fn export_is_byte_identical_and_faithful(factory: &dyn StateStoreFactory) -> Vec<ContractFinding> {
    const CASE: &str = "export_is_byte_identical_and_faithful";
    const RULE: &str = "spec 6.2 StateStore: export_jsonl is deterministic — same run history, byte-identical output";
    let Ok(store) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a store for the case".to_owned(),
        )];
    };
    let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID");
    let mut findings = Vec::new();
    findings.extend(append_events(store.as_ref(), &run_id, 2));
    if !findings.is_empty() {
        return findings;
    }
    let first = match store.export_jsonl(&run_id, bound()) {
        Ok(export) => export,
        Err(error) => return vec![finding(CASE, RULE, format!("export failed: {error}"))],
    };
    let second = match store.export_jsonl(&run_id, bound()) {
        Ok(export) => export,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("the second export failed: {error}"),
            )];
        }
    };
    if first != second {
        findings.push(finding(
            CASE,
            RULE,
            "two exports of the same history differ — export must be byte-identical".to_owned(),
        ));
        return findings;
    }
    findings.extend(append_events(store.as_ref(), &run_id, 1));
    if !findings.is_empty() {
        return findings;
    }
    match store.export_jsonl(&run_id, bound()) {
        Ok(longer) if longer != first => {}
        Ok(longer) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the export did not change as the history grew ({} bytes both times)",
                longer.len()
            ),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("export after growth failed: {error}"),
        )),
    }
    findings
}

/// The suite's own evidence store: content-addressed by insertion name, no
/// overwrite — enough store semantics for the authorities to read honest
/// bytes at a ref (the Task-10 round-9 pattern: the suite stands up its own
/// substrate rather than a minting fixture).
#[derive(Default)]
struct SuiteEvidence {
    objects: std::sync::Mutex<Vec<(EvidenceRef, Vec<u8>)>>,
}

impl SuiteEvidence {
    fn insert(&self, name: &str, bytes: &[u8]) -> EvidenceRef {
        let reference = EvidenceRef::new(name).expect("suite evidence ref is valid");
        self.objects
            .lock()
            .expect("the suite evidence store is not poisoned")
            .push((reference.clone(), bytes.to_vec()));
        reference
    }
}

impl EvidenceStore for SuiteEvidence {
    fn put(
        &self,
        bytes: &[u8],
        _: EvidenceKind,
        _: Deadline,
    ) -> Result<EvidenceRef, EvidenceStoreError> {
        let index = self
            .objects
            .lock()
            .expect("the suite evidence store is not poisoned")
            .len();
        Ok(self.insert(&format!("evd-state-suite-{index:06}"), bytes))
    }
    fn get(
        &self,
        reference: &EvidenceRef,
        _: Deadline,
    ) -> Result<Option<Vec<u8>>, EvidenceStoreError> {
        Ok(self
            .objects
            .lock()
            .expect("the suite evidence store is not poisoned")
            .iter()
            .find(|(found, _)| found == reference)
            .map(|(_, bytes)| bytes.clone()))
    }
    fn pin(&self, _: &RunId, _: &EvidenceRef, _: Deadline) -> Result<(), EvidenceStoreError> {
        Ok(())
    }
}

/// Case: the two owner writes are earned through the minting authorities —
/// the store accepts the minted values, persists the derived content digest
/// and every production fact write-once, refuses a conflicting re-record,
/// and both records read back through the port's own queries.
fn owner_writes_are_earned_through_the_authorities(
    factory: &dyn StateStoreFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "owner_writes_are_earned_through_the_authorities";
    const RULE: &str = "plan Task 12 (Task-11 C7 / Task-10 MINOR-2): the owner writes accept only authority-minted values; production facts and the derived §12.4 digest persist exactly and read back";
    let built = match factory.build() {
        Ok(store) => store,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("the factory could not build a store for the case: {error}"),
            )];
        }
    };
    // One store, shared by the authorities and the readbacks — the
    // composition root's single-adapter seam, reproduced here.
    let store: std::sync::Arc<dyn StateStore> = built.into();
    let evidence = std::sync::Arc::new(SuiteEvidence::default());
    let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID");
    // The durable substrate: a checkpoint makes the run durable (the suite's
    // own pinned case above) and stands the durable iteration anchor at 2.
    let checkpoint = match checkpoint(&run_id) {
        Ok(checkpoint) => checkpoint,
        Err(_) => {
            return vec![finding(
                CASE,
                RULE,
                "the suite checkpoint is unusable".to_owned(),
            )];
        }
    };
    if let Err(error) = store.checkpoint(checkpoint, bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("writing the durable-anchor checkpoint failed: {error}"),
        )];
    }
    let evidence_objects = evidence.clone();
    evidence_objects.insert(
        "evd-state-suite-production-0001",
        b"the suite's produced evidence bytes",
    );
    evidence_objects.insert(
        "evd-state-suite-bundle-0001",
        br#"{"schema":"observation_bundle@v1","run_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","iteration_no":2,"observed_at":1700000060,"facets":[],"observation_digest":"00"}"#,
    );
    let production_authority = EvidenceProductionAuthority::new(store.clone(), evidence.clone());
    let binding_authority = ObservationBindingAuthority::new(store.clone(), evidence.clone());
    let mut findings = Vec::new();
    let anchor = IterationNo::new(2).expect("suite iteration is 1-based");
    let production_bytes: &[u8] = b"the suite's produced evidence bytes";
    let production_ref =
        EvidenceRef::new("evd-state-suite-production-0001").expect("suite evidence ref is valid");
    let producer = InstrumentIdentity::new(
        InstrumentName::new("suite-owner-instrument").expect("suite instrument name"),
        "1".to_owned(),
    );
    let locator = gauntlet_domain::EvidenceLocator::new("suite-owner-locator.json")
        .expect("suite locator is valid");
    let produced_at = EpochSeconds::new(BASE_INSTANT + 5);
    // The authority derives the content digest from the bytes it read; the
    // suite recomputes it independently (SHA-256 of the literal bytes) so a
    // digest that is not OF THE BYTES cannot pass.
    let expected_content = {
        use sha2::Digest as _;
        Digest::new(
            "sha256",
            &sha2::Sha256::digest(production_bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
        )
        .expect("SHA-256 hex is a valid digest")
    };
    let binding = match production_authority.record(
        &run_id,
        anchor,
        &production_ref,
        &producer,
        &locator,
        produced_at,
        bound(),
    ) {
        Ok(binding) => binding,
        Err(ProducerAuthorityError::State(error)) => {
            return vec![finding(
                CASE,
                RULE,
                format!("the store refused a minted production record: {error}"),
            )];
        }
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("earning the production capability failed: {error}"),
            )];
        }
    };
    if binding.production().content() != &expected_content {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "the recorded content digest {} is not the digest of the served bytes {}",
                binding.production().content(),
                expected_content
            ),
        ));
    }
    if binding.event_log_offset().get() == 0 {
        findings.push(finding(
            CASE,
            RULE,
            "the production binding carries no store-minted ordering anchor".to_owned(),
        ));
    }
    // Readback through the port's own query: the durable facts are exactly
    // the minted ones.
    match store.evidence_production(&run_id, &production_ref, bound()) {
        Ok(Some(found)) => {
            if found.production() != binding.production() {
                findings.push(finding(
                    CASE,
                    RULE,
                    format!(
                        "the durable production record for {} differs from the minted one",
                        production_ref
                    ),
                ));
            }
        }
        Ok(None) => findings.push(finding(
            CASE,
            RULE,
            "the recorded production does not read back — no durable fact exists".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading the production back failed: {error}"),
        )),
    }
    // Write-once: a CONFLICTING record for the same ref (different producer)
    // is minted honestly by the authority — same bytes, so the same derived
    // digest — and must be refused by the STORE, never overwritten.
    let other_producer = InstrumentIdentity::new(
        InstrumentName::new("suite-other-instrument").expect("suite instrument name"),
        "1".to_owned(),
    );
    match production_authority.record(
        &run_id,
        anchor,
        &production_ref,
        &other_producer,
        &locator,
        produced_at,
        bound(),
    ) {
        Err(ProducerAuthorityError::State(crate::state_store::StateStoreError::WriteFailed {
            ..
        })) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "a conflicting re-record was refused before the store's write-once rule ({other})"
            ),
        )),
        Ok(_) => findings.push(finding(
            CASE,
            RULE,
            "a second record with different facts for the same ref succeeded — the write-once \
             rule was not enforced"
                .to_owned(),
        )),
    }
    // The observation binding: earned the same way, digest derived from the
    // bundle's bytes by the canonical §12.4 rule.
    let bundle_ref =
        EvidenceRef::new("evd-state-suite-bundle-0001").expect("suite evidence ref is valid");
    if let Err(error) = binding_authority.bind(&run_id, anchor, &bundle_ref, bound()) {
        findings.push(finding(
            CASE,
            RULE,
            format!("earning the observation-binding capability failed: {error}"),
        ));
        return findings;
    }
    let expected_observation = {
        let bytes = evidence_objects
            .get(&bundle_ref, bound())
            .expect("the suite's own evidence store serves the bundle")
            .expect("the bundle object was inserted above");
        let document = crate::json::parse(std::str::from_utf8(&bytes).expect("UTF-8 fixture"))
            .expect("the suite bundle is JSON");
        crate::observation_digest::observation_digest(&document)
    };
    match store.observation_binding(&run_id, anchor, bound()) {
        Ok(Some(found)) => {
            if found.evidence() != &bundle_ref || found.digest() != &expected_observation {
                findings.push(finding(
                    CASE,
                    RULE,
                    format!(
                        "the durable observation binding ({}, {}) is not the bundle ref with its \
                         derived §12.4 digest ({}, {})",
                        found.evidence(),
                        found.digest(),
                        bundle_ref,
                        expected_observation
                    ),
                ));
            }
        }
        Ok(None) => findings.push(finding(
            CASE,
            RULE,
            "the bound observation does not read back — no durable binding exists".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading the observation binding back failed: {error}"),
        )),
    }
    findings
}

/// Case: the controller's §15.1 selection record — chosen correction,
/// rejected alternatives with class and reason, decision trace — round-trips
/// durably (AK-037 is a recording claim: selection + alternatives + reasons
/// must read back exactly as recorded, and a run with no selections reads
/// as empty, never as an error).
fn correction_selections_round_trip(factory: &dyn StateStoreFactory) -> Vec<ContractFinding> {
    const CASE: &str = "correction_selection_round_trip";
    const RULE: &str = "spec 15.1 step 4: the selection and the rejected alternatives with reasons are recorded on the iteration and read back exactly";
    let Ok(store) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a store for the case".to_owned(),
        )];
    };
    let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID");
    let mut findings = Vec::new();
    // A run with no selections reads empty, not as an error.
    match store.correction_selections(&run_id, bound()) {
        Ok(empty) if empty.is_empty() => {}
        Ok(other) => findings.push(finding(
            CASE,
            RULE,
            format!("a fresh run read {other:?} selections — it has none"),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading a fresh run's selections errored instead of empty: {error}"),
        )),
    }
    if !findings.is_empty() {
        return findings;
    }
    // The suite's selection: a chosen bounded correction plus one
    // budget-blocked and one lower-priority rejected alternative, with a
    // trace naming all ten §15.1 dimensions (the aggregate refuses less).
    // Duration- and cost-shaped values are named facts (§8.1 literal
    // discipline), never inline numbers.
    const REQ_ROUNDS: u32 = 1;
    const REQ_SPAN: u64 = 60;
    const REQ_TOKENS: u64 = 400;
    const REQ_COST: u64 = 5;
    const REM_ROUNDS: u32 = 6;
    const REM_SPAN: u64 = 2700;
    const REM_TOKENS: u64 = 400;
    const REM_COST: u64 = 395;
    let facts = gauntlet_domain::BudgetFacts {
        requested_rounds: REQ_ROUNDS,
        requested_wall_seconds: REQ_SPAN,
        requested_tokens: REQ_TOKENS,
        requested_cost_usd: MicroUsd::new(REQ_COST),
        remaining_rounds: REM_ROUNDS,
        remaining_wall_seconds: REM_SPAN,
        remaining_tokens: REM_TOKENS,
        remaining_cost_usd: MicroUsd::new(REM_COST),
    };
    let chosen = CorrectionDescriptor::new(
        "bound the parser branch to the frozen grammar".to_owned(),
        CorrectionShape::Bounded,
        vec![UnitId::new("AK-014").expect("suite unit id")],
        facts,
    )
    .expect("suite descriptor is complete");
    let blocked = RejectedAlternative::new(
        CorrectionDescriptor::new(
            "rewrite the whole reference pipeline".to_owned(),
            CorrectionShape::Speculative,
            vec![UnitId::new("AK-015").expect("suite unit id")],
            facts,
        )
        .expect("suite descriptor is complete"),
        RejectionClass::Budget,
        RejectionReason::MeterRefusedReserve { facts },
    );
    let outranked = RejectedAlternative::new(
        CorrectionDescriptor::new(
            "adjust the neighboring lint".to_owned(),
            CorrectionShape::Bounded,
            vec![UnitId::new("AK-016").expect("suite unit id")],
            facts,
        )
        .expect("suite descriptor is complete"),
        RejectionClass::NotSelected,
        RejectionReason::LowerPriority {
            decided_by: SelectionDimension::Impact,
        },
    );
    let trace = SelectionDimension::ALL
        .iter()
        .map(|dimension| DecisionStep::new(*dimension, format!("suite fact for {dimension}")))
        .collect::<Vec<_>>();
    let selection = CorrectionSelection::new(
        run_id.clone(),
        IterationNo::new(2).expect("suite iteration is 1-based"),
        chosen,
        vec![blocked, outranked],
        trace,
    )
    .expect("the suite selection satisfies the aggregate's invariants");
    if let Err(error) = store.record_correction_selection(&selection, bound()) {
        findings.push(finding(
            CASE,
            RULE,
            format!("recording the controller's selection failed: {error}"),
        ));
        return findings;
    }
    match store.correction_selections(&run_id, bound()) {
        Ok(recorded) => {
            if recorded.is_empty() {
                findings.push(finding(
                    CASE,
                    RULE,
                    "the recorded selection does not read back — no durable selection exists"
                        .to_owned(),
                ));
            } else if recorded[0] != selection {
                findings.push(finding(
                    CASE,
                    RULE,
                    format!(
                        "the durable selection differs from the recorded one: {recorded:?} vs \
                         {selection:?}"
                    ),
                ));
            }
        }
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("reading the recorded selection back failed: {error}"),
        )),
    }
    // Lookups are run-keyed: a stranger's selections never leak.
    let stranger = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FZZ").expect("distinct ULID");
    if let Ok(leaked) = store.correction_selections(&stranger, bound())
        && !leaked.is_empty()
    {
        findings.push(finding(
            CASE,
            RULE,
            format!("another run read {leaked:?} selections — records are run-keyed"),
        ));
    }
    findings
}

/// Runs the full `StateStore` contract suite against one factory.
pub fn state_store_suite(factory: &dyn StateStoreFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(checkpoint_and_resume_round_trip(factory));
    findings.extend(checkpointed_runs_load(factory));
    findings.extend(run_identity_recording_round_trips(factory));
    findings.extend(export_is_byte_identical_and_faithful(factory));
    findings.extend(owner_writes_are_earned_through_the_authorities(factory));
    findings.extend(correction_selections_round_trip(factory));
    findings
}

/// Case: run identity recording (plan Task 12's port addition): a recorded
/// run loads back with its frozen reference intact; re-recording the SAME
/// identity is idempotent; a re-recording that moves the frozen reference
/// is a typed error, never a silent overwrite (spec §5.3.3: a run has
/// exactly one frozen `reference_version` for its whole life).
fn run_identity_recording_round_trips(factory: &dyn StateStoreFactory) -> Vec<ContractFinding> {
    const CASE: &str = "run_identity_recording_round_trips";
    const RULE: &str = "spec 5.3.3 via 18.1: record_run makes the run durable; identity re-recording is idempotent and a moved reference is refused";
    let Ok(store) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a store for the case".to_owned(),
        )];
    };
    let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID");
    let Ok(run) = run(&run_id) else {
        return vec![finding(
            CASE,
            RULE,
            "the suite run fixture is unusable".to_owned(),
        )];
    };
    if let Err(error) = store.record_run(&run, bound()) {
        return vec![finding(
            CASE,
            RULE,
            format!("recording the run identity failed: {error}"),
        )];
    }
    let mut findings = Vec::new();
    match store.load_run(&run_id, bound()) {
        Ok(Some(loaded)) => {
            if loaded.reference_version().digest() != run.reference_version().digest() {
                findings.push(finding(
                    CASE,
                    RULE,
                    "the loaded run's frozen reference digest differs from the recorded one"
                        .to_owned(),
                ));
            }
        }
        Ok(None) => findings.push(finding(
            CASE,
            RULE,
            "the recorded run does not load back".to_owned(),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("loading the recorded run failed: {error}"),
        )),
    }
    if let Err(error) = store.record_run(&run, bound()) {
        findings.push(finding(
            CASE,
            RULE,
            format!("re-recording the SAME identity must be idempotent, it failed: {error}"),
        ));
    }
    // A re-recording that moves the frozen reference is refused — the run's
    // one frozen reference is not editable through the recording path.
    let moved_version = gauntlet_domain::ReferenceVersion::new(
        run.reference_version().reference_id().clone(),
        run.reference_version().version(),
        Digest::new("sha256", &"9f9f9f9f".repeat(4)).expect("suite digest"),
    );
    let moved = Run::new(RunDefinition {
        id: run.id().clone(),
        created_at: run.created_at(),
        reference_version: moved_version,
        intent: run.intent().to_owned(),
        config_fingerprint: run.config_fingerprint().clone(),
        environment_fingerprint: run.environment_fingerprint().clone(),
        budget: run.budget().clone(),
        parent_run: run.parent_run().cloned(),
    });
    match moved {
        Ok(moved) => {
            if store.record_run(&moved, bound()).is_ok() {
                findings.push(finding(
                    CASE,
                    RULE,
                    "a re-recording that moves the frozen reference must be refused, it was \
                     accepted"
                        .to_owned(),
                ));
            }
        }
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("the suite could not build a moved-reference variant: {error}"),
        )),
    }
    findings
}
