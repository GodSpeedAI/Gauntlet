//! The `Observer` shared contract suite (spec §6.2; §5.3.6 / I8).
//!
//! Pinned semantics: a facet always names the instrument class its adapter
//! observes — the facet's kind equals the adapter's own `facet_kind`, which
//! equals the kind the FACTORY declares for the case; a probe that could
//! not run is an [`ObserverError`], never a facet that silently pretends
//! the probe happened (spec §5.3.6 records failures as `ObservationFailure`
//! entries; I8: "Missing evidence is not passing evidence").

use gauntlet_domain::{EpochSeconds, IterationNo, RunId, WorkspaceRef};

use crate::contract::ContractFinding;
use crate::deadline::Deadline;
use crate::observer::{FacetKind, ObservationRequest, Observer, ObserverError};

/// The suite's name in findings.
pub const SUITE: &str = "observer";

const BASE_INSTANT: u64 = 1_700_000_000;
/// The suite's bound span, on its own line (numeric literals never share a
/// line with a duration-shaped identifier).
const BOUND_SPAN: u64 = 60;

/// Constructs the observer under test. `kind` is the instrument class this
/// adapter fronts, fixed per factory. `live` names the workspaces the
/// adapter can actually observe in this run.
pub trait ObserverFactory {
    fn label(&self) -> &'static str;

    fn kind(&self) -> FacetKind;

    fn build(&self, live: Vec<WorkspaceRef>) -> Result<Box<dyn Observer>, String>;
}

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding::new(SUITE, case, rule, detail)
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + BOUND_SPAN))
}

fn request(run: &RunId, workspace: WorkspaceRef) -> ObservationRequest {
    ObservationRequest {
        run_id: run.clone(),
        iteration_no: IterationNo::new(2).expect("suite iteration is 1-based"),
        workspace,
    }
}

/// Case: an observed facet names the kind its adapter observes — the facet
/// kind, the adapter's `facet_kind`, and the factory-declared kind all
/// agree. A facet for another instrument class is a cross-wired adapter
/// (one adapter per class, spec §6.2).
fn facets_name_their_own_kind(factory: &dyn ObserverFactory) -> Vec<ContractFinding> {
    const CASE: &str = "facets_name_their_own_kind";
    const RULE: &str =
        "spec 6.2 Observer: one adapter per instrument class; a facet names its own class";
    let workspace = WorkspaceRef::new("ws-suite-01").expect("suite workspace ref");
    let Ok(observer) = factory.build(vec![workspace.clone()]) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build an observer for the case".to_owned(),
        )];
    };
    if observer.facet_kind() != factory.kind() {
        return vec![finding(
            CASE,
            RULE,
            format!(
                "the adapter reports facet_kind {} but the factory declares {}",
                observer.facet_kind(),
                factory.kind()
            ),
        )];
    }
    let run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID");
    let facet = match observer.observe(&request(&run, workspace), bound()) {
        Ok(facet) => facet,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("observing a live workspace failed: {error}"),
            )];
        }
    };
    if facet.kind() != observer.facet_kind() {
        vec![finding(
            CASE,
            RULE,
            format!(
                "the facet names kind {} but the adapter observes {}",
                facet.kind(),
                observer.facet_kind()
            ),
        )]
    } else {
        Vec::new()
    }
}

/// Case: a probe that cannot run is an error, never a facet (spec §5.3.6:
/// failures are recorded, not silently dropped; I8). The request names a
/// workspace the factory declared unobservable.
fn failed_probes_are_errors_not_facets(factory: &dyn ObserverFactory) -> Vec<ContractFinding> {
    const CASE: &str = "failed_probes_are_errors_not_facets";
    const RULE: &str = "spec 5.3.6 (I8): a probe that cannot run is an observer error, never a silently missing or fabricated facet";
    let Ok(observer) = factory.build(vec![WorkspaceRef::new("ws-suite-01").expect("suite ref")])
    else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build an observer for the case".to_owned(),
        )];
    };
    let run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID");
    let absent = WorkspaceRef::new("ws-suite-absent-99").expect("suite contiguous ref");
    match observer.observe(&request(&run, absent), bound()) {
        Err(ObserverError::InstrumentFailed { .. } | ObserverError::Unavailable { .. }) => {
            Vec::new()
        }
        Ok(facet) => vec![finding(
            CASE,
            RULE,
            format!(
                "a probe against an unobservable workspace returned facet {} — a failed probe \
                 is never a facet",
                facet.kind()
            ),
        )],
    }
}

/// Runs the full `Observer` contract suite against one factory.
pub fn observer_suite(factory: &dyn ObserverFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(facets_name_their_own_kind(factory));
    findings.extend(failed_probes_are_errors_not_facets(factory));
    findings
}
