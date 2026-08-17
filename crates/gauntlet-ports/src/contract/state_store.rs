//! The `StateStore` shared contract suite (spec §6.2; §18, §16.3).
//!
//! Pinned semantics: a checkpoint written is a checkpoint resumable — the
//! run's latest checkpoint comes back exactly (spec §18: resume from the
//! durable checkpoint); a run that was checkpointed LOADS with its identity
//! and frozen reference digest intact; a run never stored is `None`; and
//! export is deterministic — same run history, byte-identical JSONL (spec
//! §6.2: "Export is deterministic").

use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, EventLogOffset, EvidenceRef, InvocationId,
    IterationNo, Limits, MicroUsd, ReferenceId, ResultContractPath, Run, RunDefinition, RunError,
    RunId, UnitId, UnitStatus, VerificationRef,
};

use crate::contract::ContractFinding;
use crate::deadline::Deadline;
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

/// Runs the full `StateStore` contract suite against one factory.
pub fn state_store_suite(factory: &dyn StateStoreFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(checkpoint_and_resume_round_trip(factory));
    findings.extend(checkpointed_runs_load(factory));
    findings.extend(export_is_byte_identical_and_faithful(factory));
    findings
}
